"""Every committed case satisfies the harness's own validator, which is the one definition of the case format."""

import json
import pathlib
import sys
import unittest

CORPUS = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(CORPUS))

import generate  # noqa: E402
from occam_oracle.caseformat import validate_case  # noqa: E402


class EveryCaseIsValid(unittest.TestCase):
    def test_every_generated_case_passes_the_harness_validator(self):
        for case in generate.generate_all():
            with self.subTest(case=case["id"]):
                validate_case(case)

    def test_every_imported_case_passes_the_harness_validator(self):
        for path in sorted((CORPUS / "imported").glob("*.json")):
            with self.subTest(case=path.stem):
                validate_case(json.loads(path.read_text(encoding="utf-8")))


if __name__ == "__main__":
    unittest.main()
