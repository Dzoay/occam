//! Source: original
//! What the build script embeds as `occam view`'s page (docs/decisions/2026-11-14-the-agent-starts-the-viewer.md): the
//! files `just web-build` builds into the crate's `page/` directory, or, where it has built none there, the stub page
//! that names the recipe, so a plain `cargo build` needs no node. The build script and this crate's tests both compile
//! this file; std only.

use std::path::{Path, PathBuf};

/// The files of `page` the binary serves, by the name each is served at, in name order: every file but the
/// directory's own `.gitignore` and the source maps, which only a browser's tools ask for.
pub fn files(page: &Path) -> Vec<(String, PathBuf)> {
    let Ok(entries) = std::fs::read_dir(page) else { return Vec::new() };
    let mut found: Vec<(String, PathBuf)> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .filter_map(|entry| Some((entry.file_name().into_string().ok()?, entry.path())))
        .filter(|(name, path)| name != ".gitignore" && path.extension().is_none_or(|extension| extension != "map"))
        .collect();
    found.sort();
    found
}

/// The Rust source that embeds the page `page` holds, or `stub` as its `index.html` where it holds none, and whether
/// it is the page: `BUILT` says so, and `FILES` holds each file's name and bytes, through `include_bytes!`.
pub fn source(page: &Path, stub: &Path) -> (String, bool) {
    let mut files = files(page);
    let built = files.iter().any(|(name, _)| name == "index.html");
    if !built {
        files = vec![("index.html".to_string(), stub.to_path_buf())];
    }
    let embedded =
        files.iter().map(|(name, path)| format!("    ({name:?}, include_bytes!({:?})),", path.display().to_string()));
    let lines: Vec<String> = [
        "/// Whether `just web-build` built the page in, rather than the stub that names it.".to_string(),
        format!("pub const BUILT: bool = {built};"),
        String::new(),
        "/// Each file of the page, by the name it is served at, with its bytes.".to_string(),
        "pub static FILES: &[(&str, &[u8])] = &[".to_string(),
    ]
    .into_iter()
    .chain(embedded)
    .chain(["];".to_string(), String::new()])
    .collect();
    let out = lines.join("\n");
    (out, built)
}
