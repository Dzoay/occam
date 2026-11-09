"""A worker that crashes or hangs loses only the case it was on."""

import json
import pathlib
import subprocess
import sys
import unittest

from occam_baseline import runner

FAKE_WORKER = """
import json, pathlib, sys, time
entries = json.loads(pathlib.Path(sys.argv[1]).read_text())
results, progress = sys.argv[2], pathlib.Path(sys.argv[3])
for entry in entries:
    case_id = entry["case"]["id"]
    progress.write_text(f"{case_id}\\n{time.time()}\\n")
    if case_id == "crash":
        sys.exit(9)
    if case_id == "hang":
        time.sleep(60)
    with open(results, "a") as file:
        file.write(json.dumps({"id": case_id, "status": "ok", "ms": [1.0], "measured": [2.0]}) + "\\n")
progress.write_text("")
"""


def entry(case_id):
    return {"case": {"id": case_id}, "chosen": {"family": "f", "group": "g", "kind": "volume", "timed": ["fuse"]}}


def start_fake(entries, scratch):
    cases, results, progress = scratch / "cases.json", scratch / "results.jsonl", scratch / "progress"
    cases.write_text(json.dumps(entries))
    progress.write_text("")
    stderr = (scratch / "stderr").open("w")
    worker = subprocess.Popen(  # pylint: disable=consider-using-with
        [sys.executable, "-B", "-c", FAKE_WORKER, str(cases), str(results), str(progress)],
        stdout=subprocess.DEVNULL,
        stderr=stderr,
        start_new_session=True,
    )
    stderr.close()
    return worker, pathlib.Path(results), pathlib.Path(progress)


class TestRestart(unittest.TestCase):
    def statuses(self, ids, timeout=30.0):
        records = runner.run_entries([entry(i) for i in ids], 1, start_fake, case_timeout=timeout)
        return {record["id"]: record["status"] for record in records}

    def test_every_case_finishes_without_trouble(self):
        self.assertEqual(self.statuses(["a", "b"]), {"a": "ok", "b": "ok"})

    def test_a_crash_loses_only_that_case(self):
        self.assertEqual(self.statuses(["a", "crash", "b"]), {"a": "ok", "crash": "crashed", "b": "ok"})

    def test_a_hang_is_cut_off_and_the_rest_still_run(self):
        self.assertEqual(self.statuses(["a", "hang", "b"], timeout=0.5), {"a": "ok", "hang": "timeout", "b": "ok"})

    def test_records_come_back_in_the_order_asked_for(self):
        records = runner.run_entries([entry("b"), entry("a")], 1, start_fake, case_timeout=30.0)
        self.assertEqual([record["id"] for record in records], ["b", "a"])


if __name__ == "__main__":
    unittest.main()
