"""Runs a case in a child process with a scrubbed environment and fixed parallelism (L16).

The engine's work happens only in the child. A second-opinion engine is another `Engine` here whose worker speaks the
same protocol (README.md); none is wired up yet.
"""

import dataclasses
import json
import os
import pathlib
import subprocess
import sys
import tempfile

from .caseformat import validate_case
from .resultformat import ResultError, error_result, validate_result
from .canonical import dumps

DEFAULT_TIMEOUT = 600.0
_FIXED_PATH = "/usr/bin:/bin"


@dataclasses.dataclass(frozen=True)
class Engine:
    """An engine: who it is, and the worker that runs a case through it."""

    name: str
    version: str
    package: str
    package_version: str
    worker: pathlib.Path

    def identity(self):
        return {
            "engine": self.name,
            "engine_version": self.version,
            "package": self.package,
            "package_version": self.package_version,
        }

    @property
    def directory(self):
        return f"{self.name}-{self.version}"


OCCT = Engine(
    name="opencascade",
    version="8.0.1",
    package="cadquery-ocp",
    package_version="8.0.1.1.0",
    worker=pathlib.Path(__file__).resolve().parent / "occt_worker.py",
)
ENGINES = {"occt": OCCT}


def scrubbed_env(home):
    """The child's whole environment; nothing leaks in from the caller's (`CSF_*`, locale, library paths)."""
    return {"PATH": _FIXED_PATH, "HOME": str(home), "LC_ALL": "C", "PYTHONHASHSEED": "0"}


def default_results_root():
    """Where result files go: outside the repository, under the user's cache."""
    base = os.environ.get("XDG_CACHE_HOME") or (pathlib.Path.home() / ".cache")
    return pathlib.Path(base) / "occam-oracle" / "results"


def result_path(root, engine, threads, case_id):
    """`<root>/<engine>-<version>/threads-<n>/<case-id>.json`."""
    return pathlib.Path(root) / engine.directory / f"threads-{threads}" / f"{case_id}.json"


def write_result(result, path):
    path = pathlib.Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8", newline="\n") as file:
        file.write(dumps(result))
    return path


def _kind_of(case, op_id):
    for op in case["ops"]:
        if op["id"] == op_id:
            return op["op"]
    return None


def _in_progress(progress):
    try:
        return progress.read_text(encoding="utf-8") or None
    except OSError:
        return None


def run_case(case, engine=OCCT, threads=1, timeout=DEFAULT_TIMEOUT):
    """Runs `case` through `engine` in a scrubbed child process and returns its result document."""
    validate_case(case)
    if threads < 1:
        raise ValueError("threads must be at least 1")
    identity = engine.identity()

    def failure(code, op, message):
        return error_result(case["id"], identity, threads, code, op, _kind_of(case, op), message)

    with tempfile.TemporaryDirectory(prefix="occam-oracle-") as scratch:
        scratch = pathlib.Path(scratch)
        home = scratch / "home"
        home.mkdir()
        case_file, result_file, progress = scratch / "case.json", scratch / "result.json", scratch / "progress"
        case_file.write_text(json.dumps(case), encoding="utf-8")
        progress.write_text("", encoding="utf-8")
        command = [
            sys.executable,
            "-B",  # no bytecode written anywhere
            "-s",  # no user site-packages
            "-P",  # the worker's directory is not added to the import path
            str(engine.worker),
            "--case",
            str(case_file),
            "--result",
            str(result_file),
            "--progress",
            str(progress),
            "--threads",
            str(threads),
        ]
        try:
            completed = subprocess.run(
                command,
                env=scrubbed_env(home),
                cwd=home,
                stdin=subprocess.DEVNULL,
                capture_output=True,
                text=True,
                timeout=timeout,
                check=False,
            )
        except subprocess.TimeoutExpired:
            return failure("worker_timeout", _in_progress(progress), f"the worker ran past the {timeout:g} second timeout")
        if completed.returncode != 0 or not result_file.is_file():
            print(f"oracle: {case['id']}: worker failed:\n{completed.stderr}", file=sys.stderr)
            how = f"signal {-completed.returncode}" if completed.returncode < 0 else f"status {completed.returncode}"
            return failure("worker_crashed", _in_progress(progress), f"the worker exited with {how}")
        try:
            result = json.loads(result_file.read_text(encoding="utf-8"))
            validate_result(result, case["id"])
        except (OSError, ValueError, ResultError) as error:
            print(f"oracle: {case['id']}: malformed worker result: {error}", file=sys.stderr)
            return failure("worker_malformed", None, "the worker wrote a result that does not match the result format")
        if result["oracle"] != identity:
            return failure("identity_mismatch", None, "the worker reported a different engine than the pin")
        if result["threads"] != threads:
            return failure("worker_malformed", None, "the worker reported a different thread count than it was given")
        return result
