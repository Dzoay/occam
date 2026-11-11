"""The separate sin and cos keep IEEE's special values and the symmetries the specification implies, reduce large
arguments as a reduction at 480 digits does, and stay within their measured accuracy.
"""

import decimal
import math
import unittest
from decimal import Decimal

import accuracy
import constants
import sincos
from sincos import bits, sin_cos

SAMPLE = 150  # inputs per accuracy set: enough to catch a wrong step, few enough to stay quick
# Holds on these samples, but 30,000 inputs per set reach 0.762 ulp, for sin on the pattern set: near |r| = π/4 the
# cosine kernel carries the rounding of z = r·r into h = z/2, a quarter of an ulp of the result, uncompensated.
BOUND = 0.75
TWO_TO_THE_20 = 1_048_576.0
HARD = 6_381_956_970_095_103 * 2.0**797  # within 2^-60.9 of a multiple of π/2: a hard case from the literature


def moved(x, places):
    """The double `places` representable numbers away from a positive x."""
    return constants.double(bits(x) + places)


def same_bits(test, got, want):
    """Asserts two doubles are the same bit pattern, so a zero's sign counts."""
    test.assertEqual(bits(got), bits(want), f"{got!r} is not {want!r}")


class TheConstantsAreTheRustFiles(unittest.TestCase):
    def test_every_array_holds_the_items_it_declares(self):
        self.assertEqual([len(constants.SIN), len(constants.COS), len(constants.ATAN)], [6, 6, 9])
        self.assertEqual(len(constants.TWO_OVER_PI), 20)

    def test_the_halves_of_pi_over_two_agree_with_the_references_pi(self):
        with decimal.localcontext(accuracy.NARROW):
            self.assertEqual(float(accuracy.HALF_PI), constants.PIO2_HI)
            self.assertEqual(float(accuracy.HALF_PI - Decimal(constants.PIO2_HI)), constants.PIO2_LO)

    def test_the_words_of_two_over_pi_are_its_bits_most_significant_first(self):
        with decimal.localcontext(decimal.Context(prec=500)):
            want = int((2 / accuracy.PI * 2**1280).to_integral_value(rounding=decimal.ROUND_FLOOR))
        self.assertEqual(sincos.TABLE, want)


class TheSpecialValuesAreIeees(unittest.TestCase):
    def test_a_zero_keeps_its_sign_in_sin_and_has_cosine_one(self):
        for zero in (0.0, -0.0):
            s, c = sin_cos(zero)
            same_bits(self, s, zero)
            same_bits(self, c, 1.0)

    def test_nan_and_the_infinities_give_nan(self):
        for x in (math.nan, math.inf, -math.inf):
            s, c = sin_cos(x)
            self.assertTrue(math.isnan(s), x)
            self.assertTrue(math.isnan(c), x)

    def test_below_two_to_the_minus_27_sin_is_the_argument_and_cos_is_one(self):
        for x in (math.nextafter(2.0**-27, 0.0), -(2.0**-28), 2.0**-40, 1e-300, 2.0**-1022, 5e-324, -5e-324):
            s, c = sin_cos(x)
            same_bits(self, s, x)
            same_bits(self, c, 1.0)

    def test_sin_and_cos_are_the_halves_of_sin_cos(self):
        for x in (0.5, -2.0, 1e5, 1e300):
            self.assertEqual((sincos.sin(x), sincos.cos(x)), sin_cos(x))


class TheSymmetriesHoldBitForBit(unittest.TestCase):
    def sample(self):
        """Every accuracy set's first inputs, the edges of the two reductions, and the largest doubles."""
        sets = accuracy.SIN_COS_SETS
        found = [x for name in sets for x in accuracy.inputs(sets, "sin_cos", name, 60)]
        edges = [2.0**-27, TWO_TO_THE_20, moved(TWO_TO_THE_20, 1), HARD, 1e300, constants.double(0x7FEF_FFFF_FFFF_FFFF)]
        return [abs(x) for x in found] + edges

    def test_sin_is_odd_and_cos_even(self):
        for x in self.sample():
            s, c = sin_cos(x)
            minus_s, minus_c = sin_cos(-x)
            same_bits(self, minus_s, -s)
            same_bits(self, minus_c, c)


class TheLargeReductionIsRight(unittest.TestCase):
    def test_it_agrees_with_the_fast_one_either_side_of_two_to_the_20(self):
        near = [moved(TWO_TO_THE_20, places) for places in range(-200, 201)]
        spaced = [TWO_TO_THE_20 * (1.0 + step / 1024.0) for step in range(-40, 41)]
        for x in near + spaced:
            fast, large = sincos.fast_reduction(x), sincos.large_reduction(x)
            self.assertEqual(int(fast[2]) % 4, int(large[2]) % 4, x)
            self.assertLessEqual(abs(fast[0] - large[0]), math.ulp(large[0]), x)
            for got, want in zip(sincos.finish(x, *fast), sincos.finish(x, *large)):
                self.assertLessEqual(abs(got - want), math.ulp(want), x)

    def test_its_reduced_argument_agrees_with_a_reduction_at_480_digits(self):
        huge = accuracy.inputs(accuracy.SIN_COS_SETS, "sin_cos", "huge", 300)
        for x in huge + [HARD, -HARD, moved(TWO_TO_THE_20, 1), constants.double(0x7FEF_FFFF_FFFF_FFFF)]:
            r, rr, q = sincos.large_reduction(x)
            quadrant, exact = accuracy.reduce(x)
            self.assertEqual(int(q) % 4, quadrant, x)
            with decimal.localcontext(accuracy.NARROW):
                error = abs(Decimal(r) + Decimal(rr) - exact)
                self.assertLessEqual(error, exact.copy_abs() * Decimal(2.0**-100) + Decimal(2.0**-135), x)


class TheAccuracyIsAsMeasured(unittest.TestCase):
    def test_every_set_is_within_three_quarters_of_an_ulp(self):
        for name in accuracy.SIN_COS_SETS:
            for function, errors in accuracy.measure_sin_cos(name, SAMPLE).items():
                with self.subTest(function=function, set=name):
                    self.assertLessEqual(max(errors), BOUND)


if __name__ == "__main__":
    unittest.main()
