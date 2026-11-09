"""The measurement loop: whole operations only, measured after the timer, and nothing empty is ever counted (L32)."""

import math
import unittest

from occam_baseline import timing


class FakeClock:
    """A clock that advances by one millisecond each time it is read, so elapsed time counts readings."""

    def __init__(self):
        self.now = 0

    def __call__(self):
        self.now += 1_000_000  # one millisecond in nanoseconds
        return self.now


class Ops:
    """An operation list runner that records what was run in which order, and returns volumes."""

    def __init__(self, log, volume):
        self.log = log
        self.volume = volume

    def input(self, op):
        self.log.append(("input", op["id"]))

    def prepare(self, op):
        self.log.append(("prepare", op["id"]))
        return lambda: self.log.append(("operate", op["id"])) or op["id"]

    def finish(self, op, outcome):
        self.log.append(("finish", op["id"], outcome))

    def measure(self):
        self.log.append(("measure",))
        return self.volume


OPS = [
    {"op": "box", "id": "a"},
    {"op": "box", "id": "b"},
    {"op": "fuse", "id": "c"},
]


class TestWholeOperationTiming(unittest.TestCase):
    def test_only_the_operation_is_inside_the_timer(self):
        log = []
        clock = FakeClock()
        elapsed = timing.execute(OPS, {"fuse"}, Ops(log, 1.0), clock=clock)
        # Exactly one clock pair around the one timed operation: two readings, so 1 ms.
        self.assertEqual(elapsed, 1_000_000)
        self.assertEqual(log, [("input", "a"), ("input", "b"), ("prepare", "c"), ("operate", "c"), ("finish", "c", "c")])

    def test_a_chain_of_timed_operations_is_summed(self):
        ops = [{"op": "box", "id": "a"}, {"op": "cut", "id": "b"}, {"op": "box", "id": "x"}, {"op": "cut", "id": "c"}]
        elapsed = timing.execute(ops, {"cut"}, Ops([], 1.0), clock=FakeClock())
        self.assertEqual(elapsed, 2_000_000)

    def test_warm_up_is_excluded_and_repetitions_recorded(self):
        calls = []

        def make_runner():
            calls.append("runner")
            return Ops([], 5.0)

        record = timing.run_repetitions(OPS, {"fuse"}, make_runner, "volume", repetitions=3, warmups=1, clock=FakeClock())
        self.assertEqual(len(calls), 4)  # one warm-up, three timed; inputs rebuilt for each
        self.assertEqual(record["warmups"], 1)
        self.assertEqual(record["repetitions"], 3)
        self.assertEqual(len(record["ms"]), 3)
        self.assertEqual(record["measured"], [5.0, 5.0, 5.0])

    def test_measurement_runs_after_the_timer(self):
        log = []
        clock_at_measure = []

        class CheckingOps(Ops):
            def measure(self):
                clock_at_measure.append(clock.now)
                return super().measure()

        clock = FakeClock()
        timing.run_repetitions(OPS, {"fuse"}, lambda: CheckingOps(log, 2.0), "volume", repetitions=1, warmups=0, clock=clock)
        # The clock was read twice around the operation, before the measurement began.
        self.assertEqual(clock_at_measure, [2_000_000])


class TestRepetitionCount(unittest.TestCase):
    def test_a_fast_case_repeats_up_to_the_budget(self):
        self.assertEqual(timing.repetitions_for(4.0, budget_ms=100.0, least=5, most=30), 25)

    def test_the_bounds_hold(self):
        self.assertEqual(timing.repetitions_for(0.1, budget_ms=100.0, least=5, most=30), 30)
        self.assertEqual(timing.repetitions_for(500.0, budget_ms=100.0, least=5, most=30), 5)


class TestEmptyResultsAreRefused(unittest.TestCase):
    def test_volume_of_a_result_with_nothing_in_it_is_refused(self):
        for empty in (0.0, -1.0, float("nan"), None):
            with self.assertRaises(timing.RefusedResult):
                timing.check_measured("volume", empty)

    def test_a_mesh_with_no_triangles_is_refused(self):
        for empty in (0, -3, None, True):
            with self.assertRaises(timing.RefusedResult):
                timing.check_measured("triangles", empty)

    def test_a_real_measurement_passes(self):
        timing.check_measured("volume", 12.5)
        timing.check_measured("triangles", 64)

    def test_a_fake_op_with_an_empty_result_is_refused_before_it_is_counted(self):
        with self.assertRaises(timing.RefusedResult):
            timing.run_repetitions(OPS, {"fuse"}, lambda: Ops([], 0.0), "volume", repetitions=2, warmups=1, clock=FakeClock())

    def test_a_nan_volume_is_refused(self):
        with self.assertRaises(timing.RefusedResult):
            timing.run_repetitions(OPS, {"fuse"}, lambda: Ops([], math.nan), "volume", repetitions=1, warmups=0, clock=FakeClock())


if __name__ == "__main__":
    unittest.main()
