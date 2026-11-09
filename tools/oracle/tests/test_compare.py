"""The comparison and the invariants: geometric, with tolerances, and needing no engine. Nothing here runs OCCT."""

import unittest

from occam_oracle.compare import check_case, check_volume_identity

from .support import box_case, cut_case, error_result, ok_result


def measure(report, name):
    found = [m for m in report["measures"] if m["measure"] == name]
    if len(found) != 1:
        raise AssertionError(f"expected one {name} measure in {report}")
    return found[0]


def invariant(report, name):
    found = [i for i in report["invariants"] if i["invariant"] == name]
    if len(found) != 1:
        raise AssertionError(f"expected one {name} invariant in {report}")
    return found[0]


class ExpectTests(unittest.TestCase):
    def test_wrong_volume_is_rejected(self):
        report = check_case(box_case(expect={"volume": {"value": 2.0, "rel_tol": 1e-6}}), ok_result())
        self.assertFalse(report["pass"])
        self.assertFalse(measure(report, "volume")["pass"])

    def test_volume_within_tolerance_passes(self):
        report = check_case(box_case(expect={"volume": {"value": 1.0 + 5e-7, "rel_tol": 1e-6}}), ok_result())
        self.assertTrue(report["pass"], report)
        self.assertTrue(measure(report, "volume")["pass"])

    def test_tolerance_is_relative_or_absolute_whichever_is_wider(self):
        for expect, verdict in [
            ({"value": 1.001, "abs_tol": 1e-2}, True),
            ({"value": 1.001, "abs_tol": 1e-4}, False),
            ({"value": 1.0, "rel_tol": 1e-9, "abs_tol": 1e-2}, True),
        ]:
            report = check_case(box_case(expect={"volume": expect}), ok_result(volume=1.0))
            self.assertEqual(measure(report, "volume")["pass"], verdict, expect)

    def test_area_and_bbox_are_compared(self):
        report = check_case(
            box_case(
                expect={
                    "area": {"value": 7.0, "abs_tol": 1e-6},
                    "bbox": {"min": [0, 0, 0], "max": [1, 1, 1.5], "abs_tol": 1e-6},
                }
            ),
            ok_result(),
        )
        self.assertFalse(measure(report, "area")["pass"])
        self.assertFalse(measure(report, "bbox")["pass"])
        self.assertFalse(report["pass"])

    def test_bbox_within_tolerance_passes(self):
        report = check_case(
            box_case(expect={"bbox": {"min": [0, 0, -1e-8], "max": [1, 1, 1], "abs_tol": 1e-6}}), ok_result()
        )
        self.assertTrue(measure(report, "bbox")["pass"], report)

    def test_euler_is_exact(self):
        self.assertTrue(measure(check_case(box_case(expect={"euler": {"value": 2}}), ok_result()), "euler")["pass"])
        report = check_case(box_case(expect={"euler": {"value": 0}}), ok_result())
        self.assertFalse(measure(report, "euler")["pass"])
        self.assertFalse(report["pass"])

    def test_valid_closed_and_watertight_are_compared(self):
        mesh = {"m": {"target": "a", "triangles": 12, "vertices": 8, "watertight": False}}
        report = check_case(
            box_case(expect={"watertight": {"value": True}}), ok_result(tessellations=mesh)
        )
        self.assertFalse(measure(report, "watertight")["pass"])
        report = check_case(box_case(expect={"closed": {"value": False}}), ok_result())
        self.assertFalse(measure(report, "closed")["pass"])

    def test_watertight_expected_but_no_tessellation_fails(self):
        report = check_case(box_case(expect={"watertight": {"value": True}}), ok_result())
        self.assertFalse(measure(report, "watertight")["pass"])

    def test_no_expectations_still_applies_the_invariants(self):
        report = check_case(box_case(), ok_result())
        self.assertTrue(report["pass"], report)
        self.assertEqual(report["measures"], [])
        self.assertTrue(invariant(report, "valid")["pass"])

    def test_error_result_fails_every_expectation(self):
        report = check_case(box_case(expect={"volume": {"value": 1.0, "rel_tol": 1e-6}}), error_result())
        self.assertFalse(report["pass"])
        self.assertFalse(measure(report, "status")["pass"])

    def test_expected_error_names_the_op(self):
        expected = box_case(expect={"error": {"op": "f"}})
        self.assertTrue(check_case(expected, error_result(op="f"))["pass"])
        self.assertFalse(check_case(expected, error_result(op="g"))["pass"])
        self.assertFalse(check_case(expected, ok_result())["pass"])


class InvariantTests(unittest.TestCase):
    def test_invalid_shape_fails(self):
        report = check_case(box_case(), ok_result(valid=False))
        self.assertFalse(report["pass"])
        self.assertFalse(invariant(report, "valid")["pass"])

    def test_open_shape_fails(self):
        report = check_case(box_case(), ok_result(closed=False))
        self.assertFalse(invariant(report, "closed")["pass"])

    def test_nonpositive_volume_fails(self):
        for volume in (0.0, -1.0):
            report = check_case(box_case(), ok_result(volume=volume))
            self.assertFalse(invariant(report, "volume_positive")["pass"], volume)
            self.assertFalse(report["pass"])

    def test_expected_zero_volume_waives_volume_positive(self):
        report = check_case(
            box_case(expect={"volume": {"value": 0.0, "abs_tol": 1e-9}}),
            ok_result(volume=0.0),
        )
        self.assertTrue(report["pass"], report)

    def test_expected_invalid_waives_the_shape_invariants(self):
        report = check_case(box_case(expect={"valid": {"value": False}}), ok_result(valid=False, closed=False, volume=-1.0))
        self.assertTrue(report["pass"], report)

    def test_euler_must_match_the_counts(self):
        report = check_case(box_case(), ok_result(euler=4))
        self.assertFalse(invariant(report, "euler_counts")["pass"])
        self.assertFalse(report["pass"])

    def test_result_for_another_case_is_rejected(self):
        report = check_case(box_case("box"), ok_result(case_id="other"))
        self.assertFalse(report["pass"])


class VolumeIdentityTests(unittest.TestCase):
    def members(self, a, b, union, intersection):
        return {
            "a": ok_result("a", volume=a),
            "b": ok_result("b", volume=b),
            "union": ok_result("u", volume=union),
            "intersection": ok_result("i", volume=intersection),
        }

    def test_identity_holds_when_volumes_balance(self):
        report = check_volume_identity("g", self.members(10.0, 6.0, 13.0, 3.0))
        self.assertTrue(report["pass"], report)

    def test_identity_fails_when_volumes_do_not_balance(self):
        report = check_volume_identity("g", self.members(10.0, 6.0, 13.5, 3.0))
        self.assertFalse(report["pass"], report)

    def test_a_failed_member_fails_the_identity(self):
        members = self.members(10.0, 6.0, 13.0, 3.0)
        members["union"] = error_result("u")
        self.assertFalse(check_volume_identity("g", members)["pass"])

    def test_incomplete_group_is_skipped_not_failed(self):
        members = self.members(10.0, 6.0, 13.0, 3.0)
        del members["union"]
        report = check_volume_identity("g", members)
        self.assertIsNone(report["pass"])


class CompareGeometricallyTests(unittest.TestCase):
    def test_counts_are_not_compared(self):
        """L12: a different number of faces with the same geometry is not a disagreement."""
        report = check_case(
            cut_case(),
            ok_result(case_id="box-cut-cylinder", counts={"vertices": 99, "edges": 99 - 2 + 2, "degenerate_edges": 0, "wires": 3, "faces": 1, "shells": 1, "solids": 1}, euler=99 - (99) + 2 * 1 - 3),
        )
        self.assertEqual(report["measures"], [])


if __name__ == "__main__":
    unittest.main()
