"""The fitted coefficients and computed constants, checked against references computed here by other methods.

π comes from the Gauss-Legendre iteration rather than Machin's formula, atan from Euler's series rather than Taylor's,
sin and cos from their Taylor series in x rather than in x², and every double is read back from the committed Rust file
by a decoder written here.
"""

import decimal
import random
import re
import unittest
from decimal import Decimal
from fractions import Fraction

import fit

PI_DIGITS = 520  # 2**1281 has 386 digits, so the table's floor is settled with over 100 to spare
REFERENCE = decimal.Context(prec=50)  # the fits' references: 50 significant digits
POINTS = 4000


def gauss_legendre_pi(digits):
    """π by the Gauss-Legendre iteration of arithmetic and geometric means."""
    with decimal.localcontext(decimal.Context(prec=digits + 10)):
        a, b, t, p = Decimal(1), 1 / Decimal(2).sqrt(), Decimal(1) / 4, Decimal(1)
        while abs(a - b) > Decimal(10) ** -(digits + 5):
            a, b, t, p = (a + b) / 2, (a * b).sqrt(), t - p * ((a - b) / 2) ** 2, 2 * p
        return (a + b) ** 2 / (4 * t)


def euler_arctan(x, digits):
    """arctan(x) for x > 0 by Euler's series, the sum of t_n with t_0 = x/(1+x²) and t_n = t_(n-1) 2n/(2n+1) x²/(1+x²)."""
    with decimal.localcontext(decimal.Context(prec=digits + 10)):
        ratio = x * x / (1 + x * x)
        term, total, n = x / (1 + x * x), Decimal(0), 0
        while term > Decimal(10) ** -(digits + 5):
            total += term
            n += 1
            term = term * 2 * n / (2 * n + 1) * ratio
        return total


def taylor_sin(x):
    term, total, k = x, x, 1
    while abs(term) > Decimal(10) ** -60:
        term = -term * x * x / ((2 * k) * (2 * k + 1))
        total, k = total + term, k + 1
    return total


def taylor_cos(x):
    term, total, k = Decimal(1), Decimal(1), 1
    while abs(term) > Decimal(10) ** -60:
        term = -term * x * x / ((2 * k - 1) * (2 * k))
        total, k = total + term, k + 1
    return total


def from_bits(bits):
    """The exact value of a normal double's encoding."""
    sign, biased, fraction = bits >> 63, (bits >> 52) & 0x7FF, bits & ((1 << 52) - 1)
    if not 0 < biased < 0x7FF:
        raise ValueError(f"{bits:#x} is not a normal double")
    value = Fraction((1 << 52) | fraction) * Fraction(2) ** (biased - 1075)
    return -value if sign else value


def committed():
    """Each constant of the committed Rust file, by name, as the list of its hex words."""
    text = fit.OUT.read_text()
    found = re.findall(r"pub\(crate\) const (\w+): [^=]+= (.*?);", text, re.S)
    return {name: [int(word.replace("_", ""), 16) for word in re.findall(r"0x[0-9A-F_]+", body)] for name, body in found}


def doubles(name):
    return [from_bits(bits) for bits in committed()[name]]


def significant_bits(x):
    """The bits from a dyadic Fraction's first one to its last."""
    numerator, denominator = abs(x.numerator), x.denominator
    if denominator & (denominator - 1):
        raise ValueError(f"{x} is not dyadic")
    return (numerator >> ((numerator & -numerator).bit_length() - 1)).bit_length()


class ExactDoubleTests(unittest.TestCase):
    """The helpers that make doubles from Fractions agree with Python's correctly rounded integer division."""

    def test_rounding_to_nearest_agrees_with_integer_division(self):
        rng = random.Random(1)
        for _ in range(2000):
            numerator, denominator = rng.getrandbits(rng.randint(1, 200)) + 1, rng.getrandbits(rng.randint(1, 200)) + 1
            with self.subTest(numerator=numerator, denominator=denominator):
                self.assertEqual(fit.nearest_double(Fraction(numerator, denominator)), Fraction(numerator / denominator))

    def test_ties_round_to_even(self):
        rng = random.Random(2)
        for _ in range(500):
            mantissa, power = rng.getrandbits(52) | (1 << 52), rng.randint(-200, 200)
            tie = (Fraction(mantissa) + Fraction(1, 2)) * Fraction(2) ** power
            even = mantissa + (mantissa & 1)
            self.assertEqual(fit.nearest_double(tie), Fraction(even) * Fraction(2) ** power)
            self.assertEqual(fit.nearest_double(-tie), -Fraction(even) * Fraction(2) ** power)

    def test_truncation_keeps_the_leading_bits_and_never_rounds_up(self):
        rng = random.Random(3)
        for _ in range(500):
            x = Fraction(rng.getrandbits(300) + 1, rng.getrandbits(250) + 1) * rng.choice((1, -1))
            kept = fit.truncated(x, 33)
            self.assertLessEqual(significant_bits(kept), 33)
            self.assertLessEqual(abs(kept), abs(x))
            self.assertLess(abs(x) - abs(kept), Fraction(2) ** (fit.exponent(x) - 32))

    def test_the_encoding_reads_back(self):
        rng = random.Random(4)
        for _ in range(500):
            x = fit.nearest_double(Fraction(rng.getrandbits(80) + 1, rng.getrandbits(80) + 1) * rng.choice((1, -1)))
            self.assertEqual(from_bits(fit.double_bits(x)), x)


class ConstantTests(unittest.TestCase):
    """The constants match those computed from π by the Gauss-Legendre iteration and atan(1/2) by Euler's series."""

    @classmethod
    def setUpClass(cls):
        cls.pi = gauss_legendre_pi(PI_DIGITS)
        cls.atan_half = euler_arctan(Decimal(1) / 2, PI_DIGITS)
        cls.committed = committed()

    def test_machin_and_gauss_legendre_agree_to_450_digits(self):
        """Far beyond the 120 digits the plan asks for: the generator's bounds hold π and are that close together."""
        low, high = fit.pi_bounds()
        self.assertLess(high - low, Fraction(1, 10**450))
        self.assertTrue(low < Fraction(self.pi) < high)

    def test_taylor_and_euler_agree_on_atan_of_a_half_to_450_digits(self):
        low, high = fit.arctan_inverse(2, fit.PI_BITS)
        self.assertLess(high - low, Fraction(1, 10**450))
        self.assertTrue(low < Fraction(self.atan_half) < high)

    def test_every_constant_matches_the_independent_pi(self):
        expected = fit.reduction_constants(Fraction(self.pi), Fraction(self.atan_half))
        for name, value in expected.items():
            with self.subTest(name=name):
                self.assertEqual(self.committed[name], [fit.double_bits(value)])

    def test_the_two_over_pi_table_is_the_floor_of_two_to_the_1280_times_two_over_pi(self):
        with decimal.localcontext(decimal.Context(prec=PI_DIGITS + 10)):
            table = int((Decimal(2) ** 1281 / self.pi).to_integral_value(decimal.ROUND_FLOOR))
        words = self.committed["TWO_OVER_PI"]
        self.assertEqual(len(words), 20)
        self.assertEqual(sum(word << (64 * (19 - i)) for i, word in enumerate(words)), table)

    def test_the_pieces_of_pi_over_two_have_33_bits_and_sum_to_it(self):
        pieces = [from_bits(self.committed[f"PIO2_C{i}"][0]) for i in range(1, 5)]
        for piece in pieces[:3]:
            self.assertLessEqual(significant_bits(piece), 33)
            self.assertLessEqual(significant_bits((2**20 - 1) * piece), 53)
        self.assertLess(abs(sum(pieces) - Fraction(self.pi) / 2), Fraction(2) ** -150)

    def test_each_low_part_is_the_rest_of_its_value_rounded(self):
        pi, atan_half = Fraction(self.pi), Fraction(self.atan_half)
        three_quarters = from_bits(self.committed["THREE_PIO4"][0])
        self.assertLessEqual(abs(3 * pi / 4 - three_quarters), Fraction(2) ** (fit.exponent(three_quarters) - 53))
        for name, value in (("PIO2", pi / 2), ("PIO4", pi / 4), ("PI", pi)):
            with self.subTest(name=name):
                high, low = from_bits(self.committed[f"{name}_HI"][0]), from_bits(self.committed[f"{name}_LO"][0])
                self.assertLessEqual(abs(value - high), Fraction(2) ** (fit.exponent(high) - 53))
                self.assertLessEqual(abs(value - high - low), Fraction(2) ** (fit.exponent(low) - 53))
        high, low = from_bits(self.committed["ATAN_HALF_HI"][0]), from_bits(self.committed["ATAN_HALF_LO"][0])
        self.assertLess(abs(atan_half - high - low), Fraction(2) ** -105)


class FitTests(unittest.TestCase):
    """Each fit, with its coefficients as the committed doubles, is within its target on a dense grid."""

    @classmethod
    def setUpClass(cls):
        cls.fits = {result.kernel.name: result for result in fit.build().fits}

    def worst(self, name, reference, approximate):
        """The largest relative error on POINTS points evenly spread in x and at the fit's extrema."""
        result = self.fits[name]
        coefficients = [Decimal(x.numerator) / Decimal(x.denominator) for x in doubles(name)]
        bound = Decimal(result.kernel.bound.numerator) / Decimal(result.kernel.bound.denominator)
        with decimal.localcontext(REFERENCE):
            xs = [bound * i / POINTS for i in range(1, POINTS + 1)] + [z.sqrt() for z in result.extrema]
            worst = Decimal(0)
            for x in xs:
                if x == 0:
                    continue
                z = x * x
                p = sum((c * z**k for k, c in enumerate(coefficients)), Decimal(0))
                exact = reference(x)
                worst = max(worst, abs((approximate(x, z, p) - exact) / exact))
        return result, worst

    def assert_within_target(self, name, reference, approximate):
        result, worst = self.worst(name, reference, approximate)
        self.assertLessEqual(worst, result.kernel.limit())
        # The generator's own measure of the rounded error is the largest error, found at one of its extrema.
        self.assertLess(abs(worst - result.rounded), result.rounded * Decimal("1e-6"))
        self.assertLessEqual(result.levelled, result.rounded)

    def test_sin_is_within_its_target(self):
        self.assert_within_target("SIN", taylor_sin, lambda x, z, p: x + x * z * p)

    def test_cos_is_within_its_target(self):
        self.assert_within_target("COS", taylor_cos, lambda x, z, p: 1 - z / 2 + z * z * p)

    def test_atan_is_within_its_target(self):
        self.assert_within_target("ATAN", lambda x: euler_arctan(x, 50), lambda x, z, p: x + x * z * p)

    def test_the_targets_and_sizes_are_the_ones_chosen(self):
        chosen = {name: (result.kernel.target, len(result.coefficients)) for name, result in self.fits.items()}
        self.assertEqual(chosen, {"SIN": (Fraction("57.5"), 6), "COS": (58, 6), "ATAN": (60, 9)})

    def test_atan_has_the_fewest_coefficients_that_can_reach_its_target(self):
        """The exact minimax error bounds any rounded fit from below, so one coefficient fewer cannot reach it."""
        fewer = len(self.fits["ATAN"].coefficients) - 1
        self.assertGreater(fit.Problem(fit.ATAN).remez(fewer, {})[1], fit.ATAN.limit())

    def test_a_second_fit_gives_the_same_coefficients(self):
        self.assertEqual(fit.fit(fit.SIN, 6), self.fits["SIN"])


class GeneratedFileTests(unittest.TestCase):
    def test_the_committed_file_is_up_to_date(self):
        self.assertEqual(fit.OUT.read_text(), fit.render(fit.build()))
        self.assertEqual(fit.main(["--check"]), 0)

    def test_the_file_starts_with_its_source_line_and_keeps_to_120_columns(self):
        text = fit.render(fit.build())
        self.assertTrue(text.startswith("//! Source: original\n"))
        self.assertTrue(all(len(line) <= 120 for line in text.splitlines()))

    def test_every_constant_is_documented(self):
        lines = fit.render(fit.build()).splitlines()
        for i, line in enumerate(lines):
            if line.startswith("pub(crate) const"):
                with self.subTest(line=line):
                    self.assertTrue(lines[i - 1].startswith("/// "))


if __name__ == "__main__":
    unittest.main()
