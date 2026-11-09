#!/usr/bin/env python3
"""Whole-operation timings of OCCT on the generated corpus. `just baseline run`, `just baseline node`, `just baseline report`."""

import argparse
import getpass
import pathlib
import socket
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

from occam_baseline import cases as case_groups  # noqa: E402
from occam_baseline import runner  # noqa: E402


def _thread_list(text):
    return [int(item) for item in text.split(",") if item]


def _options(args):
    return {"budget_ms": args.budget_ms, "least": args.least, "most": args.most, "timeout": args.timeout}


def _add_common(parser):
    parser.add_argument("--groups", default=",".join(case_groups.GROUPS), help="comma separated: booleans, fillets, tessellation")
    parser.add_argument("--corpus", default=str(runner.DEFAULT_CORPUS), type=pathlib.Path)
    parser.add_argument("--out", default=None, type=pathlib.Path, help="directory for the measurement documents")
    parser.add_argument("--limit", default=None, type=int, help="at most this many cases of each corpus family")
    parser.add_argument("--only", default=None, help="only case ids matching this regular expression")
    parser.add_argument("--tag", default=None, help="a suffix for the output file names, for a run split into parts by --only")
    parser.add_argument("--budget-ms", default=100.0, type=float, help="repeat a case until its timed runs fill this much time")
    parser.add_argument("--least", default=5, type=int, help="fewest timed repetitions of a case")
    parser.add_argument("--most", default=30, type=int, help="most timed repetitions of a case")
    parser.add_argument("--timeout", default=120.0, type=float, help="seconds one case may take before it is abandoned")


def command_run(args):
    groups = [group for group in args.groups.split(",") if group]
    selected, out_of_scope = runner.select_cases(args.corpus, groups, args.limit, args.only)
    out = args.out or runner.default_results_dir()
    for group in groups:
        threads = [1] if group == "fillets" else args.threads
        for count in threads:
            runner.run_native(group, count, selected[group], out, _options(args), out_of_scope, args.tag)
    return 0


def command_node(args):
    groups = [group for group in args.groups.split(",") if group]
    selected, out_of_scope = runner.select_cases(args.corpus, groups, args.limit, args.only)
    out = args.out or runner.default_results_dir()
    ocjs_dir = args.ocjs_dir or runner.default_ocjs_dir()
    for group in groups:
        runner.run_node(group, selected[group], out, ocjs_dir, _options(args), out_of_scope, args.tag)
    return 0


def _forbidden():
    host = socket.gethostname()
    words = {host, host.split(".")[0], getpass.getuser(), str(pathlib.Path.home())}
    return tuple(word for word in words if word and word not in {"localhost"})


def command_report(args):
    import json

    from occam_baseline import report

    results = args.results or runner.default_results_dir()
    documents = [json.loads(path.read_text(encoding="utf-8")) for path in sorted(results.glob("*.json"))]
    notes = args.notes.read_text(encoding="utf-8") if args.notes else None
    text = report.render(documents, forbidden=_forbidden() + tuple(args.forbid), notes=notes)
    if args.output:
        args.output.write_text(text, encoding="utf-8")
    else:
        sys.stdout.write(text)
    return 0


def _parser():
    parser = argparse.ArgumentParser(description="Whole-operation timings of OCCT.")
    commands = parser.add_subparsers(dest="command", required=True)
    run = commands.add_parser("run", help="time the groups through the native wheel")
    _add_common(run)
    run.add_argument("--threads", default=[1, 2, 4, 6], type=_thread_list, help="comma separated thread counts")
    run.set_defaults(func=command_run)
    node = commands.add_parser("node", help="the same through the WebAssembly build under Node")
    _add_common(node)
    node.add_argument("--ocjs-dir", default=None, type=pathlib.Path, help="where opencascade.js is installed")
    node.set_defaults(func=command_node)
    report = commands.add_parser("report", help="render the Markdown report from the measurement documents")
    report.add_argument("--results", default=None, type=pathlib.Path)
    report.add_argument("--output", default=None, type=pathlib.Path)
    report.add_argument("--notes", default=None, type=pathlib.Path, help="Markdown to append as the report's run notes")
    report.add_argument("--forbid", action="append", default=[], help="another word that must not appear in the text")
    report.set_defaults(func=command_report)
    return parser


def main(argv=None):
    args = _parser().parse_args(argv)
    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
