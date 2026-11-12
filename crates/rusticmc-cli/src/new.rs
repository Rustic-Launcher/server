//! `rustic new`: a plugin crate from a template.

use crate::ui;
use anyhow::{Result, bail};
use rusticmc_core::PluginId;
use std::path::{Path, PathBuf};

/// The templates (`crates/rusticmc-cli/templates/<name>/`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum Template {
    /// Events and config: a join greeting, protected blocks, per-player data.
    Basic,
    /// Commands: `/heal [target]`, `/feed`, permissions, a cooldown, tab completion.
    Command,
    /// A chest menu with click handlers.
    Gui,
    /// A service provider: an economy (`/balance`, `/pay`), persisted balances.
    Service,
}

impl Template {
    fn name(self) -> &'static str {
        match self {
            Template::Basic => "basic",
            Template::Command => "command",
            Template::Gui => "gui",
            Template::Service => "service",
        }
    }

    fn files(self) -> [(&'static str, &'static str); 3] {
        match self {
            Template::Basic => [
                ("src/lib.rs", include_str!("../templates/basic/src/lib.rs")),
                (
                    "config.toml",
                    include_str!("../templates/basic/config.toml"),
                ),
                (
                    "tests/plugin.rs",
                    include_str!("../templates/basic/tests/plugin.rs"),
                ),
            ],
            Template::Command => [
                (
                    "src/lib.rs",
                    include_str!("../templates/command/src/lib.rs"),
                ),
                (
                    "config.toml",
                    include_str!("../templates/command/config.toml"),
                ),
                (
                    "tests/plugin.rs",
                    include_str!("../templates/command/tests/plugin.rs"),
                ),
            ],
            Template::Gui => [
                ("src/lib.rs", include_str!("../templates/gui/src/lib.rs")),
                ("config.toml", include_str!("../templates/gui/config.toml")),
                (
                    "tests/plugin.rs",
                    include_str!("../templates/gui/tests/plugin.rs"),
                ),
            ],
            Template::Service => [
                (
                    "src/lib.rs",
                    include_str!("../templates/service/src/lib.rs"),
                ),
                (
                    "config.toml",
                    include_str!("../templates/service/config.toml"),
                ),
                (
                    "tests/plugin.rs",
                    include_str!("../templates/service/tests/plugin.rs"),
                ),
            ],
        }
    }

    fn description(self) -> &'static str {
        match self {
            Template::Basic => "Greets players and protects blocks",
            Template::Command => "Heal and feed commands",
            Template::Gui => "A chest menu",
            Template::Service => "An economy service",
        }
    }
}

const COMMON: [(&str, &str); 4] = [
    (
        "Cargo.toml",
        include_str!("../templates/common/Cargo.toml.tmpl"),
    ),
    (
        "rustic.plugin.toml",
        include_str!("../templates/common/rustic.plugin.toml.tmpl"),
    ),
    (
        ".gitignore",
        include_str!("../templates/common/gitignore.tmpl"),
    ),
    (
        "README.md",
        include_str!("../templates/common/README.md.tmpl"),
    ),
];

/// Where the SDK and the harness come from.
#[derive(Clone, Debug)]
pub enum Sdk {
    /// Path dependencies on a Rustic checkout (`crates/rusticmc-api`, `crates/rusticmc-test`).
    Path(PathBuf),
    /// Registry versions.
    Version(String),
}

/// The checkout this binary was built from, if it still exists.
pub fn repo_crates() -> Option<PathBuf> {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    (p.join("rusticmc-api/Cargo.toml").is_file() && p.join("rusticmc-test/Cargo.toml").is_file())
        .then(|| clean(&p))
}

/// Canonical path without the Windows `\\?\` prefix, with forward slashes (TOML friendly).
pub fn clean(p: &Path) -> PathBuf {
    let c = p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
    let s = c.display().to_string();
    let s = s.strip_prefix(r"\\?\").unwrap_or(&s).replace('\\', "/");
    PathBuf::from(s)
}

/// `my-plugin` → `MyPlugin`.
pub fn struct_name(id: &str) -> String {
    let mut out = String::new();
    for part in id.split(['-', '_']).filter(|p| !p.is_empty()) {
        let mut c = part.chars();
        if let Some(f) = c.next() {
            out.extend(f.to_uppercase());
            out.push_str(c.as_str());
        }
    }
    if out.is_empty() || out.starts_with(|c: char| c.is_ascii_digit()) {
        out.insert_str(0, "Plugin");
    }
    out
}

/// `my-plugin` → `My Plugin`.
fn display_name(id: &str) -> String {
    id.split(['-', '_'])
        .filter(|p| !p.is_empty())
        .map(|p| {
            let mut c = p.chars();
            c.next()
                .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Options of `rustic new`.
pub struct NewOptions {
    pub id: String,
    pub template: Template,
    pub dir: Option<PathBuf>,
    pub name: Option<String>,
    pub author: Option<String>,
    pub description: Option<String>,
    pub sdk: Option<Sdk>,
    pub workspace_member: bool,
    pub force: bool,
}

fn toml_str(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Render the files of a new plugin (path → contents).
pub fn render(o: &NewOptions) -> Result<Vec<(String, String)>> {
    if let Err(e) = PluginId::new(&o.id) {
        bail!("{e} (example: `rustic new homes`)");
    }
    let sdk = o
        .sdk
        .clone()
        .or_else(|| repo_crates().map(Sdk::Path))
        .unwrap_or_else(|| Sdk::Version("0.1".into()));
    let (sdk_dep, test_dep) = match &sdk {
        Sdk::Path(p) => {
            let p = clean(p);
            let p = p.display();
            (
                format!("rustic = {{ package = \"rusticmc-api\", path = \"{p}/rusticmc-api\" }}"),
                format!("rusticmc-test = {{ path = \"{p}/rusticmc-test\" }}"),
            )
        }
        Sdk::Version(v) => (
            format!("rustic = {{ package = \"rusticmc-api\", version = \"{v}\" }}"),
            format!("rusticmc-test = \"{v}\""),
        ),
    };
    let sdk_dep = if o.template == Template::Service {
        sdk_dep.replace(" }", ", features = [\"provide-economy\"] }")
    } else {
        sdk_dep
    };
    let name = o.name.clone().unwrap_or_else(|| display_name(&o.id));
    let author = o
        .author
        .clone()
        .or_else(|| std::env::var("RUSTIC_AUTHOR").ok())
        .or_else(git_user)
        .unwrap_or_else(|| "Unknown".into());
    let description = o
        .description
        .clone()
        .unwrap_or_else(|| o.template.description().to_owned());
    let workspace = if o.workspace_member {
        String::new()
    } else {
        "\n# Its own workspace (remove this table to add the crate to an existing one).\n[workspace]\n"
            .to_owned()
    };
    let vars: [(&str, String); 10] = [
        ("id", o.id.clone()),
        ("name", toml_str(&name)),
        ("crate_name", o.id.clone()),
        ("lib_name", o.id.replace('-', "_")),
        ("struct_name", struct_name(&o.id)),
        ("author", toml_str(&author)),
        ("description", toml_str(&description)),
        ("sdk_dep", sdk_dep),
        ("test_dep", test_dep),
        ("workspace", workspace),
    ];
    let template_name = o.template.name().to_owned();
    let fill = |text: &str| {
        let mut t = text.replace("{{template}}", &template_name);
        for (k, v) in &vars {
            t = t.replace(&format!("{{{{{k}}}}}"), v);
        }
        t
    };
    Ok(COMMON
        .iter()
        .chain(o.template.files().iter())
        .map(|(path, text)| ((*path).to_owned(), fill(text)))
        .collect())
}

fn git_user() -> Option<String> {
    let out = std::process::Command::new("git")
        .args(["config", "user.name"])
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    (out.status.success() && !s.is_empty()).then_some(s)
}

/// `rustic new`.
pub fn run(o: NewOptions) -> Result<()> {
    let files = render(&o)?;
    let dir = o.dir.clone().unwrap_or_else(|| PathBuf::from(&o.id));
    if dir.exists() && std::fs::read_dir(&dir)?.next().is_some() && !o.force {
        bail!(
            "{} already exists and is not empty (use --force to write into it)",
            ui::rel(&dir)
        );
    }
    for (path, text) in &files {
        let p = dir.join(path);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&p, text)?;
    }
    ui::status(
        "Created",
        format!(
            "plugin `{}` ({} template) in {}",
            o.id,
            o.template.name(),
            ui::rel(&dir)
        ),
    );
    let cd = ui::rel(&dir);
    anstream::eprintln!(
        "\n  Next steps:\n    cd {cd}\n    rustic build      {d}# compile + package target/rustic/{id}.rplugin{d:#}\n    rustic test       {d}# run tests/plugin.rs in the in-memory server{d:#}\n    rustic run        {d}# try it (dev server or REPL){d:#}\n",
        d = ui::DIM,
        id = o.id
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(id: &str, t: Template) -> NewOptions {
        NewOptions {
            id: id.into(),
            template: t,
            dir: None,
            name: None,
            author: Some("Ann \"A\" Author".into()),
            description: None,
            sdk: Some(Sdk::Version("0.1".into())),
            workspace_member: false,
            force: false,
        }
    }

    #[test]
    fn names() {
        assert_eq!(struct_name("my-plugin"), "MyPlugin");
        assert_eq!(struct_name("homes"), "Homes");
        assert_eq!(struct_name("9lives"), "Plugin9lives");
        assert_eq!(display_name("my_cool-plugin"), "My Cool Plugin");
    }

    #[test]
    fn every_template_renders_a_valid_manifest_and_no_leftover_placeholder() {
        for t in [
            Template::Basic,
            Template::Command,
            Template::Gui,
            Template::Service,
        ] {
            let files = render(&opts("my-plugin", t)).unwrap();
            for (path, text) in &files {
                assert!(
                    !text.contains("{{"),
                    "{path} of {t:?} has a placeholder left"
                );
            }
            let manifest = &files
                .iter()
                .find(|(p, _)| p == "rustic.plugin.toml")
                .unwrap()
                .1;
            let m = rusticmc_core::Manifest::parse(manifest).unwrap();
            assert_eq!(m.entry, "my_plugin.wasm");
            let cargo = &files.iter().find(|(p, _)| p == "Cargo.toml").unwrap().1;
            let parsed: toml::Table = toml::from_str(cargo).unwrap();
            assert!(parsed.contains_key("workspace"));
            assert_eq!(
                t == Template::Service,
                cargo.contains("provide-economy"),
                "{t:?}"
            );
        }
    }

    #[test]
    fn invalid_ids_are_refused() {
        assert!(render(&opts("Bad Id", Template::Basic)).is_err());
        assert!(render(&opts("rustic", Template::Basic)).is_err());
    }
}
