//! The lints are themselves under test (L19): every banned pattern has a probe that clippy must reject.

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;
use std::{env, fs, process};

/// (file, line, lint), and for a `disallowed-types` entry the path it names after the lint.
type Hit = (String, usize, String);

fn root() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

fn parse(path: &Path) -> toml::Table {
    fs::read_to_string(path).unwrap().parse().unwrap()
}

/// A scratch workspace holding the probes under the repository's lint tables and `clippy.toml`.
fn scratch_workspace() -> std::path::PathBuf {
    let dir = env::temp_dir().join(format!("occam-probes-{}", process::id()));
    let probes = root().join("crates/occam-xtask/probes/bans");
    fs::create_dir_all(dir.join("bans/src")).unwrap();
    fs::copy(probes.join("Cargo.toml"), dir.join("bans/Cargo.toml")).unwrap();
    for entry in fs::read_dir(probes.join("src")).unwrap() {
        let entry = entry.unwrap();
        fs::copy(entry.path(), dir.join("bans/src").join(entry.file_name())).unwrap();
    }
    fs::copy(root().join("clippy.toml"), dir.join("clippy.toml")).unwrap();
    let manifest = parse(&root().join("Cargo.toml"));
    let lints = manifest["workspace"]["lints"].clone();
    let workspace = toml::toml! { members = ["bans"] resolver = "3" };
    let mut workspace = workspace;
    workspace.insert("lints".into(), lints);
    let mut table = toml::Table::new();
    table.insert("workspace".into(), workspace.into());
    fs::write(dir.join("Cargo.toml"), toml::to_string(&table).unwrap()).unwrap();
    dir
}

/// Every `// fires: <lint> [path]` mark in the probes.
fn marks(dir: &Path) -> BTreeSet<Hit> {
    let mut found = BTreeSet::new();
    for entry in fs::read_dir(dir.join("bans/src")).unwrap() {
        let path = entry.unwrap().path();
        let name = format!("bans/src/{}", path.file_name().unwrap().to_str().unwrap());
        for (line, n) in fs::read_to_string(&path).unwrap().lines().zip(1_usize..) {
            if let Some((_, mark)) = line.split_once("// fires: ") {
                found.insert((name.clone(), n, mark.trim().to_string()));
            }
        }
    }
    found
}

/// What clippy reported, each `disallowed_types` hit with the path its message names.
fn diagnostics(dir: &Path) -> BTreeSet<Hit> {
    let output = Command::new(env!("CARGO"))
        .args(["clippy", "--quiet", "--message-format=json", "--", "-D", "warnings"])
        .current_dir(dir)
        .env("CARGO_TARGET_DIR", dir.join("target"))
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let mut found = BTreeSet::new();
    for line in stdout.lines() {
        let value: serde_json::Value = serde_json::from_str(line).unwrap();
        let Some(code) = value["message"]["code"]["code"].as_str() else { continue };
        let span = value["message"]["spans"].as_array().unwrap().iter().find(|s| s["is_primary"] == true).unwrap();
        let path = value["message"]["message"].as_str().unwrap().split('`').nth(1).unwrap_or_default();
        let lint = if code == "clippy::disallowed_types" { format!("{code} {path}") } else { code.to_string() };
        found.insert((
            span["file_name"].as_str().unwrap().to_string(),
            usize::try_from(span["line_start"].as_u64().unwrap()).unwrap(),
            lint,
        ));
    }
    found
}

/// Every lint the workspace denies and every `clippy.toml` path, as a mark would name it.
fn banned() -> BTreeSet<String> {
    let manifest = parse(&root().join("Cargo.toml"));
    let lints = &manifest["workspace"]["lints"];
    let denied = |table: &toml::Value, prefix: &str| -> Vec<String> {
        let level = |v: &toml::Value| v.as_str().map(str::to_string);
        table
            .as_table()
            .unwrap()
            .iter()
            .filter(|(_, v)| matches!(level(v).as_deref(), Some("deny" | "forbid")))
            .map(|(k, _)| format!("{prefix}{k}"))
            .collect()
    };
    let mut all: BTreeSet<String> = denied(&lints["clippy"], "clippy::").into_iter().collect();
    all.extend(denied(&lints["rust"], ""));
    let clippy = parse(&root().join("clippy.toml"));
    for entry in clippy["disallowed-types"].as_array().into_iter().flatten() {
        all.insert(format!("clippy::disallowed_types {}", entry["path"].as_str().unwrap()));
    }
    all
}

#[test]
fn clippy_rejects_every_banned_pattern() {
    let dir = scratch_workspace();
    let want = marks(&dir);
    let got = diagnostics(&dir);
    fs::remove_dir_all(&dir).unwrap();
    assert_eq!(got, want, "each marked line must get exactly its diagnostic, and no other line one");
}

#[test]
fn every_banned_pattern_has_a_probe() {
    let dir = scratch_workspace();
    let marked: BTreeSet<String> = marks(&dir).into_iter().map(|(_, _, lint)| lint).collect();
    fs::remove_dir_all(&dir).unwrap();
    let banned = banned();
    assert!(!banned.is_empty(), "no lint is denied at all");
    let missing: Vec<&String> = banned.iter().filter(|b| !marked.contains(*b)).collect();
    let extra: Vec<&String> = marked.iter().filter(|m| !banned.contains(*m)).collect();
    assert!(missing.is_empty() && extra.is_empty(), "no probe: {missing:?}; no ban: {extra:?}");
}
