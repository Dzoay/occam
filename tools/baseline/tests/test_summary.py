"""Per-family statistics and thread scaling on fake timings."""

import unittest

from occam_baseline import summary


def case(case_id, per_rep, status="ok", kind="volume", measured=10.0, family="f", group="g"):
    return {
        "id": case_id,
        "family": family,
        "group": group,
        "status": status,
        "kind": kind,
        "ms": list(per_rep),
        "measured": [measured] * len(per_rep),
    }


class TestFamilySummary(unittest.TestCase):
    def test_odd_count(self):
        cases = [case("a", [10]), case("b", [30]), case("c", [20]), case("d", [40]), case("e", [50])]
        stats = summary.family_summary(cases)
        self.assertEqual(stats["count"], 5)
        self.assertEqual(stats["median"], 30.0)
        self.assertEqual(stats["min"], 10.0)
        self.assertEqual(stats["p90"], 46.0)
        self.assertEqual(stats["total"], 150.0)

    def test_even_count(self):
        stats = summary.family_summary([case("a", [1]), case("b", [2]), case("c", [3]), case("d", [4])])
        self.assertEqual(stats["median"], 2.5)
        self.assertAlmostEqual(stats["p90"], 3.7)

    def test_per_case_time_is_the_median_of_its_repetitions(self):
        self.assertEqual(summary.case_ms(case("a", [3, 100, 5])), 5.0)

    def test_failed_and_refused_cases_are_not_counted_but_are_listed(self):
        cases = [case("a", [10]), case("b", [20], status="error"), case("c", [30], status="refused")]
        stats = summary.family_summary(cases)
        self.assertEqual(stats["count"], 1)
        self.assertEqual(stats["median"], 10.0)
        self.assertEqual(stats["not_counted"], ["b", "c"])

    def test_an_empty_family_has_no_statistics(self):
        self.assertEqual(summary.family_summary([])["count"], 0)

    def test_spread_of_repetitions(self):
        self.assertEqual(summary.spread(case("a", [9, 10, 11])), 0.2)


class TestThreadScaling(unittest.TestCase):
    def documents(self, by_threads):
        """by_threads: {threads: [(id, ms, measured), ...]}."""
        return {threads: [case(i, [ms], measured=m) for i, ms, m in rows] for threads, rows in by_threads.items()}

    def test_speedup_against_the_one_thread_time(self):
        documents = self.documents({1: [("a", 100.0, 7.0)], 2: [("a", 50.0, 7.0)], 4: [("a", 25.0, 7.0)]})
        table = summary.thread_scaling(documents)
        self.assertEqual(table[1]["speedup_of_median"], 1.0)
        self.assertEqual(table[2]["speedup_of_median"], 2.0)
        self.assertEqual(table[4]["speedup_of_median"], 4.0)

    def test_identical_results_at_every_thread_count_is_no_finding(self):
        documents = self.documents({1: [("a", 100.0, 7.0)], 2: [("a", 50.0, 7.0)]})
        self.assertEqual(summary.thread_count_findings(documents), [])

    def test_a_result_that_differs_with_the_thread_count_is_a_finding(self):
        documents = self.documents({1: [("a", 100.0, 7.0)], 4: [("a", 25.0, 7.0000001)]})
        findings = summary.thread_count_findings(documents)
        self.assertEqual(len(findings), 1)
        self.assertEqual(findings[0]["case"], "a")
        self.assertIn("relative_spread", findings[0])
        self.assertGreater(findings[0]["relative_spread"], 0.0)
        self.assertEqual(findings[0]["threads"], [1, 4])

    def test_a_triangle_count_difference_is_a_finding_too(self):
        documents = {
            1: [case("m", [5.0], kind="triangles", measured=1000)],
            2: [case("m", [3.0], kind="triangles", measured=1002)],
        }
        findings = summary.thread_count_findings(documents)
        self.assertEqual(len(findings), 1)
        self.assertEqual(findings[0]["case"], "m")
        self.assertEqual(findings[0]["kind"], "triangles")

    def test_a_difference_between_repetitions_of_one_thread_count_is_a_finding(self):
        bad = case("r", [1.0, 1.0], measured=7.0)
        bad["measured"] = [7.0, 7.1]
        findings = summary.thread_count_findings({1: [bad]})
        self.assertEqual(len(findings), 1)
        self.assertEqual(findings[0]["case"], "r")

    def test_a_case_that_fails_the_same_way_at_every_count_is_consistent(self):
        failed = case("x", [], status="refused")
        self.assertEqual(summary.thread_count_findings({1: [failed], 2: [failed]}), [])

    def test_a_case_that_failed_at_one_thread_count_is_a_finding(self):
        ok = case("x", [5.0], measured=3.0)
        failed = case("x", [], status="error", measured=3.0)
        findings = summary.thread_count_findings({1: [ok], 2: [failed]})
        self.assertEqual(len(findings), 1)
        self.assertEqual(findings[0]["case"], "x")
        self.assertIn("failed", findings[0]["note"])


if __name__ == "__main__":
    unittest.main()
