"""Runs the timing workers, one (group, thread count) at a time, and writes one measurement document for each.

A worker is restarted after a crash or a hang, and only the case it was on is lost. Everything here but the worker is
standard library; the worker itself is reused from the oracle harness (see timed_worker.py).
"""

import json
import os
import pathlib
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import time

from . import cases as case_groups
from . import machine

_REPO = pathlib.Path(__file__).resolve().parents[3]
ORACLE = _REPO / "tools" / "oracle"
if str(ORACLE) not in sys.path:
    sys.path.insert(0, str(ORACLE))

from occam_oracle.caseformat import case_files, load_case  # noqa: E402
from occam_oracle.runner import scrubbed_env  # noqa: E402

WORKER = pathlib.Path(__file__).resolve().parent / "timed_worker.py"
NODE_WORKER = pathlib.Path(__file__).resolve().parents[1] / "ocjs_worker.mjs"
DEFAULT_CORPUS = _REPO / "tools" / "corpus" / "generated"
_POLL_SECONDS = 0.2


def default_results_dir():
    base = os.environ.get("XDG_CACHE_HOME") or (pathlib.Path.home() / ".cache")
    return pathlib.Path(base) / "occam-baseline" / "results"


def default_ocjs_dir():
    if os.environ.get("OCCAM_BASELINE_OCJS"):
        return pathlib.Path(os.environ["OCCAM_BASELINE_OCJS"])
    base = os.environ.get("XDG_CACHE_HOME") or (pathlib.Path.home() / ".cache")
    return pathlib.Path(base) / "occam-baseline" / "ocjs"


def select_cases(corpus, groups, limit=None, only=None):
    """`{run_group: [{"case": ..., "chosen": ...}]}` for the groups asked for, and how many cases were out of scope.

    `only` is a regular expression on case ids, for splitting a long run into shorter locked stretches.
    """
    selected, out_of_scope = {group: [] for group in groups}, 0
    pattern = re.compile(only) if only else None
    for path in case_files([pathlib.Path(corpus)]):
        case = load_case(path)
        if pattern and not pattern.search(case["id"]):
            continue
        chosen = case_groups.classify(case)
        if chosen is None:
            out_of_scope += 1
            continue
        if chosen.run_group in selected:
            selected[chosen.run_group].append(
                {
                    "case": case,
                    "chosen": {"group": chosen.group, "family": chosen.family, "timed": sorted(chosen.timed), "kind": chosen.kind},
                }
            )
    if limit is not None:
        for group, entries in selected.items():
            selected[group] = _limited(entries, limit)
    return selected, out_of_scope


def _limited(entries, per_family):
    """At most `per_family` cases of each corpus family, keeping the sorted order, so a subset stays representative."""
    counts = {}
    kept = []
    for entry in entries:
        family = entry["chosen"]["family"]
        counts[family] = counts.get(family, 0) + 1
        if counts[family] <= per_family:
            kept.append(entry)
    return kept


def _read_progress(path):
    """(case id, seconds since it started) or (None, 0) when the worker is between cases."""
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except OSError:
        return None, 0.0
    if len(lines) < 2:
        return None, 0.0
    try:
        return lines[0], time.time() - float(lines[1])
    except ValueError:
        return None, 0.0


def _kill(worker):
    try:
        os.killpg(worker.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    worker.wait()


def _drain(results_path):
    records = {}
    if results_path.is_file():
        for line in results_path.read_text(encoding="utf-8").splitlines():
            if line.strip():
                record = json.loads(line)
                records[record["id"]] = record
    return records


def _lost(entry, threads, status, message):
    """The record for a case the worker never reported."""
    chosen = entry["chosen"]
    return {
        "id": entry["case"]["id"],
        "family": chosen["family"],
        "group": chosen["group"],
        "kind": chosen["kind"],
        "threads": threads,
        "status": status,
        "message": message,
        "ms": [],
        "measured": [],
    }


def run_entries(entries, threads, start_worker, case_timeout):
    """Runs every entry to a record, restarting the worker after a crash or hang. `start_worker(entries, scratch)` returns the Popen."""
    remaining = list(entries)
    records = {}
    while remaining:
        with tempfile.TemporaryDirectory(prefix="occam-baseline-") as scratch:
            scratch = pathlib.Path(scratch)
            worker, results, progress = start_worker(remaining, scratch)
            hung = None
            while worker.poll() is None:
                in_progress, running_for = _read_progress(progress)
                if in_progress is not None and running_for > case_timeout:
                    hung = in_progress
                    _kill(worker)
                    break
                time.sleep(_POLL_SECONDS)
            worker.wait()
            stderr = (scratch / "stderr").read_text(encoding="utf-8", errors="replace")[:2000]
            finished = _drain(results)
            records.update(finished)
            lost_id, _ = _read_progress(progress)
            lost_id = hung or lost_id
            if lost_id is None and any(entry["case"]["id"] not in records for entry in remaining):
                # The worker died before reporting any progress, or finished without reporting everything.
                lost_id = next(entry["case"]["id"] for entry in remaining if entry["case"]["id"] not in records)
                print(f"baseline: the worker stopped without reporting {lost_id}: {stderr}", file=sys.stderr)
            if lost_id is not None and lost_id not in records:
                entry = next(e for e in remaining if e["case"]["id"] == lost_id)
                records[lost_id] = _lost(
                    entry,
                    threads,
                    "timeout" if hung else "crashed",
                    f"the worker ran past {case_timeout:g} s" if hung else f"the worker exited with status {worker.returncode}",
                )
            remaining = [entry for entry in remaining if entry["case"]["id"] not in records]
    return [records[entry["case"]["id"]] for entry in entries]


def _native_starter(threads, cores, options):
    def start(entries, scratch):
        cases_file, results, progress = scratch / "cases.json", scratch / "results.jsonl", scratch / "progress"
        cases_file.write_text(json.dumps(entries), encoding="utf-8")
        progress.write_text("", encoding="utf-8")
        home = scratch / "home"
        home.mkdir()
        stderr = (scratch / "stderr").open("w", encoding="utf-8")
        command = [
            sys.executable,
            "-B",
            "-s",
            "-P",
            str(WORKER),
            "--cases",
            str(cases_file),
            "--results",
            str(results),
            "--progress",
            str(progress),
            "--threads",
            str(threads),
            "--cores",
            ",".join(str(cpu) for cpu in cores),
            "--budget-ms",
            str(options["budget_ms"]),
            "--least",
            str(options["least"]),
            "--most",
            str(options["most"]),
        ]
        worker = subprocess.Popen(  # pylint: disable=consider-using-with
            command,
            env=scrubbed_env(home),
            cwd=home,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=stderr,
            start_new_session=True,
        )
        stderr.close()
        return worker, results, progress

    return start


def _node_starter(ocjs_dir, options):
    node = shutil.which("node")
    if node is None:
        raise RuntimeError("baseline: node is not installed on this machine")

    def start(entries, scratch):
        cases_file, results, progress = scratch / "cases.json", scratch / "results.jsonl", scratch / "progress"
        cases_file.write_text(json.dumps(entries), encoding="utf-8")
        progress.write_text("", encoding="utf-8")
        home = scratch / "home"
        home.mkdir()
        stderr = (scratch / "stderr").open("w", encoding="utf-8")
        command = [
            node,
            str(NODE_WORKER),
            "--ocjs-dir",
            str(ocjs_dir),
            "--cases",
            str(cases_file),
            "--results",
            str(results),
            "--progress",
            str(progress),
            "--budget-ms",
            str(options["budget_ms"]),
            "--least",
            str(options["least"]),
            "--most",
            str(options["most"]),
        ]
        worker = subprocess.Popen(  # pylint: disable=consider-using-with
            command,
            env={"PATH": str(pathlib.Path(node).parent), "HOME": str(home), "LC_ALL": "C", "NODE_OPTIONS": ""},
            cwd=home,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=stderr,
            start_new_session=True,
        )
        stderr.close()
        return worker, results, progress

    return start


def _suffix(tag):
    return f"-{tag}" if tag else ""


def _write_document(document, out, name):
    out.mkdir(parents=True, exist_ok=True)
    path = out / f"{name}.json"
    temporary = path.with_suffix(".json.tmp")
    temporary.write_text(json.dumps(document, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    temporary.replace(path)
    return path


def run_native(group, threads, entries, out, options, out_of_scope, tag=None):
    cores = machine.physical_cpus()[:threads]
    if len(cores) < threads:
        raise RuntimeError(f"this machine has {len(cores)} physical cores, fewer than the {threads} threads asked for")
    print(f"baseline: {group}, {len(entries)} cases, {threads} thread(s), native OCCT", flush=True)
    load_before = machine.load_average()
    start = time.monotonic()
    records = run_entries(entries, threads, _native_starter(threads, cores, options), options["timeout"])
    document = {
        "schema": "occam-baseline/1",
        "engine": "occt-native",
        "group": group,
        "threads": threads,
        "cores": len(cores),
        "machine": machine.record(),
        "load_before": load_before,
        "load_after": machine.load_average(),
        "seconds": round(time.monotonic() - start, 1),
        "scope": {"selected": len(entries), "out_of_scope": out_of_scope},
        "options": {key: options[key] for key in ("budget_ms", "least", "most", "timeout")},
        "cases": records,
    }
    document["tag"] = tag
    path = _write_document(document, out, f"native-{group}-t{threads}{_suffix(tag)}")
    print(f"baseline: wrote {path.name}, {document['seconds']} s, load before {load_before} after {document['load_after']}", flush=True)
    return path


def run_node(group, entries, out, ocjs_dir, options, out_of_scope, tag=None):
    print(f"baseline: {group}, {len(entries)} cases, Node WebAssembly", flush=True)
    load_before = machine.load_average()
    start = time.monotonic()
    records = run_entries(entries, 1, _node_starter(ocjs_dir, options), options["timeout"])
    detail = _ocjs_detail(ocjs_dir)
    document = {
        "schema": "occam-baseline/1",
        "engine": "ocjs-node",
        "engine_detail": detail,
        "group": group,
        "threads": 1,
        "machine": machine.record(),
        "load_before": load_before,
        "load_after": machine.load_average(),
        "seconds": round(time.monotonic() - start, 1),
        "scope": {"selected": len(entries), "out_of_scope": out_of_scope},
        "options": {key: options[key] for key in ("budget_ms", "least", "most", "timeout")},
        "cases": records,
    }
    document["tag"] = tag
    path = _write_document(document, out, f"ocjs-node-{group}{_suffix(tag)}")
    print(f"baseline: wrote {path.name}, {document['seconds']} s, load before {load_before} after {document['load_after']}", flush=True)
    return path


def _ocjs_detail(ocjs_dir):
    """The package's version, and the OCCT version the build embeds, from the product strings in its WebAssembly binary."""
    detail = {"ocjs_version": "unknown", "occt_version": "unknown"}
    package = pathlib.Path(ocjs_dir) / "node_modules" / "opencascade.js"
    try:
        detail["ocjs_version"] = json.loads((package / "package.json").read_text(encoding="utf-8"))["version"]
    except (OSError, ValueError, KeyError):
        pass
    try:
        binary = (package / "dist" / "opencascade.wasm.wasm").read_bytes()
    except OSError:
        return detail
    found = re.search(rb"Open CASCADE Technology (\d+(?:\.\d+)+)", binary)
    if found:
        detail["occt_version"] = found.group(1).decode("ascii")
    return detail
