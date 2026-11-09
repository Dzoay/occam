"""The second opinions: pinned kernels other than OCCT, built and run outside the workspace.

Each is cloned at a pinned commit under `~/.cache/occam-oracle/second/` and driven through its documented interface by a
small Rust driver built there. Nothing of theirs enters the repository. Their source is not to be read: the drivers are
written from their READMEs and their published API docs (docs.rs).
"""

import dataclasses
import os
import pathlib
import shutil
import subprocess

HERE = pathlib.Path(__file__).resolve().parent
TEMPLATES = HERE.parent / "second"


@dataclasses.dataclass(frozen=True)
class Pin:
    url: str
    commit: str
    version: str  # the release this commit is tagged as, which docs.rs documents
    crate_directory: str
    dependency: str


PINS = {
    "arris": Pin(
        url="https://github.com/arris-labs/arris.git",
        commit="f6860e379ba054888b087b8e141f47cd1e631eab",
        version="0.10.0",
        crate_directory="crates/arris",
        dependency='arris = {{ path = "{path}" }}',
    ),
}


def root():
    base = os.environ.get("XDG_CACHE_HOME") or (pathlib.Path.home() / ".cache")
    return pathlib.Path(base) / "occam-oracle" / "second"


def driver_binary(name):
    return root() / "bin" / f"{name}-driver"


def engines(engine_type):
    """An `Engine` for each pin, reporting itself missing until `build-second` has built its driver."""
    found = {}
    for name, pin in PINS.items():
        binary = driver_binary(name)
        found[name] = engine_type(
            name=name,
            version=pin.version,
            package=pin.url,
            package_version=pin.commit,
            worker=HERE / "second_worker.py",
            worker_args=("--driver", str(binary), "--engine-name", name, "--engine-version", pin.version, "--package", pin.url, "--package-version", pin.commit),
            missing=None if binary.is_file() else f"its driver is not built: run `just oracle build-second {name}`",
        )
    return found


def _clean_environment():
    """A minimal environment that sends no account, credential or user configuration to the fetch and the build."""
    kept = {key: os.environ[key] for key in ("HOME", "PATH", "RUSTUP_HOME", "CARGO_HOME", "RUSTUP_TOOLCHAIN") if key in os.environ}
    return {
        **kept,
        "GIT_TERMINAL_PROMPT": "0",
        "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_CONFIG_GLOBAL": os.devnull,
        "LC_ALL": "C",
        "RUSTFLAGS": "--cap-lints allow",  # a compiler warning quotes the kernel's source, which is not to be read
    }


def _run(command, cwd=None):
    subprocess.run(command, cwd=cwd, env=_clean_environment(), check=True)


def _checkout(name, pin):
    """The pinned commit, cloned anonymously over https, verified by its hash."""
    checkout = root() / name
    if not checkout.is_dir():
        checkout.parent.mkdir(parents=True, exist_ok=True)
        _run(["git", "init", "--quiet", str(checkout)])
        _run(["git", "remote", "add", "origin", pin.url], cwd=checkout)
    _run(["git", "fetch", "--quiet", "--depth", "1", "origin", pin.commit], cwd=checkout)
    _run(["git", "checkout", "--quiet", "--detach", "--force", pin.commit], cwd=checkout)
    head = subprocess.run(["git", "rev-parse", "HEAD"], cwd=checkout, env=_clean_environment(), check=True, capture_output=True, text=True).stdout.strip()
    if head != pin.commit:
        raise RuntimeError(f"{name}: checked out {head}, pinned {pin.commit}")
    return checkout


def build(name):
    """Fetches the pinned commit and compiles its driver, all under the cache directory."""
    pin = PINS[name]
    checkout = _checkout(name, pin)
    project = root() / f"{name}-driver"
    (project / "src").mkdir(parents=True, exist_ok=True)
    manifest = (TEMPLATES / "Cargo.toml.in").read_text(encoding="utf-8")
    dependency = pin.dependency.format(path=checkout / pin.crate_directory)
    (project / "Cargo.toml").write_text(manifest.replace("@NAME@", name).replace("@DEPENDENCY@", dependency), encoding="utf-8")
    shutil.copyfile(TEMPLATES / f"{name}_driver.rs.in", project / "src" / "main.rs")
    shutil.copyfile(checkout / "Cargo.lock", project / "Cargo.lock")  # the kernel's own locked versions
    _run(["cargo", "build", "--release", "--quiet"], cwd=project)
    binary = driver_binary(name)
    binary.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(project / "target" / "release" / f"{name}-driver", binary)
    binary.chmod(0o755)
    return binary
