//! The executable's stack: the game runs the dedicated server on the process main thread, which
//! needs 8 MB, as Horizon's own executable gets it (`horizon/packaging/windows/exe.rs`). Windows
//! gives a main thread 1 MB unless the linker says otherwise; Linux and macOS already give 8 MB.
fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "windows" {
        let arg = match std::env::var("CARGO_CFG_TARGET_ENV").as_deref() {
            Ok("msvc") => "/STACK:8388608",
            _ => "-Wl,--stack,8388608",
        };
        println!("cargo:rustc-link-arg-bin=rustic-server={arg}");
    }
    println!("cargo:rerun-if-changed=build.rs");
}
