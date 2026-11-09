"""The arris importer: only recipes that pass the per-case gate become cases, each attributed. Synthetic fixtures only."""

import json
import math
import pathlib
import tempfile
import unittest

import generate
import import_arris
import schema

COMMIT = "1a7bc1e660b9d20a677ac06a317de83200ffd639"
IMPORTED = pathlib.Path(import_arris.__file__).resolve().parent / "imported"
PI = math.pi


def plate_hole(**overrides):
    """A plate 40x30x10 with a through-hole r 4: V = 12000 - 160*pi."""
    fixture = {
        "description": "a plate with a through hole",
        "steps": [
            {"name": "plate", "op": "box", "min": [0, 0, 0], "max": [40, 30, 10]},
            {"name": "hole", "op": "cylinder", "base": [20, 15, -1], "axis": [0, 0, 1], "radius": 4, "height": 12},
            {"name": "result", "op": "cut", "target": "plate", "tool": "hole"},
        ],
        "result": "result",
        "analytic": {"volume": "12000 - pi * 16 * 10", "area": "3800 - pi * 32 + pi * 8 * 10", "counts": {"shells": 1}, "genus": 1},
    }
    fixture.update(overrides)
    return fixture


class GateTests(unittest.TestCase):
    def assert_excluded(self, fixture, reason):
        with self.assertRaises(import_arris.Excluded) as caught:
            import_arris.convert_fixture(fixture, "boolean/x", COMMIT)
        self.assertIn(reason, str(caught.exception))

    def test_a_clean_recipe_converts(self):
        cases = import_arris.convert_fixture(plate_hole(), "boolean/through-hole", COMMIT)
        self.assertEqual(len(cases), 1)
        case = cases[0]
        self.assertEqual(case["id"], "arris-boolean-through-hole")
        self.assertEqual(case["licence"], "MIT OR Apache-2.0")
        self.assertIn(COMMIT, case["origin"])
        self.assertIn("tests/fixtures/boolean/through-hole/fixture.json", case["origin"])
        self.assertIn("Copyright (c) 2026 Sergei Sergienko", case["origin"])
        schema.validate_case(case)

    def test_the_answer_is_computed_from_the_expression(self):
        case = import_arris.convert_fixture(plate_hole(), "boolean/x", COMMIT)[0]
        self.assertAlmostEqual(case["expect"]["volume"]["value"], 12000 - 160 * PI, places=6)
        self.assertAlmostEqual(case["expect"]["area"]["value"], 3800 + 48 * PI, places=6)
        self.assertEqual(case["expect"]["euler"], {"value": 0})

    def test_ops_are_translated_to_the_case_format(self):
        case = import_arris.convert_fixture(plate_hole(), "boolean/x", COMMIT)[0]
        self.assertEqual(
            case["ops"],
            [
                {"op": "box", "id": "plate", "size": [40, 30, 10], "origin": [0, 0, 0]},
                {"op": "cylinder", "id": "hole", "radius": 4, "height": 12, "origin": [20, 15, -1], "axis": [0, 0, 1]},
                {"op": "cut", "id": "result", "target": "plate", "tool": "hole"},
            ],
        )

    def test_expressions_and_variants_use_the_params(self):
        fixture = plate_hole(
            params={"r": 4, "t": 10},
            variants={"wide": {"r": 5}},
            steps=[
                {"name": "plate", "op": "box", "min": [0, 0, 0], "max": [40, 30, "t"]},
                {"name": "hole", "op": "cylinder", "base": [20, 15, -1], "axis": [0, 0, 1], "radius": "r", "height": "t + 2"},
                {"name": "result", "op": "cut", "target": "plate", "tool": "hole"},
            ],
            analytic={"volume": "40 * 30 * t - pi * r ^ 2 * t", "genus": 1, "counts": {"shells": 1}},
        )
        cases = {case["id"]: case for case in import_arris.convert_fixture(fixture, "boolean/x", COMMIT)}
        self.assertEqual(set(cases), {"arris-boolean-x", "arris-boolean-x-wide"})
        self.assertAlmostEqual(cases["arris-boolean-x-wide"]["expect"]["volume"]["value"], 12000 - 250 * PI, places=6)
        self.assertEqual(cases["arris-boolean-x-wide"]["ops"][1]["radius"], 5)

    def test_many_at_once_operands_fold_in_order(self):
        fixture = plate_hole(
            steps=[
                {"name": "a", "op": "box", "min": [0, 0, 0], "max": [2, 2, 2]},
                {"name": "b", "op": "box", "min": [1, 0, 0], "max": [3, 2, 2]},
                {"name": "c", "op": "box", "min": [2, 0, 0], "max": [4, 2, 2]},
                {"name": "result", "op": "fuse", "bodies": ["a", "b", "c"]},
            ],
            analytic={"volume": "16"},
        )
        case = import_arris.convert_fixture(fixture, "boolean/x", COMMIT)[0]
        fuses = [op for op in case["ops"] if op["op"] == "fuse"]
        self.assertEqual([(op["target"], op["tool"]) for op in fuses], [("a", "b"), (fuses[0]["id"], "c")])
        self.assertEqual(case["result"], fuses[-1]["id"])
        self.assertEqual(fuses[-1]["id"], "result")

    def test_a_cut_with_several_tools_folds_in_order(self):
        fixture = plate_hole(
            steps=[
                {"name": "a", "op": "box", "min": [0, 0, 0], "max": [6, 2, 2]},
                {"name": "t1", "op": "box", "min": [1, 0, 0], "max": [2, 2, 2]},
                {"name": "t2", "op": "box", "min": [3, 0, 0], "max": [4, 2, 2]},
                {"name": "result", "op": "cut", "target": "a", "tools": ["t1", "t2"]},
            ],
            analytic={"volume": "16"},
        )
        case = import_arris.convert_fixture(fixture, "boolean/x", COMMIT)[0]
        cuts = [op for op in case["ops"] if op["op"] == "cut"]
        self.assertEqual([(op["target"], op["tool"]) for op in cuts], [("a", "t1"), (cuts[0]["id"], "t2")])
        self.assertEqual(case["result"], "result")

    def test_a_third_party_part_is_out(self):
        self.assert_excluded(plate_hole(source="https://www.nist.gov/x"), "third-party")

    def test_a_per_fixture_licence_field_is_out(self):
        self.assert_excluded(plate_hole(licence="the project's own"), "third-party")

    def test_a_step_file_operand_is_out(self):
        self.assert_excluded(plate_hole(steps=[{"name": "result", "op": "step", "file": "a.step"}]), "step")

    def test_an_unsupported_operation_is_out(self):
        self.assert_excluded(plate_hole(steps=[{"name": "result", "op": "revolve"}]), "revolve")

    def test_a_bare_number_answer_is_out_because_it_may_be_a_measured_value(self):
        self.assert_excluded(plate_hole(analytic={"volume": 123.4}), "bare")

    def test_a_case_without_a_closed_form_volume_is_out(self):
        self.assert_excluded(plate_hole(analytic={"area": "1"}), "volume")

    def test_a_description_naming_another_kernel_is_out(self):
        for word in ("Open CASCADE's fillet", "the OCCT result", "the oracle's reading"):
            with self.subTest(word=word):
                self.assert_excluded(plate_hole(description=f"tuned against {word}"), "another kernel")

    def test_a_downstream_consumer_probe_is_out_for_unclear_origin(self):
        self.assert_excluded(plate_hole(description="the consumer's hollow box in its own units"), "unclear origin")

    def test_a_recipe_derived_from_a_consumer_probe_is_out(self):
        self.assert_excluded(plate_hole(description="parallel-cylinders-cut's operands, fused"), "unclear origin")

    def test_an_expected_error_is_out(self):
        self.assert_excluded(plate_hole(analytic={"volume": "1", "expect_error": "Degenerate"}), "must fail")

    def test_a_regression_fixture_is_out(self):
        with self.assertRaises(import_arris.Excluded):
            import_arris.convert_fixture(plate_hole(), "regression/x", COMMIT)

    def test_expressions_cannot_escape_the_grammar(self):
        for bad in ("__import__('os').system('true')", "(1).real", "2 ** 3", "open('x')", "9 ^ 9 ^ 9 ^ 9"):
            with self.subTest(bad=bad):
                with self.assertRaises(import_arris.Excluded):
                    import_arris.evaluate(bad, {})

    def test_the_grammar_matches_its_documented_precedence(self):
        self.assertEqual(import_arris.evaluate("-2 ^ 2", {}), -4)
        self.assertEqual(import_arris.evaluate("2 ^ 3 ^ 2", {}), 512)
        self.assertAlmostEqual(import_arris.evaluate("sqrt(2) * cos(radians(60))", {}), math.sqrt(2) / 2)


class SurveyTests(unittest.TestCase):
    def test_survey_reports_taken_and_excluded_with_reasons(self):
        with tempfile.TemporaryDirectory() as root:
            fixtures = pathlib.Path(root)
            for slug, fixture in {
                "boolean/good": plate_hole(),
                "boolean/external": plate_hole(source="https://example.invalid/part.step"),
                "blend/fancy": plate_hole(steps=[{"name": "result", "op": "fillet"}]),
            }.items():
                (fixtures / slug).mkdir(parents=True)
                (fixtures / slug / "fixture.json").write_text(json.dumps(fixture))
                (fixtures / slug / "expected.json").write_text('{"note": "never read"}')
            cases, exclusions = import_arris.survey(fixtures, COMMIT)
        self.assertEqual([case["id"] for case in cases], ["arris-boolean-good"])
        self.assertEqual(sorted(sum(exclusions.values(), [])), ["blend/fancy", "boolean/external"])


class CommittedImportTests(unittest.TestCase):
    def committed(self):
        return [json.loads(path.read_text()) for path in sorted(IMPORTED.glob("*.json"))]

    def test_every_imported_case_is_attributed_and_valid(self):
        cases = self.committed()
        self.assertTrue(cases)
        for case in cases:
            with self.subTest(case=case["id"]):
                schema.validate_case(case)
                self.assertEqual(case["licence"], "MIT OR Apache-2.0")
                self.assertRegex(case["origin"], r"arris-labs/arris@[0-9a-f]{40} tests/fixtures/\S+/fixture\.json")

    def test_imported_ids_do_not_collide_with_generated_ids(self):
        imported = {case["id"] for case in self.committed()}
        generated = {case["id"] for case in generate.generate_all()}
        self.assertFalse(imported & generated)


if __name__ == "__main__":
    unittest.main()
