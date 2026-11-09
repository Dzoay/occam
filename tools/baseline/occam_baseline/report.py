"""The report: machine, per-family tables, thread scaling, Node WebAssembly, and what occam has to beat.

Only the fields named here reach the text, and a final guard refuses anything naming this machine or its user. None of
the oracle's measured volumes or areas appear, because whether oracle outputs carry licence terms is open as O6.
"""

import re

from . import summary

MACHINE_FIELDS = (
    ("cpu", "CPU"),
    ("physical_cores", "Physical cores"),
    ("logical_cores", "Logical cores"),
    ("governor", "Governor"),
    ("kernel", "Kernel"),
    ("memory_gib", "Memory (GiB)"),
    ("wheel", "OCCT wheel"),
    ("python", "CPython"),
    ("node", "Node"),
)


class PrivateText(Exception):
    """The rendered report names this machine or its user."""


def check_private(text, forbidden):
    """Raises PrivateText if any forbidden word appears as a whole word in `text`."""
    for word in forbidden:
        if word and re.search(rf"(?<![\w]){re.escape(word)}(?![\w])", text, re.IGNORECASE):
            raise PrivateText("the report would name this machine or its user; refusing to write it")


def _f(value, digits=3):
    return "-" if value is None else f"{value:.{digits}f}"


def _row(cells):
    return "| " + " | ".join(str(cell) for cell in cells) + " |"


def _family_table(rows):
    """`rows` is [(label, summary), ...]."""
    lines = [
        _row(["Family", "Cases", "Median (ms)", "Min (ms)", "P90 (ms)", "Total (ms)", "Spread of repeats (median)"]),
        _row(["---"] * 7),
    ]
    for label, stats in rows:
        lines.append(
            _row(
                [
                    label,
                    stats["count"],
                    _f(stats["median"]),
                    _f(stats["min"]),
                    _f(stats["p90"]),
                    _f(stats["total"]),
                    _f(stats["median_spread"], 3),
                ]
            )
        )
    return lines


def _not_counted_lines(by_label):
    lines = []
    for label, stats in by_label:
        if stats["not_counted"]:
            lines.append(f"Not counted in {label}: {', '.join(f'`{case}`' for case in stats['not_counted'])}.")
    return lines


def _cases(documents, predicate):
    return [case for document in documents for case in document["cases"] if predicate(case)]


def _grouped(documents, key):
    groups = {}
    for case in _cases(documents, lambda _: True):
        groups.setdefault(case[key], []).append(case)
    return sorted(groups.items())


def _machine(documents):
    machine = documents[0]["machine"]
    lines = ["| | |", "| --- | --- |"]
    for field, label in MACHINE_FIELDS:
        if field in machine:
            lines.append(_row([label, machine[field]]))
    return lines


def _load_table(documents):
    lines = [_row(["Run", "Threads", "Load before (1/5/15 min)", "Load after (1/5/15 min)"]), _row(["---"] * 4)]
    for document in documents:
        lines.append(
            _row(
                [
                    f"{document['engine']} {document['group']}" + (f" ({document['tag']})" if document.get("tag") else ""),
                    document.get("threads", 1),
                    " ".join(f"{value:.2f}" for value in document["load_before"]),
                    " ".join(f"{value:.2f}" for value in document["load_after"]),
                ]
            )
        )
    return lines


def _expect_lines(documents):
    """The cases whose measured volume disagreed with their closed-form answer: named, never numbered (O6)."""
    wrong = sorted(case["id"] for document in documents for case in document["cases"] if case.get("expect_ok") is False)
    if not wrong:
        return ["Every case with a closed-form volume agreed with it within the case's tolerance."]
    return [f"Timed, but disagreed with their closed-form volume: {', '.join(f'`{case}`' for case in wrong)}."]


def _reps_note(documents):
    reps = [case.get("repetitions") for document in documents for case in document["cases"] if case.get("repetitions")]
    if not reps:
        return "No repetitions recorded."
    return f"Per case: 1 warm-up run excluded, then between {min(reps)} and {max(reps)} timed repetitions (median {int(sorted(reps)[len(reps) // 2])})."


def _scaling_rows(documents):
    """{table-label: {threads: cases}} from the native multi-thread documents."""
    groups = {}
    for document in documents:
        if document["engine"] != "occt-native":
            continue
        for case in document["cases"]:
            label = _scaling_label(case["group"])
            if label is None:
                continue
            groups.setdefault(label, {}).setdefault(document["threads"], []).append(case)
    return groups


def _scaling_label(group):
    if group.startswith("near-tangent-"):
        return "near-tangent booleans"
    if group.startswith("booleans-"):
        return "booleans"
    if group == "tessellation":
        return "tessellation"
    return None


def _thread_table(by_threads):
    table = summary.thread_scaling(by_threads)
    lines = [
        _row(["Threads", "Cases", "Median (ms)", "Total (ms)", "Speed-up of median", "Speed-up of total", "Median per-case speed-up", "CPU/wall"]),
        _row(["---"] * 8),
    ]
    for threads, row in sorted(table.items()):
        cpu = [case["cpu_wall_ratio"] for case in by_threads[threads] if case.get("cpu_wall_ratio")]
        ratio = sorted(cpu)[len(cpu) // 2] if cpu else None
        lines.append(
            _row(
                [
                    threads,
                    row["count"],
                    _f(row["median_ms"]),
                    _f(row["total_ms"]),
                    _f(row["speedup_of_median"], 2),
                    _f(row["speedup_of_total"], 2),
                    _f(row["median_case_speedup"], 2),
                    _f(ratio, 2),
                ]
            )
        )
    return lines


def _finding_lines(findings):
    if not findings:
        return ["No difference in the measured result between repetitions or thread counts."]
    lines = []
    for finding in findings:
        spread = finding["relative_spread"]
        detail = "" if spread is None else f" (relative spread {spread:.3e})"
        lines.append(f"- `{finding['case']}` ({finding['kind']}): {finding['note']}, at threads {finding['threads']}{detail}.")
    return lines


def _beat_lines(native_docs, node_docs):
    lines = []
    def stats_of(documents, group):
        return summary.family_summary([c for d in documents for c in d["cases"] if c["group"] == group])

    single = [d for d in native_docs if d.get("threads", 1) == 1]
    for group in sorted({c["group"] for d in single for c in d["cases"]}):
        native = stats_of(single, group)
        line = f"- **{group}**: median per-case {_f(native['median'])} ms, total {_f(native['total'])} ms, one thread, native. Goal, not a gate."
        if node_docs:
            wasm = stats_of(node_docs, group)
            if wasm["count"]:
                line += f" Node WebAssembly: median {_f(wasm['median'])} ms. Goal, not a gate."
        lines.append(line)
    return lines


def render(documents, forbidden=(), notes=None):
    """The whole report as Markdown, ending with any `notes`. Raises PrivateText if the text names the machine or its user."""
    if not documents:
        raise ValueError("no measurement documents")
    native = [d for d in documents if d["engine"] == "occt-native"]
    node = [d for d in documents if d["engine"] == "ocjs-node"]
    single = [d for d in native if d.get("threads", 1) == 1]
    timed_ids = {case["id"] for document in documents for case in document["cases"]}
    left_out = max((document.get("scope", {}).get("out_of_scope", 0) for document in native), default=0)
    parts = ["# OCCT whole-operation baseline", ""]
    parts += [
        "What occam has to beat, measured from outside on the generated corpus. Every target stated from these numbers is a",
        "**goal, not a gate** (L46). OCCT is run as a black box through a pinned wheel outside the workspace; none of its",
        "measured volumes or areas appear here (O6).",
        "",
        "## Method and limits",
        "",
        f"- Scope: {len(timed_ids)} distinct cases timed; {left_out} corpus cases left out because they have no boolean, fillet or",
        "  tessellate operation to time (primitives alone, and the single-solid roles of the near-tangent groups).",
        f"- {_reps_note(documents)}",
        "- Whole operations only (L32): the timer wraps the entire boolean, fillet or tessellate call, forced to completion;",
        "  the result (volume or triangle count) is measured after the timer stops, and a repetition whose result is empty is",
        "  refused. Inputs (primitives, transforms, earlier booleans that are inputs) are rebuilt fresh, outside the timer, for",
        "  every repetition. Times are wall clock (`perf_counter` for native runs, `performance.now()` under Node), in a scrubbed",
        "  environment, one process per run of a group at a thread count, restarted only after a crash or hang, so the cost of",
        "  importing the binding is not counted or repeated.",
        "- Each native timing process is pinned to as many physical cores as it has threads, one logical CPU per core, by CPU",
        "  affinity. The Node process is not pinned, and V8 may use helper threads for compilation and garbage collection.",
        "- Per-case time is the median of its timed repetitions. Family statistics are over the per-case times: count, median,",
        "  minimum, 90th percentile (linear interpolation) and total. \"Spread\" is the median across cases of (slowest - fastest",
        "  repetition) / median repetition.",
        "",
        "## The recorded machine",
        "",
    ]
    parts += _machine(documents) + [""]
    parts += _load_table(documents) + [""]
    parts += ["## Per-family tables (OCCT native, one thread)", ""]
    groups = _grouped(single, "group")
    rows = [(group, summary.family_summary(cases)) for group, cases in groups]
    parts += _family_table(rows) + [""]
    parts += _not_counted_lines(rows) + _expect_lines(single) + [""]
    fine = [(family, summary.family_summary(cases)) for family, cases in _grouped(single, "family")]
    parts += ["By corpus family:", ""] + _family_table(fine) + [""]
    parts += ["## Thread scaling (OCCT native)", ""]
    parts += _thread_section(native)
    fillets = [case for document in single for case in document["cases"] if case["group"] == "fillets"]
    if fillets:
        parts += ["### fillets (one thread only)", "", "Determinism across repetitions:", ""]
        parts += _finding_lines(summary.thread_count_findings({1: fillets})) + [""]
    parts += ["## Node WebAssembly (`opencascade.js`)", ""]
    parts += _node_section(node)
    parts += ["## What occam has to beat", ""]
    parts += _beat_lines(native, node) + [""]
    if notes:
        parts += ["## Run notes", "", notes.strip(), ""]
    text = "\n".join(parts).rstrip() + "\n"
    check_private(text, forbidden)
    return text


def _thread_section(native):
    parts = []
    scaling = _scaling_rows(native)
    if not scaling:
        return ["No multi-thread runs recorded.", ""]
    parts += [
        "OCCT's own parallel switches, through the oracle harness's `fix_parallelism`. At one thread the booleans' global",
        "parallel mode (`BOPAlgo_Options.SetParallelMode_s`) and the mesher's default (`BRepMesh_IncrementalMesh.SetParallelDefault_s`)",
        "are off. Above one thread both are on, the mesher's `isInParallel` argument is true, and the size of the default pool is",
        "fixed once, before any work, with `OSD_ThreadPool.DefaultPool_s(N)`; every record states the size the pool reported. Each",
        "process is pinned to N physical cores. Fillets are left out here: the fillet API has no parallel switch.",
        "",
        "\"CPU/wall\" is the median across cases of process CPU time over wall time inside the timed calls. A value near 1 means",
        "the switch did not turn into parallel work for those cases, whatever the thread count says. Speed-ups are over the",
        "cases that succeeded at every thread count. They include any change of clock speed with the number of busy cores.",
        "",
    ]
    for label, by_threads in sorted(scaling.items()):
        parts += [f"### {label}", ""]
        parts += _thread_table(by_threads) + [""]
        findings = summary.thread_count_findings(by_threads)
        parts += ["Determinism across repetitions and thread counts:", ""] + _finding_lines(findings) + [""]
    return parts


def _node_section(node):
    if not node:
        return ["No Node WebAssembly runs recorded.", ""]
    detail = node[0].get("engine_detail", {})
    parts = [
        "These are WebAssembly numbers under Node's V8, **not a browser**: the host has no Chromium. The build embeds its own",
        f"OCCT ({detail.get('occt_version', 'unknown')}), which differs from the native wheel's 8.0.1, so the two are not strictly the same code.",
        f"`opencascade.js` {detail.get('ocjs_version', 'unknown')}.",
        "",
    ]
    groups = _grouped(node, "group")
    rows = [(group, summary.family_summary(cases)) for group, cases in groups]
    parts += _family_table(rows) + [""]
    parts += _not_counted_lines(rows)
    parts += _expect_lines(node) + [""]
    fine = [(family, summary.family_summary(cases)) for family, cases in _grouped(node, "family")]
    parts += ["By corpus family:", ""] + _family_table(fine) + [""]
    all_cases = [case for document in node for case in document["cases"]]
    parts += ["Determinism across repetitions:", ""] + _finding_lines(summary.thread_count_findings({1: all_cases})) + [""]
    return parts
