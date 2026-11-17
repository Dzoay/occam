//! The repository and the programs around it. Every input a gate's pure check is given arrives through here. Process
//! glue only: the gates' own runs exercise it, and `.cargo/mutants.toml` excludes it.

use std::num::NonZero;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::{env, fs, process, thread};

use crate::verdict::Finding;

/// The branch every PR is squash-merged into.
const BASE: &str = "origin/main";

/// The lists cargo-mutants writes, in the order [`MutantsRun::outcomes`] and [`Repo::mutants_record`] hold them.
const LISTS: [&str; 4] = ["caught.txt", "missed.txt", "timeout.txt", "unviable.txt"];

/// The list of what earlier runs settled, which cargo-mutants reads for `--iterate` beside the caught and the unviable.
const SETTLED: &str = "previously_caught.txt";

/// Removes the mutants gate's directory `$1` once the gate is gone, however it went: its standard input is a pipe only
/// the gate holds, so it ends when the gate does. The gate removes the directory itself when it returns. Where it could
/// not, what it started may still be writing there: all of that runs in this process group, and is interrupted first.
const JANITOR: &str = r#"trap '' INT TERM HUP
cat > /dev/null
for _ in 1 2 3 4 5 6 7 8 9 10; do
  [ -e "$1" ] || exit 0
  kill -INT 0 2> /dev/null
  sleep 1
  rm -rf -- "$1" 2> /dev/null
done
"#;

/// One copy's cargo-mutants run: whether it succeeded, then its caught, missed, timeout and unviable lists.
pub struct MutantsRun {
    pub success: bool,
    pub outcomes: [String; 4],
}

/// One pass of the mutants gate over copies of the built tree.
pub struct Copies<'a> {
    /// The pass's name, and its directory in `mutants.out`.
    pub pass: &'a str,
    /// The mutants earlier runs settled, one per line, for `--iterate`; empty otherwise.
    pub settled: &'a str,
    /// The cargo arguments that built the tree's tests, which a copy must find built.
    pub build: &'a [&'a str],
    /// Refuses that build's output in a copy, if it compiled anything.
    pub fresh: fn(&str) -> Result<(), Finding>,
    /// The cargo-mutants arguments of each copy.
    pub shards: &'a [Vec<&'a str>],
}

/// The mutants gate's directory outside the checkout, `$TMPDIR/occam-mutants-warm/<process id>`: `staging`, the
/// checkout's files with the unmutated tests built in its own `target`, and the copies of it that are mutated. The gate
/// holds a lock on the directory while it runs, removes it on returning, and leaves a janitor to remove it if it is
/// killed first.
pub struct Warm {
    home: PathBuf,
    _held: fs::File,
    janitor: Child,
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
        captured(command.current_dir(&self.root), program)
    }

    /// Runs a program from the root, returning whether it succeeded and its output, stdout then stderr.
    pub fn run(&self, program: &str, args: &[&str]) -> Result<(bool, String), Finding> {
        let (ok, out, err) = self.capture(program, args)?;
        Ok((ok, out + &err))
    }

    /// Runs a program and turns a failure into a finding carrying the end of its output.
    pub fn checked(&self, code: &'static str, program: &str, args: &[&str]) -> Result<(), Finding> {
        let (ok, output) = self.run(program, args)?;
        if ok { Ok(()) } else { Err(failed(code, program, args, &output)) }
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

    /// The workspace's mutants as `cargo mutants --list` names them, one listing under each of `configs`, the text of a
    /// configuration each. They are written to `mutants.out/exclusions` at the root, which git ignores, and listed side
    /// by side, as many at once as the machine has threads.
    pub fn mutants_listings(&self, configs: &[String]) -> Result<Vec<String>, Finding> {
        let dir = self.root.join("mutants.out").join("exclusions");
        fs::create_dir_all(&dir).map_err(|e| io(&dir.display().to_string(), &e))?;
        let jobs: Vec<(PathBuf, &String)> =
            configs.iter().enumerate().map(|(k, config)| (dir.join(format!("{k}.toml")), config)).collect();
        let width = thread::available_parallelism().map_or(1, NonZero::get);
        let mut listings = Vec::with_capacity(jobs.len());
        for chunk in jobs.chunks(width) {
            let listed: Vec<Result<String, Finding>> = thread::scope(|scope| {
                let running: Vec<_> =
                    chunk.iter().map(|(file, config)| scope.spawn(|| self.mutants_listing(file, config))).collect();
                let lost = || Err(Finding::new("repo.thread", "", "a listing's thread panicked", "run it again"));
                running.into_iter().map(|job| job.join().unwrap_or_else(|_| lost())).collect()
            });
            for listing in listed {
                listings.push(listing?);
            }
        }
        Ok(listings)
    }

    /// `cargo mutants --list` of the whole workspace under the configuration `config`, written to `file` first.
    fn mutants_listing(&self, file: &Path, config: &str) -> Result<String, Finding> {
        fs::write(file, config).map_err(|e| io(&file.display().to_string(), &e))?;
        let mut command = Command::new(env!("CARGO"));
        command.args(["mutants", "--list", "--workspace", "--config"]).arg(file);
        let (ok, listing, errors) = self.captured(&mut command, "cargo-mutants")?;
        if ok {
            return Ok(listing);
        }
        let remedy = format!("run `cargo mutants --list --config {}` and fix what it reports", file.display());
        Err(Finding::new("mutants.listing-failed", ".cargo/mutants.toml", errors, remedy))
    }

    /// `OCCAM_MUTANTS_JOBS`, if it is set, and the hardware threads.
    pub fn mutants_jobs() -> (Option<String>, usize) {
        (env::var("OCCAM_MUTANTS_JOBS").ok(), thread::available_parallelism().map_or(1, std::num::NonZero::get))
    }

    /// Where the mutants gate keeps `pass`'s lists: `mutants.out/<pass>` at the root, which git ignores.
    fn mutants_out(&self, pass: &str) -> PathBuf {
        self.root.join("mutants.out").join(pass)
    }

    /// The mutants of `diff` that `args` select, one per line as cargo-mutants names them. Nothing is built. With
    /// `--iterate` among `args`, cargo-mutants leaves out what the lists of the last [`Repo::mutants_record`] settled.
    pub fn mutants_list(&self, pass: &str, diff: &str, args: &[&str]) -> Result<String, Finding> {
        let dir = self.mutants_out(pass);
        let shown = dir.display().to_string();
        fs::create_dir_all(&dir).map_err(|e| io(&shown, &e))?;
        let diff_file = dir.join("changes.diff");
        fs::write(&diff_file, diff).map_err(|e| io(&shown, &e))?;
        let mut command = Command::new(env!("CARGO"));
        // Plain names, whatever `CARGO_TERM_COLOR` says: the gate compares them with the names the copies' lists hold.
        command
            .args(["mutants", "--colors", "never", "--in-diff"])
            .arg(&diff_file)
            .args(args)
            .arg("--output")
            .arg(&dir);
        let (ok, listed, errors) = self.captured(&mut command, "cargo-mutants")?;
        if ok { Ok(listed) } else { Err(failed("mutants.run-failed", "cargo mutants --list", args, &errors)) }
    }

    /// What the runs before this one settled in `pass`, one mutant per line: the lists `--iterate` reads.
    pub fn mutants_settled(&self, pass: &str) -> String {
        let dir = self.mutants_out(pass).join("mutants.out");
        let lists = ["caught.txt", SETTLED, "unviable.txt"].map(|name| fs::read_to_string(dir.join(name)));
        lists.into_iter().flatten().collect()
    }

    /// Keeps a pass's `lists` where a later run with `--iterate` finds them, in `mutants.out/<pass>/mutants.out`, with
    /// what was `settled` before it. Each copy's own output, logs included, is beside it in `shard-<j>`.
    pub fn mutants_record(&self, pass: &str, settled: &str, lists: &[String; 4]) -> Result<(), Finding> {
        let dir = self.mutants_out(pass);
        let out = dir.join("mutants.out");
        let shown = out.display().to_string();
        drop(fs::remove_dir_all(dir.join("mutants.out.old")));
        drop(fs::remove_dir_all(&out));
        fs::create_dir_all(&out).map_err(|e| io(&shown, &e))?;
        for (name, list) in LISTS.into_iter().zip(lists).chain([(SETTLED, &settled.to_string())]) {
            fs::write(out.join(name), list).map_err(|e| io(&shown, &e))?;
        }
        Ok(())
    }

    /// A copy of every file git tracks or would track, in a new directory under `TMPDIR` of this run's own, where the
    /// mutants gate builds and mutates. `leftover` says whether a name there is a run's directory: one whose run is
    /// gone, so that nothing holds its lock, is removed first, and none is ever used again.
    pub fn warm(&self, leftover: fn(&str) -> bool) -> Result<Warm, Finding> {
        let all = env::var_os("TMPDIR").map(PathBuf::from).ok_or_else(|| {
            let message = "TMPDIR is not set, and the copies of the tree must not go to a /tmp held in memory";
            Finding::new("mutants.no-tmpdir", "justfile", message, "run `just mutants`, which sets it")
        })?;
        let all = all.join("occam-mutants-warm");
        let shown = all.display().to_string();
        fs::create_dir_all(&all).map_err(|e| io(&shown, &e))?;
        for entry in fs::read_dir(&all).map_err(|e| io(&shown, &e))?.flatten() {
            let run = entry.file_name().to_str().is_some_and(leftover);
            if run && fs::File::open(entry.path()).is_ok_and(|dir| dir.try_lock().is_ok()) {
                drop(fs::remove_dir_all(entry.path()));
            }
        }
        let home = all.join(process::id().to_string());
        fs::create_dir(&home).map_err(|e| io(&shown, &e))?;
        let held = fs::File::open(&home).map_err(|e| io(&shown, &e))?;
        held.try_lock().map_err(|e| Finding::new("repo.io", &shown, e.to_string(), "run `just mutants` again"))?;
        let mut janitor = Command::new("sh");
        grouped(janitor.args(["-c", JANITOR, "sh"]).arg(&home).stdin(Stdio::piped()), 0);
        let janitor = janitor.spawn().map_err(|e| {
            drop(fs::remove_dir_all(&home));
            io("sh", &e)
        })?;
        let warm = Warm { home, _held: held, janitor };
        for path in self.paths()? {
            let to = warm.staging().join(&path);
            to.parent().map_or(Ok(()), fs::create_dir_all).map_err(|e| io(&path, &e))?;
            fs::copy(self.root.join(&path), to).map_err(|e| io(&path, &e))?;
        }
        Ok(warm)
    }
}

impl Warm {
    fn staging(&self) -> PathBuf {
        self.home.join("staging")
    }

    /// `program`, to run in `tree` as cargo-mutants runs cargo: building in the tree's own `target`. It runs in the
    /// janitor's process group, apart from the gate's, so that the janitor can stop it.
    fn command(&self, program: &str, tree: &Path) -> Command {
        let mut command = Command::new(program);
        command.current_dir(tree).env_remove("CARGO_TARGET_DIR").env("CARGO_INCREMENTAL", "1").stdin(Stdio::null());
        grouped(&mut command, i32::try_from(self.janitor.id()).unwrap_or(0));
        command
    }

    /// Runs cargo in the staging tree and turns a failure into a finding carrying the end of its output.
    pub fn checked(&self, code: &'static str, args: &[&str]) -> Result<(), Finding> {
        let (ok, out, errors) = captured(self.command(env!("CARGO"), &self.staging()).args(args), "cargo")?;
        if ok { Ok(()) } else { Err(failed(code, "cargo", args, &(out + &errors))) }
    }

    /// Runs one pass: every copy at once, each made from the staging tree with its file times, checked to be built
    /// already, and mutated in place by a cargo-mutants of its own. The copies are removed afterwards.
    pub fn shards(&self, repo: &Repo, copies: &Copies) -> Result<Vec<MutantsRun>, Finding> {
        let dir = repo.mutants_out(copies.pass);
        // An earlier run's, which may have had more copies: none of its lists may pass for this run's.
        for old in fs::read_dir(&dir).into_iter().flatten().flatten() {
            if old.file_name().to_str().is_some_and(|name| name.starts_with("shard-")) {
                drop(fs::remove_dir_all(old.path()));
            }
        }
        let died = || Finding::new("mutants.run-failed", "", "a copy's run panicked", "run `just mutants` again");
        let runs: Vec<_> = thread::scope(|scope| {
            let outs = (0..copies.shards.len()).map(|index| (index, dir.join(format!("shard-{index}"))));
            let started: Vec<_> =
                outs.map(|(index, out)| scope.spawn(move || self.shard(&out, copies, index))).collect();
            started.into_iter().map(|run| run.join().unwrap_or_else(|_| Err(died()))).collect()
        });
        for index in 0..copies.shards.len() {
            drop(fs::remove_dir_all(self.copy(index)));
        }
        runs.into_iter().collect()
    }

    fn copy(&self, index: usize) -> PathBuf {
        self.home.join(format!("copy-{index}"))
    }

    /// One copy's run, writing what cargo-mutants writes to `out`.
    fn shard(&self, out: &Path, copies: &Copies, index: usize) -> Result<MutantsRun, Finding> {
        let (copy, shown) = (self.copy(index), out.display().to_string());
        // `-a` keeps the file times cargo judges a build fresh by; a reflink where the file system has them.
        let mut cp = self.command("cp", &self.home);
        let (ok, _, errors) = captured(cp.args(["-a", "--reflink=auto", "staging"]).arg(&copy), "cp")?;
        if !ok {
            return Err(failed("mutants.copy-failed", "cp", &["-a", "--reflink=auto"], &errors));
        }
        let (ok, built, errors) = captured(self.command(env!("CARGO"), &copy).args(copies.build), "cargo")?;
        if !ok {
            return Err(failed("mutants.copy-failed", "cargo", copies.build, &errors));
        }
        (copies.fresh)(&(built + &errors))?;
        let lists = out.join("mutants.out");
        if !copies.settled.is_empty() {
            fs::create_dir_all(&lists).map_err(|e| io(&shown, &e))?;
            fs::write(lists.join(SETTLED), copies.settled).map_err(|e| io(&shown, &e))?;
        }
        let args = copies.shards.get(index).map_or(&[][..], Vec::as_slice);
        let status = self
            .command(env!("CARGO"), &copy)
            .args(["mutants", "--baseline=skip", "--in-diff"])
            .arg(out.with_file_name("changes.diff"))
            .args(args)
            .arg("--output")
            .arg(out)
            .status()
            .map_err(|e| Finding::new("repo.no-tool", "cargo-mutants", e.to_string(), "run `just setup`"))?;
        // Only what this run wrote: the list seeded above is not among these.
        let outcomes = LISTS.map(|name| fs::read_to_string(lists.join(name)).unwrap_or_default());
        Ok(MutantsRun { success: status.success(), outcomes })
    }
}

impl Drop for Warm {
    fn drop(&mut self) {
        drop(fs::remove_dir_all(&self.home));
        // The janitor's input ends, and it finds nothing left to remove.
        drop(self.janitor.stdin.take());
        drop(self.janitor.wait());
    }
}

/// Puts `command` in process group `group`, or in a new one of its own for 0.
#[cfg(unix)]
fn grouped(command: &mut Command, group: i32) {
    std::os::unix::process::CommandExt::process_group(command, group);
}

/// Off Unix there are no process groups, and the mutants gate, the one caller, refuses to run before it gets here.
#[cfg(not(unix))]
const fn grouped(_command: &mut Command, _group: i32) {}

/// Runs `command`, returning whether it succeeded, its stdout and its stderr.
fn captured(command: &mut Command, program: &str) -> Result<(bool, String, String), Finding> {
    let output = command.output().map_err(|e| {
        Finding::new("repo.no-tool", program, e.to_string(), "run `just setup` to install the gate tools")
    })?;
    let text = |bytes: &[u8]| String::from_utf8_lossy(bytes).into_owned();
    Ok((output.status.success(), text(&output.stdout), text(&output.stderr)))
}

/// A program's failure as a finding carrying the end of its output.
fn failed(code: &'static str, program: &str, args: &[&str], output: &str) -> Finding {
    let mut tail: Vec<&str> = output.lines().rev().take(20).collect();
    tail.reverse();
    let shown = format!("{program} {} failed:\n{}", args.join(" "), tail.join("\n"));
    Finding::new(code, "", shown, format!("run {program} {} and fix what it reports", args.join(" ")))
}
