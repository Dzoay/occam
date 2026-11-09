"""End to end through the pinned OCCT wheel: the plan's two tests and the closed-form anchors."""

import json
import math
import os
import pathlib
import subprocess
import sys
import tempfile
import unittest

from occam_oracle.caseformat import validate_case
from occam_oracle.compare import check_case
from occam_oracle.runner import OCCT, run_case

from .support import CUT_VOLUME, boolean, box, case, cut_case, cylinder, sphere

HERE = pathlib.Path(__file__).resolve().parent
ORACLE = HERE.parent / "oracle.py"
EXAMPLES = HERE.parent / "examples"


def measured(result):
    if result["status"] != "ok":
        raise AssertionError(f"expected an ok result, got {result}")
    return result["shape"]


def oracle_cli(*args, env=None):
    return subprocess.run(
        [sys.executable, str(ORACLE), *args], capture_output=True, text=True, env=env, cwd=HERE.parent, check=False
    )


class ClosedFormTests(unittest.TestCase):
    """Closed-form answers anchor the harness: these are arithmetic, not OCCT's word."""

    def assert_close(self, actual, expected):
        self.assertLess(abs(actual - expected), 1e-6 * abs(expected), f"{actual} vs {expected}")

    def test_box_volume(self):
        shape = measured(run_case(case("box", [box("a", (10, 20, 30))], "a")))
        self.assert_close(shape["volume"], 6000.0)
        self.assert_close(shape["area"], 2200.0)

    def test_cylinder_volume(self):
        shape = measured(run_case(case("cyl", [cylinder("a", 2, 40)], "a")))
        self.assert_close(shape["volume"], math.pi * 4.0 * 40.0)
        self.assert_close(shape["area"], 2.0 * math.pi * 2.0 * 42.0)

    def test_cut_volume(self):
        self.assert_close(measured(run_case(cut_case()))["volume"], CUT_VOLUME)

    def test_bbox_is_tight(self):
        shape = measured(run_case(case("s", [sphere("a", 5, origin=(1, 2, 3))], "a")))
        for got, want in zip(shape["bbox"]["min"] + shape["bbox"]["max"], [-4, -3, -2, 6, 7, 8]):
            self.assertLess(abs(got - want), 1e-6, shape["bbox"])


class EulerTests(unittest.TestCase):
    def test_sphere_gives_two(self):
        shape = measured(run_case(case("sphere", [sphere("a", 5)], "a")))
        self.assertEqual(shape["euler"], 2)
        self.assertEqual(shape["counts"]["degenerate_edges"], 2)

    def test_cylinder_gives_two(self):
        self.assertEqual(measured(run_case(case("cyl", [cylinder("a", 2, 3)], "a")))["euler"], 2)

    def test_plate_with_one_through_hole_gives_zero(self):
        ops = [box("p", (10, 10, 1)), cylinder("h", 2, 3, origin=(5, 5, -1)), boolean("cut", "c", "p", "h")]
        self.assertEqual(measured(run_case(case("plate", ops, "c")))["euler"], 0)


class PlanTests(unittest.TestCase):
    """The two tests the plan names."""

    def test_the_harness_rejects_a_deliberately_wrong_volume(self):
        document = case("wrong", [box("a", (10, 20, 30))], "a", {"volume": {"value": 6001.0, "rel_tol": 1e-6}})
        report = check_case(document, run_case(document))
        self.assertFalse(report["pass"])
        self.assertFalse(next(m for m in report["measures"] if m["measure"] == "volume")["pass"])

    def test_the_cli_exits_nonzero_on_a_wrong_volume(self):
        document = case("wrong", [box("a", (10, 20, 30))], "a", {"volume": {"value": 6001.0, "rel_tol": 1e-6}})
        good = case("right", [box("a", (10, 20, 30))], "a", {"volume": {"value": 6000.0, "rel_tol": 1e-6}})
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            (root / "wrong.json").write_text(json.dumps(document))
            (root / "right.json").write_text(json.dumps(good))
            results = root / "results"
            for item, code in [("right.json", 0), ("wrong.json", 1)]:
                self.assertEqual(oracle_cli("run", "--out", str(results), str(root / item)).returncode, 0)
                outcome = oracle_cli("check", "--results", str(results), str(root / item))
                self.assertEqual(outcome.returncode, code, outcome.stdout + outcome.stderr)

    def test_two_runs_give_identical_result_files(self):
        documents = self.documents()
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            cases = root / "cases"
            cases.mkdir()
            for document in documents:
                validate_case(document)
                (cases / f"{document['id']}.json").write_text(json.dumps(document))
            first, second = root / "first", root / "second"
            quiet = dict(os.environ)
            noisy = dict(os.environ, CSF_DEBUG="1", OCCT_NOISE="1", LC_ALL="de_DE.UTF-8", TZ="Asia/Tokyo")
            self.assertEqual(oracle_cli("run", "--out", str(first), str(cases), env=quiet).returncode, 0)
            self.assertEqual(oracle_cli("run", "--out", str(second), str(cases), env=noisy).returncode, 0)
            one = sorted(p.relative_to(first) for p in first.rglob("*.json"))
            two = sorted(p.relative_to(second) for p in second.rglob("*.json"))
            self.assertEqual(one, two)
            self.assertEqual(len(one), len(documents))
            for relative in one:
                self.assertEqual((first / relative).read_bytes(), (second / relative).read_bytes(), str(relative))
                self.assert_clean((first / relative).read_text())

    def assert_clean(self, text):
        self.assertNotIn(os.path.expanduser("~"), text)
        self.assertNotIn("\r", text)
        self.assertTrue(text.endswith("\n"))
        document = json.loads(text)
        for forbidden in ("date", "time", "timing", "host", "hostname", "path"):
            self.assertNotIn(forbidden, document)
        self.assertEqual(document["oracle"]["engine"], "opencascade")

    @staticmethod
    def documents():
        return [
            case("box", [box("a", (10, 20, 30))], "a"),
            cut_case("cut"),
            case(
                "fuse-rotate-fillet",
                [
                    box("a", (10, 10, 10)),
                    {"op": "rotate", "id": "r", "target": "a", "axis_origin": [0, 0, 0], "axis": [0, 0, 1], "angle_degrees": 30},
                    {"op": "translate", "id": "t", "target": "r", "vector": [3, 1, 2]},
                    boolean("fuse", "f", "a", "t"),
                    {"op": "fillet", "id": "g", "target": "a", "radius": 1, "edges": "all"},
                ],
                "g",
            ),
            case(
                "sphere-mesh",
                [sphere("a", 5), {"op": "tessellate", "id": "m", "target": "a", "linear_deflection": 0.05, "angular_deflection": 0.3}],
                "a",
            ),
        ]


class ValidityTests(unittest.TestCase):
    def test_a_deliberately_broken_shape_is_reported_invalid(self):
        ops = [
            box("a", (10, 20, 30)),
            {"op": "remove_face", "id": "broken", "target": "a", "in_box": {"min": [-1, -1, -1], "max": [11, 21, 0.5]}},
        ]
        result = run_case(case("broken", ops, "broken"))
        shape = measured(result)
        self.assertFalse(shape["valid"], shape)
        self.assertFalse(shape["closed"], shape)
        self.assertEqual(shape["counts"]["faces"], 5)
        report = check_case(case("broken", ops, "broken"), result)
        self.assertFalse(report["pass"])

    def test_a_valid_shape_is_reported_valid_and_closed(self):
        shape = measured(run_case(cut_case()))
        self.assertTrue(shape["valid"])
        self.assertTrue(shape["closed"])

    def test_the_seam_and_poles_do_not_open_a_sphere(self):
        self.assertTrue(measured(run_case(case("s", [sphere("a", 5)], "a")))["closed"])


class StructuredErrorTests(unittest.TestCase):
    def test_an_oversized_fillet_is_a_structured_error_naming_the_op(self):
        ops = [box("a", (10, 10, 10)), {"op": "fillet", "id": "too-big", "target": "a", "radius": 50, "edges": "all"}]
        result = run_case(case("oversized", ops, "too-big"))
        self.assertEqual(result["status"], "error")
        self.assertEqual(result["error"]["code"], "op_failed")
        self.assertEqual(result["error"]["op"], "too-big")
        self.assertEqual(result["error"]["kind"], "fillet")
        self.assertNotIn("shape", result)

    def test_selecting_no_edges_is_a_structured_error(self):
        ops = [
            box("a", (10, 10, 10)),
            {"op": "fillet", "id": "none", "target": "a", "radius": 1, "edges": {"in_box": {"min": [90, 90, 90], "max": [91, 91, 91]}}},
        ]
        result = run_case(case("no-edges", ops, "none"))
        self.assertEqual(result["status"], "error")
        self.assertEqual(result["error"]["op"], "none")
        self.assertIn("no edge", result["error"]["message"])


class FilletTests(unittest.TestCase):
    def test_all_edges_of_a_box_round_every_corner(self):
        ops = [box("a", (10, 10, 10)), {"op": "fillet", "id": "f", "target": "a", "radius": 1, "edges": "all"}]
        shape = measured(run_case(case("fillet", ops, "f")))
        self.assertLess(shape["volume"], 1000.0)
        self.assertGreater(shape["volume"], 900.0)
        self.assertTrue(shape["valid"])

    def test_a_bounding_predicate_selects_only_the_edges_inside_it(self):
        """A box filleted along its bottom face loses the same volume as four quarter-round strips with mitred ends."""
        selector = {"in_box": {"min": [-1, -1, -1], "max": [11, 11, 0.5]}}
        ops = [box("a", (10, 10, 10)), {"op": "fillet", "id": "f", "target": "a", "radius": 1, "edges": selector}]
        shape = measured(run_case(case("bottom-edges", ops, "f")))
        allround = measured(run_case(case("all", [box("a", (10, 10, 10)), {"op": "fillet", "id": "f", "target": "a", "radius": 1, "edges": "all"}], "f")))
        self.assertGreater(shape["volume"], allround["volume"])
        self.assertLess(shape["volume"], 1000.0)

    def test_seam_edges_are_never_selected(self):
        """A cylinder's seam is not a geometric edge, so `all` rounds only its two rims."""
        ops = [cylinder("a", 5, 10), {"op": "fillet", "id": "f", "target": "a", "radius": 1, "edges": "all"}]
        shape = measured(run_case(case("cyl-fillet", ops, "f")))
        self.assertTrue(shape["valid"])
        self.assertLess(shape["volume"], math.pi * 25.0 * 10.0)


class TessellateTests(unittest.TestCase):
    def tessellation(self, ops, result_id, key="m"):
        result = run_case(case("mesh", ops, result_id))
        measured(result)
        return result["tessellations"][key]

    def test_a_sphere_mesh_is_watertight(self):
        stats = self.tessellation(
            [sphere("a", 5), {"op": "tessellate", "id": "m", "target": "a", "linear_deflection": 0.05, "angular_deflection": 0.3}],
            "a",
        )
        self.assertTrue(stats["watertight"], stats)
        self.assertGreater(stats["triangles"], 0)
        self.assertGreater(stats["vertices"], 0)
        self.assertEqual(stats["target"], "a")
        self.assertEqual(stats["linear_deflection"], 0.05)
        self.assertEqual(stats["angular_deflection"], 0.3)
        self.assertEqual(stats["relative"], False)

    def test_a_cut_mesh_is_watertight(self):
        ops = cut_case()["ops"] + [
            {"op": "tessellate", "id": "m", "target": "c", "linear_deflection": 0.1, "angular_deflection": 0.5}
        ]
        self.assertTrue(self.tessellation(ops, "c")["watertight"])

    def test_a_closed_box_mesh_has_the_box_counts(self):
        stats = self.tessellation(
            [box("a", (1, 2, 3)), {"op": "tessellate", "id": "m", "target": "a", "linear_deflection": 0.1, "angular_deflection": 0.5}],
            "a",
        )
        self.assertEqual((stats["triangles"], stats["vertices"]), (12, 8))

    def test_tessellating_does_not_change_the_shape_measures(self):
        plain = measured(run_case(case("plain", [sphere("a", 5)], "a")))
        meshed = measured(
            run_case(
                case(
                    "plain",
                    [sphere("a", 5), {"op": "tessellate", "id": "m", "target": "a", "linear_deflection": 0.5, "angular_deflection": 1.0}],
                    "a",
                )
            )
        )
        self.assertEqual(plain, meshed)


class ThreadsTests(unittest.TestCase):
    def test_threads_are_recorded(self):
        result = run_case(cut_case(), threads=2)
        self.assertEqual(result["status"], "ok", result)
        self.assertEqual(result["threads"], 2)

    def test_default_is_one_thread(self):
        self.assertEqual(run_case(cut_case())["threads"], 1)


class OracleIdentityTests(unittest.TestCase):
    def test_the_identity_is_the_pin(self):
        result = run_case(case("box", [box("a", (1, 1, 1))], "a"))
        self.assertEqual(result["oracle"], {"engine": "opencascade", "engine_version": "8.0.1", "package": "cadquery-ocp", "package_version": "8.0.1.1.0"})
        self.assertEqual(result["oracle"]["engine_version"], OCCT.version)


class ExampleTests(unittest.TestCase):
    def test_the_committed_examples_pass_their_own_expectations(self):
        files = sorted(EXAMPLES.glob("*.json"))
        self.assertTrue(files)
        for file in files:
            document = json.loads(file.read_text())
            validate_case(document)
            report = check_case(document, run_case(document))
            self.assertTrue(report["pass"], report)


class VolumeIdentityEndToEndTests(unittest.TestCase):
    def test_the_identity_holds_through_the_oracle(self):
        a = [box("a", (10, 10, 10))]
        b = [cylinder("b", 3, 20, origin=(5, 5, -5))]
        both = a + b
        cases = {
            "a": case("id-a", a, "a"),
            "b": case("id-b", b, "b"),
            "union": case("id-u", both + [boolean("fuse", "u", "a", "b")], "u"),
            "intersection": case("id-i", both + [boolean("common", "i", "a", "b")], "i"),
        }
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            scenarios = root / "cases"
            scenarios.mkdir()
            for role, document in cases.items():
                document["volume_identity"] = {"group": "pair", "role": role}
                (scenarios / f"{document['id']}.json").write_text(json.dumps(document))
            results = root / "results"
            self.assertEqual(oracle_cli("run", "--out", str(results), str(scenarios)).returncode, 0)
            outcome = oracle_cli("check", "--results", str(results), "--json", str(scenarios))
            self.assertEqual(outcome.returncode, 0, outcome.stdout + outcome.stderr)
            report = json.loads(outcome.stdout)
            self.assertEqual([g["pass"] for g in report["volume_identity"]], [True])


class RegisterCommandTests(unittest.TestCase):
    def test_the_committed_register_checks_out(self):
        outcome = oracle_cli("register")
        self.assertEqual(outcome.returncode, 0, outcome.stdout + outcome.stderr)


if __name__ == "__main__":
    unittest.main()
