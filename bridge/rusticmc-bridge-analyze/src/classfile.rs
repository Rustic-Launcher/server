//! A compact, defensive reader for JVM class files (JVMS chapter 4), written for this crate.
//!
//! It extracts only what the analyzer needs: the class's name and supertypes, the members it
//! declares, every class it references (constant pool `Class` entries plus every descriptor or
//! generic signature), every field/method it references together with the number of bytecode
//! sites that use it (`get*`/`put*`, `invoke*`, method handles in `invokedynamic` bootstrap
//! arguments and `ldc`), and its identifier-like string constants (for reflection checks).
//!
//! Input is untrusted: every read is bounds-checked and a malformed method body only stops the
//! walk of that body (`ClassFile::warnings`), never the whole class.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fmt;

/// Why a class file could not be read at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassError(pub String);

impl fmt::Display for ClassError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "class file: {}", self.0)
    }
}

impl std::error::Error for ClassError {}

/// Field or method.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum MemberKind {
    Field,
    Method,
}

/// A reference to a member of some class, in JVM internal form (`org/bukkit/entity/Player`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MemberRef {
    pub kind: MemberKind,
    pub owner: String,
    pub name: String,
    pub descriptor: String,
}

/// A parsed class, reduced to references.
#[derive(Debug, Clone, Default)]
pub struct ClassFile {
    /// Internal name (`com/example/Plugin`).
    pub name: String,
    pub super_name: Option<String>,
    pub interfaces: Vec<String>,
    pub major_version: u16,
    /// `(name, descriptor)` of declared methods.
    pub declared_methods: HashSet<(String, String)>,
    /// Names of declared fields.
    pub declared_fields: HashSet<String>,
    /// Every class referenced, internal names, without this class itself.
    pub type_refs: BTreeSet<String>,
    /// Every member referenced with its bytecode site count (at least 1 when it is in the
    /// constant pool at all).
    pub member_refs: BTreeMap<MemberRef, u32>,
    /// String constants that look like identifiers or dotted/slashed names (reflection targets).
    pub strings: BTreeSet<String>,
    /// Non-fatal problems (unknown opcode, truncated code...).
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
enum Cp {
    Empty,
    Utf8(String),
    Class(u16),
    Str(u16),
    Member(MemberKind, u16, u16),
    NameAndType(u16, u16),
    MethodHandle(u16),
    /// Dynamic / InvokeDynamic: bootstrap method index (the name-and-type is skipped).
    Dynamic(u16),
    Other,
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], ClassError> {
        let end = self
            .pos
            .checked_add(n)
            .filter(|&e| e <= self.data.len())
            .ok_or_else(|| ClassError(format!("truncated at byte {}", self.pos)))?;
        let s = &self.data[self.pos..end];
        self.pos = end;
        Ok(s)
    }
    fn u1(&mut self) -> Result<u8, ClassError> {
        Ok(self.take(1)?[0])
    }
    fn u2(&mut self) -> Result<u16, ClassError> {
        let b = self.take(2)?;
        Ok(u16::from_be_bytes([b[0], b[1]]))
    }
    fn u4(&mut self) -> Result<u32, ClassError> {
        let b = self.take(4)?;
        Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }
}

/// Decode "modified UTF-8" leniently: plain UTF-8 covers nearly everything in practice; the
/// two-byte NUL and surrogate pairs fall back to a lossy decode, which is fine for names.
fn decode_utf8(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_owned(),
        Err(_) => String::from_utf8_lossy(bytes).into_owned(),
    }
}

/// Collect the class names in a field/method descriptor or a generic signature
/// (`(Lorg/bukkit/entity/Player;I)V`, `Ljava/util/List<Lorg/bukkit/World;>;`).
pub fn descriptor_classes(desc: &str, out: &mut BTreeSet<String>) {
    let b = desc.as_bytes();
    // End of the identifier starting at `from`: the first of `; < . :` (or the end).
    let ident_end = |from: usize| {
        let mut j = from;
        while j < b.len() && !matches!(b[j], b';' | b'<' | b'.' | b':' | b'>') {
            j += 1;
        }
        j
    };
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            // A formal type parameter name (`<LEFT:Ljava/lang/Object;>`, `<T::Lx/I;>`): skip it.
            b'L' | b'T' if b.get(ident_end(i)) == Some(&b':') => i = ident_end(i) + 1,
            b'L' => {
                let start = i + 1;
                let j = ident_end(start);
                if j > start && valid_class_name(&desc[start..j]) {
                    out.insert(desc[start..j].to_owned());
                }
                i = j;
            }
            // Type variable `TT;`.
            b'T' => i = ident_end(i) + 1,
            // Inner class of a parameterized outer (`Lo/Outer<TK;>.Inner;`): a simple name.
            b'.' => i = ident_end(i + 1),
            _ => i += 1,
        }
    }
}

/// A plausible internal class name (rejects annotation strings that merely look like descriptors).
fn valid_class_name(n: &str) -> bool {
    !n.starts_with('/')
        && !n.ends_with('/')
        && !n.contains("//")
        && n.chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '_' | '$' | '/' | '-'))
}

/// Class name of a `CONSTANT_Class` value: plain internal names, or array descriptors.
fn class_entry_names(name: &str, out: &mut BTreeSet<String>) {
    if name.starts_with('[') {
        descriptor_classes(name, out);
    } else if !name.is_empty() {
        out.insert(name.to_owned());
    }
}

fn looks_like_descriptor(s: &str) -> bool {
    (s.starts_with('(') || s.starts_with('L') || s.starts_with('[') || s.starts_with('<'))
        && s.contains(';')
        && s.contains('/')
}

/// Identifier-ish strings worth keeping: `net.minecraft.server.Foo`, `getHandle`, `a/b/C`,
/// `.entity.CraftPlayer` (a suffix appended to a runtime package name).
fn keep_string(s: &str) -> bool {
    (2..=200).contains(&s.len())
        && s.chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || matches!(c, '_' | '$' | '.'))
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '$' | '.' | '/'))
}

struct Pool {
    entries: Vec<Cp>,
}

impl Pool {
    fn get(&self, i: u16) -> Option<&Cp> {
        self.entries.get(i as usize)
    }
    fn utf8(&self, i: u16) -> Option<&str> {
        match self.get(i) {
            Some(Cp::Utf8(s)) => Some(s),
            _ => None,
        }
    }
    fn class_name(&self, i: u16) -> Option<&str> {
        match self.get(i) {
            Some(Cp::Class(n)) => self.utf8(*n),
            _ => None,
        }
    }
    fn name_and_type(&self, i: u16) -> Option<(&str, &str)> {
        match self.get(i) {
            Some(Cp::NameAndType(n, d)) => Some((self.utf8(*n)?, self.utf8(*d)?)),
            _ => None,
        }
    }
    fn member(&self, i: u16) -> Option<MemberRef> {
        match self.get(i) {
            Some(Cp::Member(kind, class, nat)) => {
                let owner = self.class_name(*class)?;
                let (name, descriptor) = self.name_and_type(*nat)?;
                // Array pseudo-owners (`[Ljava/lang/Object;.clone()`) are not classes.
                if owner.starts_with('[') {
                    return None;
                }
                Some(MemberRef {
                    kind: *kind,
                    owner: owner.to_owned(),
                    name: name.to_owned(),
                    descriptor: descriptor.to_owned(),
                })
            }
            Some(Cp::MethodHandle(r)) => self.member(*r),
            _ => None,
        }
    }
}

fn parse_pool(r: &mut Reader<'_>) -> Result<Pool, ClassError> {
    let count = r.u2()? as usize;
    let mut entries = Vec::with_capacity(count);
    entries.push(Cp::Empty);
    while entries.len() < count {
        let tag = r.u1()?;
        let e = match tag {
            1 => {
                let len = r.u2()? as usize;
                Cp::Utf8(decode_utf8(r.take(len)?))
            }
            3 | 4 => {
                r.u4()?;
                Cp::Other
            }
            5 | 6 => {
                r.take(8)?;
                entries.push(Cp::Other);
                Cp::Empty // 8-byte constants take two slots
            }
            7 => Cp::Class(r.u2()?),
            8 => Cp::Str(r.u2()?),
            9 => Cp::Member(MemberKind::Field, r.u2()?, r.u2()?),
            10 | 11 => Cp::Member(MemberKind::Method, r.u2()?, r.u2()?),
            12 => Cp::NameAndType(r.u2()?, r.u2()?),
            15 => {
                r.u1()?;
                Cp::MethodHandle(r.u2()?)
            }
            16 => {
                r.u2()?; // MethodType: its descriptor is a Utf8 scanned like the others
                Cp::Other
            }
            17 | 18 => {
                let bsm = r.u2()?;
                r.u2()?;
                Cp::Dynamic(bsm)
            }
            19 | 20 => {
                r.u2()?;
                Cp::Other
            }
            t => return Err(ClassError(format!("unknown constant pool tag {t}"))),
        };
        entries.push(e);
    }
    entries.truncate(count.max(1));
    Ok(Pool { entries })
}

struct RawMember {
    name: String,
    descriptor: String,
    code: Option<(usize, usize)>, // range in the class bytes
}

fn parse_members(
    r: &mut Reader<'_>,
    pool: &Pool,
    want_code: bool,
) -> Result<Vec<RawMember>, ClassError> {
    let count = r.u2()?;
    let mut out = Vec::with_capacity(count as usize);
    for _ in 0..count {
        r.u2()?; // access flags
        let name = pool.utf8(r.u2()?).unwrap_or_default().to_owned();
        let descriptor = pool.utf8(r.u2()?).unwrap_or_default().to_owned();
        let mut code = None;
        let attrs = r.u2()?;
        for _ in 0..attrs {
            let attr_name = r.u2()?;
            let len = r.u4()? as usize;
            let start = r.pos;
            r.take(len)?;
            if want_code && pool.utf8(attr_name) == Some("Code") {
                code = Some((start, start + len));
            }
        }
        out.push(RawMember {
            name,
            descriptor,
            code,
        });
    }
    Ok(out)
}

/// Parse a class file.
pub fn parse(data: &[u8]) -> Result<ClassFile, ClassError> {
    let mut r = Reader::new(data);
    if r.u4()? != 0xCAFE_BABE {
        return Err(ClassError("bad magic".into()));
    }
    let _minor = r.u2()?;
    let major = r.u2()?;
    let pool = parse_pool(&mut r)?;
    r.u2()?; // access
    let this = pool.class_name(r.u2()?).unwrap_or_default().to_owned();
    let super_idx = r.u2()?;
    let super_name = if super_idx == 0 {
        None
    } else {
        pool.class_name(super_idx).map(str::to_owned)
    };
    let n_if = r.u2()?;
    let mut interfaces = Vec::new();
    for _ in 0..n_if {
        if let Some(n) = pool.class_name(r.u2()?) {
            interfaces.push(n.to_owned());
        }
    }
    let fields = parse_members(&mut r, &pool, false)?;
    let methods = parse_members(&mut r, &pool, true)?;

    // Class attributes: only BootstrapMethods matters.
    let mut bootstraps: Vec<(u16, Vec<u16>)> = Vec::new();
    let n_attr = r.u2()?;
    for _ in 0..n_attr {
        let name = r.u2()?;
        let len = r.u4()? as usize;
        let body = r.take(len)?;
        if pool.utf8(name) == Some("BootstrapMethods") {
            let mut br = Reader::new(body);
            let n = br.u2()?;
            for _ in 0..n {
                let mh = br.u2()?;
                let argc = br.u2()?;
                let mut args = Vec::with_capacity(argc as usize);
                for _ in 0..argc {
                    args.push(br.u2()?);
                }
                bootstraps.push((mh, args));
            }
        }
    }

    let mut cf = ClassFile {
        name: this.clone(),
        super_name,
        interfaces,
        major_version: major,
        ..ClassFile::default()
    };

    // Constant pool: classes, descriptors (anything not used as a string literal) and strings.
    let mut string_utf8: HashSet<u16> = HashSet::new();
    for e in &pool.entries {
        if let Cp::Str(i) = e {
            string_utf8.insert(*i);
        }
    }
    for (idx, e) in pool.entries.iter().enumerate() {
        match e {
            Cp::Class(n) => {
                if let Some(name) = pool.utf8(*n) {
                    class_entry_names(name, &mut cf.type_refs);
                }
            }
            Cp::Utf8(s) => {
                let idx = u16::try_from(idx).unwrap_or(u16::MAX);
                if string_utf8.contains(&idx) {
                    if keep_string(s) {
                        cf.strings.insert(s.clone());
                    }
                } else if looks_like_descriptor(s) {
                    descriptor_classes(s, &mut cf.type_refs);
                }
            }
            _ => {}
        }
    }
    for f in &fields {
        cf.declared_fields.insert(f.name.clone());
        descriptor_classes(&f.descriptor, &mut cf.type_refs);
    }
    for m in &methods {
        cf.declared_methods
            .insert((m.name.clone(), m.descriptor.clone()));
        descriptor_classes(&m.descriptor, &mut cf.type_refs);
    }

    // Every member in the pool counts at least once; bytecode sites add to it.
    let mut sites: BTreeMap<MemberRef, u32> = BTreeMap::new();
    for i in 0..pool.entries.len() {
        if let Some(Cp::Member(..)) = pool.entries.get(i)
            && let Some(m) = pool.member(i as u16)
        {
            sites.entry(m).or_insert(0);
        }
    }
    for m in &methods {
        if let Some((start, end)) = m.code
            && let Err(e) = walk_code(&data[start..end], &pool, &bootstraps, &mut sites)
        {
            cf.warnings
                .push(format!("{}{}: {}", m.name, m.descriptor, e.0));
        }
    }
    for (m, n) in sites {
        cf.member_refs.insert(m, n.max(1));
    }
    cf.type_refs.remove(&this);
    Ok(cf)
}

/// Walk one `Code` attribute's instructions, counting member uses.
fn walk_code(
    attr: &[u8],
    pool: &Pool,
    bootstraps: &[(u16, Vec<u16>)],
    sites: &mut BTreeMap<MemberRef, u32>,
) -> Result<(), ClassError> {
    let mut r = Reader::new(attr);
    r.u2()?; // max_stack
    r.u2()?; // max_locals
    let len = r.u4()? as usize;
    let code = r.take(len)?;
    let mut bump = |m: Option<MemberRef>| {
        if let Some(m) = m {
            *sites.entry(m).or_insert(0) += 1;
        }
    };
    let u2 = |pc: usize| -> Result<u16, ClassError> {
        code.get(pc..pc + 2)
            .map(|b| u16::from_be_bytes([b[0], b[1]]))
            .ok_or_else(|| ClassError(format!("truncated operand at pc {pc}")))
    };
    let i4 = |pc: usize| -> Result<i32, ClassError> {
        code.get(pc..pc + 4)
            .map(|b| i32::from_be_bytes([b[0], b[1], b[2], b[3]]))
            .ok_or_else(|| ClassError(format!("truncated operand at pc {pc}")))
    };
    let mut pc = 0usize;
    while pc < code.len() {
        let op = code[pc];
        let size = match op {
            // getstatic putstatic getfield putfield invokevirtual invokespecial invokestatic
            0xb2..=0xb8 => {
                bump(pool.member(u2(pc + 1)?));
                3
            }
            0xb9 => {
                bump(pool.member(u2(pc + 1)?));
                5
            }
            0xba => {
                // invokedynamic: method handles among the bootstrap arguments are the real
                // targets (`Player::getName`, lambdas' bodies are own methods).
                if let Some(Cp::Dynamic(bsm)) = pool.get(u2(pc + 1)?)
                    && let Some((mh, args)) = bootstraps.get(*bsm as usize)
                {
                    bump(pool.member(*mh));
                    for a in args {
                        if let Some(Cp::MethodHandle(_)) = pool.get(*a) {
                            bump(pool.member(*a));
                        }
                    }
                }
                5
            }
            0x12 => {
                // ldc of a method handle
                if let Some(Cp::MethodHandle(_)) =
                    pool.get(u16::from(*code.get(pc + 1).unwrap_or(&0)))
                {
                    bump(pool.member(u16::from(code[pc + 1])));
                }
                2
            }
            0x13 => {
                let i = u2(pc + 1)?;
                if let Some(Cp::MethodHandle(_)) = pool.get(i) {
                    bump(pool.member(i));
                }
                3
            }
            0x10 | 0x15..=0x19 | 0x36..=0x3a | 0xa9 | 0xbc => 2,
            0x11 | 0x14 | 0x84 | 0x99..=0xa8 | 0xbb | 0xbd | 0xc0 | 0xc1 | 0xc6 | 0xc7 => 3,
            0xc5 => 4,
            0xc8 | 0xc9 => 5,
            0xaa => {
                let base = (pc + 4) & !3;
                let low = i4(base + 4)?;
                let high = i4(base + 8)?;
                let n = i64::from(high) - i64::from(low) + 1;
                if !(0..=65536).contains(&n) {
                    return Err(ClassError(format!("bad tableswitch at pc {pc}")));
                }
                base + 12 + 4 * n as usize - pc
            }
            0xab => {
                let base = (pc + 4) & !3;
                let n = i4(base + 4)?;
                if !(0..=65536).contains(&n) {
                    return Err(ClassError(format!("bad lookupswitch at pc {pc}")));
                }
                base + 8 + 8 * n as usize - pc
            }
            0xc4 => {
                if code.get(pc + 1) == Some(&0x84) {
                    6
                } else {
                    4
                }
            }
            0x00..=0xc9 => 1,
            other => {
                return Err(ClassError(format!(
                    "unknown opcode 0x{other:02x} at pc {pc}"
                )));
            }
        };
        pc += size;
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A tiny class-file writer for tests: enough to put member refs and code in a class.
    #[derive(Default)]
    pub(crate) struct ClassBuilder {
        pool: Vec<Vec<u8>>,
        methods: Vec<Vec<u8>>,
        this: u16,
        sup: u16,
        bootstrap: Vec<(u16, Vec<u16>)>,
    }

    impl ClassBuilder {
        pub(crate) fn new(name: &str, sup: &str) -> Self {
            let mut b = Self::default();
            b.this = b.class(name);
            b.sup = b.class(sup);
            b
        }
        fn add(&mut self, e: Vec<u8>) -> u16 {
            self.pool.push(e);
            self.pool.len() as u16
        }
        pub(crate) fn utf8(&mut self, s: &str) -> u16 {
            let mut e = vec![1];
            e.extend((s.len() as u16).to_be_bytes());
            e.extend(s.as_bytes());
            self.add(e)
        }
        pub(crate) fn class(&mut self, name: &str) -> u16 {
            let n = self.utf8(name);
            let mut e = vec![7];
            e.extend(n.to_be_bytes());
            self.add(e)
        }
        pub(crate) fn string(&mut self, s: &str) -> u16 {
            let n = self.utf8(s);
            let mut e = vec![8];
            e.extend(n.to_be_bytes());
            self.add(e)
        }
        pub(crate) fn member(&mut self, tag: u8, owner: &str, name: &str, desc: &str) -> u16 {
            let c = self.class(owner);
            let n = self.utf8(name);
            let d = self.utf8(desc);
            let mut nat = vec![12];
            nat.extend(n.to_be_bytes());
            nat.extend(d.to_be_bytes());
            let nat = self.add(nat);
            let mut e = vec![tag];
            e.extend(c.to_be_bytes());
            e.extend(nat.to_be_bytes());
            self.add(e)
        }
        pub(crate) fn long(&mut self) -> u16 {
            let i = self.add(vec![5, 0, 0, 0, 0, 0, 0, 0, 1]);
            self.pool.push(Vec::new()); // second slot, written as nothing
            i
        }
        pub(crate) fn method_handle(&mut self, kind: u8, member: u16) -> u16 {
            let mut e = vec![15, kind];
            e.extend(member.to_be_bytes());
            self.add(e)
        }
        pub(crate) fn indy(
            &mut self,
            bootstrap_mh: u16,
            args: Vec<u16>,
            name: &str,
            desc: &str,
        ) -> u16 {
            let bsm = self.bootstrap.len() as u16;
            self.bootstrap.push((bootstrap_mh, args));
            let n = self.utf8(name);
            let d = self.utf8(desc);
            let mut nat = vec![12];
            nat.extend(n.to_be_bytes());
            nat.extend(d.to_be_bytes());
            let nat = self.add(nat);
            let mut e = vec![18];
            e.extend(bsm.to_be_bytes());
            e.extend(nat.to_be_bytes());
            self.add(e)
        }
        pub(crate) fn method(&mut self, name: &str, desc: &str, code: &[u8]) {
            let n = self.utf8(name);
            let d = self.utf8(desc);
            let code_name = self.utf8("Code");
            let mut m = vec![0, 1];
            m.extend(n.to_be_bytes());
            m.extend(d.to_be_bytes());
            m.extend(1u16.to_be_bytes());
            m.extend(code_name.to_be_bytes());
            let attr_len = 2 + 2 + 4 + code.len() + 2 + 2;
            m.extend((attr_len as u32).to_be_bytes());
            m.extend([0, 4, 0, 4]);
            m.extend((code.len() as u32).to_be_bytes());
            m.extend(code);
            m.extend([0, 0, 0, 0]);
            self.methods.push(m);
        }
        pub(crate) fn build(mut self) -> Vec<u8> {
            let bsm_name = if self.bootstrap.is_empty() {
                0
            } else {
                self.utf8("BootstrapMethods")
            };
            let mut out = vec![0xCA, 0xFE, 0xBA, 0xBE, 0, 0, 0, 65];
            out.extend(((self.pool.len() + 1) as u16).to_be_bytes());
            for e in &self.pool {
                out.extend(e);
            }
            out.extend([0, 0x21]);
            out.extend(self.this.to_be_bytes());
            out.extend(self.sup.to_be_bytes());
            out.extend([0, 0]); // interfaces
            out.extend([0, 0]); // fields
            out.extend((self.methods.len() as u16).to_be_bytes());
            for m in &self.methods {
                out.extend(m);
            }
            if self.bootstrap.is_empty() {
                out.extend([0, 0]);
            } else {
                let mut body = (self.bootstrap.len() as u16).to_be_bytes().to_vec();
                for (mh, args) in &self.bootstrap {
                    body.extend(mh.to_be_bytes());
                    body.extend((args.len() as u16).to_be_bytes());
                    for a in args {
                        body.extend(a.to_be_bytes());
                    }
                }
                out.extend([0, 1]);
                out.extend(bsm_name.to_be_bytes());
                out.extend((body.len() as u32).to_be_bytes());
                out.extend(body);
            }
            out
        }
    }

    fn mref(owner: &str, name: &str, desc: &str) -> MemberRef {
        MemberRef {
            kind: MemberKind::Method,
            owner: owner.into(),
            name: name.into(),
            descriptor: desc.into(),
        }
    }

    #[test]
    fn descriptors_and_signatures() {
        let mut s = BTreeSet::new();
        descriptor_classes(
            "(Lorg/bukkit/entity/Player;I[Lorg/bukkit/World;)Ljava/lang/String;",
            &mut s,
        );
        descriptor_classes(
            "Ljava/util/Map<TK;Ljava/util/List<Lorg/bukkit/Location;>;>;",
            &mut s,
        );
        descriptor_classes("Lfoo/Outer<TT;>.Inner;", &mut s);
        descriptor_classes(
            "Lcom/google/gson/internal/LinkedTreeMap<TK;TV;>.LinkedTreeMapIterator;",
            &mut s,
        );
        descriptor_classes(
            "<LEFT:Ljava/lang/Object;T::Lorg/bukkit/Keyed;>Ljava/lang/Object;",
            &mut s,
        );
        let v: Vec<_> = s.into_iter().collect();
        assert_eq!(
            v,
            [
                "com/google/gson/internal/LinkedTreeMap",
                "foo/Outer",
                "java/lang/Object",
                "java/lang/String",
                "java/util/List",
                "java/util/Map",
                "org/bukkit/Keyed",
                "org/bukkit/Location",
                "org/bukkit/World",
                "org/bukkit/entity/Player"
            ]
        );
    }

    #[test]
    fn counts_invoke_sites_switches_and_wide_longs() {
        let mut b = ClassBuilder::new("a/Main", "java/lang/Object");
        let send = b.member(
            11,
            "org/bukkit/entity/Player",
            "sendMessage",
            "(Ljava/lang/String;)V",
        );
        let online = b.member(
            10,
            "org/bukkit/Bukkit",
            "getOnlinePlayers",
            "()Ljava/util/Collection;",
        );
        let field = b.member(9, "org/bukkit/Material", "STONE", "Lorg/bukkit/Material;");
        let _l = b.long();
        let s = b.string("net.minecraft.server.MinecraftServer");
        let [s1, s2] = send.to_be_bytes();
        let [o1, o2] = online.to_be_bytes();
        let [f1, f2] = field.to_be_bytes();
        let mut code = vec![0xb8, o1, o2]; // invokestatic, pc 0..3
        code.push(0x03); // iconst_0, pc 3
        // tableswitch at pc 4: pad to 8, default, low 0, high 1, two offsets
        code.extend([0xaa, 0, 0, 0]);
        code.extend(0i32.to_be_bytes());
        code.extend(0i32.to_be_bytes());
        code.extend(1i32.to_be_bytes());
        code.extend(0i32.to_be_bytes());
        code.extend(0i32.to_be_bytes());
        code.extend([0xc4, 0x84, 0, 1, 0, 1]); // wide iinc
        code.extend([0xb9, s1, s2, 2, 0]); // invokeinterface
        code.extend([0xb9, s1, s2, 2, 0]);
        code.extend([0xb2, f1, f2]); // getstatic
        code.extend([0x12, s as u8, 0xb1]); // ldc string, return
        b.method("run", "(Lorg/bukkit/entity/Player;)V", &code);
        let cf = parse(&b.build()).expect("parses");
        assert!(cf.warnings.is_empty(), "{:?}", cf.warnings);
        assert_eq!(cf.name, "a/Main");
        assert_eq!(
            cf.member_refs[&mref(
                "org/bukkit/entity/Player",
                "sendMessage",
                "(Ljava/lang/String;)V"
            )],
            2
        );
        assert_eq!(
            cf.member_refs[&mref(
                "org/bukkit/Bukkit",
                "getOnlinePlayers",
                "()Ljava/util/Collection;"
            )],
            1
        );
        assert!(cf.type_refs.contains("org/bukkit/entity/Player"));
        assert!(cf.type_refs.contains("org/bukkit/Material"));
        assert!(cf.strings.contains("net.minecraft.server.MinecraftServer"));
        assert!(
            cf.declared_methods
                .contains(&("run".into(), "(Lorg/bukkit/entity/Player;)V".into()))
        );
    }

    #[test]
    fn invokedynamic_method_handle_arguments_count() {
        let mut b = ClassBuilder::new("a/Main", "java/lang/Object");
        let meta = b.member(
            10,
            "java/lang/invoke/LambdaMetafactory",
            "metafactory",
            "(Ljava/lang/invoke/MethodHandles$Lookup;Ljava/lang/String;Ljava/lang/invoke/MethodType;Ljava/lang/invoke/MethodType;Ljava/lang/invoke/MethodHandle;Ljava/lang/invoke/MethodType;)Ljava/lang/invoke/CallSite;",
        );
        let meta_mh = b.method_handle(6, meta);
        let get_name = b.member(
            11,
            "org/bukkit/entity/Player",
            "getName",
            "()Ljava/lang/String;",
        );
        let get_name_mh = b.method_handle(9, get_name);
        let indy = b.indy(
            meta_mh,
            vec![get_name_mh],
            "apply",
            "()Ljava/util/function/Function;",
        );
        let [i1, i2] = indy.to_be_bytes();
        b.method("f", "()V", &[0xba, i1, i2, 0, 0, 0x57, 0xb1]);
        let cf = parse(&b.build()).expect("parses");
        assert_eq!(
            cf.member_refs[&mref(
                "org/bukkit/entity/Player",
                "getName",
                "()Ljava/lang/String;"
            )],
            1
        );
        assert!(cf.type_refs.contains("java/util/function/Function"));
    }

    #[test]
    fn malformed_input_is_an_error_or_warning_never_a_panic() {
        assert!(parse(b"").is_err());
        assert!(parse(&[0xCA, 0xFE, 0xBA, 0xBE, 0, 0]).is_err());
        let mut b = ClassBuilder::new("a/Main", "java/lang/Object");
        b.method("bad", "()V", &[0xfe]);
        let cf = parse(&b.build()).expect("class still parses");
        assert_eq!(cf.warnings.len(), 1);
        // Truncate every prefix of a valid class: errors only.
        let mut b = ClassBuilder::new("a/Main", "java/lang/Object");
        let m = b.member(10, "x/Y", "z", "()V");
        let [m1, m2] = m.to_be_bytes();
        b.method("f", "()V", &[0xb8, m1, m2, 0xaa, 0, 0]);
        let bytes = b.build();
        for n in 0..bytes.len() {
            let _ = parse(&bytes[..n]);
        }
    }
}
