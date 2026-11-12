//! Plugin packages: `.rplugin` archives (zip) and unpacked plugin directories (dev mode).
//!
//! Layout (both forms):
//!
//! ```text
//! rustic.plugin.toml      manifest (required, at the root)
//! <entry>.wasm            the component named by `plugin.entry` (required)
//! config.toml             default configuration (optional, must be valid TOML)
//! resources/...           read-only resources (optional)
//! ```
//!
//! Reading is defensive: entry names must be relative without `..`, sizes are capped
//! ([`limits`]) while decompressing, and anything outside the layout is ignored.

use crate::manifest::{MANIFEST_FILE, Manifest, ManifestError, validate_entry_path};
use std::collections::BTreeMap;
use std::fs;
use std::io::{Cursor, Read, Seek, Write};
use std::path::{Path, PathBuf};

/// File extension of plugin archives.
pub const ARCHIVE_EXTENSION: &str = "rplugin";
/// Default configuration file name.
pub const DEFAULT_CONFIG_FILE: &str = "config.toml";
/// Resources directory name.
pub const RESOURCES_DIR: &str = "resources";

/// Size caps applied while reading packages.
pub mod limits {
    /// Manifest.
    pub const MANIFEST_BYTES: u64 = 64 * 1024;
    /// Default config.
    pub const CONFIG_BYTES: u64 = 1024 * 1024;
    /// The component.
    pub const ENTRY_BYTES: u64 = 256 * 1024 * 1024;
    /// One resource.
    pub const RESOURCE_BYTES: u64 = 64 * 1024 * 1024;
    /// Entries in an archive.
    pub const ENTRIES: usize = 10_000;
}

/// Errors reading or writing a plugin package.
#[derive(Debug, thiserror::Error)]
pub enum ArchiveError {
    /// File system error.
    #[error("{path}: {source}")]
    Io {
        /// Path.
        path: PathBuf,
        /// Cause.
        #[source]
        source: std::io::Error,
    },
    /// Not a readable zip.
    #[error("{path}: not a valid .rplugin archive: {message}")]
    Zip {
        /// Path.
        path: PathBuf,
        /// Cause.
        message: String,
    },
    /// No manifest at the root.
    #[error("{0}: missing {MANIFEST_FILE} at the root")]
    MissingManifest(PathBuf),
    /// Invalid manifest.
    #[error("{path}: {source}")]
    Manifest {
        /// Package path.
        path: PathBuf,
        /// Cause.
        #[source]
        source: ManifestError,
    },
    /// The entry named by the manifest is missing.
    #[error("{path}: entry `{entry}` named in {MANIFEST_FILE} does not exist")]
    MissingEntry {
        /// Package path.
        path: PathBuf,
        /// Entry.
        entry: String,
    },
    /// A file in the archive has an unsafe name.
    #[error("{path}: unsafe file name `{name}` in archive")]
    UnsafeName {
        /// Package path.
        path: PathBuf,
        /// The name.
        name: String,
    },
    /// A file exceeds its size cap.
    #[error("{path}: `{name}` is larger than {limit} bytes")]
    TooLarge {
        /// Package path.
        path: PathBuf,
        /// File.
        name: String,
        /// Cap.
        limit: u64,
    },
    /// The default config is not valid TOML / not UTF-8.
    #[error("{path}: invalid {DEFAULT_CONFIG_FILE}: {message}")]
    Config {
        /// Package path.
        path: PathBuf,
        /// Cause.
        message: String,
    },
    /// Bad input to the writer.
    #[error("cannot build package: {0}")]
    Build(String),
}

/// Where a plugin comes from.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum PluginSource {
    /// A `.rplugin` file.
    Archive(PathBuf),
    /// An unpacked directory (dev mode).
    Directory(PathBuf),
    /// Built in memory (tests, the test harness); the string is a label shown in messages.
    Memory(String),
}

impl PluginSource {
    /// The path.
    pub fn path(&self) -> &Path {
        match self {
            PluginSource::Archive(p) | PluginSource::Directory(p) => p,
            PluginSource::Memory(label) => Path::new(label.as_str()),
        }
    }

    /// Whether this is an unpacked directory.
    pub fn is_directory(&self) -> bool {
        matches!(self, PluginSource::Directory(_))
    }
}

/// A plugin package whose manifest has been read and validated and whose entry exists.
#[derive(Clone, Debug)]
pub struct PluginPackage {
    /// The validated manifest.
    pub manifest: Manifest,
    /// Where it comes from.
    pub source: PluginSource,
    has_default_config: bool,
    resources: Vec<String>,
    memory: Option<std::sync::Arc<MemoryFiles>>,
}

#[derive(Debug)]
struct MemoryFiles {
    entry: Vec<u8>,
    config: Option<String>,
    resources: BTreeMap<String, Vec<u8>>,
}

fn io_err(path: &Path) -> impl FnOnce(std::io::Error) -> ArchiveError + '_ {
    move |source| ArchiveError::Io {
        path: path.to_owned(),
        source,
    }
}

/// Whether `name` is a safe relative path inside a package.
pub fn is_safe_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('/')
        && !name.contains('\\')
        && !name.contains(':')
        && !name.contains('\0')
        && name
            .trim_end_matches('/')
            .split('/')
            .all(|seg| !seg.is_empty() && seg != "." && seg != "..")
}

fn read_capped<R: Read>(
    r: R,
    limit: u64,
    path: &Path,
    name: &str,
) -> Result<Vec<u8>, ArchiveError> {
    let mut buf = Vec::new();
    r.take(limit + 1)
        .read_to_end(&mut buf)
        .map_err(io_err(path))?;
    if buf.len() as u64 > limit {
        return Err(ArchiveError::TooLarge {
            path: path.to_owned(),
            name: name.to_owned(),
            limit,
        });
    }
    Ok(buf)
}

fn utf8(bytes: Vec<u8>, path: &Path, name: &str) -> Result<String, ArchiveError> {
    String::from_utf8(bytes).map_err(|_| ArchiveError::Config {
        path: path.to_owned(),
        message: format!("`{name}` is not UTF-8"),
    })
}

fn check_config(text: &str, path: &Path) -> Result<(), ArchiveError> {
    toml::from_str::<toml::Table>(text)
        .map(|_| ())
        .map_err(|e| ArchiveError::Config {
            path: path.to_owned(),
            message: e.to_string().trim().to_owned(),
        })
}

fn zip_err(path: &Path) -> impl FnOnce(zip::result::ZipError) -> ArchiveError + '_ {
    move |e| ArchiveError::Zip {
        path: path.to_owned(),
        message: e.to_string(),
    }
}

fn open_zip(path: &Path) -> Result<zip::ZipArchive<fs::File>, ArchiveError> {
    let f = fs::File::open(path).map_err(io_err(path))?;
    zip::ZipArchive::new(f).map_err(zip_err(path))
}

fn zip_read<R: Read + Seek>(
    zip: &mut zip::ZipArchive<R>,
    name: &str,
    limit: u64,
    path: &Path,
) -> Result<Option<Vec<u8>>, ArchiveError> {
    match zip.by_name(name) {
        Ok(file) => {
            if file.size() > limit {
                return Err(ArchiveError::TooLarge {
                    path: path.to_owned(),
                    name: name.to_owned(),
                    limit,
                });
            }
            read_capped(file, limit, path, name).map(Some)
        }
        Err(zip::result::ZipError::FileNotFound) => Ok(None),
        Err(e) => Err(zip_err(path)(e)),
    }
}

fn list_dir_files(root: &Path, dir: &Path, out: &mut Vec<String>) -> Result<(), ArchiveError> {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .map_err(io_err(dir))?
        .collect::<Result<_, _>>()
        .map_err(io_err(dir))?;
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let p = e.path();
        let ft = e.file_type().map_err(io_err(&p))?;
        if ft.is_dir() {
            list_dir_files(root, &p, out)?;
        } else if ft.is_file() {
            if let Ok(rel) = p.strip_prefix(root) {
                let rel: Vec<String> = rel
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect();
                out.push(rel.join("/"));
            }
            if out.len() > limits::ENTRIES {
                return Err(ArchiveError::Build(format!(
                    "{}: more than {} resources",
                    root.display(),
                    limits::ENTRIES
                )));
            }
        }
    }
    Ok(())
}

impl PluginPackage {
    /// Open a `.rplugin` file or an unpacked plugin directory.
    pub fn open(path: &Path) -> Result<Self, ArchiveError> {
        if path.is_dir() {
            Self::open_dir(path)
        } else {
            Self::open_archive(path)
        }
    }

    /// Open a `.rplugin` archive.
    pub fn open_archive(path: &Path) -> Result<Self, ArchiveError> {
        let mut zip = open_zip(path)?;
        if zip.len() > limits::ENTRIES {
            return Err(ArchiveError::Zip {
                path: path.to_owned(),
                message: format!("more than {} entries", limits::ENTRIES),
            });
        }
        let mut resources = Vec::new();
        for i in 0..zip.len() {
            let f = zip.by_index_raw(i).map_err(zip_err(path))?;
            let name = f.name().map_err(zip_err(path))?.into_owned();
            if !is_safe_name(&name) {
                return Err(ArchiveError::UnsafeName {
                    path: path.to_owned(),
                    name,
                });
            }
            if !f.is_dir()
                && let Some(rest) = name.strip_prefix("resources/")
                && !rest.is_empty()
            {
                resources.push(rest.to_owned());
            }
        }
        resources.sort();
        let manifest_bytes = zip_read(&mut zip, MANIFEST_FILE, limits::MANIFEST_BYTES, path)?
            .ok_or_else(|| ArchiveError::MissingManifest(path.to_owned()))?;
        let manifest_text = utf8(manifest_bytes, path, MANIFEST_FILE)?;
        let manifest =
            Manifest::parse(&manifest_text).map_err(|source| ArchiveError::Manifest {
                path: path.to_owned(),
                source,
            })?;
        match zip.by_name(&manifest.entry) {
            Ok(_) => {}
            Err(zip::result::ZipError::FileNotFound) => {
                return Err(ArchiveError::MissingEntry {
                    path: path.to_owned(),
                    entry: manifest.entry.clone(),
                });
            }
            Err(e) => return Err(zip_err(path)(e)),
        }
        let has_default_config =
            match zip_read(&mut zip, DEFAULT_CONFIG_FILE, limits::CONFIG_BYTES, path)? {
                Some(bytes) => {
                    check_config(&utf8(bytes, path, DEFAULT_CONFIG_FILE)?, path)?;
                    true
                }
                None => false,
            };
        Ok(Self {
            manifest,
            source: PluginSource::Archive(path.to_owned()),
            has_default_config,
            resources,
            memory: None,
        })
    }

    /// Open an unpacked plugin directory.
    pub fn open_dir(dir: &Path) -> Result<Self, ArchiveError> {
        let mpath = dir.join(MANIFEST_FILE);
        if !mpath.is_file() {
            return Err(ArchiveError::MissingManifest(dir.to_owned()));
        }
        let meta = fs::metadata(&mpath).map_err(io_err(&mpath))?;
        if meta.len() > limits::MANIFEST_BYTES {
            return Err(ArchiveError::TooLarge {
                path: dir.to_owned(),
                name: MANIFEST_FILE.into(),
                limit: limits::MANIFEST_BYTES,
            });
        }
        let text = fs::read_to_string(&mpath).map_err(io_err(&mpath))?;
        let manifest = Manifest::parse(&text).map_err(|source| ArchiveError::Manifest {
            path: dir.to_owned(),
            source,
        })?;
        if !dir.join(&manifest.entry).is_file() {
            return Err(ArchiveError::MissingEntry {
                path: dir.to_owned(),
                entry: manifest.entry.clone(),
            });
        }
        let cpath = dir.join(DEFAULT_CONFIG_FILE);
        let has_default_config = if cpath.is_file() {
            let text = fs::read_to_string(&cpath).map_err(io_err(&cpath))?;
            check_config(&text, dir)?;
            true
        } else {
            false
        };
        let mut resources = Vec::new();
        let rdir = dir.join(RESOURCES_DIR);
        if rdir.is_dir() {
            list_dir_files(&rdir, &rdir, &mut resources)?;
        }
        Ok(Self {
            manifest,
            source: PluginSource::Directory(dir.to_owned()),
            has_default_config,
            resources,
            memory: None,
        })
    }

    /// A package that exists only in memory (tests, the test harness, native plugins).
    /// Reloading it gives the same package back.
    pub fn in_memory(
        manifest: Manifest,
        entry: Vec<u8>,
        default_config: Option<String>,
        resources: BTreeMap<String, Vec<u8>>,
    ) -> Result<Self, ArchiveError> {
        let label = format!("<memory:{}>", manifest.id);
        if let Some(c) = &default_config {
            check_config(c, Path::new(&label))?;
        }
        if let Some(bad) = resources.keys().find(|k| !is_safe_name(k)) {
            return Err(ArchiveError::UnsafeName {
                path: PathBuf::from(&label),
                name: bad.clone(),
            });
        }
        Ok(Self {
            manifest,
            source: PluginSource::Memory(label),
            has_default_config: default_config.is_some(),
            resources: resources.keys().cloned().collect(),
            memory: Some(std::sync::Arc::new(MemoryFiles {
                entry,
                config: default_config,
                resources,
            })),
        })
    }

    /// Read the package again from its source (reload). Memory packages return themselves.
    pub fn reopen(&self) -> Result<Self, ArchiveError> {
        match &self.source {
            PluginSource::Archive(p) => Self::open_archive(p),
            PluginSource::Directory(p) => Self::open_dir(p),
            PluginSource::Memory(_) => Ok(self.clone()),
        }
    }

    /// The package path.
    pub fn path(&self) -> &Path {
        self.source.path()
    }

    /// Read the component bytes.
    pub fn read_entry(&self) -> Result<Vec<u8>, ArchiveError> {
        self.read_file(&self.manifest.entry.clone(), limits::ENTRY_BYTES)?
            .ok_or_else(|| ArchiveError::MissingEntry {
                path: self.path().to_owned(),
                entry: self.manifest.entry.clone(),
            })
    }

    /// Whether the package ships a default `config.toml`.
    pub fn has_default_config(&self) -> bool {
        self.has_default_config
    }

    /// The default `config.toml` text, if any.
    pub fn default_config(&self) -> Result<Option<String>, ArchiveError> {
        if !self.has_default_config {
            return Ok(None);
        }
        match self.read_file(DEFAULT_CONFIG_FILE, limits::CONFIG_BYTES)? {
            Some(bytes) => utf8(bytes, self.path(), DEFAULT_CONFIG_FILE).map(Some),
            None => Ok(None),
        }
    }

    /// Resource paths (relative to `resources/`, forward slashes, sorted).
    pub fn resources(&self) -> &[String] {
        &self.resources
    }

    /// Read a resource by its path relative to `resources/`.
    pub fn read_resource(&self, name: &str) -> Result<Option<Vec<u8>>, ArchiveError> {
        if !is_safe_name(name) {
            return Err(ArchiveError::UnsafeName {
                path: self.path().to_owned(),
                name: name.to_owned(),
            });
        }
        if !self.resources.iter().any(|r| r == name) {
            return Ok(None);
        }
        self.read_file(&format!("{RESOURCES_DIR}/{name}"), limits::RESOURCE_BYTES)
    }

    fn read_file(&self, name: &str, limit: u64) -> Result<Option<Vec<u8>>, ArchiveError> {
        if let Some(mem) = &self.memory {
            if name == self.manifest.entry {
                return Ok(Some(mem.entry.clone()));
            }
            if name == DEFAULT_CONFIG_FILE {
                return Ok(mem.config.clone().map(String::into_bytes));
            }
            return Ok(name
                .strip_prefix("resources/")
                .and_then(|r| mem.resources.get(r).cloned()));
        }
        match &self.source {
            PluginSource::Archive(path) => {
                let mut zip = open_zip(path)?;
                zip_read(&mut zip, name, limit, path)
            }
            PluginSource::Directory(dir) => {
                let p = dir.join(name);
                if !p.is_file() {
                    return Ok(None);
                }
                let f = fs::File::open(&p).map_err(io_err(&p))?;
                read_capped(f, limit, dir, name).map(Some)
            }
            PluginSource::Memory(_) => Ok(None),
        }
    }
}

/// Builds `.rplugin` archives (used by `rustic build` / `rustic package`).
#[derive(Clone, Debug)]
pub struct RpluginBuilder {
    manifest: Manifest,
    entry: Vec<u8>,
    config: Option<String>,
    resources: BTreeMap<String, Vec<u8>>,
}

impl RpluginBuilder {
    /// Start with a validated manifest and the component bytes (stored at `manifest.entry`).
    pub fn new(manifest: Manifest, entry: Vec<u8>) -> Self {
        Self {
            manifest,
            entry,
            config: None,
            resources: BTreeMap::new(),
        }
    }

    /// Start from manifest text (validated).
    pub fn from_manifest_text(text: &str, entry: Vec<u8>) -> Result<Self, ArchiveError> {
        let manifest = Manifest::parse(text).map_err(|source| ArchiveError::Manifest {
            path: PathBuf::from(MANIFEST_FILE),
            source,
        })?;
        Ok(Self::new(manifest, entry))
    }

    /// Add the default `config.toml` (must be valid TOML).
    pub fn default_config(mut self, text: impl Into<String>) -> Result<Self, ArchiveError> {
        let text = text.into();
        check_config(&text, Path::new(DEFAULT_CONFIG_FILE))?;
        self.config = Some(text);
        Ok(self)
    }

    /// Add a resource at `resources/<name>`.
    pub fn resource(mut self, name: &str, bytes: Vec<u8>) -> Result<Self, ArchiveError> {
        if !is_safe_name(name) || name.ends_with('/') {
            return Err(ArchiveError::Build(format!(
                "unsafe resource name `{name}`"
            )));
        }
        if bytes.len() as u64 > limits::RESOURCE_BYTES {
            return Err(ArchiveError::Build(format!(
                "resource `{name}` is too large"
            )));
        }
        self.resources.insert(name.to_owned(), bytes);
        Ok(self)
    }

    /// The manifest.
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// The archive bytes (deterministic: fixed timestamps, sorted entries).
    pub fn to_bytes(&self) -> Result<Vec<u8>, ArchiveError> {
        validate_entry_path(&self.manifest.entry).map_err(ArchiveError::Build)?;
        if self.entry.len() as u64 > limits::ENTRY_BYTES {
            return Err(ArchiveError::Build("the component is too large".into()));
        }
        let path = Path::new("<memory>");
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o644);
        let add = |zip: &mut zip::ZipWriter<Cursor<Vec<u8>>>, name: &str, bytes: &[u8]| {
            zip.start_file(name, opts).map_err(zip_err(path))?;
            zip.write_all(bytes).map_err(io_err(path))
        };
        add(&mut zip, MANIFEST_FILE, self.manifest.to_toml().as_bytes())?;
        add(&mut zip, &self.manifest.entry, &self.entry)?;
        if let Some(c) = &self.config {
            add(&mut zip, DEFAULT_CONFIG_FILE, c.as_bytes())?;
        }
        for (name, bytes) in &self.resources {
            add(&mut zip, &format!("{RESOURCES_DIR}/{name}"), bytes)?;
        }
        let cursor = zip.finish().map_err(zip_err(path))?;
        Ok(cursor.into_inner())
    }

    /// Write the archive atomically to `out` (temporary file + rename).
    pub fn write_to(&self, out: &Path) -> Result<(), ArchiveError> {
        let bytes = self.to_bytes()?;
        crate::storage::write_atomic(out, &bytes).map_err(io_err(out))
    }
}

/// Pack an unpacked plugin directory into a `.rplugin` file; returns its manifest.
pub fn pack_directory(dir: &Path, out: &Path) -> Result<Manifest, ArchiveError> {
    let pkg = PluginPackage::open_dir(dir)?;
    let mut b = RpluginBuilder::new(pkg.manifest.clone(), pkg.read_entry()?);
    if let Some(c) = pkg.default_config()? {
        b = b.default_config(c)?;
    }
    for r in pkg.resources() {
        if let Some(bytes) = pkg.read_resource(r)? {
            b = b.resource(r, bytes)?;
        }
    }
    b.write_to(out)?;
    Ok(pkg.manifest)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn manifest_text(id: &str, version: &str, deps: &str) -> String {
        format!(
            "[plugin]\nid = \"{id}\"\nname = \"{id}\"\nversion = \"{version}\"\napi = \"0.1\"\n\
             authors = [\"t\"]\nlicense = \"MIT\"\nentry = \"{id}.wasm\"\n{deps}"
        )
    }

    #[test]
    fn write_and_read_archive() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("homes.rplugin");
        RpluginBuilder::from_manifest_text(&manifest_text("homes", "1.0.0", ""), b"\0asm".to_vec())
            .unwrap()
            .default_config("max_homes = 3\n")
            .unwrap()
            .resource("lang/en.toml", b"hi = 'Hi'".to_vec())
            .unwrap()
            .write_to(&out)
            .unwrap();
        let pkg = PluginPackage::open(&out).unwrap();
        assert_eq!(pkg.manifest.id, "homes");
        assert_eq!(pkg.read_entry().unwrap(), b"\0asm");
        assert_eq!(pkg.default_config().unwrap().unwrap(), "max_homes = 3\n");
        assert_eq!(pkg.resources(), ["lang/en.toml"]);
        assert_eq!(
            pkg.read_resource("lang/en.toml").unwrap().unwrap(),
            b"hi = 'Hi'"
        );
        assert!(pkg.read_resource("nope").unwrap().is_none());
        assert!(pkg.read_resource("../x").is_err());
        // Deterministic output.
        let a = RpluginBuilder::from_manifest_text(&manifest_text("homes", "1.0.0", ""), vec![1])
            .unwrap();
        assert_eq!(a.to_bytes().unwrap(), a.to_bytes().unwrap());
    }

    #[test]
    fn directory_and_pack() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("homes-dev");
        fs::create_dir_all(src.join("resources/sub")).unwrap();
        fs::write(src.join(MANIFEST_FILE), manifest_text("homes", "1.0.0", "")).unwrap();
        fs::write(src.join("homes.wasm"), b"wasm").unwrap();
        fs::write(src.join("resources/sub/a.txt"), b"a").unwrap();
        let pkg = PluginPackage::open(&src).unwrap();
        assert!(pkg.source.is_directory());
        assert!(!pkg.has_default_config());
        assert_eq!(pkg.resources(), ["sub/a.txt"]);
        let out = dir.path().join("homes.rplugin");
        pack_directory(&src, &out).unwrap();
        let packed = PluginPackage::open(&out).unwrap();
        assert_eq!(packed.read_entry().unwrap(), b"wasm");
        assert_eq!(packed.read_resource("sub/a.txt").unwrap().unwrap(), b"a");
    }

    #[test]
    fn errors() {
        let dir = tempfile::tempdir().unwrap();
        // Missing entry.
        let d = dir.path().join("x");
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join(MANIFEST_FILE), manifest_text("x", "1.0.0", "")).unwrap();
        assert!(matches!(
            PluginPackage::open(&d),
            Err(ArchiveError::MissingEntry { .. })
        ));
        // Bad default config.
        fs::write(d.join("x.wasm"), b"w").unwrap();
        fs::write(d.join(DEFAULT_CONFIG_FILE), "= nope").unwrap();
        assert!(matches!(
            PluginPackage::open(&d),
            Err(ArchiveError::Config { .. })
        ));
        // Missing manifest.
        let e = dir.path().join("empty");
        fs::create_dir_all(&e).unwrap();
        assert!(matches!(
            PluginPackage::open(&e),
            Err(ArchiveError::MissingManifest(_))
        ));
        // Not a zip.
        let z = dir.path().join("bad.rplugin");
        fs::write(&z, b"not a zip").unwrap();
        assert!(matches!(
            PluginPackage::open(&z),
            Err(ArchiveError::Zip { .. })
        ));
        // Unsafe names written by a hostile tool.
        let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let o = zip::write::SimpleFileOptions::default();
        w.start_file(MANIFEST_FILE, o).unwrap();
        w.write_all(manifest_text("y", "1.0.0", "").as_bytes())
            .unwrap();
        w.start_file("../evil.wasm", o).unwrap();
        w.write_all(b"x").unwrap();
        let bytes = w.finish().unwrap().into_inner();
        let h = dir.path().join("hostile.rplugin");
        fs::write(&h, bytes).unwrap();
        assert!(matches!(
            PluginPackage::open(&h),
            Err(ArchiveError::UnsafeName { .. })
        ));
        // Builder refuses unsafe resource names and bad config.
        let b =
            RpluginBuilder::from_manifest_text(&manifest_text("z", "1.0.0", ""), vec![]).unwrap();
        assert!(b.clone().resource("../a", vec![]).is_err());
        assert!(b.default_config("[[").is_err());
    }

    #[test]
    fn safe_names() {
        assert!(is_safe_name("a/b.txt"));
        assert!(is_safe_name("resources/"));
        assert!(!is_safe_name("/a"));
        assert!(!is_safe_name("a/../b"));
        assert!(!is_safe_name("a\\b"));
        assert!(!is_safe_name("C:/x"));
        assert!(!is_safe_name(""));
    }
}
