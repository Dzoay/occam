"""The schema check is itself a claim: it must refuse what the case format refuses."""

import copy
import pathlib
import sys
import unittest

import generate
import schema

ORACLE = pathlib.Path(generate.__file__).resolve().parents[1] / "oracle"


def good():
    return {
        "schema": "occam-oracle-case/1",
        "id": "box-0001",
        "origin": "tools/corpus/generate.py family prim-box seed 1",
        "licence": "MIT OR Apache-2.0",
        "ops": [
            {"op": "box", "id": "a", "size": [1, 2, 3], "origin": [0, 0, 0]},
            {"op": "cylinder", "id": "b", "radius": 1, "height": 2, "origin": [0, 0, 0], "axis": [0, 0, 1]},
            {"op": "cut", "id": "c", "target": "a", "tool": "b"},
            {"op": "tessellate", "id": "m", "target": "c", "linear_deflection": 0.1, "angular_deflection": 0.5},
        ],
        "result": "c",
        "expect": {
            "volume": {"value": 1.0, "rel_tol": 1e-6},
            "area": {"value": 1.0, "abs_tol": 1e-6},
            "bbox": {"min": [0, 0, 0], "max": [1, 1, 1], "abs_tol": 1e-6},
            "euler": {"value": 2},
            "watertight": {"value": True},
        },
    }


class SchemaTests(unittest.TestCase):
    def assert_rejected(self, mutate, fragment):
        document = good()
        mutate(document)
        with self.assertRaises(schema.CaseError) as caught:
            schema.validate_case(document)
        self.assertIn(fragment, str(caught.exception))

    def test_a_good_case_passes(self):
        schema.validate_case(good())

    def test_wrong_schema_string(self):
        self.assert_rejected(lambda d: d.update(schema="occam-oracle-case/2"), "schema")

    def test_missing_origin(self):
        self.assert_rejected(lambda d: d.pop("origin"), "origin")

    def test_missing_licence(self):
        self.assert_rejected(lambda d: d.pop("licence"), "licence")

    def test_copyleft_licence(self):
        self.assert_rejected(lambda d: d.update(licence="LGPL-2.1-only"), "licence")

    def test_id_must_be_a_file_name(self):
        self.assert_rejected(lambda d: d.update(id="../x"), "id")

    def test_unknown_op(self):
        self.assert_rejected(lambda d: d["ops"].append({"op": "extrude", "id": "x"}), "extrude")

    def test_missing_field(self):
        self.assert_rejected(lambda d: d["ops"][0].pop("size"), "size")

    def test_unknown_field(self):
        self.assert_rejected(lambda d: d["ops"][0].update(colour="red"), "colour")

    def test_duplicate_op_id(self):
        self.assert_rejected(lambda d: d["ops"][1].update(id="a"), "'a'")

    def test_forward_reference(self):
        self.assert_rejected(lambda d: d["ops"][2].update(tool="zz"), "'zz'")

    def test_result_must_be_a_shape(self):
        self.assert_rejected(lambda d: d.update(result="m"), "result")

    def test_non_positive_size(self):
        self.assert_rejected(lambda d: d["ops"][0].update(size=[1, 0, 1]), "'a'")

    def test_zero_axis(self):
        self.assert_rejected(lambda d: d["ops"][1].update(axis=[0, 0, 0]), "'b'")

    def test_expect_needs_a_tolerance(self):
        def strip(document):
            document["expect"]["volume"].pop("rel_tol")
        self.assert_rejected(strip, "volume")

    def test_expect_refuses_counts(self):
        self.assert_rejected(lambda d: d["expect"].update(faces={"value": 6}), "faces")

    def test_volume_identity_role_is_checked(self):
        self.assert_rejected(lambda d: d.update(volume_identity={"group": "g", "role": "minus"}), "role")

    def test_a_volume_identity_is_accepted(self):
        document = copy.deepcopy(good())
        document["volume_identity"] = {"group": "g", "role": "union"}
        schema.validate_case(document)


@unittest.skipUnless((ORACLE / "occam_oracle" / "caseformat.py").exists(), "the oracle harness is not on this branch")
class AgreesWithTheHarness(unittest.TestCase):
    """Once the harness is merged, its own validator judges every generated case too."""

    def test_every_generated_case_passes_the_harness_validator(self):
        sys.path.insert(0, str(ORACLE))
        try:
            from occam_oracle.caseformat import validate_case  # pylint: disable=import-outside-toplevel
        finally:
            sys.path.pop(0)
        for case in generate.generate_all():
            with self.subTest(case=case["id"]):
                validate_case(case)


if __name__ == "__main__":
    unittest.main()
