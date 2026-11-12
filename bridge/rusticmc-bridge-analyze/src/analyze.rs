//! Classifying one plugin jar: which external classes and members it uses, how they map onto
//! Rustic, and its verdict.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::classfile::{ClassFile, MemberKind};
use crate::coverage::{Coverage, Status};
use crate::jar::{self, JarContents};
use crate::manifest::{self, PluginMeta};

/// 🟢 / 🟡 / 🔴, or the jar could not be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    /// Only API members Rustic covers.
    Green,
    /// Bukkit/Paper API Rustic does not cover yet (see `uncovered`).
    Yellow,
    /// Server internals, packet libraries, instrumentation or reflection into internals.
    Red,
    /// The jar could not be analyzed.
    Error,
}

impl Verdict {
    pub fn emoji(self) -> &'static str {
        match self {
            Verdict::Green => "🟢",
            Verdict::Yellow => "🟡",
            Verdict::Red => "🔴",
            Verdict::Error => "⚠️",
        }
    }
}

/// Where a referenced class lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Namespace {
    /// Bukkit, Spigot, Paper, Adventure, Brigadier, BungeeCord chat (and Vault): the plugin API.
    Api,
    /// `net.minecraft`, CraftBukkit, Mojang server libraries, Netty, Paper internals.
    Internal,
    /// ProtocolLib, PacketEvents.
    PacketLibrary,
    /// ASM, ByteBuddy, Javassist, `java.lang.instrument`, `Unsafe`, `jdk.internal`.
    Instrumentation,
    /// The Java platform.
    Jdk,
    /// Libraries the Bukkit/Paper server puts on the plugin classpath (Gson, Guava, SnakeYAML...).
    PlatformLibrary,
    /// Classes of other platforms (BungeeCord proxy, Velocity, Fabric...) in multi-platform jars.
    OtherPlatform,
    /// Anything else: other plugins' APIs, libraries fetched through `libraries:`.
    External,
}

const INSTRUMENTATION: &[&str] = &[
    "sun/misc/Unsafe",
    "jdk/internal/",
    "java/lang/instrument/",
    "org/objectweb/asm/",
    "net/bytebuddy/",
    "javassist/",
];
const INTERNAL: &[&str] = &[
    "net/minecraft/",
    "org/bukkit/craftbukkit/",
    "com/mojang/authlib/",
    "com/mojang/datafixers/",
    "com/mojang/serialization/",
    "com/mojang/logging/",
    "com/mojang/math/",
    "com/mojang/util/",
    "io/netty/",
    "ca/spottedleaf/",
    "io/papermc/paper/adventure/PaperAdventure",
    "org/spigotmc/SpigotConfig",
    "org/spigotmc/ActivationRange",
];
const PACKET_LIBS: &[&str] = &[
    "com/comphenix/protocol/",
    "com/github/retrooper/packetevents/",
    "io/github/retrooper/packetevents/",
];
const API: &[&str] = &[
    "org/bukkit/",
    "io/papermc/paper/",
    "com/destroystokyo/paper/",
    "net/kyori/",
    "org/spigotmc/",
    "net/md_5/bungee/api/chat/",
    "net/md_5/bungee/api/ChatColor",
    "net/md_5/bungee/api/ChatMessageType",
    "net/md_5/bungee/chat/",
    "co/aikar/timings/",
    "com/mojang/brigadier/",
    "org/purpurmc/purpur/",
    // Vault: not Bukkit, but the de-facto service API every economy/permission plugin uses; the
    // coverage table maps it onto rustic:services.
    "net/milkbowl/vault/",
];
const JDK: &[&str] = &[
    "java/",
    "javax/",
    "jdk/",
    "sun/",
    "com/sun/",
    "org/w3c/",
    "org/xml/",
    "org/ietf/",
    "org/omg/",
    "org/jcp/",
];
const PLATFORM_LIBS: &[&str] = &[
    "com/google/gson/",
    "com/google/common/",
    "com/google/thirdparty/",
    "com/google/errorprone/",
    "com/google/j2objc/",
    "org/yaml/snakeyaml/",
    "org/apache/commons/lang/",
    "org/apache/commons/lang3/",
    "org/apache/logging/log4j/",
    "org/slf4j/",
    "org/json/simple/",
    "org/jetbrains/annotations/",
    "org/intellij/lang/annotations/",
    "org/checkerframework/",
    "org/jspecify/",
    "it/unimi/dsi/fastutil/",
    "org/joml/",
    "org/apache/maven/",
    "org/eclipse/aether/",
];
const OTHER_PLATFORMS: &[&str] = &[
    "net/md_5/bungee/",
    "com/velocitypowered/",
    "org/spongepowered/",
    "net/fabricmc/",
    "net/minecraftforge/",
    "net/neoforged/",
    "cn/nukkit/",
    "org/geysermc/geyser/platform/",
];

fn starts(name: &str, list: &[&str]) -> bool {
    list.iter().any(|p| name.starts_with(p))
}

/// Classify a class name in internal form.
pub fn namespace(internal: &str) -> Namespace {
    if starts(internal, INSTRUMENTATION) {
        Namespace::Instrumentation
    } else if starts(internal, INTERNAL)
        || (internal.starts_with("com/mojang/") && !internal.starts_with("com/mojang/brigadier/"))
    {
        Namespace::Internal
    } else if starts(internal, PACKET_LIBS) {
        Namespace::PacketLibrary
    } else if starts(internal, API) {
        Namespace::Api
    } else if starts(internal, JDK) {
        Namespace::Jdk
    } else if starts(internal, PLATFORM_LIBS) {
        Namespace::PlatformLibrary
    } else if starts(internal, OTHER_PLATFORMS) {
        Namespace::OtherPlatform
    } else {
        Namespace::External
    }
}

/// Dotted binary name (`org.bukkit.entity.Player$Spigot`).
pub fn dotted(internal: &str) -> String {
    internal.replace('/', ".")
}

/// Kinds of red signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RedKind {
    /// `net.minecraft.*` referenced directly.
    Nms,
    /// `org.bukkit.craftbukkit.*` referenced directly.
    CraftBukkit,
    /// Other server internals (Mojang libraries, Netty, Paper internals).
    ServerInternal,
    /// ProtocolLib / PacketEvents.
    PacketLibrary,
    /// Reflection with string constants naming server internals.
    ReflectionIntoInternals,
    /// Bytecode instrumentation (ASM, ByteBuddy, Javassist, java.lang.instrument).
    Instrumentation,
    /// `sun.misc.Unsafe` / `jdk.internal`.
    Unsafe,
    /// A hard dependency (`depend`) on a plugin that is itself red by nature.
    HardDependency,
}

impl RedKind {
    pub fn label(self) -> &'static str {
        match self {
            RedKind::Nms => "NMS (net.minecraft)",
            RedKind::CraftBukkit => "CraftBukkit",
            RedKind::ServerInternal => "server internals (Mojang libs, Netty, Paper internals)",
            RedKind::PacketLibrary => "ProtocolLib / PacketEvents",
            RedKind::ReflectionIntoInternals => "reflection into internals",
            RedKind::Instrumentation => "bytecode instrumentation",
            RedKind::Unsafe => "Unsafe / jdk.internal",
            RedKind::HardDependency => "hard dependency on a red plugin",
        }
    }
}

/// One red finding, aggregated per (kind, package, origin).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RedSignal {
    pub kind: RedKind,
    /// Package (or plugin name, or string constant) involved.
    pub target: String,
    /// `own` or the bundled library's package root.
    pub origin: String,
    /// References (classes or bytecode sites).
    pub refs: u32,
    /// Whether this signal decides the verdict (instrumentation and Unsafe inside a bundled
    /// general-purpose library do not).
    pub decisive: bool,
}

/// An API class the plugin references.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypeUse {
    pub class: String,
    /// How many classes of the jar reference it.
    pub classes: u32,
    pub status: Status,
}

/// An API member the plugin references.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemberUse {
    pub kind: MemberKind,
    pub owner: String,
    pub name: String,
    pub descriptor: String,
    /// Bytecode sites (invoke/field instructions, method handles).
    pub sites: u32,
    /// Whether the plugin's own code uses it (not only a bundled library).
    pub own_code: bool,
    pub status: Status,
}

impl MemberUse {
    /// `org.bukkit.entity.Player#sendMessage`.
    pub fn key(&self) -> String {
        format!("{}#{}", self.owner, self.name)
    }
}

/// A package root and how often it appears.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Count {
    pub name: String,
    pub count: u32,
}

/// Metadata of a panel entry (from `bridge/panel/panel.json`), attached by the caller.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PanelInfo {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub project_url: String,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub sha256: String,
    #[serde(default)]
    pub jar: String,
}

/// Counters for one jar.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stats {
    pub classes: u32,
    pub own_classes: u32,
    pub library_classes: u32,
    pub nested_jars: Vec<String>,
    pub parse_errors: u32,
    /// Calls to `Class.forName`, `getDeclared*`, `setAccessible`, `MethodHandles.Lookup#find*`...
    pub reflection_calls: u32,
    /// Highest class-file major version (65 = Java 21).
    pub max_class_version: u16,
    /// Package prefix taken as the plugin's own code.
    pub own_prefix: String,
}

/// The analysis of one plugin jar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginReport {
    pub file: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub panel: Option<PanelInfo>,
    pub meta: Option<PluginMeta>,
    pub verdict: Verdict,
    pub reasons: Vec<String>,
    /// Red only because of bundled libraries (the plugin's own code is not red).
    pub red_only_in_libraries: bool,
    pub stats: Stats,
    pub bundled_libraries: Vec<Count>,
    pub api_types: Vec<TypeUse>,
    pub api_members: Vec<MemberUse>,
    pub red_signals: Vec<RedSignal>,
    pub platform_libraries: Vec<Count>,
    pub external_apis: Vec<Count>,
    pub other_platforms: Vec<Count>,
    pub errors: Vec<String>,
}

impl PluginReport {
    /// Display name: descriptor name, else panel name, else file name.
    pub fn display_name(&self) -> String {
        if let Some(m) = &self.meta
            && !m.name.is_empty()
        {
            return m.name.clone();
        }
        if let Some(p) = &self.panel
            && !p.name.is_empty()
        {
            return p.name.clone();
        }
        self.file.clone()
    }

    pub fn uncovered_members(&self) -> impl Iterator<Item = &MemberUse> {
        self.api_members.iter().filter(|m| !m.status.is_covered())
    }

    pub fn partial_members(&self) -> impl Iterator<Item = &MemberUse> {
        self.api_members.iter().filter(|m| m.status.is_partial())
    }

    pub fn uncovered_types(&self) -> impl Iterator<Item = &TypeUse> {
        self.api_types.iter().filter(|t| !t.status.is_covered())
    }

    fn failed(file: &str, err: String) -> Self {
        PluginReport {
            file: file.to_owned(),
            panel: None,
            meta: None,
            verdict: Verdict::Error,
            reasons: vec![err.clone()],
            red_only_in_libraries: false,
            stats: Stats::default(),
            bundled_libraries: vec![],
            api_types: vec![],
            api_members: vec![],
            red_signals: vec![],
            platform_libraries: vec![],
            external_apis: vec![],
            other_platforms: vec![],
            errors: vec![err],
        }
    }
}

/// Plugins whose purpose is packet or server-internal access: a hard `depend` on one is red.
pub const RED_PLUGINS: &[&str] = &[
    "ProtocolLib",
    "packetevents",
    "PacketEvents",
    "ProtocolSupport",
    "TinyProtocol",
    "NBTAPI",
];

/// Path segments that mark relocated (shaded) library code.
const SHADE_MARKERS: &[&str] = &[
    "lib",
    "libs",
    "libraries",
    "library",
    "shaded",
    "shade",
    "relocated",
    "relocate",
    "relocations",
    "dependencies",
    "deps",
    "dependency",
    "thirdparty",
    "third_party",
    "vendor",
    "external",
    "repack",
    "repackaged",
];

/// Reflection entry points (owner, name).
const REFLECTION: &[(&str, &str)] = &[
    ("java/lang/Class", "forName"),
    ("java/lang/Class", "getDeclaredField"),
    ("java/lang/Class", "getDeclaredFields"),
    ("java/lang/Class", "getDeclaredMethod"),
    ("java/lang/Class", "getDeclaredMethods"),
    ("java/lang/Class", "getDeclaredConstructor"),
    ("java/lang/Class", "getDeclaredConstructors"),
    ("java/lang/Class", "getField"),
    ("java/lang/Class", "getMethod"),
    ("java/lang/Class", "getConstructor"),
    ("java/lang/ClassLoader", "loadClass"),
    ("java/lang/reflect/AccessibleObject", "setAccessible"),
    ("java/lang/reflect/Field", "setAccessible"),
    ("java/lang/reflect/Method", "setAccessible"),
    ("java/lang/reflect/Constructor", "setAccessible"),
    ("java/lang/invoke/MethodHandles", "privateLookupIn"),
    ("java/lang/invoke/MethodHandles$Lookup", "findVirtual"),
    ("java/lang/invoke/MethodHandles$Lookup", "findStatic"),
    ("java/lang/invoke/MethodHandles$Lookup", "findGetter"),
    ("java/lang/invoke/MethodHandles$Lookup", "findSetter"),
    ("java/lang/invoke/MethodHandles$Lookup", "findStaticGetter"),
    ("java/lang/invoke/MethodHandles$Lookup", "findConstructor"),
    ("java/lang/invoke/MethodHandles$Lookup", "findVarHandle"),
    ("java/lang/invoke/MethodHandles$Lookup", "unreflect"),
    ("java/lang/invoke/MethodHandles$Lookup", "unreflectGetter"),
    ("java/lang/invoke/MethodHandles$Lookup", "unreflectSetter"),
];

/// String constants that, with reflection in the jar, mean reflective access to internals.
const INTERNAL_STRINGS: &[&str] = &[
    "net.minecraft.",
    "net/minecraft/",
    "org.bukkit.craftbukkit",
    "org/bukkit/craftbukkit",
    "com.mojang.authlib",
    "io.netty.channel",
];

/// Whether a string constant names server internals: an NMS/CraftBukkit/authlib/Netty name,
/// `getHandle` (the CraftBukkit accessor), or a CraftBukkit class name built at run time
/// (`".entity.CraftPlayer"`, `"CraftServer"`).
pub(crate) fn is_internal_string(s: &str) -> bool {
    if INTERNAL_STRINGS.iter().any(|p| s.starts_with(p)) || s == "getHandle" {
        return true;
    }
    let last = s.rsplit(['.', '/']).next().unwrap_or("");
    last != "CraftBukkit"
        && last
            .strip_prefix("Craft")
            .is_some_and(|r| r.chars().next().is_some_and(|c| c.is_ascii_uppercase()))
        && last.chars().all(|c| c.is_ascii_alphanumeric() || c == '$')
}

/// Analyze a jar file on disk.
pub fn analyze_path(path: &Path, cov: &Coverage) -> PluginReport {
    let file = path
        .file_name()
        .map(|f| f.to_string_lossy().into_owned())
        .unwrap_or_default();
    match std::fs::read(path) {
        Ok(bytes) => analyze_bytes(&file, &bytes, cov),
        Err(e) => PluginReport::failed(&file, format!("cannot read: {e}")),
    }
}

/// Analyze a jar held in memory. `file` is only used as a label.
pub fn analyze_bytes(file: &str, bytes: &[u8], cov: &Coverage) -> PluginReport {
    match jar::read_jar(bytes) {
        Ok(c) => analyze_contents(file, c, cov),
        Err(e) => PluginReport::failed(file, e.to_string()),
    }
}

fn package_root(internal: &str, segments: usize) -> String {
    let parts: Vec<&str> = internal.split('/').collect();
    let pkg = &parts[..parts.len().saturating_sub(1)];
    if pkg.is_empty() {
        return "(default package)".to_owned();
    }
    pkg[..pkg.len().min(segments)].join(".")
}

/// The plugin's own package prefix (internal form, no trailing slash): the `main` class's
/// package cut to three segments. Without a `main`, the most common 3-segment prefix.
fn own_prefix(meta: Option<&PluginMeta>, classes: &[ClassFile]) -> String {
    if let Some(m) = meta
        && !m.main.is_empty()
    {
        let parts: Vec<&str> = m.main.split('.').collect();
        let pkg = &parts[..parts.len().saturating_sub(1)];
        if !pkg.is_empty() {
            return pkg[..pkg.len().min(3)].join("/");
        }
    }
    let mut counts: HashMap<String, u32> = HashMap::new();
    for c in classes {
        *counts
            .entry(package_root(&c.name, 3).replace('.', "/"))
            .or_default() += 1;
    }
    counts
        .into_iter()
        .max_by_key(|(k, n)| (*n, k.clone()))
        .map(|(k, _)| k)
        .unwrap_or_default()
}

/// `None` for own code; otherwise the bundled library's root (dotted).
fn library_of(class: &str, own: &str) -> Option<String> {
    let segs: Vec<&str> = class.split('/').collect();
    let pkg = &segs[..segs.len().saturating_sub(1)];
    let in_own = !own.is_empty() && class.starts_with(own) && class[own.len()..].starts_with('/');
    if let Some(i) = pkg
        .iter()
        .position(|s| SHADE_MARKERS.contains(&s.to_ascii_lowercase().as_str()))
    {
        // Relocated: `me/clip/papi/libs/kyori/adventure/...` → "me.clip.papi.libs.kyori.adventure".
        let end = (i + 3).min(pkg.len());
        return Some(pkg[..end].join("."));
    }
    if in_own {
        None
    } else {
        Some(pkg[..pkg.len().min(3)].join("."))
    }
}

#[derive(Default)]
struct MemberAgg {
    sites: u32,
    own: bool,
}

fn analyze_contents(file: &str, contents: JarContents, cov: &Coverage) -> PluginReport {
    let JarContents {
        classes,
        plugin_yml,
        paper_plugin_yml,
        nested_jars,
        errors,
    } = contents;
    let meta = manifest::merge(
        plugin_yml.as_deref().map(manifest::parse_plugin_yml),
        paper_plugin_yml
            .as_deref()
            .map(manifest::parse_paper_plugin_yml),
    );
    let own = own_prefix(meta.as_ref(), &classes);
    let by_name: HashMap<&str, &ClassFile> = classes.iter().map(|c| (c.name.as_str(), c)).collect();

    let mut stats = Stats {
        classes: classes.len() as u32,
        nested_jars,
        parse_errors: errors.len() as u32,
        own_prefix: own.replace('/', "."),
        ..Stats::default()
    };
    let mut libraries: BTreeMap<String, u32> = BTreeMap::new();
    let mut types: BTreeMap<String, u32> = BTreeMap::new();
    let mut members: BTreeMap<(MemberKind, String, String, String), MemberAgg> = BTreeMap::new();
    let mut red: BTreeMap<(RedKind, String, String), u32> = BTreeMap::new();
    let mut platform: BTreeMap<String, u32> = BTreeMap::new();
    let mut external: BTreeMap<String, u32> = BTreeMap::new();
    let mut other: BTreeMap<String, u32> = BTreeMap::new();
    let mut internal_strings: Vec<(String, String)> = Vec::new(); // (string, origin)

    for c in &classes {
        stats.max_class_version = stats.max_class_version.max(c.major_version);
        let lib = library_of(&c.name, &own);
        let origin = lib.clone().unwrap_or_else(|| "own".to_owned());
        match &lib {
            None => stats.own_classes += 1,
            Some(l) => {
                stats.library_classes += 1;
                *libraries.entry(l.clone()).or_default() += 1;
            }
        }

        let mut note_external =
            |t: &str, n: u32, red: &mut BTreeMap<(RedKind, String, String), u32>| {
                let ns = namespace(t);
                let kind = match ns {
                    Namespace::Internal if t.starts_with("net/minecraft/") => Some(RedKind::Nms),
                    Namespace::Internal if t.starts_with("org/bukkit/craftbukkit/") => {
                        Some(RedKind::CraftBukkit)
                    }
                    Namespace::Internal => Some(RedKind::ServerInternal),
                    Namespace::PacketLibrary => Some(RedKind::PacketLibrary),
                    Namespace::Instrumentation
                        if t.contains("Unsafe") || t.starts_with("jdk/internal/") =>
                    {
                        Some(RedKind::Unsafe)
                    }
                    Namespace::Instrumentation => Some(RedKind::Instrumentation),
                    _ => None,
                };
                if let Some(k) = kind {
                    *red.entry((k, package_root(t, 4), origin.clone()))
                        .or_default() += n;
                }
                match ns {
                    Namespace::PlatformLibrary => {
                        *platform.entry(package_root(t, 3)).or_default() += n
                    }
                    Namespace::External => *external.entry(package_root(t, 3)).or_default() += n,
                    Namespace::OtherPlatform => *other.entry(package_root(t, 3)).or_default() += n,
                    _ => {}
                }
                ns
            };

        for t in &c.type_refs {
            if by_name.contains_key(t.as_str()) {
                continue;
            }
            if note_external(t, 1, &mut red) == Namespace::Api {
                *types.entry(dotted(t)).or_default() += 1;
            }
        }
        for (m, &n) in &c.member_refs {
            if REFLECTION
                .iter()
                .any(|(o, name)| *o == m.owner && *name == m.name)
            {
                stats.reflection_calls += n;
            }
            let owner = if by_name.contains_key(m.owner.as_str()) {
                match resolve_inherited(&by_name, &m.owner, m.kind, &m.name, &m.descriptor) {
                    Some(ext) => ext,
                    None => continue, // the plugin's own member
                }
            } else {
                m.owner.clone()
            };
            // Types are counted from the constant pool already; members count bytecode sites.
            if namespace(&owner) == Namespace::Api {
                let agg = members
                    .entry((m.kind, dotted(&owner), m.name.clone(), m.descriptor.clone()))
                    .or_default();
                agg.sites += n;
                agg.own |= lib.is_none();
                if by_name.contains_key(m.owner.as_str()) {
                    // Inherited API member reached through an own subclass: the type is used.
                    types.entry(dotted(&owner)).or_insert(1);
                }
            }
        }
        for s in &c.strings {
            if is_internal_string(s) {
                internal_strings.push((s.clone(), origin.clone()));
            }
        }
    }

    if stats.reflection_calls > 0 {
        for (s, origin) in &internal_strings {
            let target = if s == "getHandle" {
                "getHandle()".to_owned()
            } else if !INTERNAL_STRINGS.iter().any(|p| s.starts_with(p)) {
                "Craft* class name".to_owned()
            } else {
                let t = s.replace('/', ".");
                let segs: Vec<&str> = t.split('.').take(3).collect();
                segs.join(".")
            };
            *red.entry((RedKind::ReflectionIntoInternals, target, origin.clone()))
                .or_default() += 1;
        }
    }
    if let Some(m) = &meta {
        for d in &m.depend {
            if RED_PLUGINS.iter().any(|r| r.eq_ignore_ascii_case(d)) {
                *red.entry((RedKind::HardDependency, d.clone(), "plugin.yml".into()))
                    .or_default() += 1;
            }
        }
    }

    let red_signals: Vec<RedSignal> = red
        .into_iter()
        .map(|((kind, target, origin), refs)| {
            let decisive =
                !matches!(kind, RedKind::Instrumentation | RedKind::Unsafe) || origin == "own";
            RedSignal {
                kind,
                target,
                origin,
                refs,
                decisive,
            }
        })
        .collect();

    let mut api_types: Vec<TypeUse> = types
        .into_iter()
        .map(|(class, classes)| {
            let status = cov.class_status(&class);
            TypeUse {
                class,
                classes,
                status,
            }
        })
        .collect();
    api_types.sort_by(|a, b| {
        b.classes
            .cmp(&a.classes)
            .then_with(|| a.class.cmp(&b.class))
    });
    let mut api_members: Vec<MemberUse> = members
        .into_iter()
        .map(|((kind, owner, name, descriptor), agg)| {
            let status = cov.member_status(kind, &owner, &name, &descriptor);
            MemberUse {
                kind,
                owner,
                name,
                descriptor,
                sites: agg.sites,
                own_code: agg.own,
                status,
            }
        })
        .collect();
    api_members.sort_by(|a, b| b.sites.cmp(&a.sites).then_with(|| a.key().cmp(&b.key())));

    // Verdict and reasons.
    let decisive: Vec<&RedSignal> = red_signals.iter().filter(|r| r.decisive).collect();
    let mut reasons = Vec::new();
    if meta.is_none() {
        reasons.push("no plugin.yml or paper-plugin.yml: not a Bukkit/Paper plugin (library or other platform)".into());
    }
    let verdict = if !decisive.is_empty() {
        let kinds: BTreeSet<RedKind> = decisive.iter().map(|r| r.kind).collect();
        for k in kinds {
            let mut origins: BTreeSet<&str> = BTreeSet::new();
            let mut targets: BTreeSet<&str> = BTreeSet::new();
            let mut refs = 0;
            for r in decisive.iter().filter(|r| r.kind == k) {
                origins.insert(&r.origin);
                targets.insert(&r.target);
                refs += r.refs;
            }
            let t: Vec<&str> = targets.into_iter().take(3).collect();
            let o: Vec<&str> = origins.into_iter().take(3).collect();
            reasons.push(format!(
                "{}: {} ({} refs, in {})",
                k.label(),
                t.join(", "),
                refs,
                o.join(", ")
            ));
        }
        Verdict::Red
    } else if api_members.iter().any(|m| !m.status.is_covered())
        || api_types.iter().any(|t| !t.status.is_covered())
    {
        Verdict::Yellow
    } else {
        Verdict::Green
    };
    let red_only_in_libraries =
        verdict == Verdict::Red && decisive.iter().all(|r| r.origin != "own");
    let n_unc = api_members
        .iter()
        .filter(|m| !m.status.is_covered())
        .count();
    let n_unc_types = api_types.iter().filter(|t| !t.status.is_covered()).count();
    if n_unc + n_unc_types > 0 {
        let top: Vec<String> = api_members
            .iter()
            .filter(|m| !m.status.is_covered())
            .take(5)
            .map(MemberUse::key)
            .collect();
        reasons.push(format!(
            "uses {n_unc} uncovered API members and {n_unc_types} uncovered API types (top: {})",
            top.join(", ")
        ));
    }
    if classes.is_empty() {
        reasons.push("no readable classes".into());
    }

    let counts = |m: BTreeMap<String, u32>| -> Vec<Count> {
        let mut v: Vec<Count> = m
            .into_iter()
            .map(|(name, count)| Count { name, count })
            .collect();
        v.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.name.cmp(&b.name)));
        v
    };
    PluginReport {
        file: file.to_owned(),
        panel: None,
        meta,
        verdict,
        reasons,
        red_only_in_libraries,
        stats,
        bundled_libraries: counts(libraries),
        api_types,
        api_members,
        red_signals,
        platform_libraries: counts(platform),
        external_apis: counts(external),
        other_platforms: counts(other),
        errors: errors.into_iter().take(20).collect(),
    }
}

/// A member referenced on a class of the jar but declared outside it (`this.getConfig()` in a
/// `JavaPlugin` subclass, `task.runTaskTimer(...)` on a `BukkitRunnable` subclass): the external
/// class it comes from, or `None` when the jar declares it. The superclass chain wins; external
/// interfaces are used when the chain ends in the JDK (default/abstract interface methods).
fn resolve_inherited(
    by_name: &HashMap<&str, &ClassFile>,
    owner: &str,
    kind: MemberKind,
    name: &str,
    desc: &str,
) -> Option<String> {
    let declares = |c: &ClassFile| match kind {
        MemberKind::Method => c
            .declared_methods
            .contains(&(name.to_owned(), desc.to_owned())),
        MemberKind::Field => c.declared_fields.contains(name),
    };
    let mut ext_super: Option<String> = None;
    let mut iface_queue: Vec<String> = Vec::new();
    let mut cur = Some(owner.to_owned());
    let mut steps = 0;
    while let Some(name_) = cur.take() {
        steps += 1;
        let Some(c) = by_name.get(name_.as_str()).filter(|_| steps < 64) else {
            ext_super = Some(name_);
            break;
        };
        if declares(c) {
            return None;
        }
        iface_queue.extend(c.interfaces.iter().cloned());
        cur = c.super_name.clone();
    }
    let mut ext_ifaces = Vec::new();
    let mut seen = BTreeSet::new();
    while let Some(i) = iface_queue.pop() {
        if !seen.insert(i.clone()) || seen.len() > 256 {
            continue;
        }
        match by_name.get(i.as_str()) {
            Some(c) if declares(c) => return None,
            Some(c) => iface_queue.extend(c.interfaces.iter().cloned()),
            None => ext_ifaces.push(i),
        }
    }
    match ext_super {
        Some(s) if namespace(&s) != Namespace::Jdk => Some(s),
        s => ext_ifaces
            .into_iter()
            .find(|i| namespace(i) != Namespace::Jdk)
            .or(s),
    }
}
