//! Link setup for the embedded libmpv, and the Windows executable icon.
//!
//! Linux links the system `libmpv` through the `#[link]` attribute in `player/ffi.rs`.
//! Windows links `mpv-2` (the import library of the bundled `libmpv-2.dll`); its directory
//! comes from the `MPV_LIB_DIR` environment variable, which the release workflow sets to
//! the extracted mpv-dev package.
//!
//! Windows builds also compile `assets/vidcue.ico` into the executable with `windres`, so
//! Explorer, Start-menu shortcuts and the installer show the logo. The running window's
//! icon is set separately at startup (`rune::set_app_icon` in `main.rs`).

use std::{env, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    if let Ok(dir) = env::var("MPV_LIB_DIR") {
        println!("cargo:rustc-link-search=native={dir}");
    }
    println!("cargo:rerun-if-env-changed=MPV_LIB_DIR");
    embed_icon();
}

/// Compiles a one-line resource script naming the icon and links the object into the
/// binaries. Only GNU targets are handled (the release builds x86_64-pc-windows-gnu).
fn embed_icon() {
    println!("cargo:rerun-if-env-changed=WINDRES");
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let icon = manifest.join("assets").join("vidcue.ico");
    println!("cargo:rerun-if-changed={}", icon.display());
    if env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("gnu") {
        println!("cargo:warning=The exe icon is only embedded for windows-gnu builds");
        return;
    }
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    let rc = out.join("vidcue.rc");
    // windres reads forward slashes on every host and needs no escaping for them.
    let icon_path = icon.display().to_string().replace('\\', "/");
    std::fs::write(&rc, format!("1 ICON \"{icon_path}\"\n")).expect("write vidcue.rc");
    let obj = out.join("vidcue-icon.o");
    let windres = env::var("WINDRES").ok().or_else(|| {
        ["x86_64-w64-mingw32-windres", "windres"]
            .into_iter()
            .find(|tool| Command::new(tool).arg("--version").output().is_ok())
            .map(str::to_owned)
    });
    let Some(windres) = windres else {
        // A Linux cross-check without mingw binutils still builds, just without the icon.
        println!("cargo:warning=windres not found; the exe has no embedded icon");
        return;
    };
    let status = Command::new(&windres)
        .args(["--input-format=rc", "--output-format=coff", "-i"])
        .arg(&rc)
        .arg("-o")
        .arg(&obj)
        .status()
        .unwrap_or_else(|e| panic!("cannot run {windres}: {e}"));
    assert!(status.success(), "{windres} failed to compile vidcue.rc");
    println!("cargo:rustc-link-arg-bins={}", obj.display());
}
