//! Execution model A of docs/BRIDGE.md: the bridge's Java layer in a JVM created inside the
//! server process with JNI, driven from the server thread. Same Java code, same requests as the
//! sidecar (`rusticmc-bridge/examples/measure.rs`), so the numbers compare the two transports and
//! process models. No `unsafe` is needed for this direction (Rust calling Java); a production
//! JNI bridge would also need Java-to-Rust native callbacks (`unsafe` registration).
use jni::objects::{JByteArray, JValue};
use jni::{InitArgsBuilder, JNIVersion, JavaVM, jni_sig, jni_str};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn pct(v: &mut [Duration], p: f64) -> f64 {
    v.sort();
    v[((v.len() as f64 - 1.0) * p).round() as usize].as_secs_f64() * 1e6
}

fn rss_mib() -> f64 {
    let pid = std::process::id();
    if cfg!(windows) {
        let out = Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                &format!("(Get-Process -Id {pid}).WorkingSet64"),
            ])
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout)
            .trim()
            .parse::<f64>()
            .unwrap_or(0.0)
            / 1048576.0
    } else {
        std::fs::read_to_string("/proc/self/status")
            .ok()
            .and_then(|s| {
                s.lines()
                    .find(|l| l.starts_with("VmRSS:"))
                    .and_then(|l| l.split_whitespace().nth(1)?.parse::<f64>().ok())
            })
            .unwrap_or(0.0)
            / 1024.0
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let iters = 20_000usize;
    let build = repo().join("bridge/java/build");
    let jdk = std::env::var_os("JAVA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| repo().join("tools/jdk25"));
    let libjvm = if cfg!(windows) {
        jdk.join("bin/server/jvm.dll")
    } else {
        jdk.join("lib/server/libjvm.so")
    };
    let sep = if cfg!(windows) { ";" } else { ":" };
    let mut cp = vec![build.join("rustic-bridge.jar").display().to_string()];
    for e in std::fs::read_dir(build.join("libs"))?.flatten() {
        cp.push(e.path().display().to_string());
    }
    let dir = tempfile::tempdir()?;
    let bukkit = dir.path().join("plugins/bukkit");
    std::fs::create_dir_all(&bukkit)?;
    std::fs::copy(build.join("plugins/Probe.jar"), bukkit.join("Probe.jar"))?;

    println!("# Embedded JVM (JNI) spike ({iters} events)\n");
    println!("process before the JVM: {:.1} MiB RSS", rss_mib());
    let t0 = Instant::now();
    let args = InitArgsBuilder::new()
        .version(JNIVersion::V21)
        .option(format!("-Djava.class.path={}", cp.join(sep)))
        .option("-Xrs")
        .option("-Xmx512m")
        .option("--enable-native-access=ALL-UNNAMED")
        .option("-Dslf4j.internal.verbosity=ERROR")
        .build()?;
    let jvm = JavaVM::with_libjvm(args, || Ok(libjvm.clone()))?;
    let created = t0.elapsed();
    jvm.attach_current_thread(|env| -> jni::errors::Result<()> {
        env.call_static_method(jni_str!("rustic/bridge/JniEntry"), jni_str!("start"), jni_sig!("()Ljava/lang/String;"), &[])?;
        let mut call = |body: &serde_json::Value| -> jni::errors::Result<serde_json::Value> {
            let bytes = serde_json::to_vec(body).unwrap();
            let arr = env.byte_array_from_slice(&bytes)?;
            let out = env
                .call_static_method(jni_str!("rustic/bridge/JniEntry"), jni_str!("handle"), jni_sig!("([B)[B"), &[JValue::Object(&arr)])?
                .l()?;
            let out = env.cast_local::<JByteArray>(out)?;
            let reply = env.convert_byte_array(&out)?;
            assert_eq!(reply[0], 2, "error reply: {}", String::from_utf8_lossy(&reply[1..]));
            Ok(serde_json::from_slice(&reply[1..]).unwrap_or(serde_json::Value::Null))
        };
        let steve = json!({"hi": 0x069a79f444e94726u64, "lo": 0xa5befca90e38aaf5u64});
        call(&json!({"op": "hello", "server": {}, "players": [{"uuid": steve, "name": "Steve"}]}))?;
        let scan = call(&json!({"op": "scan", "dir": bukkit}))?;
        call(&json!({"op": "load", "plugin": "Probe", "file": scan[0]["file"], "data_dir": dir.path().join("probe")}))?;
        call(&json!({"op": "enable", "plugin": "Probe"}))?;
        let ready = t0.elapsed();
        println!("startup: JVM created in {} ms; plugin enabled at {} ms", created.as_millis(), ready.as_millis());
        println!("memory: whole process {:.1} MiB RSS (JVM inside)", rss_mib());
        let mut v = Vec::with_capacity(iters);
        let mut cancelled = 0;
        for i in 0..iters {
            let msg = if i % 2 == 0 { "hello" } else { "cancelme" };
            let req = json!({"op": "event", "plugin": "Probe", "priority": "normal", "event": {"player-chat": {
                "player": steve, "name": "Steve", "message": msg, "cancelled": false, "display": null}}});
            let t = Instant::now();
            let out = call(&req)?;
            v.push(t.elapsed());
            cancelled += usize::from(out["event"]["player-chat"]["cancelled"] == json!(true));
        }
        assert_eq!(cancelled, iters / 2);
        println!("chat (cancellable, 1 listener plugin): p50 {:.1} us, p99 {:.1} us", pct(&mut v, 0.5), pct(&mut v, 0.99));
        let mut v = Vec::with_capacity(iters);
        for i in 0..iters {
            let at = |x: f64| json!({"world": "minecraft:overworld", "x": x, "y": 64.0, "z": 0.0, "yaw": 0.0, "pitch": 0.0});
            let req = json!({"op": "event", "plugin": "Probe", "priority": "normal", "event": {"player-move": {
                "player": steve, "from": at(i as f64 * 0.1), "to": at(i as f64 * 0.1 + 0.1), "cancelled": false}}});
            let t = Instant::now();
            call(&req)?;
            v.push(t.elapsed());
        }
        println!("move (1 listener plugin): p50 {:.1} us, p99 {:.1} us", pct(&mut v, 0.5), pct(&mut v, 0.99));
        let mut v = Vec::with_capacity(2000);
        for t in 0..2000u64 {
            let s = Instant::now();
            call(&json!({"op": "tick", "tick": t + 1}))?;
            v.push(s.elapsed());
        }
        println!("tick pulse (1 plugin, 1-tick task): p50 {:.1} us, p99 {:.1} us", pct(&mut v, 0.5), pct(&mut v, 0.99));
        let mut v = Vec::with_capacity(iters);
        for _ in 0..iters {
            let s = Instant::now();
            call(&json!({"op": "ping"}))?;
            v.push(s.elapsed());
        }
        println!("raw round trip (ping): p50 {:.1} us, p99 {:.1} us", pct(&mut v, 0.5), pct(&mut v, 0.99));
        Ok(())
    })?;
    Ok(())
}
