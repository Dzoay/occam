//! Source: original
//! Where the harness writes its results, and which engine each must have come from.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The engine a result must have come from. Anything else is treated as no result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pin {
    /// The engine's name.
    pub engine: &'static str,
    /// The engine's version.
    pub engine_version: &'static str,
    /// The package or repository it came from.
    pub package: &'static str,
    /// The package's version or commit.
    pub package_version: &'static str,
}

/// The reference.
pub const OPENCASCADE: Pin =
    Pin { engine: "opencascade", engine_version: "8.0.1", package: "cadquery-ocp", package_version: "8.0.1.1.0" };

/// The second opinion.
pub const ARRIS: Pin = Pin {
    engine: "arris",
    engine_version: "0.10.0",
    package: "https://github.com/arris-labs/arris.git",
    package_version: "f6860e379ba054888b087b8e141f47cd1e631eab",
};

impl Pin {
    /// The result file for `case` run at `threads`, under `root`.
    #[must_use]
    pub fn path(self, root: &Path, threads: u32, case: &str) -> PathBuf {
        root.join(format!("{}-{}", self.engine, self.engine_version))
            .join(format!("threads-{threads}"))
            .join(format!("{case}.json"))
    }
}

/// Where the harness writes by default: `$XDG_CACHE_HOME/occam-oracle/results`, or `$HOME/.cache/occam-oracle/results` if that
/// is unset or empty. `variable` reads the environment. `None` if neither is set.
#[must_use]
pub fn default_root(variable: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let cache = variable("XDG_CACHE_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| variable("HOME").map(|home| PathBuf::from(home).join(".cache")))?;
    Some(cache.join("occam-oracle").join("results"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_result_is_found_by_engine_version_threads_and_case() {
        let got = OPENCASCADE.path(Path::new("/results"), 4, "box-cut-cylinder-0001");
        assert_eq!(got, Path::new("/results/opencascade-8.0.1/threads-4/box-cut-cylinder-0001.json"));
        let got = ARRIS.path(Path::new("/results"), 1, "a");
        assert_eq!(got, Path::new("/results/arris-0.10.0/threads-1/a.json"));
    }

    fn lookup<'a>(set: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<OsString> + 'a {
        move |name| set.iter().find(|(key, _)| *key == name).map(|(_, value)| OsString::from(value))
    }

    #[test]
    fn the_default_root_prefers_the_cache_variable() {
        let got = default_root(lookup(&[("XDG_CACHE_HOME", "/cache"), ("HOME", "/home/someone")]));
        assert_eq!(got.as_deref(), Some(Path::new("/cache/occam-oracle/results")));
    }

    #[test]
    fn the_default_root_falls_back_to_the_home_directory() {
        let got = default_root(lookup(&[("HOME", "/home/someone")]));
        assert_eq!(got.as_deref(), Some(Path::new("/home/someone/.cache/occam-oracle/results")));
    }

    #[test]
    fn an_empty_cache_variable_counts_as_unset() {
        let got = default_root(lookup(&[("XDG_CACHE_HOME", ""), ("HOME", "/home/someone")]));
        assert_eq!(got.as_deref(), Some(Path::new("/home/someone/.cache/occam-oracle/results")));
    }

    #[test]
    fn without_either_variable_there_is_no_default() {
        assert_eq!(default_root(lookup(&[])), None);
    }
}
