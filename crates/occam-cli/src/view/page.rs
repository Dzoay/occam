//! Source: original
//! The page `occam view` serves, built into the binary by the build script (src/view/embed.rs): its files with their
//! types, and its `index.html` marked as the live page, which follows the model through the server's events.

mod embedded {
    #![cfg_attr(not(test), expect(dead_code, reason = "only the tests ask which page was built in"))]
    include!(concat!(env!("OUT_DIR"), "/page.rs"));
}

#[cfg(test)]
pub use embedded::BUILT;
pub use embedded::FILES;

/// A page's files, `index.html` among them, by the name each is served at, with its bytes.
pub type Files = &'static [(&'static str, &'static [u8])];

/// What the page's `index.html` opens with, and what the server sends in its place: the same element marked live.
const OPENING: &str = r#"<html lang="en">"#;
const LIVE: &str = r#"<html lang="en" data-live>"#;

/// Each of `all` but `index.html`, which the page's own address serves, by the name it is served at, with its bytes.
pub fn files(all: Files) -> impl Iterator<Item = (&'static str, &'static [u8])> {
    all.iter().copied().filter(|(name, _)| *name != "index.html")
}

/// The page's `index.html`, or the stub's, marked live.
pub fn index() -> String {
    let index = embedded::FILES.iter().find(|(name, _)| *name == "index.html").map_or(&[][..], |(_, bytes)| bytes);
    String::from_utf8_lossy(index).replacen(OPENING, LIVE, 1)
}

/// The type a file named `name` is served as. A module's must be `application/wasm` for the browser to compile it as
/// it arrives.
pub fn content_type(name: &str) -> &'static str {
    match name.rsplit_once('.').map(|(_, extension)| extension) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("wasm") => "application/wasm",
        Some("json" | "map") => "application/json",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_index_is_marked_live_and_each_file_has_its_type() {
        assert!(index().contains(LIVE), "an index, built or the stub, marked live");
        assert_eq!(index().contains("viewer.js"), BUILT, "the built page's script, or the stub");
        let map = |name: &str| std::path::Path::new(name).extension().is_some_and(|extension| extension == "map");
        assert!(files(FILES).all(|(name, _)| name != "index.html" && !map(name)));
        let page: Files = &[("index.html", b"<html>"), ("viewer.js", b"js"), ("occam.wasm", b"wasm")];
        assert_eq!(files(page).collect::<Vec<_>>(), [("viewer.js", &b"js"[..]), ("occam.wasm", &b"wasm"[..])]);
        assert_eq!(content_type("index.html"), "text/html; charset=utf-8");
        assert_eq!(content_type("data.json"), "application/json");
        assert_eq!(content_type("viewer.js.map"), "application/json");
        assert_eq!(content_type("occam.wasm"), "application/wasm");
        assert_eq!(content_type("viewer.js"), "text/javascript; charset=utf-8");
        assert_eq!(content_type("viewer.css"), "text/css; charset=utf-8");
        assert_eq!(content_type("README"), "application/octet-stream");
    }
}
