//! End-to-end tests on tiny plugin jars assembled in memory from class files compiled with
//! `javac` against stub API classes written for these tests (`tests/fixtures/regen.sh`).

use std::io::Write;
use std::path::Path;

use crate::analyze::{Namespace, RedKind, Verdict, namespace};
use crate::{Coverage, analyze_bytes, build_panel, render_panel_markdown, render_plugin_markdown};

fn fixture(class: &str) -> Vec<u8> {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/classes")
        .join(format!("{class}.class"));
    std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn jar(plugin_yml: Option<&str>, classes: &[&str], extra: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default();
    if let Some(y) = plugin_yml {
        w.start_file("plugin.yml", opts).unwrap();
        w.write_all(y.as_bytes()).unwrap();
    }
    for c in classes {
        w.start_file(format!("{c}.class"), opts).unwrap();
        w.write_all(&fixture(c)).unwrap();
    }
    for (name, data) in extra {
        w.start_file(*name, opts).unwrap();
        w.write_all(data).unwrap();
    }
    w.finish().unwrap().into_inner()
}

fn yml(name: &str, main: &str) -> String {
    format!("name: {name}\nversion: 1.0\nmain: {main}\napi-version: '1.21'\n")
}

#[test]
fn green_plugin_with_inherited_calls_lambdas_and_a_shaded_unsafe_library() {
    let cov = Coverage::builtin();
    let j = jar(
        Some(&yml("Green", "com.example.green.GreenPlugin")),
        &[
            "com/example/green/GreenPlugin",
            "com/example/green/Ticker",
            "com/example/green/libs/util/Helper",
        ],
        &[],
    );
    let r = analyze_bytes("green.jar", &j, &cov);
    let keys: Vec<String> = r.api_members.iter().map(|m| m.key()).collect();
    assert_eq!(r.verdict, Verdict::Green, "{:#?}\n{keys:?}", r.reasons);
    // Own classes are never targets; inherited API members are attributed to the API class.
    assert!(
        !keys.iter().any(|k| k.starts_with("com.example")),
        "{keys:?}"
    );
    for k in [
        "org.bukkit.plugin.java.JavaPlugin#getLogger",
        "org.bukkit.scheduler.BukkitRunnable#runTaskTimer",
        "org.bukkit.entity.Player#sendMessage",
        "org.bukkit.entity.Player#getName",
        "org.bukkit.event.player.PlayerJoinEvent#getPlayer",
        "org.bukkit.Bukkit#getOnlinePlayers",
        "org.bukkit.plugin.PluginManager#registerEvents",
    ] {
        assert!(keys.contains(&k.to_owned()), "{k} missing from {keys:?}");
    }
    let send = r
        .api_members
        .iter()
        .find(|m| m.key() == "org.bukkit.entity.Player#sendMessage")
        .unwrap();
    assert_eq!(send.sites, 2);
    assert!(
        r.api_types
            .iter()
            .any(|t| t.class == "org.bukkit.event.EventHandler")
    );
    // Unsafe inside a bundled library is recorded but does not decide the verdict.
    let unsafe_sig = r
        .red_signals
        .iter()
        .find(|s| s.kind == RedKind::Unsafe)
        .expect("Unsafe recorded");
    assert!(!unsafe_sig.decisive);
    assert_eq!(unsafe_sig.origin, "com.example.green.libs.util");
    assert_eq!(r.stats.own_classes, 2);
    assert_eq!(r.stats.library_classes, 1);
    assert!(r.stats.reflection_calls >= 2);
}

#[test]
fn yellow_plugin_lists_uncovered_members_and_types() {
    let cov = Coverage::builtin();
    let j = jar(
        Some(&yml("Yellow", "com.example.yellow.YellowPlugin")),
        &["com/example/yellow/YellowPlugin"],
        &[],
    );
    let r = analyze_bytes("yellow.jar", &j, &cov);
    assert_eq!(r.verdict, Verdict::Yellow);
    let unc: Vec<String> = r.uncovered_members().map(|m| m.key()).collect();
    assert_eq!(unc, ["org.bukkit.entity.Player#setScoreboard"]);
    let types: Vec<&str> = r.uncovered_types().map(|t| t.class.as_str()).collect();
    assert_eq!(types, ["org.bukkit.entity.Zombie"]);
}

#[test]
fn red_plugin_craftbukkit_and_reflection() {
    let cov = Coverage::builtin();
    let j = jar(
        Some(&yml("Red", "com.example.red.RedPlugin")),
        &["com/example/red/RedPlugin"],
        &[],
    );
    let r = analyze_bytes("red.jar", &j, &cov);
    assert_eq!(r.verdict, Verdict::Red);
    assert!(!r.red_only_in_libraries);
    let kinds: Vec<RedKind> = r.red_signals.iter().map(|s| s.kind).collect();
    assert!(kinds.contains(&RedKind::CraftBukkit), "{kinds:?}");
    assert!(
        kinds.contains(&RedKind::ReflectionIntoInternals),
        "{kinds:?}"
    );
    // CraftBukkit members are never counted as API.
    assert!(
        r.api_members
            .iter()
            .all(|m| !m.owner.contains("craftbukkit"))
    );
}

#[test]
fn red_only_because_of_a_bundled_library() {
    let cov = Coverage::builtin();
    let j = jar(
        Some(&yml("LibRed", "com.example.libred.Main")),
        &["com/example/libred/Main", "com/example/libred/libs/nbt/Nms"],
        &[],
    );
    let r = analyze_bytes("libred.jar", &j, &cov);
    assert_eq!(r.verdict, Verdict::Red);
    assert!(r.red_only_in_libraries);
    assert!(
        r.red_signals
            .iter()
            .any(|s| s.kind == RedKind::ReflectionIntoInternals
                && s.origin == "com.example.libred.libs.nbt")
    );
}

#[test]
fn hard_dependency_on_protocollib_is_red_and_nested_jars_are_read() {
    let cov = Coverage::builtin();
    let inner = jar(
        None,
        &["com/example/green/GreenPlugin", "com/example/green/Ticker"],
        &[],
    );
    let y = format!(
        "{}depend: [ProtocolLib]\n",
        yml("Dep", "com.example.green.GreenPlugin")
    );
    let j = jar(Some(&y), &[], &[("loader/inner.jarinjar", inner)]);
    let r = analyze_bytes("dep.jar", &j, &cov);
    assert_eq!(r.stats.nested_jars, ["loader/inner.jarinjar"]);
    assert_eq!(r.stats.classes, 2);
    assert_eq!(r.verdict, Verdict::Red);
    assert!(
        r.red_signals
            .iter()
            .any(|s| s.kind == RedKind::HardDependency && s.target == "ProtocolLib")
    );
}

#[test]
fn garbage_and_non_plugins() {
    let cov = Coverage::builtin();
    let r = analyze_bytes("garbage.jar", b"not a zip", &cov);
    assert_eq!(r.verdict, Verdict::Error);
    let j = jar(
        None,
        &["com/example/yellow/YellowPlugin"],
        &[("com/x/Bad.class", vec![0xCA, 0xFE, 0xBA, 0xBE, 0])],
    );
    let r = analyze_bytes("lib.jar", &j, &cov);
    assert!(r.meta.is_none());
    assert_eq!(r.stats.parse_errors, 1);
    assert!(r.reasons[0].contains("no plugin.yml"));
}

#[test]
fn panel_aggregation_and_markdown() {
    let cov = Coverage::builtin();
    let reports = vec![
        analyze_bytes(
            "green.jar",
            &jar(
                Some(&yml("Green", "com.example.green.GreenPlugin")),
                &["com/example/green/GreenPlugin", "com/example/green/Ticker"],
                &[],
            ),
            &cov,
        ),
        analyze_bytes(
            "yellow.jar",
            &jar(
                Some(&yml("Yellow", "com.example.yellow.YellowPlugin")),
                &["com/example/yellow/YellowPlugin"],
                &[],
            ),
            &cov,
        ),
        analyze_bytes(
            "red.jar",
            &jar(
                Some(&yml("Red", "com.example.red.RedPlugin")),
                &["com/example/red/RedPlugin"],
                &[],
            ),
            &cov,
        ),
    ];
    let p = build_panel(reports, &cov.meta.api);
    assert_eq!(
        (p.summary.green, p.summary.yellow, p.summary.red),
        (1, 1, 1)
    );
    let top = p
        .top_classes
        .iter()
        .find(|c| c.name == "org.bukkit.plugin.java.JavaPlugin")
        .unwrap();
    assert_eq!(top.plugins, 3);
    assert_eq!(
        p.top_uncovered_members[0].name,
        "org.bukkit.entity.Player#setScoreboard"
    );
    // Covering the single blocking member and type turns the yellow plugin green.
    assert_eq!(p.coverage_curve[0].green_after, 1);
    let md = render_panel_markdown(&p);
    assert!(md.contains("| 🟢 green | 1 | 33% |"), "{md}");
    assert!(render_plugin_markdown(&p.plugins[1]).contains("setScoreboard"));
    let json = serde_json::to_string(&p).unwrap();
    let back: crate::PanelReport = serde_json::from_str(&json).unwrap();
    assert_eq!(back.summary, p.summary);
}

#[test]
fn internal_strings() {
    use crate::analyze::is_internal_string;
    for s in [
        "net.minecraft.server.Main",
        "org.bukkit.craftbukkit.v1_20_R1",
        "getHandle",
        ".entity.CraftPlayer",
        "CraftServer",
    ] {
        assert!(is_internal_string(s), "{s}");
    }
    for s in [
        "CraftBukkit",
        "Crafting",
        "net.kyori.adventure",
        "getHandlers",
        "craftItem",
    ] {
        assert!(!is_internal_string(s), "{s}");
    }
}

#[test]
fn namespaces() {
    assert_eq!(namespace("org/bukkit/entity/Player"), Namespace::Api);
    assert_eq!(
        namespace("org/bukkit/craftbukkit/CraftServer"),
        Namespace::Internal
    );
    assert_eq!(
        namespace("com/mojang/brigadier/CommandDispatcher"),
        Namespace::Api
    );
    assert_eq!(
        namespace("com/mojang/authlib/GameProfile"),
        Namespace::Internal
    );
    assert_eq!(
        namespace("com/comphenix/protocol/ProtocolLibrary"),
        Namespace::PacketLibrary
    );
    assert_eq!(namespace("sun/misc/Unsafe"), Namespace::Instrumentation);
    assert_eq!(namespace("java/lang/String"), Namespace::Jdk);
    assert_eq!(
        namespace("com/google/gson/Gson"),
        Namespace::PlatformLibrary
    );
    assert_eq!(
        namespace("net/md_5/bungee/api/chat/TextComponent"),
        Namespace::Api
    );
    assert_eq!(
        namespace("net/md_5/bungee/api/plugin/Plugin"),
        Namespace::OtherPlatform
    );
    assert_eq!(
        namespace("net/milkbowl/vault/economy/Economy"),
        Namespace::Api
    );
    assert_eq!(
        namespace("me/clip/placeholderapi/PlaceholderAPI"),
        Namespace::External
    );
}
