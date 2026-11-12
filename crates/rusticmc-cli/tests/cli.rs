//! End-to-end tests of the `rustic` binary in temporary directories:
//! `new → build → check → info → package → test → run (REPL)`, and `bridge analyze`.
//!
//! Generated plugins build in a shared target directory (`<repo>/target/cli-e2e`) so the SDK,
//! the harness and wasmtime are compiled once across runs.

use assert_cmd::Command;
use predicates::prelude::*;
use std::io::Write;
use std::path::{Path, PathBuf};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn rustic(dir: &Path) -> Command {
    let mut c = Command::cargo_bin("rustic").unwrap();
    c.current_dir(dir)
        .env("CARGO_TARGET_DIR", repo().join("target").join("cli-e2e"))
        .env("NO_COLOR", "1")
        .env("RUSTIC_AUTHOR", "Test Author")
        .env_remove("RUSTIC_SERVER_BIN")
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS");
    c
}

#[test]
fn help_lists_every_command() {
    let out = Command::cargo_bin("rustic")
        .unwrap()
        .arg("--help")
        .assert()
        .success();
    let text = String::from_utf8_lossy(&out.get_output().stdout).into_owned();
    for c in [
        "new", "build", "check", "test", "run", "package", "info", "bridge",
    ] {
        assert!(text.contains(c), "{c} missing from --help:\n{text}");
    }
}

#[test]
fn new_refuses_bad_ids_and_existing_directories() {
    let tmp = tempfile::tempdir().unwrap();
    rustic(tmp.path())
        .args(["new", "Bad Id"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("plugin id"));
    rustic(tmp.path()).args(["new", "ok"]).assert().success();
    rustic(tmp.path())
        .args(["new", "ok"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not empty"));
}

#[test]
fn check_reports_manifest_problems() {
    let tmp = tempfile::tempdir().unwrap();
    rustic(tmp.path())
        .args(["new", "broken"])
        .assert()
        .success();
    let m = tmp.path().join("broken/rustic.plugin.toml");
    let text = std::fs::read_to_string(&m)
        .unwrap()
        .replace("api = \"0.1\"", "api = \"9.0\"\nsurprise = 1");
    std::fs::write(&m, text).unwrap();
    rustic(&tmp.path().join("broken"))
        .arg("check")
        .assert()
        .failure()
        .stderr(predicate::str::contains("surprise").or(predicate::str::contains("unknown")));
}

#[test]
fn outside_a_project_the_error_says_what_to_do() {
    let tmp = tempfile::tempdir().unwrap();
    rustic(tmp.path())
        .arg("build")
        .assert()
        .failure()
        .stderr(predicate::str::contains("rustic new"));
}

#[test]
fn new_build_check_info_package_test_run() {
    let tmp = tempfile::tempdir().unwrap();
    rustic(tmp.path())
        .args(["new", "demo", "--template", "command"])
        .assert()
        .success()
        .stderr(predicate::str::contains("Created plugin `demo`"));
    let demo = tmp.path().join("demo");
    for f in [
        "Cargo.toml",
        "rustic.plugin.toml",
        "config.toml",
        "src/lib.rs",
        "tests/plugin.rs",
        ".gitignore",
        "README.md",
    ] {
        assert!(demo.join(f).is_file(), "{f} not created");
    }

    rustic(&demo)
        .arg("build")
        .assert()
        .success()
        .stderr(predicate::str::contains("Packaged"));
    let rplugin = repo().join("target/cli-e2e/rustic/demo.rplugin");
    assert!(rplugin.is_file());

    rustic(&demo)
        .arg("check")
        .assert()
        .success()
        .stdout(predicate::str::contains("asks to"))
        .stderr(predicate::str::contains("no problem found"));
    rustic(&demo)
        .args(["check"])
        .arg(&rplugin)
        .assert()
        .success();
    rustic(&demo)
        .arg("info")
        .assert()
        .success()
        .stdout(predicate::str::contains("rustic:api/plugin@0.1.0"));

    let out = tmp.path().join("dist/demo.rplugin");
    rustic(&demo)
        .args(["package", "--out"])
        .arg(&out)
        .assert()
        .success()
        .stdout(predicate::str::contains("demo.rplugin"));
    assert!(out.is_file());

    // Both profiles are built now: `check` defaults to the release build `package` ships, and
    // either can be asked for. A relative path from another cwd finds the same project.
    rustic(&demo)
        .args(["check", "--deny-warnings"])
        .assert()
        .success();
    rustic(&demo).args(["check", "--debug"]).assert().success();
    rustic(tmp.path())
        .args(["check", "--release", "demo"])
        .assert()
        .success()
        .stderr(predicate::str::contains("no problem found"));
    rustic(&demo)
        .args(["check", "--release", "--debug"])
        .assert()
        .failure();

    rustic(&demo).arg("test").assert().success();

    rustic(&demo)
        .args(["run", "--repl"])
        .write_stdin("join Admin op\ncmd Admin /heal\ntick 20\nexit\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("You have been healed."));
}

#[test]
fn bridge_analyze_reports_a_verdict() {
    let fixtures = repo().join("bridge/rusticmc-bridge-analyze/tests/fixtures/classes");
    let tmp = tempfile::tempdir().unwrap();
    let jar = tmp.path().join("Green.jar");
    let mut w = zip::ZipWriter::new(std::fs::File::create(&jar).unwrap());
    let opts = zip::write::SimpleFileOptions::default();
    w.start_file("plugin.yml", opts).unwrap();
    w.write_all(
        b"name: Green\nversion: 1.0\nmain: com.example.green.GreenPlugin\napi-version: '1.21'\n",
    )
    .unwrap();
    for c in [
        "com/example/green/GreenPlugin",
        "com/example/green/Ticker",
        "com/example/green/libs/util/Helper",
    ] {
        w.start_file(format!("{c}.class"), opts).unwrap();
        w.write_all(&std::fs::read(fixtures.join(format!("{c}.class"))).unwrap())
            .unwrap();
    }
    w.finish().unwrap();
    let json = tmp.path().join("report.json");
    rustic(tmp.path())
        .args(["bridge", "analyze"])
        .arg(&jar)
        .arg("--json")
        .arg(&json)
        .assert()
        .success()
        .stdout(predicate::str::contains("Green"))
        .stderr(predicate::str::contains("1 green"));
    assert!(
        std::fs::read_to_string(json)
            .unwrap()
            .contains("\"verdict\"")
    );
    rustic(tmp.path())
        .args(["bridge", "analyze"])
        .arg(tmp.path().join("missing"))
        .assert()
        .failure();
}
