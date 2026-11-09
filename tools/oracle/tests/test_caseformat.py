"""The case format: what the corpus step will generate in bulk."""

import unittest

from occam_oracle.caseformat import CaseError, validate_case

from .support import box, box_case, boolean, case


class ValidCaseTests(unittest.TestCase):
    def test_a_well_formed_case_is_accepted(self):
        validate_case(box_case(expect={"volume": {"value": 6000.0, "rel_tol": 1e-6}}))

    def test_every_operation_kind_is_accepted(self):
        ops = [
            box("a", (1, 1, 1)),
            {"op": "cylinder", "id": "b", "radius": 1, "height": 2, "origin": [0, 0, 0], "axis": [0, 0, 1]},
            {"op": "sphere", "id": "c", "radius": 1, "origin": [0, 0, 0]},
            {"op": "cone", "id": "d", "radius1": 2, "radius2": 1, "height": 3, "origin": [0, 0, 0], "axis": [0, 0, 1]},
            {"op": "torus", "id": "e", "major_radius": 3, "minor_radius": 1, "origin": [0, 0, 0], "axis": [0, 0, 1]},
            {"op": "translate", "id": "f", "target": "a", "vector": [1, 0, 0]},
            {"op": "rotate", "id": "g", "target": "a", "axis_origin": [0, 0, 0], "axis": [0, 0, 1], "angle_degrees": 45},
            boolean("fuse", "h", "a", "f"),
            boolean("cut", "i", "a", "c"),
            boolean("common", "j", "a", "c"),
            {"op": "fillet", "id": "k", "target": "a", "radius": 0.1, "edges": "all"},
            {"op": "fillet", "id": "l", "target": "a", "radius": 0.1, "edges": {"in_box": {"min": [0, 0, 0], "max": [1, 1, 0]}}},
            {"op": "tessellate", "id": "m", "target": "a", "linear_deflection": 0.1, "angular_deflection": 0.5},
            {"op": "tessellate", "id": "n", "target": "a", "linear_deflection": 0.1, "angular_deflection": 0.5, "relative": True},
            {"op": "remove_face", "id": "o", "target": "a", "in_box": {"min": [0, 0, 0], "max": [1, 1, 0]}},
        ]
        validate_case(case("all-ops", ops, "k"))


class InvalidCaseTests(unittest.TestCase):
    def assert_rejected(self, document, fragment):
        with self.assertRaises(CaseError) as caught:
            validate_case(document)
        self.assertIn(fragment, str(caught.exception))

    def test_wrong_schema(self):
        document = box_case()
        document["schema"] = "occam-oracle-case/2"
        self.assert_rejected(document, "schema")

    def test_missing_id(self):
        document = box_case()
        del document["id"]
        self.assert_rejected(document, "id")

    def test_id_must_be_a_file_name(self):
        document = box_case("../escape")
        self.assert_rejected(document, "id")

    def test_missing_origin_or_licence(self):
        for key in ("origin", "licence"):
            document = box_case()
            del document[key]
            self.assert_rejected(document, key)

    def test_copyleft_licence_is_refused(self):
        for licence in ("LGPL-2.1", "GPL-3.0-only", "AGPL-3.0", "MPL-2.0"):
            document = box_case()
            document["licence"] = licence
            self.assert_rejected(document, "licence")

    def test_unknown_operation_names_the_op(self):
        document = case("bad-op", [{"op": "extrude", "id": "a"}], "a")
        self.assert_rejected(document, "'a'")

    def test_missing_field_names_the_op(self):
        self.assert_rejected(case("bad", [{"op": "box", "id": "a", "size": [1, 1, 1]}], "a"), "'a'")

    def test_unknown_field_is_refused(self):
        document = case("bad", [dict(box("a", (1, 1, 1)), colour="red")], "a")
        self.assert_rejected(document, "colour")

    def test_duplicate_ids_are_refused(self):
        self.assert_rejected(case("bad", [box("a", (1, 1, 1)), box("a", (2, 2, 2))], "a"), "'a'")

    def test_forward_reference_is_refused(self):
        document = case("bad", [boolean("cut", "c", "a", "b"), box("a", (1, 1, 1)), box("b", (1, 1, 1))], "c")
        self.assert_rejected(document, "'a'")

    def test_result_must_name_a_shape(self):
        ops = [box("a", (1, 1, 1)), {"op": "tessellate", "id": "m", "target": "a", "linear_deflection": 0.1, "angular_deflection": 0.5}]
        self.assert_rejected(case("bad", ops, "m"), "result")

    def test_tessellate_produces_no_shape_to_refer_to(self):
        ops = [
            box("a", (1, 1, 1)),
            {"op": "tessellate", "id": "m", "target": "a", "linear_deflection": 0.1, "angular_deflection": 0.5},
            boolean("cut", "c", "a", "m"),
        ]
        self.assert_rejected(case("bad", ops, "c"), "'m'")

    def test_non_positive_lengths_are_refused(self):
        self.assert_rejected(case("bad", [box("a", (1, 0, 1))], "a"), "'a'")
        self.assert_rejected(
            case("bad", [{"op": "sphere", "id": "a", "radius": -1, "origin": [0, 0, 0]}], "a"), "'a'"
        )

    def test_zero_axis_is_refused(self):
        document = case("bad", [{"op": "cylinder", "id": "a", "radius": 1, "height": 1, "origin": [0, 0, 0], "axis": [0, 0, 0]}], "a")
        self.assert_rejected(document, "'a'")

    def test_booleans_are_not_numbers(self):
        self.assert_rejected(case("bad", [box("a", (True, 1, 1))], "a"), "'a'")

    def test_expect_needs_a_tolerance_for_measured_numbers(self):
        self.assert_rejected(box_case(expect={"volume": {"value": 1.0}}), "volume")
        self.assert_rejected(box_case(expect={"bbox": {"min": [0, 0, 0], "max": [1, 1, 1]}}), "bbox")

    def test_counts_cannot_be_expected(self):
        """L12: topology is never an expectation against an oracle."""
        self.assert_rejected(box_case(expect={"faces": {"value": 6}}), "faces")

    def test_unknown_expect_is_refused(self):
        self.assert_rejected(box_case(expect={"mass": {"value": 1.0, "rel_tol": 1e-6}}), "mass")

    def test_error_expectation_stands_alone(self):
        self.assert_rejected(box_case(expect={"error": {"op": "a"}, "volume": {"value": 1.0, "rel_tol": 1e-6}}), "error")

    def test_volume_identity_field_is_checked(self):
        document = box_case()
        document["volume_identity"] = {"group": "g", "role": "minus"}
        self.assert_rejected(document, "role")


if __name__ == "__main__":
    unittest.main()
