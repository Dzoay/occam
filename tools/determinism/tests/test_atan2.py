"""The separate atan2 gives C99 Annex F's special cases, is odd in y bit for bit, and stays within its measured
accuracy.
"""

import decimal
import math
import unittest

import accuracy
from atan2 import atan2
from sincos import bits

SAMPLE = 150  # inputs per accuracy set
# 30,000 inputs per set reach 1.469 ulp, on the uniform set: a quotient rounded just above a power of two whose
# arctangent falls just below it carries a whole ulp of the result into it, before the last rounding adds half.
BOUND = 1.5

with decimal.localcontext(accuracy.NARROW):  # the doubles nearest π and its fractions, from the reference's own π
    PI = float(accuracy.PI)
    HALF_PI = float(accuracy.HALF_PI)
    QUARTER_PI = float(accuracy.HALF_PI / 2)
    THREE_QUARTERS_PI = float(accuracy.HALF_PI * 3 / 2)

FINITE = (5e-324, 1e-300, 0.5, 1.0, 3.0, 1e300, 1.7976931348623157e308)


class TheSpecialCasesAreC99s(unittest.TestCase):
    def assert_same(self, y, x, want):
        """Asserts atan2(y, x) has exactly the bits of `want`, so a zero's sign counts."""
        got = atan2(y, x)
        self.assertEqual(bits(got), bits(want), f"atan2({y!r}, {x!r}) is {got!r}, not {want!r}")

    def test_a_zero_over_plus_zero_is_that_zero(self):
        self.assert_same(0.0, 0.0, 0.0)
        self.assert_same(-0.0, 0.0, -0.0)

    def test_a_zero_over_minus_zero_is_pi_with_its_sign(self):
        self.assert_same(0.0, -0.0, PI)
        self.assert_same(-0.0, -0.0, -PI)

    def test_a_zero_over_a_negative_is_pi_with_its_sign(self):
        for x in FINITE:
            self.assert_same(0.0, -x, PI)
            self.assert_same(-0.0, -x, -PI)

    def test_a_zero_over_a_positive_is_that_zero(self):
        for x in FINITE:
            self.assert_same(0.0, x, 0.0)
            self.assert_same(-0.0, x, -0.0)

    def test_a_non_zero_over_a_zero_is_a_quarter_turn_with_its_sign(self):
        for y in FINITE:
            for zero in (0.0, -0.0):
                self.assert_same(y, zero, HALF_PI)
                self.assert_same(-y, zero, -HALF_PI)

    def test_a_finite_over_plus_infinity_is_a_zero_with_its_sign(self):
        for y in (0.0,) + FINITE:
            self.assert_same(y, math.inf, 0.0)
            self.assert_same(-y, math.inf, -0.0)

    def test_a_finite_over_minus_infinity_is_pi_with_its_sign(self):
        for y in (0.0,) + FINITE:
            self.assert_same(y, -math.inf, PI)
            self.assert_same(-y, -math.inf, -PI)

    def test_an_infinity_over_a_finite_is_a_quarter_turn_with_its_sign(self):
        for x in FINITE + (0.0,):
            for signed in (x, -x):
                self.assert_same(math.inf, signed, HALF_PI)
                self.assert_same(-math.inf, signed, -HALF_PI)

    def test_an_infinity_over_plus_infinity_is_an_eighth_turn_with_its_sign(self):
        self.assert_same(math.inf, math.inf, QUARTER_PI)
        self.assert_same(-math.inf, math.inf, -QUARTER_PI)

    def test_an_infinity_over_minus_infinity_is_three_eighths_of_a_turn_with_its_sign(self):
        self.assert_same(math.inf, -math.inf, THREE_QUARTERS_PI)
        self.assert_same(-math.inf, -math.inf, -THREE_QUARTERS_PI)

    def test_nan_in_either_input_gives_nan(self):
        others = (0.0, -0.0, 1.0, -1.0, 1e300, math.inf, -math.inf, math.nan)
        for other in others:
            self.assertTrue(math.isnan(atan2(math.nan, other)), other)
            self.assertTrue(math.isnan(atan2(other, math.nan)), other)


class NegatingYNegatesTheAngle(unittest.TestCase):
    def test_bit_for_bit(self):
        sets = accuracy.ATAN2_SETS
        sample = [pair for name in sets for pair in accuracy.inputs(sets, "atan2", name, 60)]
        edges = [(y, x) for y in (0.0, 1.0, math.inf) for x in (0.0, -0.0, 1.0, -1.0, math.inf, -math.inf)]
        for y, x in sample + edges:
            self.assertEqual(bits(atan2(-y, x)), bits(-atan2(y, x)), (y, x))


class TheAccuracyIsAsMeasured(unittest.TestCase):
    def test_every_set_is_within_one_and_a_half_ulps(self):
        for name in accuracy.ATAN2_SETS:
            with self.subTest(set=name):
                self.assertLessEqual(max(accuracy.measure_atan2(name, SAMPLE)), BOUND)


if __name__ == "__main__":
    unittest.main()
