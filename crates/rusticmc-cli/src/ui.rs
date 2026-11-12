//! Terminal output: cargo-like status lines, colors (off when not a terminal, `NO_COLOR`,
//! `CLICOLOR=0`; forced with `CLICOLOR_FORCE=1`), sizes.

use anstyle::{AnsiColor, Style};

pub const GREEN: Style = AnsiColor::Green.on_default().bold();
pub const CYAN: Style = AnsiColor::Cyan.on_default().bold();
pub const YELLOW: Style = AnsiColor::Yellow.on_default().bold();
pub const RED: Style = AnsiColor::Red.on_default().bold();
pub const DIM: Style = Style::new().dimmed();
pub const BOLD: Style = Style::new().bold();

/// `   Compiling hello` (verb right-aligned in 12 columns, like cargo), on stderr.
pub fn status(verb: &str, message: impl std::fmt::Display) {
    anstream::eprintln!("{GREEN}{verb:>12}{GREEN:#} {message}");
}

/// A cyan status line (information).
pub fn info(verb: &str, message: impl std::fmt::Display) {
    anstream::eprintln!("{CYAN}{verb:>12}{CYAN:#} {message}");
}

/// `warning: ...` on stderr.
pub fn warn(message: impl std::fmt::Display) {
    anstream::eprintln!("{YELLOW}warning{YELLOW:#}: {message}");
}

/// `error: ...` on stderr.
pub fn error(message: impl std::fmt::Display) {
    anstream::eprintln!("{RED}error{RED:#}: {message}");
}

/// `note: ...` on stderr.
pub fn note(message: impl std::fmt::Display) {
    anstream::eprintln!("{CYAN}note{CYAN:#}: {message}");
}

/// `12.3 KiB`.
pub fn size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KiB", "MiB", "GiB"];
    let mut v = bytes as f64;
    let mut u = 0;
    while v >= 1024.0 && u < UNITS.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    if u == 0 {
        format!("{bytes} B")
    } else {
        format!("{v:.1} {}", UNITS[u])
    }
}

/// A path relative to the current directory when possible, with forward slashes.
pub fn rel(path: &std::path::Path) -> String {
    let cwd = std::env::current_dir().ok();
    let shown = cwd
        .as_deref()
        .and_then(|c| path.strip_prefix(c).ok())
        .unwrap_or(path);
    let s = shown.display().to_string().replace('\\', "/");
    if s.is_empty() { ".".to_owned() } else { s }
}

#[cfg(test)]
mod tests {
    #[test]
    fn sizes() {
        assert_eq!(super::size(10), "10 B");
        assert_eq!(super::size(2048), "2.0 KiB");
        assert_eq!(super::size(5 * 1024 * 1024 + 1), "5.0 MiB");
    }
}
