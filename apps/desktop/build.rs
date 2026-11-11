//! Link setup for the embedded libmpv.
//!
//! Linux links the system `libmpv` through the `#[link]` attribute in `player/ffi.rs`.
//! Windows links `mpv-2` (the import library of the bundled `libmpv-2.dll`); its directory
//! comes from the `MPV_LIB_DIR` environment variable, which the release workflow sets to
//! the extracted mpv-dev package.

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        if let Ok(dir) = std::env::var("MPV_LIB_DIR") {
            println!("cargo:rustc-link-search=native={dir}");
        }
        println!("cargo:rerun-if-env-changed=MPV_LIB_DIR");
    }
    println!("cargo:rerun-if-changed=build.rs");
}
