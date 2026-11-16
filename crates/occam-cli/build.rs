//! Source: original
//! Embeds `occam view`'s page in the binary: the files `just web-build` builds into `page/`, or the stub page that names
//! the recipe where it has built none (src/view/embed.rs). It runs again whenever `page/` changes. Std only.
#![allow(clippy::print_stdout, reason = "a build script speaks to cargo on standard output")]

use std::path::{Path, PathBuf};

#[path = "src/view/embed.rs"]
mod embed;

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let (page, stub) = (root.join("page"), root.join("src/view/stub.html"));
    println!("cargo::rerun-if-changed={}", page.display());
    println!("cargo::rerun-if-changed={}", stub.display());
    let (source, built) = embed::source(&page, &stub);
    println!("cargo::rustc-env=OCCAM_PAGE={}", if built { "built" } else { "stub" });
    let out = std::env::var_os("OUT_DIR").map_or_default(PathBuf::from).join("page.rs");
    if let Err(error) = std::fs::write(&out, source) {
        println!("cargo::error=the embedded page could not be written to {}: {error}", out.display());
    }
}
