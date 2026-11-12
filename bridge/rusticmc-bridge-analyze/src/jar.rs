//! Reading a plugin jar: its class files (including jars nested inside it, as LuckPerms and
//! others ship their real code in a `*.jarinjar`) and its plugin descriptors.

use std::io::{Cursor, Read};

use crate::classfile::{self, ClassFile};

/// Largest single entry we inflate (class, descriptor or nested jar).
pub const MAX_ENTRY_BYTES: u64 = 64 * 1024 * 1024;
/// Most entries we look at in one archive (zip bombs, absurd jars).
pub const MAX_ENTRIES: usize = 200_000;
/// How deep nested jars are followed.
pub const MAX_NESTING: u32 = 2;

/// What a jar holds, as far as the analyzer cares.
#[derive(Debug, Default)]
pub struct JarContents {
    pub classes: Vec<ClassFile>,
    pub plugin_yml: Option<String>,
    pub paper_plugin_yml: Option<String>,
    /// Paths of nested jars whose classes were included.
    pub nested_jars: Vec<String>,
    /// Entries that could not be read or parsed (path: reason).
    pub errors: Vec<String>,
}

/// Why a jar could not be opened at all.
#[derive(Debug)]
pub struct JarError(pub String);

impl std::fmt::Display for JarError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "jar: {}", self.0)
    }
}

impl std::error::Error for JarError {}

/// Read a jar from memory.
pub fn read_jar(bytes: &[u8]) -> Result<JarContents, JarError> {
    let mut out = JarContents::default();
    read_into(bytes, "", 0, &mut out)?;
    // A class present twice (multi-release copies, nested duplicates): keep the first.
    let mut seen = std::collections::HashSet::new();
    out.classes.retain(|c| seen.insert(c.name.clone()));
    Ok(out)
}

fn read_into(
    bytes: &[u8],
    prefix: &str,
    depth: u32,
    out: &mut JarContents,
) -> Result<(), JarError> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| JarError(e.to_string()))?;
    let n = zip.len().min(MAX_ENTRIES);
    if zip.len() > MAX_ENTRIES {
        out.errors.push(format!(
            "{prefix}: more than {MAX_ENTRIES} entries, rest ignored"
        ));
    }
    for i in 0..n {
        let mut entry = match zip.by_index(i) {
            Ok(e) => e,
            Err(e) => {
                out.errors.push(format!("{prefix}#{i}: {e}"));
                continue;
            }
        };
        if entry.is_dir() {
            continue;
        }
        let name = match entry.name() {
            Ok(n) => n.into_owned(),
            Err(_) => String::from_utf8_lossy(entry.name_raw()).into_owned(),
        };
        let is_class = name.ends_with(".class")
            && !name.starts_with("META-INF/versions/")
            && !name.ends_with("module-info.class");
        let is_descriptor = depth == 0 && (name == "plugin.yml" || name == "paper-plugin.yml");
        let is_nested =
            depth < MAX_NESTING && (name.ends_with(".jar") || name.ends_with(".jarinjar"));
        if !(is_class || is_descriptor || is_nested) {
            continue;
        }
        if entry.size() > MAX_ENTRY_BYTES {
            out.errors.push(format!(
                "{prefix}{name}: larger than {MAX_ENTRY_BYTES} bytes, skipped"
            ));
            continue;
        }
        let mut data = Vec::with_capacity(entry.size() as usize);
        if let Err(e) = (&mut entry)
            .take(MAX_ENTRY_BYTES + 1)
            .read_to_end(&mut data)
        {
            out.errors.push(format!("{prefix}{name}: {e}"));
            continue;
        }
        drop(entry);
        if is_class {
            match classfile::parse(&data) {
                Ok(c) => out.classes.push(c),
                Err(e) => out.errors.push(format!("{prefix}{name}: {e}")),
            }
        } else if is_descriptor {
            let text = String::from_utf8_lossy(&data).into_owned();
            if name == "plugin.yml" {
                out.plugin_yml = Some(text);
            } else {
                out.paper_plugin_yml = Some(text);
            }
        } else {
            let path = format!("{prefix}{name}");
            match read_into(&data, &format!("{path}!/"), depth + 1, out) {
                Ok(()) => out.nested_jars.push(path),
                Err(e) => out.errors.push(format!("{path}: {e}")),
            }
        }
    }
    Ok(())
}
