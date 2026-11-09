"""The measurement loop: whole operations only, measured after the timer, and nothing empty is ever counted (L32).

Inputs are built outside the timer, the timer wraps the entire operation, and the result is measured after the timer
stops, so a deferred computation cannot look fast. A repetition whose measured result is empty is refused, never counted.
Standard library only, so it can be tested without the engine.
"""

import gc
import math
import time


class RefusedResult(Exception):
    """An operation returned without having computed anything measurable."""


def check_measured(kind, value):
    """Refuses a measured result with nothing in it. `kind` is "volume" or "triangles"."""
    if kind == "triangles":
        good = isinstance(value, int) and not isinstance(value, bool) and value > 0
    elif kind == "volume":
        good = isinstance(value, (int, float)) and not isinstance(value, bool) and math.isfinite(value) and value > 0
    else:
        raise ValueError(f"unknown measurement {kind!r}")
    if not good:
        raise RefusedResult(f"the measured {kind} is {value!r}, so the operation computed nothing measurable")


def execute(ops, timed_kinds, runner, clock=time.perf_counter_ns):
    """Runs one repetition of the operation list and returns the nanoseconds spent inside the timed operations.

    `runner.input(op)` builds an input, outside the timer. `runner.prepare(op)` does any argument preparation, also
    outside the timer, and returns the zero-argument operation itself; only that call is timed. `runner.finish(op,
    outcome)` keeps the result. Several timed operations in one list are summed.
    """
    elapsed = 0
    for op in ops:
        if op["op"] not in timed_kinds:
            runner.input(op)
            continue
        operation = runner.prepare(op)
        start = clock()
        outcome = operation()
        end = clock()
        runner.finish(op, outcome)
        elapsed += end - start
    return elapsed


def run_repetitions(ops, timed_kinds, make_runner, kind, repetitions, warmups=1, clock=time.perf_counter_ns):
    """Warm-up runs, then timed repetitions, each on freshly built inputs, each measured after its timer.

    Fresh inputs each time because a repeat on the same shapes could reuse work the first run left on them. Python's
    collector is off during the timed part and run between repetitions, as `timeit` does. The result of every repetition
    is kept, so a repetition that differs from the others is a finding rather than noise.
    """
    ms, measured = [], []
    for index in range(warmups + repetitions):
        runner = make_runner()
        gc.collect()
        gc.disable()
        try:
            elapsed = execute(ops, timed_kinds, runner, clock=clock)
        finally:
            gc.enable()
        value = runner.measure()
        check_measured(kind, value)
        if index >= warmups:
            ms.append(elapsed / 1e6)
            measured.append(value)
    return {"ms": ms, "measured": measured, "repetitions": repetitions, "warmups": warmups}


def repetitions_for(first_ms, budget_ms=100.0, least=5, most=30):
    """How many timed repetitions a case gets: enough to fill the budget, within bounds, from its first timed run."""
    if first_ms <= 0:
        return most
    return max(least, min(most, math.ceil(budget_ms / first_ms)))


def measure_case(ops, timed_kinds, make_runner, kind, budget_ms=100.0, least=5, most=30, clock=time.perf_counter_ns):
    """One warm-up, then repetitions sized from the first timed run (`repetitions_for`)."""
    record = run_repetitions(ops, timed_kinds, make_runner, kind, repetitions=1, warmups=1, clock=clock)
    extra = repetitions_for(record["ms"][0], budget_ms, least, most) - 1
    if extra > 0:
        more = run_repetitions(ops, timed_kinds, make_runner, kind, repetitions=extra, warmups=0, clock=clock)
        record["ms"].extend(more["ms"])
        record["measured"].extend(more["measured"])
    record["repetitions"] = len(record["ms"])
    return record
