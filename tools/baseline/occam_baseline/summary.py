"""Per-family statistics and thread scaling, from the per-case records. Standard library only."""

import statistics


def case_ms(case):
    """A case's time: the median of its timed repetitions, in milliseconds."""
    return float(statistics.median(case["ms"]))


def spread(case):
    """How much a case's repetitions vary: (slowest - fastest) / median."""
    middle = case_ms(case)
    return 0.0 if middle == 0 else (max(case["ms"]) - min(case["ms"])) / middle


def percentile(values, fraction):
    """Linear interpolation between order statistics (the common "R-7" definition)."""
    ordered = sorted(values)
    position = (len(ordered) - 1) * fraction
    below = int(position)
    above = min(below + 1, len(ordered) - 1)
    return ordered[below] + (ordered[above] - ordered[below]) * (position - below)


def family_summary(cases):
    """Count, median, minimum, 90th percentile and total of the per-case milliseconds, over the cases that succeeded.

    A case that failed or was refused is not counted, and is named in `not_counted`.
    """
    counted = [case for case in cases if case["status"] == "ok"]
    summary = {
        "count": len(counted),
        "median": None,
        "min": None,
        "p90": None,
        "total": 0.0,
        "median_spread": None,
        "not_counted": sorted(case["id"] for case in cases if case["status"] != "ok"),
    }
    if not counted:
        return summary
    times = [case_ms(case) for case in counted]
    summary.update(
        median=statistics.median(times),
        min=min(times),
        p90=percentile(times, 0.90),
        total=sum(times),
        median_spread=statistics.median(spread(case) for case in counted),
    )
    return summary


def _common_ok_ids(documents):
    """Case ids that succeeded at every thread count present."""
    sets = [{case["id"] for case in cases if case["status"] == "ok"} for cases in documents.values()]
    return set.intersection(*sets) if sets else set()


def thread_scaling(documents):
    """`documents` is `{threads: cases}` for one group. Speed-ups are against the one-thread run, over common cases only."""
    common = _common_ok_ids(documents)
    table = {}
    for threads, cases in sorted(documents.items()):
        times = sorted(case_ms(case) for case in cases if case["id"] in common)
        table[threads] = {
            "count": len(times),
            "median_ms": statistics.median(times) if times else None,
            "total_ms": sum(times),
        }
    base = table.get(1)
    for threads, row in table.items():
        row["speedup_of_median"] = None
        row["speedup_of_total"] = None
        if base and base["median_ms"] and row["median_ms"]:
            row["speedup_of_median"] = base["median_ms"] / row["median_ms"]
            row["speedup_of_total"] = base["total_ms"] / row["total_ms"]
        ratios = _case_ratios(documents, threads, common)
        row["median_case_speedup"] = statistics.median(ratios) if ratios else None
    return table


def _case_ratios(documents, threads, common):
    """Per-case speed-ups against one thread, over the common cases."""
    if 1 not in documents or threads not in documents:
        return []
    base = {case["id"]: case_ms(case) for case in documents[1] if case["id"] in common}
    now = {case["id"]: case_ms(case) for case in documents[threads] if case["id"] in common}
    return [base[case] / now[case] for case in sorted(common) if now[case] > 0]


def _relative_spread(values):
    largest = max(abs(value) for value in values)
    return 0.0 if largest == 0 else (max(values) - min(values)) / largest


def thread_count_findings(documents):
    """Every disagreement in a measured result: between repetitions, between thread counts, or a failure at some counts.

    A case that fails the same way at every count is consistent, so it is not one of these; the family tables name it.

    A difference here is a finding, never noise. Only the relative spread is reported, never the values, because the
    oracle's measured volumes are not published (O6).
    """
    findings = []
    by_case = {}
    for threads, cases in sorted(documents.items()):
        for case in cases:
            by_case.setdefault(case["id"], {})[threads] = case
    for case_id, per_threads in sorted(by_case.items()):
        ok = {threads: case for threads, case in per_threads.items() if case["status"] == "ok"}
        if ok and len(ok) != len(per_threads):
            failed = sorted(threads for threads, case in per_threads.items() if case["status"] != "ok")
            findings.append(
                {
                    "case": case_id,
                    "kind": per_threads[failed[0]].get("kind", "volume"),
                    "threads": failed,
                    "relative_spread": None,
                    "note": f"failed at thread counts {failed}, ok at {sorted(ok)}",
                }
            )
        for threads, case in sorted(ok.items()):
            if len(set(case["measured"])) > 1:
                findings.append(
                    {
                        "case": case_id,
                        "kind": case["kind"],
                        "threads": [threads],
                        "relative_spread": _relative_spread(case["measured"]),
                        "note": f"the result differs between repetitions at {threads} threads",
                    }
                )
        firsts = {threads: case["measured"][0] for threads, case in sorted(ok.items()) if case["measured"]}
        if len(set(firsts.values())) > 1:
            kind = next(iter(ok.values()))["kind"]
            findings.append(
                {
                    "case": case_id,
                    "kind": kind,
                    "threads": sorted(firsts),
                    "relative_spread": _relative_spread(list(firsts.values())),
                    "note": "the result differs with the thread count",
                }
            )
    return findings
