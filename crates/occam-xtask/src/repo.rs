//! The repository and the programs around it. Every input a gate's pure check is given arrives through here. Process
//! glue only: the gates' own runs exercise it, and `.cargo/mutants.toml` excludes it.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::{env, fs, process, thread};

use crate::verdict::Finding;

/// The branch every PR is squash-merged into.
const BASE: &str = "origin/main";

/// One cargo-mutants pass: whether it succeeded, then its caught, missed, timeout and unviable lists.
pub struct MutantsRun {
    pub success: bool,
    pub outcomes: [String; 4],
}

pub struct Repo {
    root: PathBuf,
}

fn io(path: &str, error: &std::io::Error) -> Finding {
    Finding::new("repo.io", path, error.to_string(), "check that the file exists and is readable")
}

impl Repo {
    /// The repository this binary was built from.
    pub fn locate() -> Self {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        Self { root: manifest.ancestors().nth(2).map_or_else(|| PathBuf::from("."), Path::to_path_buf) }
    }

    pub fn read(&self, rel: &str) -> Result<String, Finding> {
        fs::read_to_string(self.root.join(rel)).map_err(|e| io(rel, &e))
    }

    pub fn write(&self, rel: &str, text: &str) -> Result<(), Finding> {
        fs::write(self.root.join(rel), text).map_err(|e| io(rel, &e))
    }

    /// Reads a file wherever it is, such as a PR body or a merge driver's temporary file.
    pub fn read_outside(path: &str) -> Result<String, Finding> {
        fs::read_to_string(path).map_err(|e| io(path, &e))
    }

    /// Runs a program from the root, returning whether it succeeded and its output, stdout then stderr.
    pub fn run(&self, program: &str, args: &[&str]) -> Result<(bool, String), Finding> {
        let output = Command::new(program).args(args).current_dir(&self.root).output().map_err(|e| {
            Finding::new("repo.no-tool", program, e.to_string(), "run `just setup` to install the gate tools")
        })?;
        let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
        text.push_str(&String::from_utf8_lossy(&output.stderr));
        Ok((output.status.success(), text))
    }

    /// Runs a program and turns a failure into a finding carrying the end of its output.
    pub fn checked(&self, code: &'static str, program: &str, args: &[&str]) -> Result<(), Finding> {
        let (ok, output) = self.run(program, args)?;
        if ok {
            return Ok(());
        }
        let mut tail: Vec<&str> = output.lines().rev().take(20).collect();
        tail.reverse();
        let shown = format!("{program} {} failed:\n{}", args.join(" "), tail.join("\n"));
        Err(Finding::new(code, "", shown, format!("run {program} {} and fix what it reports", args.join(" "))))
    }

    pub fn git(&self, args: &[&str]) -> Result<String, Finding> {
        let (ok, output) = self.run("git", args)?;
        if ok {
            Ok(output)
        } else {
            Err(Finding::new(
                "repo.git",
                "",
                format!("git {} failed: {output}", args.join(" ")),
                "fetch origin/main and retry",
            ))
        }
    }

    /// Every path git tracks or would track, that exists.
    pub fn paths(&self) -> Result<Vec<String>, Finding> {
        let listed = self.git(&["ls-files", "--cached", "--others", "--exclude-standard", "--deduplicate"])?;
        Ok(listed.lines().filter(|path| self.root.join(path).is_file()).map(str::to_string).collect())
    }

    /// The text of every path `filter` accepts; a file that is not UTF-8 is skipped.
    pub fn texts(&self, filter: impl Fn(&str) -> bool) -> Result<Vec<(String, String)>, Finding> {
        let paths = self.paths()?;
        let accepted = paths.into_iter().filter(|path| filter(path));
        Ok(accepted.filter_map(|path| Some((path.clone(), fs::read_to_string(self.root.join(&path)).ok()?))).collect())
    }

    /// The merge base with `origin/main`.
    pub fn base(&self) -> Result<String, Finding> {
        Ok(self.git(&["merge-base", "HEAD", BASE])?.trim().to_string())
    }

    /// A file as it was at `rev`, if it existed.
    pub fn at(&self, rev: &str, path: &str) -> Option<String> {
        self.git(&["show", &format!("{rev}:{path}")]).ok()
    }

    /// Every commit message on `origin/main`.
    pub fn main_log(&self) -> Result<String, Finding> {
        self.git(&["log", BASE, "--format=%B"])
    }

    /// One line per test: `<binary> <test>`.
    pub fn test_listing(&self) -> Result<String, Finding> {
        let args =
            ["nextest", "list", "--workspace", "--message-format", "oneline", "--color", "never", "--cargo-quiet"];
        let (ok, output) = self.run(env!("CARGO"), &args)?;
        if ok {
            Ok(output)
        } else {
            Err(Finding::new("counts.listing-failed", "", output, "fix the build, then run `just counts`"))
        }
    }

    pub fn cargo_mutants(&self, pass: &str, diff: &str, args: &[&str]) -> Result<MutantsRun, Finding> {
        let dir = env::temp_dir().join(format!("occam-mutants-{}-{pass}", process::id()));
        let shown = dir.display().to_string();
        drop(fs::remove_dir_all(&dir));
        fs::create_dir_all(&dir).map_err(|e| io(&shown, &e))?;
        let diff_file = dir.join("changes.diff");
        fs::write(&diff_file, diff).map_err(|e| io(&shown, &e))?;
        // By default a quarter of the hardware threads, 1 to 4: each job runs its own parallel build and test binaries.
        let quarter = || thread::available_parallelism().map_or(1, |n| (n.get() / 4).clamp(1, 4)).to_string();
        let jobs = env::var("OCCAM_MUTANTS_JOBS").unwrap_or_else(|_| quarter());
        let status = Command::new(env!("CARGO"))
            .args(["mutants", "--baseline=skip", "--jobs", &jobs, "--in-diff"])
            .arg(&diff_file)
            .args(args)
            .arg("--output")
            .arg(&dir)
            .current_dir(&self.root)
            .env_remove("CARGO_TARGET_DIR")
            .env("CARGO_INCREMENTAL", "1")
            .status()
            .map_err(|e| Finding::new("repo.no-tool", "cargo-mutants", e.to_string(), "run `just setup`"))?;
        let out = dir.join("mutants.out");
        let outcomes = ["caught.txt", "missed.txt", "timeout.txt", "unviable.txt"]
            .map(|name| fs::read_to_string(out.join(name)).unwrap_or_default());
        if status.success() {
            drop(fs::remove_dir_all(&dir));
        }
        Ok(MutantsRun { success: status.success(), outcomes })
    }
}
