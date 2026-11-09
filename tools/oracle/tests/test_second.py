"""The second-opinion seam: the line protocol against a fake driver, and arris itself once it has been built."""

import json
import math
import os
import pathlib
import stat
import sys
import tempfile
import textwrap
import time
import unittest

from occam_oracle.compare import check_case, compare_results
from occam_oracle.runner import ENGINES, OCCT, Engine, run_case
from occam_oracle.second import PINS, driver_binary
from occam_oracle.second_worker import request
from occam_oracle.meshcheck import summarize

from .support import CUT_VOLUME, boolean, box, case, cut_case, cylinder, error_result, ok_result, sphere

WORKER = pathlib.Path(__file__).resolve().parents[1] / "occam_oracle" / "second_worker.py"
ARRIS_BUILT = driver_binary("arris").is_file()


def fake_engine(body):
    """An engine whose compiled driver is a throwaway script reading the same lines a real driver reads."""
    folder = tempfile.TemporaryDirectory(prefix="occam-oracle-driver-")
    driver = pathlib.Path(folder.name) / "driver"
    driver.write_text(f"#!{sys.executable}\nimport os, signal, sys\nlines = sys.stdin.read().splitlines()\n" + textwrap.dedent(body))
    driver.chmod(driver.stat().st_mode | stat.S_IXUSR)
    args = ("--driver", str(driver), "--engine-name", "fake", "--engine-version", "1", "--package", "fake", "--package-version", "abc")
    return Engine(name="fake", version="1", package="fake", package_version="abc", worker=WORKER, worker_args=args), folder


GOOD = """\
print("ok")
print("volume 6000.0"); print("area 2200.0"); print("bbox 0 0 0 10 20 30")
print("counts 8 12 0 6 6 1 1"); print("valid 1"); print("closed 1")
"""


class ProtocolTests(unittest.TestCase):
    def test_a_case_becomes_one_line_per_op_with_round_trip_numbers(self):
        document = cut_case()
        text = request(document)
        self.assertIn("box a 10.0 20.0 30.0 0.0 0.0 0.0\n", text)
        self.assertIn("cut c a b\n", text)
        self.assertTrue(text.endswith("result c\n"))

    def test_a_fillet_selector_and_tessellate_are_tokens(self):
        ops = [
            box("a", (1, 1, 1)),
            {"op": "fillet", "id": "f", "target": "a", "radius": 0.1, "edges": {"in_box": {"min": [0, 0, 0], "max": [1, 1, 1]}}},
            {"op": "tessellate", "id": "m", "target": "f", "linear_deflection": 0.1, "angular_deflection": 0.5},
        ]
        text = request(case("c", ops, "f"))
        self.assertIn("fillet f a 0.1 box 0.0 0.0 0.0 1.0 1.0 1.0\n", text)
        self.assertIn("tessellate m f 0.1 0.5 0\n", text)

    def test_the_answer_becomes_a_result_with_euler_computed_from_counts(self):
        engine, folder = fake_engine(GOOD)
        with folder:
            result = run_case(case("box", [box("a", (10, 20, 30))], "a"), engine=engine)
        self.assertEqual(result["status"], "ok", result)
        self.assertEqual(result["shape"]["euler"], 2)
        self.assertEqual(result["shape"]["volume"], 6000.0)
        self.assertEqual(result["oracle"], engine.identity())

    def test_a_mesh_is_judged_by_the_shared_check(self):
        tetra = "\n".join(["0 0 0", "1 0 0", "0 1 0", "0 0 1", "0 2 1", "0 1 3", "0 3 2", "1 2 3"])
        ops = [box("a", (1, 1, 1)), {"op": "tessellate", "id": "m", "target": "a", "linear_deflection": 0.1, "angular_deflection": 0.5}]
        engine, folder = fake_engine(GOOD + f'print("mesh m 4 4")\nprint("""{tetra}""")\n')
        with folder:
            result = run_case(case("mesh", ops, "a"), engine=engine)
        self.assertEqual(result["tessellations"]["m"]["watertight"], True, result)
        self.assertEqual((result["tessellations"]["m"]["triangles"], result["tessellations"]["m"]["vertices"]), (4, 4))

    def test_an_engine_refusal_names_the_op(self):
        engine, folder = fake_engine('print("error a OpError: refused")\n')
        with folder:
            result = run_case(case("box", [box("a", (1, 1, 1))], "a"), engine=engine)
        self.assertEqual(result["error"], {"code": "op_failed", "op": "a", "kind": "box", "message": "OpError: refused"})

    def test_an_unsupported_op_has_its_own_code(self):
        engine, folder = fake_engine('print("error a sphere is not supported by this engine")\n')
        with folder:
            result = run_case(case("s", [sphere("a", 1)], "a"), engine=engine)
        self.assertEqual(result["error"]["code"], "op_unsupported")

    def test_a_crashing_driver_blames_the_op_in_progress(self):
        engine, folder = fake_engine('open(sys.argv[1], "w").write("a")\nos.kill(os.getpid(), signal.SIGSEGV)\n')
        with folder:
            result = run_case(case("box", [box("a", (1, 1, 1))], "a"), engine=engine)
        self.assertEqual((result["error"]["code"], result["error"]["op"]), ("worker_crashed", "a"))

    def test_a_hung_driver_is_killed_with_the_worker(self):
        marker = pathlib.Path(tempfile.gettempdir()) / f"occam-oracle-hung-{os.getpid()}"
        engine, folder = fake_engine(f'import time, pathlib\npathlib.Path("{marker}").write_text(str(os.getpid()))\ntime.sleep(120)\n')
        with folder:
            result = run_case(case("box", [box("a", (1, 1, 1))], "a"), engine=engine, timeout=1.0)
        self.assertEqual(result["error"]["code"], "worker_timeout")
        pid = int(marker.read_text())
        marker.unlink()
        self.assert_gone(pid)

    def assert_gone(self, pid):
        """Waits for the process to go; a killed process awaiting its reaper counts as gone."""
        deadline = time.monotonic() + 10.0
        status = pathlib.Path(f"/proc/{pid}/stat")
        while time.monotonic() < deadline:
            try:
                if status.read_text().rsplit(")", 1)[1].split()[0] == "Z":
                    return
            except (FileNotFoundError, ProcessLookupError):
                return
            time.sleep(0.05)
        self.fail(f"the driver (pid {pid}) outlived its timeout")


class EngineListTests(unittest.TestCase):
    def test_second_opinions_are_pinned_to_full_commits(self):
        for name, pin in PINS.items():
            self.assertEqual(len(pin.commit), 40, name)
            self.assertTrue(pin.url.startswith("https://"), name)

    def test_an_unbuilt_engine_says_how_to_build_it(self):
        for name in PINS:
            if not driver_binary(name).is_file():
                self.assertIn("build-second", ENGINES[name].missing)

    def test_the_result_directory_names_the_engine_and_version(self):
        self.assertEqual(ENGINES["arris"].directory, f"arris-{PINS['arris'].version}")


class MeshCheckTests(unittest.TestCase):
    TETRA = [(0.0, 0.0, 0.0), (1.0, 0.0, 0.0), (0.0, 1.0, 0.0), (0.0, 0.0, 1.0)]
    FACES = [(0, 2, 1), (0, 1, 3), (0, 3, 2), (1, 2, 3)]

    def test_a_closed_mesh_is_watertight(self):
        self.assertEqual(summarize(self.TETRA, self.FACES, 1e-9), {"triangles": 4, "vertices": 4, "watertight": True})

    def test_a_missing_triangle_is_not(self):
        self.assertFalse(summarize(self.TETRA, self.FACES[:3], 1e-9)["watertight"])

    def test_a_flipped_triangle_is_not(self):
        faces = [*self.FACES[:3], (1, 3, 2)]
        self.assertFalse(summarize(self.TETRA, faces, 1e-9)["watertight"])

    def test_coincident_points_weld_across_faces(self):
        points = [*self.TETRA, (1.0 + 1e-12, 0.0, 0.0)]
        faces = [(0, 2, 1), (0, 4, 3), (0, 3, 2), (1, 2, 3)]
        self.assertTrue(summarize(points, faces, 1e-9)["watertight"])

    def test_degenerate_triangles_are_dropped(self):
        self.assertEqual(summarize(self.TETRA, [*self.FACES, (0, 0, 1)], 1e-9)["triangles"], 4)


class CompareResultsTests(unittest.TestCase):
    def test_close_results_agree(self):
        self.assertTrue(compare_results(ok_result(volume=1.0), ok_result(volume=1.0 + 1e-9))["agree"])

    def test_a_different_volume_disagrees(self):
        report = compare_results(ok_result(volume=1.0), ok_result(volume=1.1))
        self.assertFalse(report["agree"])
        self.assertIn("volume", report["detail"])

    def test_different_validity_disagrees(self):
        self.assertFalse(compare_results(ok_result(valid=True), ok_result(valid=False))["agree"])

    def test_topology_is_not_compared(self):
        other = ok_result(counts={"vertices": 1, "edges": 1, "degenerate_edges": 0, "wires": 1, "faces": 1, "shells": 1, "solids": 1})
        self.assertTrue(compare_results(ok_result(), other)["agree"])

    def test_both_failing_agree_and_one_failing_does_not(self):
        self.assertTrue(compare_results(error_result(), error_result())["agree"])
        self.assertFalse(compare_results(ok_result(), error_result())["agree"])

    def test_an_unsupported_op_compares_nothing(self):
        self.assertIsNone(compare_results(ok_result(), error_result(code="op_unsupported"))["agree"])


@unittest.skipUnless(ARRIS_BUILT, "arris's driver is not built; run `just oracle build-second arris`")
class ArrisTests(unittest.TestCase):
    """arris through its documented interface, against closed-form answers and OCCT."""

    engine = ENGINES["arris"]

    def assert_close(self, actual, expected, rel_tol=1e-6):
        self.assertLess(abs(actual - expected), rel_tol * abs(expected), f"{actual} vs {expected}")

    def shape(self, document):
        result = run_case(document, engine=self.engine)
        self.assertEqual(result["status"], "ok", result)
        return result["shape"]

    def test_box_and_cylinder_match_their_closed_forms(self):
        self.assert_close(self.shape(case("b", [box("a", (10, 20, 30))], "a"))["volume"], 6000.0)
        self.assert_close(self.shape(case("c", [cylinder("a", 2, 40)], "a"))["volume"], math.pi * 160.0)

    def test_the_revolved_primitives_match_their_closed_forms(self):
        self.assert_close(self.shape(case("s", [sphere("a", 5)], "a"))["volume"], 4.0 / 3.0 * math.pi * 125.0)
        torus = {"op": "torus", "id": "a", "major_radius": 5, "minor_radius": 1, "origin": [0, 0, 0], "axis": [0, 0, 1]}
        self.assert_close(self.shape(case("t", [torus], "a"))["volume"], 2.0 * math.pi**2 * 5.0)

    def test_a_cut_matches_the_closed_form_and_occt(self):
        document = cut_case()
        ours = run_case(document, engine=self.engine)
        self.assert_close(ours["shape"]["volume"], CUT_VOLUME)
        self.assertTrue(compare_results(run_case(document, engine=OCCT), ours)["agree"])

    def test_the_euler_cases_hold(self):
        self.assertEqual(self.shape(case("s", [sphere("a", 5)], "a"))["euler"], 2)
        self.assertEqual(self.shape(case("c", [cylinder("a", 2, 3)], "a"))["euler"], 2)
        ops = [box("p", (10, 10, 1)), cylinder("h", 2, 3, origin=(5, 5, -1)), boolean("cut", "c", "p", "h")]
        self.assertEqual(self.shape(case("plate", ops, "c"))["euler"], 0)

    def test_a_result_passes_the_same_invariants(self):
        document = cut_case()
        self.assertTrue(check_case(document, run_case(document, engine=self.engine))["pass"])

    def test_an_oversized_fillet_is_a_structured_error(self):
        ops = [box("a", (10, 10, 10)), {"op": "fillet", "id": "too-big", "target": "a", "radius": 50, "edges": "all"}]
        result = run_case(case("big", ops, "too-big"), engine=self.engine)
        self.assertEqual((result["status"], result["error"]["op"]), ("error", "too-big"))

    def test_fault_injection_is_reported_as_unsupported(self):
        ops = [box("a", (1, 1, 1)), {"op": "remove_face", "id": "x", "target": "a", "in_box": {"min": [-1, -1, -1], "max": [2, 2, 0.5]}}]
        self.assertEqual(run_case(case("x", ops, "x"), engine=self.engine)["error"]["code"], "op_unsupported")

    def test_a_mesh_is_watertight(self):
        ops = [cylinder("a", 3, 10), {"op": "tessellate", "id": "m", "target": "a", "linear_deflection": 0.05, "angular_deflection": 0.3}]
        result = run_case(case("m", ops, "a"), engine=self.engine)
        self.assertTrue(result["tessellations"]["m"]["watertight"], result)

    def test_two_runs_give_the_same_answer(self):
        document = cut_case()
        self.assertEqual(json.dumps(run_case(document, engine=self.engine)), json.dumps(run_case(document, engine=self.engine)))


if __name__ == "__main__":
    unittest.main()
