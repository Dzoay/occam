"""The separate exp, expm1, sinh and cosh build their table from first principles, keep IEEE's special values and the
symmetries the specification implies, and stay within their measured accuracy. The stream records all four.
"""

import decimal
import math
import unittest
from decimal import Decimal

import accuracy
import stream
from exp import MASK, TABLE, bits, double, exp, expm1, sinh_cosh

SAMPLE = 150  # inputs per accuracy set: enough to catch a wrong step, few enough to stay quick
# Hold on these samples. 150,000 inputs per set reach 0.507 ulp for exp and 0.610 for expm1 (near 0). For sinh and
# cosh they reach 0.797 and 1.083 within 1, 0.500 near 0, and 1.583 on the uniform set, at 709.996: from 709 both square
# e^(|x|/2), which doubles its error. Below 709 the uniform set reaches 1.069 for sinh and 0.997 for cosh.
BOUNDS = {"exp": 0.51, "expm1": 0.65}
HYPERBOLIC_BOUNDS = {"uniform": (1.6, 1.6), "within 1": (0.8, 1.1), "near 0": (0.51, 0.51)}  # sinh's, cosh's

SMALLEST = double(1)  # the smallest subnormal, 2^-1074


def same_bits(test, got, want):
    """Asserts two doubles are the same bit pattern, so a zero's sign counts."""
    test.assertEqual(bits(got), bits(want), f"{got!r} is not {want!r}")


class TheTableIsTwoToTheKOver128(unittest.TestCase):
    def test_it_holds_256_words(self):
        self.assertEqual(len(TABLE), 256)
        self.assertTrue(all(0 <= word <= MASK for word in TABLE))

    def test_its_first_entry_is_no_tail_and_the_bits_of_one(self):
        self.assertEqual((TABLE[0], TABLE[1]), (0, bits(1.0)))

    def test_every_entry_agrees_with_a_power_found_in_decimal(self):
        with decimal.localcontext(decimal.Context(prec=80)):
            for k in range(128):
                power = Decimal(2) ** (Decimal(k) / 128)
                head = float(power)  # CPython rounds a Decimal to the nearest double
                tail = float((power - Decimal(head)) / Decimal(head))
                self.assertEqual(TABLE[2 * k], bits(tail), k)
                self.assertEqual(TABLE[2 * k + 1], (bits(head) - (k << 45)) & MASK, k)


class TheSpecialValuesAreIeees(unittest.TestCase):
    def test_at_a_zero_exp_and_cosh_are_one_and_expm1_and_sinh_keep_its_sign(self):
        for zero in (0.0, -0.0):
            same_bits(self, exp(zero), 1.0)
            same_bits(self, expm1(zero), zero)
            s, c = sinh_cosh(zero)
            same_bits(self, s, zero)
            same_bits(self, c, 1.0)

    def test_nan_gives_nan(self):
        s, c = sinh_cosh(math.nan)
        for got in (exp(math.nan), expm1(math.nan), s, c):
            self.assertTrue(math.isnan(got))

    def test_the_infinities(self):
        same_bits(self, exp(math.inf), math.inf)
        same_bits(self, exp(-math.inf), 0.0)
        same_bits(self, expm1(math.inf), math.inf)
        same_bits(self, expm1(-math.inf), -1.0)
        self.assertEqual(sinh_cosh(math.inf), (math.inf, math.inf))
        self.assertEqual(sinh_cosh(-math.inf), (-math.inf, math.inf))

    def test_below_two_to_the_minus_54_exp_is_one_and_expm1_is_the_argument(self):
        for x in (math.nextafter(2.0**-54, 0.0), 2.0**-60, 1e-300, SMALLEST, -SMALLEST, -(2.0**-55)):
            same_bits(self, exp(x), 1.0)
            same_bits(self, expm1(x), x)

    def test_exp_overflows_between_709_78_and_709_79(self):
        self.assertTrue(math.isfinite(exp(709.78)))
        self.assertTrue(math.isfinite(expm1(709.78)))
        same_bits(self, exp(709.79), math.inf)
        same_bits(self, expm1(709.79), math.inf)
        same_bits(self, exp(1000.0), math.inf)

    def test_exp_reaches_the_smallest_subnormal_and_then_zero(self):
        same_bits(self, exp(-745.1), SMALLEST)
        same_bits(self, exp(-745.2), 0.0)
        same_bits(self, exp(-1000.0), 0.0)

    def test_far_below_zero_expm1_is_minus_one(self):
        for x in (-40.0, -600.0, -1000.0):
            same_bits(self, expm1(x), -1.0)

    def test_sinh_and_cosh_overflow_after_710_47(self):
        for function in (0, 1):
            self.assertTrue(math.isfinite(sinh_cosh(710.47)[function]))
        self.assertEqual(sinh_cosh(711.0), (math.inf, math.inf))
        self.assertEqual(sinh_cosh(-711.0), (-math.inf, math.inf))


class KnownValuesAreWithinAnUlp(unittest.TestCase):
    def assert_within_an_ulp(self, name, got, exact):
        error = accuracy.ulps(got, exact)
        self.assertLessEqual(error, 1.0, f"{name} is {error:.4f} ulp away")

    def test_exp(self):
        for x in (1.0, -1.0, 0.5, 10.0, -10.0, 100.0, 700.0, -700.0, -720.0, -740.0):
            self.assert_within_an_ulp(f"exp({x!r})", exp(x), accuracy.reference_exp(x))

    def test_expm1(self):
        for x in (1e-10, -1e-10, 0.01, 0.03, 2.0**-5, -0.5, 1.0, 2.0, 40.0, -40.0, 600.0):
            self.assert_within_an_ulp(f"expm1({x!r})", expm1(x), accuracy.reference_expm1(x))

    def test_sinh_and_cosh(self):
        for x in (1e-5, 0.1, 0.5, -0.5, 1.0, 2.0, -2.0, 5.0, 30.0, 700.0):
            s, c = sinh_cosh(x)
            exact_s, exact_c = accuracy.reference_sinh_cosh(x)
            self.assert_within_an_ulp(f"sinh({x!r})", s, exact_s)
            self.assert_within_an_ulp(f"cosh({x!r})", c, exact_c)


class TheSymmetriesHoldBitForBit(unittest.TestCase):
    def test_sinh_is_odd_and_cosh_even(self):
        sets = accuracy.SINH_COSH_SETS
        sample = [abs(x) for name in sets for x in accuracy.inputs(sets, "sinh_cosh", name, 60)]
        edges = [1.0, math.nextafter(1.0, 0.0), 709.0, math.nextafter(709.0, 0.0), 710.47, 711.0, math.inf]
        for x in sample + edges:
            s, c = sinh_cosh(x)
            minus_s, minus_c = sinh_cosh(-x)
            same_bits(self, minus_s, -s)
            same_bits(self, minus_c, c)


class TheAccuracyIsAsMeasured(unittest.TestCase):
    def test_every_set_is_within_its_bound(self):
        measured = [("exp", name, BOUNDS["exp"], accuracy.measure_exp(name, SAMPLE)) for name in accuracy.EXP_SETS]
        for name in accuracy.EXPM1_SETS:
            measured.append(("expm1", name, BOUNDS["expm1"], accuracy.measure_expm1(name, SAMPLE)))
        for name in accuracy.SINH_COSH_SETS:
            errors = accuracy.measure_sinh_cosh(name, SAMPLE)
            for function, bound in zip(("sinh", "cosh"), HYPERBOLIC_BOUNDS[name]):
                measured.append((function, name, bound, errors[function]))
        for function, name, bound, errors in measured:
            with self.subTest(function=function, set=name):
                self.assertLessEqual(max(errors), bound)


class TheStreamRecordsThem(unittest.TestCase):
    def test_the_four_come_last_after_the_sixteen_before_them(self):
        self.assertEqual(len(stream.OPERATIONS), 20)
        self.assertEqual(stream.OPERATIONS[-4:], ["exp", "expm1", "sinh", "cosh"])


if __name__ == "__main__":
    unittest.main()
