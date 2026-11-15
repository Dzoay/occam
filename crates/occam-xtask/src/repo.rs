//! The repository and the programs around it. Every input a gate's pure check is given arrives through here. Process
//! glue only: the gates' own runs exercise it, and `.cargo/mutants.toml` excludes it.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::{env, fs, thread};

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

    /// A repository at `root`, for a test that builds one in a scratch directory.
    #[cfg(test)]
    pub const fn rooted(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn read(&self, rel: &str) -> Result<String, Finding> {
        fs::read_to_string(self.root.join(rel)).map_err(|e| io(rel, &e))
    }

    pub fn write(&self, rel: &str, text: &str) -> Result<(), Finding> {
        fs::write(self.root.join(rel), text).map_err(|e| io(rel, &e))
    }

    /// Reads a file wherever it is, such as a PR body.
    pub fn read_outside(path: &str) -> Result<String, Finding> {
        fs::read_to_string(path).map_err(|e| io(path, &e))
    }

    /// Runs a program from the root, returning whether it succeeded, its stdout and its stderr. A caller that parses the
    /// output wants stdout alone, because stderr carries the compiler's warnings.
    pub fn capture(&self, program: &str, args: &[&str]) -> Result<(bool, String, String), Finding> {
        self.captured(Command::new(program).args(args), program)
    }

    /// Runs `command` from the root, as [`Repo::capture`] does `program`.
    fn captured(&self, command: &mut Command, program: &str) -> Result<(bool, String, String), Finding> {
        let output = command.current_dir(&self.root).output().map_err(|e| {
            Finding::new("repo.no-tool", program, e.to_string(), "run `just setup` to install the gate tools")
        })?;
        let text = |bytes: &[u8]| String::from_utf8_lossy(bytes).into_owned();
        Ok((output.status.success(), text(&output.stdout), text(&output.stderr)))
    }

    /// Runs a program from the root, returning whether it succeeded and its output, stdout then stderr.
    pub fn run(&self, program: &str, args: &[&str]) -> Result<(bool, String), Finding> {
        let (ok, out, err) = self.capture(program, args)?;
        Ok((ok, out + &err))
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

    /// A directory of its own for a build whose flags differ, nested in this checkout's cargo target directory.
    pub fn nested_target(&self, name: &str) -> String {
        env::var_os("CARGO_TARGET_DIR")
            .map_or_else(|| self.root.join("target"), PathBuf::from)
            .join(name)
            .display()
            .to_string()
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
        self.merge_base(BASE)
    }

    /// The merge base of `HEAD` and `rev`.
    pub fn merge_base(&self, rev: &str) -> Result<String, Finding> {
        Ok(self.git(&["merge-base", "HEAD", rev])?.trim().to_string())
    }

    /// The commit `HEAD` names.
    pub fn head(&self) -> Result<String, Finding> {
        Ok(self.git(&["rev-parse", "HEAD"])?.trim().to_string())
    }

    /// A file as it was at `rev`, if it existed.
    pub fn at(&self, rev: &str, path: &str) -> Option<String> {
        self.git(&["show", &format!("{rev}:{path}")]).ok()
    }

    /// Every commit message on `origin/main`.
    pub fn main_log(&self) -> Result<String, Finding> {
        self.messages(BASE)
    }

    /// Every commit message in `range`, as `git log` takes it.
    pub fn messages(&self, range: &str) -> Result<String, Finding> {
        self.git(&["log", range, "--format=%B"])
    }

    /// One line per test: `<binary> <test>`.
    pub fn test_listing(&self) -> Result<String, Finding> {
        self.listing(None, |errors| Finding::new("counts.listing-failed", "", errors, "fix the build"))
    }

    /// The test listing of commit `rev`, from a worktree of it built in a target directory of its own: a second build,
    /// whose dependencies stay built there for the next. The worktree is removed afterwards.
    pub fn test_listing_at(&self, rev: &str) -> Result<String, Finding> {
        let tree = self.nested_target("counts-base-tree");
        self.remove_worktree(&tree);
        self.git(&["worktree", "add", "--detach", "--quiet", &tree, rev])?;
        let listed = Self { root: PathBuf::from(&tree) }.listing(Some(&self.nested_target("counts-base")), |errors| {
            let remedy = format!("check that the base, {rev}, builds and lists its tests");
            Finding::new("counts.base-listing-failed", "", errors, remedy)
        });
        self.remove_worktree(&tree);
        listed
    }

    /// Removes the worktree at `tree`, and what is left of it if git no longer knows it.
    fn remove_worktree(&self, tree: &str) {
        drop(self.git(&["worktree", "remove", "--force", tree]));
        drop(fs::remove_dir_all(tree));
        drop(self.git(&["worktree", "prune"]));
    }

    /// Lists the tests from the root, built in `target` if one is given; `failed` turns the build's errors into a
    /// finding.
    fn listing(&self, target: Option<&str>, failed: impl FnOnce(String) -> Finding) -> Result<String, Finding> {
        let args =
            ["nextest", "list", "--workspace", "--message-format", "oneline", "--color", "never", "--cargo-quiet"];
        let mut command = Command::new(env!("CARGO"));
        command.args(args);
        if let Some(target) = target {
            command.env("CARGO_TARGET_DIR", target);
        }
        let (ok, listing, errors) = self.captured(&mut command, env!("CARGO"))?;
        if ok { Ok(listing) } else { Err(failed(errors)) }
    }

    /// One cargo-mutants pass over `diff`. What it writes stays in `mutants.out/<pass>` at the root, which git ignores,
    /// where a later run with `--iterate` finds what this one caught.
    pub fn cargo_mutants(&self, pass: &str, diff: &str, args: &[&str]) -> Result<MutantsRun, Finding> {
        let dir = self.root.join("mutants.out").join(pass);
        let shown = dir.display().to_string();
        fs::create_dir_all(&dir).map_err(|e| io(&shown, &e))?;
        let diff_file = dir.join("changes.diff");
        fs::write(&diff_file, diff).map_err(|e| io(&shown, &e))?;
        // cargo-mutants moves the last run's output aside as it starts. Where there was one and it is still in place,
        // this run never started, and the lists there are the last run's: they are not read.
        let (out, aside) = (dir.join("mutants.out"), dir.join("mutants.out.old"));
        drop(fs::remove_dir_all(&aside));
        let stale = out.is_dir();
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
        let fresh = !stale || aside.is_dir();
        let outcomes = ["caught.txt", "missed.txt", "timeout.txt", "unviable.txt"]
            .map(|name| if fresh { fs::read_to_string(out.join(name)).unwrap_or_default() } else { String::new() });
        Ok(MutantsRun { success: status.success(), outcomes })
    }
}
