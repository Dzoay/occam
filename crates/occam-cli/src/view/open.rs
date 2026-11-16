//! Source: original
//! Opening the page in a browser, through the program `BROWSER` names if it is set, as many tools read it, or the
//! system's own opener: `open` on macOS, `start` on Windows and `xdg-open` elsewhere. It is started on its own and not
//! waited for; `--no-open` skips it, and a test names its own program in `BROWSER`.

use std::ffi::OsString;
use std::process::{Command, Stdio};

/// The command that opens `url`, given what `BROWSER` holds, if anything.
fn opener(browser: Option<OsString>, url: &str) -> Command {
    let mut command = match browser.filter(|program| !program.is_empty()) {
        Some(program) => Command::new(program),
        None if cfg!(target_os = "macos") => Command::new("open"),
        None if cfg!(windows) => {
            let mut start = Command::new("cmd");
            start.args(["/C", "start", ""]);
            start
        }
        None => Command::new("xdg-open"),
    };
    command.arg(url);
    command
}

/// Opens `url` in a browser; whether the program that opens it could be started.
pub fn page(url: &str) -> bool {
    let mut command = opener(std::env::var_os("BROWSER"), url);
    command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_browser_named_opens_the_page_and_otherwise_the_systems_opener() {
        let named = opener(Some("firefox".into()), "http://127.0.0.1:1/t/");
        assert_eq!(named.get_program(), "firefox");
        assert_eq!(named.get_args().collect::<Vec<_>>(), ["http://127.0.0.1:1/t/"]);
        let system = opener(Some(OsString::new()), "http://127.0.0.1:1/t/");
        let want = if cfg!(target_os = "macos") {
            "open"
        } else if cfg!(windows) {
            "cmd"
        } else {
            "xdg-open"
        };
        assert_eq!(system.get_program(), want, "an empty BROWSER names nothing");
        assert_eq!(system.get_args().last().and_then(|arg| arg.to_str()), Some("http://127.0.0.1:1/t/"));
    }
}
