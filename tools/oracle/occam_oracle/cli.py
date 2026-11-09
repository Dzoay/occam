"""The oracle harness's command line: `run`, `check` and `register`."""

import argparse
import json
import pathlib
import sys

from .canonical import dumps
from .caseformat import CaseError, case_files, load_case
from .compare import check_case, check_volume_identity
from .register import check_register
from .resultformat import ResultError, validate_result
from .runner import ENGINES, default_results_root, result_path, run_case, write_result

DEFAULT_REGISTER = pathlib.Path(__file__).resolve().parents[1] / "divergences.json"


def _load_cases(paths):
    files = case_files(paths)
    if not files:
        raise CaseError("no case files found")
    cases = [load_case(file) for file in files]
    identifiers = [case["id"] for case in cases]
    duplicates = sorted({name for name in identifiers if identifiers.count(name) > 1})
    if duplicates:
        raise CaseError(f"duplicate case ids: {', '.join(duplicates)}")
    return cases


def _engine_arguments(parser):
    parser.add_argument("--engine", default="occt", choices=sorted(ENGINES))
    parser.add_argument("--threads", type=int, default=1, help="parallelism fixed for the run (default 1)")


def command_run(args):
    engine = ENGINES[args.engine]
    root = pathlib.Path(args.out) if args.out else default_results_root()
    cases = _load_cases(args.paths)
    for case in cases:
        result = run_case(case, engine=engine, threads=args.threads, timeout=args.timeout)
        path = write_result(result, result_path(root, engine, args.threads, case["id"]))
        if result["status"] == "ok":
            print(f"ok    {case['id']} volume={result['shape']['volume']!r} -> {path}")
        else:
            error = result["error"]
            print(f"error {case['id']} op={error['op']} {error['code']}: {error['message']} -> {path}")
    return 0


def _read_result(case, engine, threads, root):
    path = result_path(root, engine, threads, case["id"])
    if not path.is_file():
        return None, {"id": case["id"], "pass": False, "measures": [{"measure": "status", "pass": False, "detail": f"no result file at {path}"}], "invariants": []}
    try:
        result = validate_result(json.loads(path.read_text(encoding="utf-8")), case["id"])
    except (ValueError, ResultError) as error:
        return None, {"id": case["id"], "pass": False, "measures": [{"measure": "status", "pass": False, "detail": f"bad result file {path}: {error}"}], "invariants": []}
    return result, None


def _print_report(report):
    state = "PASS" if report["pass"] else "FAIL"
    print(f"{state}  {report['id']}")
    for entry in report["measures"]:
        if not entry["pass"]:
            print(f"      measure {entry['measure']}: expected {entry.get('expected')!r}, got {entry.get('actual')!r}")
    for entry in report["invariants"]:
        if not entry["pass"]:
            print(f"      invariant {entry['invariant']}: {entry['detail']}")


def command_check(args):
    engine = ENGINES[args.engine]
    root = pathlib.Path(args.results) if args.results else default_results_root()
    cases = _load_cases(args.paths)
    reports = []
    groups = {}
    for case in cases:
        result, missing = _read_result(case, engine, args.threads, root)
        if result is None:
            reports.append(missing)
            continue
        reports.append(check_case(case, result))
        identity = case.get("volume_identity")
        if identity:
            groups.setdefault(identity["group"], {})[identity["role"]] = result
    identities = [check_volume_identity(group, members) for group, members in sorted(groups.items())]
    passed = all(report["pass"] for report in reports) and all(item["pass"] is not False for item in identities)
    if args.json:
        print(dumps({"cases": reports, "volume_identity": identities, "pass": passed}), end="")
    else:
        for report in reports:
            _print_report(report)
        for item in identities:
            state = "SKIP" if item["pass"] is None else ("PASS" if item["pass"] else "FAIL")
            print(f"{state}  volume identity {item['group']}: {item['detail']}")
        print(f"{sum(report['pass'] for report in reports)} of {len(reports)} cases pass")
    return 0 if passed else 1


def command_register(args):
    path = pathlib.Path(args.path) if args.path else DEFAULT_REGISTER
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        print(f"{path}: {error}", file=sys.stderr)
        return 1
    errors = check_register(document)
    for problem in errors:
        print(f"{path}: {problem}", file=sys.stderr)
    if errors:
        return 1
    print(f"{path}: well-formed, {len(document['entries'])} entries")
    return 0


def _parser():
    parser = argparse.ArgumentParser(prog="oracle", description="Run cases through a pinned engine and check the results.")
    commands = parser.add_subparsers(dest="command", required=True)
    run = commands.add_parser("run", help="run cases and write one result file per case")
    run.add_argument("paths", nargs="+", help="case files, or directories searched for *.json")
    run.add_argument("--out", help="root of the result tree (default: under ~/.cache/occam-oracle/results)")
    run.add_argument("--timeout", type=float, default=600.0, help="seconds a case may run")
    _engine_arguments(run)
    run.set_defaults(handler=command_run)
    check = commands.add_parser("check", help="compare result files with the cases' expect and apply the invariants")
    check.add_argument("paths", nargs="+", help="case files, or directories searched for *.json")
    check.add_argument("--results", help="root of the result tree (default: the `run` default)")
    check.add_argument("--json", action="store_true", help="print the full report as JSON")
    _engine_arguments(check)
    check.set_defaults(handler=command_check)
    register = commands.add_parser("register", help="check that the divergence register is well-formed")
    register.add_argument("path", nargs="?", help="the register file (default: the committed divergences.json)")
    register.set_defaults(handler=command_register)
    return parser


def main(argv=None):
    args = _parser().parse_args(argv)
    try:
        return args.handler(args)
    except CaseError as error:
        print(f"oracle: {error}", file=sys.stderr)
        return 2
