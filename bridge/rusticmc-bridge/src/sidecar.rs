//! Launching and supervising the JVM sidecar.
//!
//! The host listens on an ephemeral loopback port, starts
//! `java <jvm-args> -Xrs -XX:+ExitOnOutOfMemoryError ... -cp rustic-bridge.jar<sep>libs/* rustic.bridge.Main --port P`
//! with a random token in `RUSTIC_BRIDGE_TOKEN`, and accepts one connection whose first line is
//! `RUSTIC-BRIDGE/1 <token>`. The JVM is in its own process group (console Ctrl+C reaches the
//! server, which stops the bridge cleanly) and its stdout/stderr lines are forwarded to the `log`
//! crate (target `bukkit`). Dropping the [`Sidecar`] kills the process.

use crate::config::BridgeConfig;
use crate::link::Link;
use std::hash::{BuildHasher, Hasher};
use std::io::{BufRead, BufReader, Read};
use std::net::{Ipv4Addr, TcpListener};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// A running JVM.
#[derive(Debug)]
pub struct Sidecar {
    child: Child,
    /// Its process id.
    pub pid: u32,
    /// When it was started.
    pub started: Instant,
}

impl Sidecar {
    /// Whether the process still runs.
    pub fn is_running(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    /// Wait up to `grace` for the process to exit by itself, then kill it.
    pub fn stop(&mut self, grace: Duration) {
        let deadline = Instant::now() + grace;
        while Instant::now() < deadline {
            if !self.is_running() {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        self.kill();
    }

    /// Kill the process now.
    pub fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for Sidecar {
    fn drop(&mut self) {
        if self.is_running() {
            self.kill();
        }
    }
}

/// 128 random bits as hex (from the standard library's randomly seeded hasher keys; the token only
/// keeps other local processes from taking the bridge's connection).
fn token() -> String {
    let mut out = String::new();
    for i in 0..2u64 {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u64(i ^ u64::from(std::process::id()));
        h.write_u128(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0),
        );
        out.push_str(&format!("{:016x}", h.finish()));
    }
    out
}

fn forward(source: impl Read + Send + 'static, stderr: bool) {
    let _ = std::thread::Builder::new()
        .name(
            if stderr {
                "bridge-jvm-stderr"
            } else {
                "bridge-jvm-stdout"
            }
            .into(),
        )
        .spawn(move || {
            for line in BufReader::new(source).lines() {
                let Ok(line) = line else { return };
                log_line(&line, stderr);
            }
        });
}

/// One line of the JVM's output: `R|LEVEL|source|text` from the bridge's logger, anything else raw.
pub fn log_line(line: &str, stderr: bool) {
    let mut parts = line.splitn(4, '|');
    if let (Some("R"), Some(level), Some(src), Some(text)) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    {
        let level = match level {
            "ERROR" => log::Level::Error,
            "WARN" => log::Level::Warn,
            "DEBUG" => log::Level::Debug,
            "TRACE" => log::Level::Trace,
            _ => log::Level::Info,
        };
        log::log!(target: "bukkit", level, "[{src}] {text}");
    } else if stderr {
        log::warn!(target: "bukkit", "[jvm] {line}");
    } else {
        log::info!(target: "bukkit", "[jvm] {line}");
    }
}

/// Start the JVM and connect to it.
pub fn spawn(config: &BridgeConfig, server_dir: &Path) -> Result<(Sidecar, Link), String> {
    let home = config.home(server_dir)?;
    let jar = home.join("rustic-bridge.jar");
    if !jar.is_file() {
        return Err(format!(
            "{} not found (build it with bridge/java/build.sh, or set `home` in rustic-bridge.toml)",
            jar.display()
        ));
    }
    let cp = std::env::join_paths([jar.clone(), home.join("libs").join("*")])
        .map_err(|e| format!("classpath: {e}"))?;
    let listener =
        TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).map_err(|e| format!("cannot listen: {e}"))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("cannot listen: {e}"))?
        .port();
    let token = token();
    let java = config.java();
    let mut cmd = Command::new(&java);
    cmd.args(&config.jvm_args)
        .args([
            "-Xrs",
            "-XX:+ExitOnOutOfMemoryError",
            "-Dfile.encoding=UTF-8",
            // Shaded native libraries (sqlite-jdbc...) load from plugin jars, as on Paper.
            "--enable-native-access=ALL-UNNAMED",
            // No SLF4J provider is bundled: plugins' SLF4J loggers are no-ops (java.util.logging works).
            "-Dslf4j.internal.verbosity=ERROR",
        ])
        .arg(format!(
            "-Drustic.bridge.watchdogMs={}",
            config.watchdog_ms.min(config.call_timeout_ms)
        ))
        .arg("-cp")
        .arg(&cp)
        .arg("rustic.bridge.Main")
        .arg("--port")
        .arg(port.to_string())
        .env("RUSTIC_BRIDGE_TOKEN", &token)
        .current_dir(server_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let started = Instant::now();
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("cannot start {}: {e}", java.display()))?;
    if let Some(out) = child.stdout.take() {
        forward(out, false);
    }
    if let Some(err) = child.stderr.take() {
        forward(err, true);
    }
    let pid = child.id();
    let mut sidecar = Sidecar {
        child,
        pid,
        started,
    };
    listener
        .set_nonblocking(true)
        .map_err(|e| format!("listener: {e}"))?;
    let deadline = started + Duration::from_millis(config.startup_timeout_ms);
    let stream = loop {
        match listener.accept() {
            Ok((s, addr)) if addr.ip().is_loopback() => break s,
            Ok(_) => continue,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if !sidecar.is_running() {
                    return Err("the JVM exited during startup (see the [bukkit] log lines)".into());
                }
                if Instant::now() > deadline {
                    return Err(format!(
                        "the JVM did not connect within {} ms",
                        config.startup_timeout_ms
                    ));
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(e) => return Err(format!("accept: {e}")),
        }
    };
    stream
        .set_nonblocking(false)
        .map_err(|e| format!("socket: {e}"))?;
    stream
        .set_read_timeout(Some(Duration::from_millis(config.startup_timeout_ms)))
        .map_err(|e| format!("socket: {e}"))?;
    let mut hello = Vec::new();
    let mut byte = [0u8; 1];
    while hello.len() < 128 {
        match (&stream).read(&mut byte) {
            Ok(1) if byte[0] == b'\n' => break,
            Ok(1) => hello.push(byte[0]),
            _ => return Err("the JVM closed the connection during the handshake".into()),
        }
    }
    if hello != format!("RUSTIC-BRIDGE/1 {token}").as_bytes() {
        return Err("bad handshake from the bridge JVM (wrong token or version)".into());
    }
    let link = Link::new(stream, Duration::from_millis(config.call_timeout_ms))
        .map_err(|e| format!("socket: {e}"))?;
    Ok((sidecar, link))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_differ() {
        let (a, b) = (token(), token());
        assert_eq!(a.len(), 32);
        assert_ne!(a, b);
    }

    #[test]
    fn log_lines_parse() {
        log_line("R|WARN|Welcome|hello|with bars", false);
        log_line("plain line", true);
    }
}
