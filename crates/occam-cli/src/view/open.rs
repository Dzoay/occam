//! Source: original
//! Opening the page in a browser, through the program `BROWSER` names if it is set, as many tools read it, or the
//! system's own opener: `open` on macOS, `start` on Windows and `xdg-open` elsewhere. It is started on its own and not
//! waited for; `--no-open` skips it, and a test names its own program in `BROWSER`.

use std::ffi::OsString;
use std::process::{Command, Stdio};

/// The system's own opener where `os` runs, as `std::env::consts::OS` names it: its program, and what it takes before
/// the address.
fn system(os: &str) -> (&'static str, &'static [&'static str]) {
    match os {
        "macos" => ("open", &[]),
        "windows" => ("cmd", &["/C", "start", ""]),
        _ => ("xdg-open", &[]),
    }
}

/// The command that opens `url` where `os` runs, given what `BROWSER` holds, if anything.
fn opener(browser: Option<OsString>, os: &str, url: &str) -> Command {
    let named = browser.filter(|program| !program.is_empty()).map(|program| (program, &[][..]));
    let (program, before) = named.unwrap_or_else(|| {
        let (program, before) = system(os);
        (program.into(), before)
    });
    let mut command = Command::new(program);
    command.args(before);
    command.arg(url);
    command
}

/// Opens `url` in a browser; whether the program that opens it could be started.
pub fn page(url: &str) -> bool {
    let mut command = opener(std::env::var_os("BROWSER"), std::env::consts::OS, url);
    command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The program and arguments of `command`.
    fn said(command: &Command) -> (String, Vec<String>) {
        let arguments = command.get_args().map(|argument| argument.to_string_lossy().into_owned());
        (command.get_program().to_string_lossy().into_owned(), arguments.collect())
    }

    #[test]
    fn the_browser_named_opens_the_page_and_otherwise_each_systems_own_opener() {
        let url = "http://127.0.0.1:1/t/";
        let at = |program: &str, before: &[&str]| {
            let arguments = before.iter().chain(&[url]).map(ToString::to_string).collect();
            (program.to_string(), arguments)
        };
        assert_eq!(said(&opener(Some("firefox".into()), "linux", url)), at("firefox", &[]));
        assert_eq!(
            said(&opener(Some(OsString::new()), "linux", url)),
            at("xdg-open", &[]),
            "an empty BROWSER names none"
        );
        assert_eq!(said(&opener(None, "macos", url)), at("open", &[]));
        assert_eq!(said(&opener(None, "windows", url)), at("cmd", &["/C", "start", ""]));
        assert_eq!(said(&opener(None, "freebsd", url)), at("xdg-open", &[]));
    }
}
