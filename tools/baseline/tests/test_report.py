"""The report: nothing that names a machine or person, and none of the oracle's measured geometry."""

import unittest

from occam_baseline import report


def document():
    return {
        "schema": "occam-baseline/1",
        "engine": "occt-native",
        "threads": 1,
        "group": "booleans",
        "reps": {"warmups": 1, "min_repetitions": 5},
        "machine": {
            "cpu": "Fake CPU 3000",
            "physical_cores": 6,
            "logical_cores": 12,
            "governor": "powersave",
            "kernel": "7.3.1-test",
            "memory_gib": 31.5,
            "wheel": "cadquery-ocp 8.0.1.1.0",
            "python": "3.13.1",
            "node": "v26.13.0",
            # Things that must never reach the report even when something put them in the record.
            "hostname": "secret-host",
            "user": "secret-user",
            "results_dir": "/home/secret-user/.cache/results",
        },
        "load_before": [0.5, 0.4, 0.3],
        "load_after": [1.5, 1.4, 1.3],
        "cases": [
            {
                "id": "box-corner-fuse-0001",
                "family": "box-corner-fuse",
                "group": "booleans-fuse",
                "status": "ok",
                "kind": "volume",
                "ms": [10.0, 12.0, 14.0],
                "measured": [987654.321, 987654.321, 987654.321],
                "warmups": 1,
                "repetitions": 3,
                "cores": [0],
            }
        ],
    }


FORBIDDEN = ("secret-host", "secret-user", "/home/secret-user")


class TestRender(unittest.TestCase):
    def test_contains_the_family_table_numbers(self):
        text = report.render([document()], forbidden=FORBIDDEN)
        self.assertIn("box-corner-fuse", text)
        self.assertIn("12.0", text)  # the case's median, in the table

    def test_labels_targets_as_goals_not_gates(self):
        text = report.render([document()], forbidden=FORBIDDEN)
        self.assertIn("goal, not a gate", text.lower())

    def test_no_hostname_username_or_home_path(self):
        text = report.render([document()], forbidden=FORBIDDEN)
        for word in FORBIDDEN:
            self.assertNotIn(word, text)

    def test_no_measured_geometry_of_the_oracle(self):
        text = report.render([document()], forbidden=FORBIDDEN)
        self.assertNotIn("987654", text)

    def test_notes_are_appended_and_also_guarded(self):
        text = report.render([document()], forbidden=FORBIDDEN, notes="The lock waited a while.")
        self.assertIn("The lock waited a while.", text)
        with self.assertRaises(report.PrivateText):
            report.render([document()], forbidden=FORBIDDEN, notes="ran on secret-host")

    def test_the_guard_refuses_text_that_names_the_machine(self):
        with self.assertRaises(report.PrivateText):
            report.check_private("the run was on secret-host today", FORBIDDEN)

    def test_a_machine_record_field_that_reaches_the_text_is_caught(self):
        doc = document()
        doc["machine"]["cpu"] = "secret-host CPU"  # a whitelisted field carrying a forbidden word
        with self.assertRaises(report.PrivateText):
            report.render([doc], forbidden=FORBIDDEN)


if __name__ == "__main__":
    unittest.main()
