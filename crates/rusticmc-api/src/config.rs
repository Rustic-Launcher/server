//! `config.toml` helpers: load a serde struct with defaults from the plugin's data folder, write
//! the defaults when the file is missing, and migrate old files with `config_version`.
//!
//! Versioning follows `docs/PLUGIN_FORMAT.md`: the packaged default config starts with
//! `config_version = N`; pass it with [`Config::defaults`] (its version becomes the target) and
//! upgrade older admin files with the hook [`Config::migration`]
//! (`|from: u32, table: &mut toml::Table|`, once per step) or per-step [`Config::migrate`]
//! closures. Migrated files are written back (comments of the old file are lost).
//!
//! The data folder is preopened as `/` through WASI when the manifest grants
//! `capabilities.filesystem = "data-folder"`, so plain `std::fs` works on it.
//!
//! ```
//! # #[cfg(feature = "config")] {
//! use rusticmc_api::config::Config;
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Serialize, Deserialize, Default, Debug, PartialEq)]
//! #[serde(default)]
//! struct Settings { greeting: String, max_homes: u32 }
//!
//! let config = Config::new()
//!     .version(2)
//!     // v1 called it `homes`; v2 renamed it to `max_homes`.
//!     .migrate(1, |t| {
//!         if let Some(v) = t.remove("homes") { t.insert("max_homes".into(), v); }
//!         Ok(())
//!     });
//! // `load()` reads /config.toml; `load_from_str` is the same logic without the file.
//! let (s, rewritten) = config.load_from_str::<Settings>(Some("config_version = 1\nhomes = 3")).unwrap();
//! assert_eq!(s.max_homes, 3);
//! assert!(rewritten.unwrap().starts_with("config_version = 2\n"));
//! # }
//! ```
#![cfg(feature = "config")]

use std::path::{Path, PathBuf};

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::error::{Error, Result};

/// Where the data folder is mounted inside the component.
pub const DATA_DIR: &str = "/";

/// The key holding the file's schema version.
pub const VERSION_KEY: &str = "config_version";

type Migration = Box<dyn Fn(&mut toml::Table) -> Result<()>>;
type MigrationHook = Box<dyn Fn(u32, &mut toml::Table) -> Result<()>>;

/// A config file loader (see the module docs).
pub struct Config {
    path: PathBuf,
    version: Option<u32>,
    migrations: Vec<(u32, Migration)>,
    hook: Option<MigrationHook>,
    defaults: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Config::new()
    }
}

impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("path", &self.path)
            .field("version", &self.version)
            .field(
                "migrations",
                &self.migrations.iter().map(|(v, _)| v).collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl Config {
    /// `config.toml` in the data folder.
    pub fn new() -> Config {
        Config::file("config.toml")
    }

    /// Another file of the data folder (`messages.toml`).
    pub fn file(name: impl AsRef<Path>) -> Config {
        Config {
            path: Path::new(DATA_DIR).join(name),
            version: None,
            migrations: Vec::new(),
            hook: None,
            defaults: None,
        }
    }

    /// The current schema version. Files written by the SDK start with `config_version = N`; a
    /// file without the key counts as version 1.
    pub fn version(mut self, version: u32) -> Config {
        self.version = Some(version);
        self
    }

    /// A migration from version `from` to `from + 1`, editing the raw TOML table. Migrations
    /// run in order; a missing step just bumps the version.
    pub fn migrate<F>(mut self, from: u32, migration: F) -> Config
    where
        F: Fn(&mut toml::Table) -> Result<()> + 'static,
    {
        self.migrations.push((from, Box::new(migration)));
        self.migrations.sort_by_key(|(v, _)| *v);
        self
    }

    /// The migration hook described in `docs/PLUGIN_FORMAT.md`: called once per version step
    /// with the version being upgraded **from** (`from` → `from + 1`) and the admin's raw table,
    /// after any per-step [`Config::migrate`] closure for that step.
    pub fn migration<F>(mut self, hook: F) -> Config
    where
        F: Fn(u32, &mut toml::Table) -> Result<()> + 'static,
    {
        self.hook = Some(Box::new(hook));
        self
    }

    /// The plugin's packaged default config (usually `include_str!("../config.toml")`). Its
    /// `config_version` is the target version unless [`Config::version`] sets one, and it is
    /// written verbatim (comments included) when the data folder has no file yet. The server
    /// normally installs the packaged file itself on first enable; this covers dev setups and
    /// plugins loaded without one.
    pub fn defaults(mut self, text: impl Into<String>) -> Config {
        self.defaults = Some(text.into());
        self
    }

    /// The version files are migrated to: [`Config::version`], else the defaults'
    /// `config_version`.
    pub fn target_version(&self) -> Result<Option<u32>> {
        if self.version.is_some() {
            return Ok(self.version);
        }
        let Some(d) = &self.defaults else {
            return Ok(None);
        };
        let table: toml::Table = d
            .parse()
            .map_err(|e: toml::de::Error| Error::Config(format!("packaged defaults: {e}")))?;
        match table.get(VERSION_KEY) {
            None => Ok(None),
            Some(v) => read_version(v).map(Some),
        }
    }

    /// The file path inside the component.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Loads the file into `T` (missing keys take `T`'s serde defaults; use
    /// `#[serde(default)]`). Writes the defaults when the file is missing and the migrated file
    /// after a migration.
    pub fn load<T>(&self) -> Result<T>
    where
        T: DeserializeOwned + Serialize + Default,
    {
        let existing = match std::fs::read_to_string(&self.path) {
            Ok(s) => Some(s),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => {
                return Err(Error::Io(format!(
                    "{}: {e} (does the manifest grant capabilities.filesystem = \"data-folder\"?)",
                    self.path.display()
                )));
            }
        };
        let (value, rewrite) = self.load_from_str::<T>(existing.as_deref())?;
        if let Some(text) = rewrite {
            std::fs::write(&self.path, text)
                .map_err(|e| Error::Io(format!("{}: {e}", self.path.display())))?;
        }
        Ok(value)
    }

    /// Writes `value` to the file (with `config_version` when a version is set). Comments in
    /// the old file are lost.
    pub fn save<T: Serialize>(&self, value: &T) -> Result<()> {
        let text = self.render(value)?;
        std::fs::write(&self.path, text)
            .map_err(|e| Error::Io(format!("{}: {e}", self.path.display())))
    }

    /// The loading logic without the file system: parses `existing` (or defaults when `None`),
    /// migrates, and returns the value plus the text to write back, if any.
    pub fn load_from_str<T>(&self, existing: Option<&str>) -> Result<(T, Option<String>)>
    where
        T: DeserializeOwned + Serialize + Default,
    {
        let target_version = self.target_version()?;
        let Some(text) = existing else {
            if let Some(defaults) = &self.defaults {
                let (value, _) = self.parse_into::<T>(defaults, target_version)?;
                return Ok((value, Some(defaults.clone())));
            }
            let value = T::default();
            let rendered = render_table(to_table(&value)?, target_version)?;
            return Ok((value, Some(rendered)));
        };
        self.parse_into(text, target_version)
    }

    fn parse_into<T>(&self, text: &str, target_version: Option<u32>) -> Result<(T, Option<String>)>
    where
        T: DeserializeOwned,
    {
        let mut table: toml::Table = text
            .parse()
            .map_err(|e: toml::de::Error| Error::Config(format!("{}: {e}", self.path.display())))?;

        let mut rewrite = None;
        if let Some(target) = target_version {
            let current = match table.get(VERSION_KEY) {
                None => 1,
                Some(v) => read_version(v)?,
            };
            if current < target {
                for v in current..target {
                    for (_, m) in self.migrations.iter().filter(|(from, _)| *from == v) {
                        m(&mut table).map_err(|e| {
                            Error::Config(format!("migration {v} -> {}: {e}", v + 1))
                        })?;
                    }
                    if let Some(hook) = &self.hook {
                        hook(v, &mut table).map_err(|e| {
                            Error::Config(format!("migration {v} -> {}: {e}", v + 1))
                        })?;
                    }
                }
                table.insert(
                    VERSION_KEY.to_owned(),
                    toml::Value::Integer(i64::from(target)),
                );
                rewrite = Some(render_table(table.clone(), Some(target))?);
            } else if current > target {
                crate::warn!(
                    "{} has {VERSION_KEY} = {current}, newer than this plugin's {target}; loading it anyway",
                    self.path.display()
                );
            }
        }
        table.remove(VERSION_KEY);
        let value: T = table
            .try_into()
            .map_err(|e: toml::de::Error| Error::Config(format!("{}: {e}", self.path.display())))?;
        Ok((value, rewrite))
    }

    fn render<T: Serialize>(&self, value: &T) -> Result<String> {
        render_table(to_table(value)?, self.target_version()?)
    }
}

fn to_table<T: Serialize>(value: &T) -> Result<toml::Table> {
    toml::Table::try_from(value).map_err(|e| Error::Config(format!("cannot serialize config: {e}")))
}

fn read_version(v: &toml::Value) -> Result<u32> {
    match v {
        toml::Value::Integer(i) if *i >= 0 && *i <= i64::from(u32::MAX) => Ok(*i as u32),
        other => Err(Error::Config(format!(
            "`{VERSION_KEY}` must be a non-negative integer, found {other}"
        ))),
    }
}

fn render_table(mut table: toml::Table, version: Option<u32>) -> Result<String> {
    table.remove(VERSION_KEY);
    let body = toml::to_string(&table)?;
    Ok(match version {
        Some(v) if body.is_empty() => format!("{VERSION_KEY} = {v}\n"),
        Some(v) => format!("{VERSION_KEY} = {v}\n\n{body}"),
        None => body,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    #[serde(default)]
    struct Cfg {
        greeting: String,
        banned: Vec<String>,
        limit: u32,
    }

    impl Default for Cfg {
        fn default() -> Self {
            Cfg {
                greeting: "hi".into(),
                banned: vec!["bad".into()],
                limit: 5,
            }
        }
    }

    #[test]
    fn missing_file_writes_defaults() {
        let (v, w) = Config::new().version(3).load_from_str::<Cfg>(None).unwrap();
        assert_eq!(v, Cfg::default());
        let w = w.unwrap();
        assert!(w.starts_with("config_version = 3\n\n"));
        assert!(w.contains("greeting = \"hi\""));
        // What we wrote loads back without a rewrite.
        let (v2, w2) = Config::new()
            .version(3)
            .load_from_str::<Cfg>(Some(&w))
            .unwrap();
        assert_eq!(v2, v);
        assert_eq!(w2, None);
    }

    #[test]
    fn partial_file_takes_defaults() {
        let (v, w) = Config::new()
            .load_from_str::<Cfg>(Some("limit = 9"))
            .unwrap();
        assert_eq!(v.limit, 9);
        assert_eq!(v.greeting, "hi");
        assert_eq!(w, None);
    }

    #[test]
    fn migrations_run_in_order() {
        let cfg = Config::new()
            .version(3)
            .migrate(2, |t| {
                let l = t
                    .get("limit")
                    .and_then(toml::Value::as_integer)
                    .unwrap_or(0);
                t.insert("limit".into(), toml::Value::Integer(l * 10));
                Ok(())
            })
            .migrate(1, |t| {
                if let Some(v) = t.remove("max") {
                    t.insert("limit".into(), v);
                }
                Ok(())
            });
        let (v, w) = cfg
            .load_from_str::<Cfg>(Some("greeting = \"yo\"\nmax = 2\n"))
            .unwrap();
        assert_eq!(v.limit, 20);
        assert_eq!(v.greeting, "yo");
        let w = w.unwrap();
        assert!(w.starts_with("config_version = 3\n"));
        assert!(!w.contains("max"));
        assert!(w.contains("limit = 20"));
    }

    #[test]
    fn newer_file_loads_and_errors_are_reported() {
        let (v, w) = Config::new()
            .version(1)
            .load_from_str::<Cfg>(Some("config_version = 5\nlimit = 1"))
            .unwrap();
        assert_eq!((v.limit, w), (1, None));
        assert!(
            Config::new()
                .load_from_str::<Cfg>(Some("limit = "))
                .is_err()
        );
        assert!(
            Config::new()
                .load_from_str::<Cfg>(Some("limit = \"x\""))
                .is_err()
        );
        assert!(
            Config::new()
                .version(2)
                .load_from_str::<Cfg>(Some("config_version = \"a\""))
                .is_err()
        );
        let failing = Config::new()
            .version(2)
            .migrate(1, |_| Err(Error::Other("boom".into())));
        let err = failing.load_from_str::<Cfg>(Some("")).unwrap_err();
        assert!(err.to_string().contains("migration 1 -> 2: boom"));
    }

    #[test]
    fn hook_and_packaged_defaults() {
        const DEFAULTS: &str = "# comment kept
config_version = 3
limit = 7
";
        let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let s2 = seen.clone();
        let cfg = Config::new().defaults(DEFAULTS).migration(move |from, t| {
            s2.borrow_mut().push(from);
            if from == 1
                && let Some(v) = t.remove("max")
            {
                t.insert("limit".into(), v);
            }
            Ok(())
        });
        assert_eq!(cfg.target_version().unwrap(), Some(3));
        // Missing file: the packaged text is written verbatim.
        let (v, w) = cfg.load_from_str::<Cfg>(None).unwrap();
        assert_eq!(v.limit, 7);
        assert_eq!(w.as_deref(), Some(DEFAULTS));
        // Old admin file: the hook runs for 1 -> 2 and 2 -> 3.
        let (v, w) = cfg.load_from_str::<Cfg>(Some("max = 4")).unwrap();
        assert_eq!(v.limit, 4);
        assert_eq!(*seen.borrow(), vec![1, 2]);
        assert!(w.unwrap().starts_with(
            "config_version = 3
"
        ));
        // Up to date: no hook, no rewrite.
        seen.borrow_mut().clear();
        let (_, w) = cfg
            .load_from_str::<Cfg>(Some(
                "config_version = 3
limit = 1",
            ))
            .unwrap();
        assert!(w.is_none());
        assert!(seen.borrow().is_empty());
        assert!(
            Config::new()
                .defaults("config_version = -1")
                .target_version()
                .is_err()
        );
    }

    #[test]
    fn paths() {
        assert_eq!(Config::new().path(), Path::new("/config.toml"));
        assert_eq!(Config::file("m.toml").path(), Path::new("/m.toml"));
    }
}
