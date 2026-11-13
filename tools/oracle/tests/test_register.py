"""The divergence register's well-formedness check."""

import copy
import json
import pathlib
import unittest

from occam_oracle.register import SCHEMA, WITHHELD, check_register

COMMITTED = pathlib.Path(__file__).resolve().parents[1] / "divergences.json"


def entry(**overrides):
    record = {
        "case": "box-cut-cylinder-0001",
        "measure": "volume",
        "values": {"closed-form": 5623.1},
        "decided_by": "closed-form",
        "verdict": "occt-wrong",
        "note": "A hand-computed closed form settles it.",
    }
    record.update(overrides)
    return record


class RegisterTests(unittest.TestCase):
    def test_the_committed_register_is_well_formed(self):
        self.assertEqual(check_register(json.loads(COMMITTED.read_text())), [])

    def test_an_empty_register_is_well_formed(self):
        self.assertEqual(check_register({"schema": SCHEMA, "entries": []}), [])

    def test_a_complete_entry_is_well_formed(self):
        self.assertEqual(check_register({"schema": SCHEMA, "entries": [entry()]}), [])

    def test_the_schema_is_the_one_without_occts_values(self):
        self.assertEqual(SCHEMA, "occam-oracle-divergences/2")
        self.assertNotEqual(check_register({"schema": "occam-oracle-divergences/1", "entries": []}), [])

    def test_an_entry_may_hold_one_value_or_none(self):
        for values in ({"closed-form": 1.0}, {"quadrature": 2.0, "occam": 2.0}, {}):
            record = entry(values=values)
            self.assertEqual(check_register({"schema": SCHEMA, "entries": [record]}), [], values)

    def test_occts_values_are_withheld_while_o6_is_open(self):
        self.assertIn("occt", WITHHELD)
        record = entry(values={"occt": 5623.0, "closed-form": 5623.1})
        errors = check_register({"schema": SCHEMA, "entries": [record]})
        self.assertTrue(any("not recorded" in error for error in errors), errors)

    def test_undecided_is_allowed_with_no_deciding_invariant(self):
        record = entry(decided_by="none", verdict="undecided")
        self.assertEqual(check_register({"schema": SCHEMA, "entries": [record]}), [])

    def test_malformed_registers_are_reported(self):
        bad = {
            "wrong schema": {"schema": "other", "entries": []},
            "no entries": {"schema": SCHEMA},
            "entries not a list": {"schema": SCHEMA, "entries": {}},
        }
        for label, document in bad.items():
            self.assertNotEqual(check_register(document), [], label)

    def test_malformed_entries_are_reported(self):
        bad = {
            "unknown verdict": entry(verdict="maybe"),
            "unknown invariant": entry(decided_by="vibes"),
            "values not an object": entry(values=[1.0]),
            "an occt value": entry(values={"occt": 1.0, "closed-form": 2.0}),
            "text value": entry(values={"closed-form": "two"}),
            "bad source name": entry(values={"Closed-Form": 2.0}),
            "empty note": entry(note=""),
            "no case": entry(case=""),
            "no measure": entry(measure=""),
            "extra key": dict(entry(), extra=1),
            "undecided but decided": entry(decided_by="closed-form", verdict="undecided"),
            "decided by nothing": entry(decided_by="none", verdict="occt-wrong"),
        }
        for label, record in bad.items():
            errors = check_register({"schema": SCHEMA, "entries": [record]})
            self.assertNotEqual(errors, [], label)

    def test_duplicate_case_and_measure_are_reported(self):
        self.assertNotEqual(check_register({"schema": SCHEMA, "entries": [entry(), copy.deepcopy(entry())]}), [])

    def test_entries_must_be_sorted(self):
        unsorted = [entry(case="b"), entry(case="a")]
        self.assertNotEqual(check_register({"schema": SCHEMA, "entries": unsorted}), [])


if __name__ == "__main__":
    unittest.main()
