"""The child process: scrubbed environment, fixed parallelism, structured failure. Fake workers stand in for OCCT."""

import json
import os
import pathlib
import tempfile
import textwrap
import unittest

from occam_oracle.runner import OCCT, Engine, result_path, run_case, scrubbed_env, write_result

from .support import box_case, ok_result


def fake_engine(body):
    """An engine whose worker is a throwaway script, so the runner can be tested without OCCT."""
    scripts = tempfile.TemporaryDirectory(prefix="occam-oracle-test-")
    script = pathlib.Path(scripts.name) / "worker.py"
    preamble = textwrap.dedent(
        """\
        import argparse, json, os, pathlib, signal, sys, time
        parser = argparse.ArgumentParser()
        for name in ("--case", "--result", "--progress", "--threads"):
            parser.add_argument(name)
        args = parser.parse_args()
        case = json.loads(pathlib.Path(args.case).read_text())
        identity = {"engine": "fake", "engine_version": "1.2.3", "package": "fake-package", "package_version": "4.5.6"}
        def shape():
            return {"volume": 1.0, "area": 6.0, "bbox": {"min": [0.0, 0.0, 0.0], "max": [1.0, 1.0, 1.0]}, "euler": 2,
                    "counts": {"vertices": 8, "edges": 12, "degenerate_edges": 0, "wires": 6, "faces": 6, "shells": 1, "solids": 1},
                    "valid": True, "closed": True}
        def ok(extra=None):
            result = {"schema": "occam-oracle-result/1", "id": case["id"], "oracle": identity, "threads": int(args.threads),
                      "status": "ok", "shape": shape(), "tessellations": {}}
            result.update(extra or {})
            pathlib.Path(args.result).write_text(json.dumps(result))
        """
    )
    script.write_text(preamble + textwrap.dedent(body))
    engine = Engine(name="fake", version="1.2.3", package="fake-package", package_version="4.5.6", worker=script)
    return engine, scripts


class ScrubbedEnvTests(unittest.TestCase):
    def test_only_the_documented_variables_survive(self):
        env = scrubbed_env(pathlib.Path("/tmp/oracle-home"))
        self.assertEqual(sorted(env), ["HOME", "LC_ALL", "PATH", "PYTHONHASHSEED"])
        self.assertEqual(env["HOME"], "/tmp/oracle-home")
        self.assertEqual(env["LC_ALL"], "C")
        self.assertEqual(env["PYTHONHASHSEED"], "0")

    def test_the_worker_really_runs_in_the_scrubbed_environment(self):
        engine, scripts = fake_engine(
            """\
            ok({"tessellations": {"env": {"names": sorted(os.environ),
                                         "home": os.environ["HOME"],
                                         "cwd": os.getcwd(),
                                         "hash": hash("occam")}}})
            """
        )
        with scripts:
            os.environ["CSF_TESTING"] = "leak"
            try:
                result = run_case(box_case(), engine=engine)
            finally:
                del os.environ["CSF_TESTING"]
        report = result["tessellations"]["env"]
        self.assertEqual(report["names"], ["HOME", "LC_ALL", "PATH", "PYTHONHASHSEED"])
        self.assertNotEqual(report["home"], os.environ.get("HOME"))
        self.assertEqual(report["cwd"], report["home"])

    def test_the_hash_seed_is_fixed_across_runs(self):
        engine, scripts = fake_engine(
            """\
            ok({"tessellations": {"env": {"hash": hash("occam")}}})
            """
        )
        with scripts:
            first = run_case(box_case(), engine=engine)
            second = run_case(box_case(), engine=engine)
        self.assertEqual(first["tessellations"]["env"]["hash"], second["tessellations"]["env"]["hash"])


class ThreadsTests(unittest.TestCase):
    def test_threads_reach_the_worker_and_the_result(self):
        engine, scripts = fake_engine(
            """\
            ok({"tessellations": {"env": {"threads": args.threads}}})
            """
        )
        with scripts:
            result = run_case(box_case(), engine=engine, threads=3)
        self.assertEqual(result["threads"], 3)
        self.assertEqual(result["tessellations"]["env"]["threads"], "3")

    def test_threads_must_be_positive(self):
        with self.assertRaises(ValueError):
            run_case(box_case(), threads=0)


class FailureTests(unittest.TestCase):
    def test_a_crashing_worker_gives_a_structured_error_naming_the_op_in_progress(self):
        engine, scripts = fake_engine(
            """\
            pathlib.Path(args.progress).write_text("a")
            os.kill(os.getpid(), signal.SIGSEGV)
            """
        )
        with scripts:
            result = run_case(box_case(), engine=engine)
        self.assertEqual(result["status"], "error")
        self.assertEqual(result["error"]["code"], "worker_crashed")
        self.assertEqual(result["error"]["op"], "a")
        self.assertEqual(result["error"]["kind"], "box")
        self.assertNotIn("shape", result)

    def test_a_worker_that_exits_without_a_result_is_structured(self):
        engine, scripts = fake_engine("sys.exit(3)\n")
        with scripts:
            result = run_case(box_case(), engine=engine)
        self.assertEqual(result["error"]["code"], "worker_crashed")
        self.assertIsNone(result["error"]["op"])

    def test_a_hung_worker_times_out_with_a_structured_error(self):
        engine, scripts = fake_engine("time.sleep(60)\n")
        with scripts:
            result = run_case(box_case(), engine=engine, timeout=0.5)
        self.assertEqual(result["error"]["code"], "worker_timeout")

    def test_a_malformed_result_is_structured(self):
        engine, scripts = fake_engine('pathlib.Path(args.result).write_text("not json")\n')
        with scripts:
            result = run_case(box_case(), engine=engine)
        self.assertEqual(result["error"]["code"], "worker_malformed")

    def test_a_result_from_the_wrong_engine_is_refused(self):
        engine, scripts = fake_engine(
            """\
            identity["engine_version"] = "9.9.9"
            ok()
            """
        )
        with scripts:
            result = run_case(box_case(), engine=engine)
        self.assertEqual(result["error"]["code"], "identity_mismatch")

    def test_failure_results_carry_no_timing_host_or_path(self):
        engine, scripts = fake_engine("time.sleep(60)\n")
        with scripts:
            result = run_case(box_case(), engine=engine, timeout=0.2)
        text = json.dumps(result)
        self.assertNotIn(os.path.expanduser("~"), text)
        self.assertNotIn(str(scripts.name), text)


class ResultFileTests(unittest.TestCase):
    def test_the_path_has_the_documented_layout(self):
        path = result_path(pathlib.Path("/results"), OCCT, 1, "box-0001")
        self.assertEqual(path, pathlib.Path("/results/opencascade-8.0.1/threads-1/box-0001.json"))

    def test_writing_is_canonical_and_creates_directories(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = pathlib.Path(tmp) / "deep" / "down" / "x.json"
            write_result(ok_result("x"), path)
            bytes_on_disk = path.read_bytes()
        self.assertTrue(bytes_on_disk.endswith(b"\n"))
        self.assertNotIn(b"\r", bytes_on_disk)
        self.assertEqual(json.loads(bytes_on_disk.decode("utf-8"))["id"], "x")
        document = bytes_on_disk.decode("utf-8")
        self.assertLess(document.index('"id"'), document.index('"oracle"'))


if __name__ == "__main__":
    unittest.main()
