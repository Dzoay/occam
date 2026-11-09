"""Result files are written byte-for-byte reproducibly."""

import json
import unittest

from occam_oracle.canonical import dumps


class CanonicalTests(unittest.TestCase):
    def test_keys_are_sorted_and_output_ends_with_lf(self):
        text = dumps({"b": 1, "a": {"z": 1, "y": 2}})
        self.assertTrue(text.endswith("\n"))
        self.assertFalse(text.endswith("\n\n"))
        self.assertNotIn("\r", text)
        self.assertLess(text.index('"a"'), text.index('"b"'))
        self.assertLess(text.index('"y"'), text.index('"z"'))

    def test_floats_are_shortest_round_trip(self):
        for value in (0.1, 1.0, 1e-07, 5623.008881569224, 1e22, 2.220446049250313e-16):
            self.assertEqual(json.loads(dumps({"x": value}))["x"], value)
            self.assertIn(repr(value), dumps({"x": value}))

    def test_negative_zero_is_normalised(self):
        self.assertEqual(dumps({"x": -0.0}), dumps({"x": 0.0}))
        self.assertEqual(dumps([-0.0]), dumps([0.0]))

    def test_non_finite_numbers_are_refused(self):
        for value in (float("nan"), float("inf"), float("-inf")):
            with self.assertRaises(ValueError):
                dumps({"x": value})

    def test_same_value_gives_same_bytes(self):
        self.assertEqual(dumps({"a": [1.5, 2], "b": None}), dumps({"b": None, "a": [1.5, 2]}))


if __name__ == "__main__":
    unittest.main()
