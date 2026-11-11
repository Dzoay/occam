#!/usr/bin/env python3
"""Fits the polynomials and computes the constants of occam's own sin, cos and atan2, and writes them as Rust.

Everything here comes from mathematics and published methods, never from another library's tables: π by Machin's
formula in integers, π/2 split for Cody and Waite's reduction, the bits of 2/π for Payne and Hanek's, and each
polynomial by Remez's exchange algorithm, weighted so that the error it levels is relative. The coefficients are then
rounded to doubles one at a time, lowest power first, and the ones above are refitted around each, so the rounding
costs little.

π is held between two bounds, and every constant is computed from both: they must agree, so each is right. The fits run
in Decimal at 60 digits, which leaves about 40 to spare over errors near 1e-18.

`python3 -B tools/coefficients/fit.py` writes crates/occam-math/src/trig/constants.rs and prints what each fit reached.
With --check it writes nothing and exits non-zero if that file is out of date.
"""

import argparse
import dataclasses
import decimal
import functools
import math
import pathlib
import sys
import textwrap
from collections.abc import Callable
from decimal import Decimal
from fractions import Fraction

ROOT = pathlib.Path(__file__).resolve().parents[2]
OUT = ROOT / "crates" / "occam-math" / "src" / "trig" / "constants.rs"

PI_BITS = 1600  # binary places of π: the 1280 bits of 2/π need about 1290
TABLE_WORDS = 20  # 64-bit words of 2/π after the binary point
SPLIT_BITS = 33  # each of Cody and Waite's first three pieces of π/2, so k times one is exact for k below 2**20
DIGITS = 60  # Decimal digits for the fits
GRID = 400  # points, uniform in x, on which each Remez step scans the error
TOLERANCE = Decimal("1e-9")  # Remez stops when the largest error is within this, relatively, of the levelled one
CONTEXT = decimal.Context(prec=DIGITS, rounding=decimal.ROUND_HALF_EVEN, Emin=-99999, Emax=99999)

with decimal.localcontext(CONTEXT):
    GOLDEN = (3 - Decimal(5).sqrt()) / 2


def in_context(function):
    """Runs `function` with the fits' Decimal context, whatever the caller's."""

    @functools.wraps(function)
    def wrapped(*args, **kwargs):
        with decimal.localcontext(CONTEXT):
            return function(*args, **kwargs)

    return wrapped


# Exact doubles ------------------------------------------------------------------------------------------------------


def exponent(x):
    """The e with 2**e <= |x| < 2**(e + 1), for a non-zero Fraction x."""
    numerator, denominator = abs(x.numerator), x.denominator
    e = numerator.bit_length() - denominator.bit_length()
    if (numerator << max(-e, 0)) < (denominator << max(e, 0)):
        e -= 1
    return e


def _scaled(x, power):
    """|x| * 2**power as a numerator and denominator."""
    numerator, denominator = abs(x.numerator), x.denominator
    if power >= 0:
        return numerator << power, denominator
    return numerator, denominator << -power


def _signed(x, magnitude):
    return magnitude if x > 0 else -magnitude


def nearest_double(x):
    """x rounded to the nearest double, ties to even, as an exact Fraction. Only normal doubles are needed here."""
    x = Fraction(x)
    if x == 0:
        return Fraction(0)
    e = exponent(x)
    numerator, denominator = _scaled(x, 52 - e)
    quotient, remainder = divmod(numerator, denominator)
    if 2 * remainder > denominator or (2 * remainder == denominator and quotient & 1):
        quotient += 1
    if not -1022 <= e + (quotient >> 53) <= 1023:
        raise ValueError(f"{x} is outside the normal doubles")
    return _signed(x, Fraction(quotient) * Fraction(2) ** (e - 52))


def truncated(x, bits):
    """x truncated toward zero to `bits` significant bits, as an exact Fraction."""
    x = Fraction(x)
    e = exponent(x)
    numerator, denominator = _scaled(x, bits - 1 - e)
    return _signed(x, Fraction(numerator // denominator) * Fraction(2) ** (e - bits + 1))


def double_bits(x):
    """The binary64 encoding of x, which must be zero or a normal double exactly."""
    if x == 0:
        return 0
    e = exponent(x)
    mantissa, remainder = divmod(*_scaled(x, 52 - e))
    if remainder or not -1022 <= e <= 1023:
        raise ValueError(f"{x} is not a normal double")
    sign = 1 << 63 if x < 0 else 0
    return sign | (e + 1023) << 52 | (mantissa - (1 << 52))


# π and its constants ------------------------------------------------------------------------------------------------


def arctan_inverse(n, bits):
    """Bounds on arctan(1/n), for an integer n >= 2, from its Taylor series in integers with `bits` binary places.

    Each floored power is at most 4/3 of a unit low, each term at most 7/3, and the tail left off under 2, so the sum
    is within 3 units for each term taken."""
    power, square, total, k = (1 << bits) // n, n * n, 0, 0
    while power:
        term = power // (2 * k + 1)
        total += -term if k & 1 else term
        power //= square
        k += 1
    error, scale = 3 * (k + 1), 1 << bits
    return Fraction(total - error, scale), Fraction(total + error, scale)


def pi_bounds(bits=PI_BITS):
    """Bounds low < π < high, a few units of 2**-bits apart, by Machin's π = 16 atan(1/5) - 4 atan(1/239)."""
    fifth, inverse_239 = arctan_inverse(5, bits), arctan_inverse(239, bits)
    return 16 * fifth[0] - 4 * inverse_239[1], 16 * fifth[1] - 4 * inverse_239[0]


def reduction_constants(pi, atan_half):
    """The reductions' constants, keyed by their Rust names, as exact doubles computed from π and atan(1/2)."""
    half = pi / 2
    c1 = truncated(half, SPLIT_BITS)
    c2 = truncated(half - c1, SPLIT_BITS)
    c3 = truncated(half - c1 - c2, SPLIT_BITS)
    constants = {"PIO2_C1": c1, "PIO2_C2": c2, "PIO2_C3": c3, "PIO2_C4": nearest_double(half - c1 - c2 - c3)}
    constants["INV_PIO2"] = nearest_double(2 / pi)
    for name, value in (("PIO2", half), ("PIO4", pi / 4), ("PI", pi)):
        constants |= double_double(name, value)
    constants["THREE_PIO4"] = nearest_double(3 * pi / 4)
    return constants | double_double("ATAN_HALF", atan_half)


def double_double(name, value):
    """value as a pair of doubles, the high part rounded to nearest and the low part the rest rounded to nearest."""
    high = nearest_double(value)
    return {f"{name}_HI": high, f"{name}_LO": nearest_double(value - high)}


def two_over_pi_words(pi):
    """The first 64 * TABLE_WORDS bits of 2/π after the binary point, as 64-bit words, most significant first."""
    bits = 64 * TABLE_WORDS
    table = (pi.denominator << (bits + 1)) // pi.numerator
    return [(table >> (64 * (TABLE_WORDS - 1 - i))) & ((1 << 64) - 1) for i in range(TABLE_WORDS)]


# Remez's exchange ---------------------------------------------------------------------------------------------------


def _sin_taylor(k):
    return Fraction((-1) ** k, math.factorial(2 * k + 1))


def _cos_taylor(k):
    return Fraction((-1) ** k, math.factorial(2 * k))


def _atan_taylor(k):
    return Fraction((-1) ** k, 2 * k + 1)


@dataclasses.dataclass(frozen=True)
class Kernel:
    """A function on |x| <= bound written as x * g(z), or as g(z) alone for cos, with z = x*x.

    g's first `exact` Taylor terms in z are kept as they are, and the rest, F(z) = (g(z) - head(z)) / z**exact, is
    replaced by a polynomial p. The relative error is then z**exact * (p - F) / g, which Remez levels with the weight
    z**exact / g. The rounded fit's error must be within 2**-target."""

    name: str
    variable: str
    bound: Fraction
    taylor: Callable[[int], Fraction]
    exact: int
    target: Fraction
    sketch: str

    def limit(self):
        """2**-target in Decimal."""
        with decimal.localcontext(CONTEXT):
            return Decimal(2) ** -to_decimal(self.target)


SIN = Kernel(
    name="SIN",
    variable="x",
    bound=Fraction("0.786"),
    taylor=_sin_taylor,
    exact=1,
    # Six coefficients level at 2^-57.92 here, so 2^-58 is out of reach; seven would reach 2^-63 once rounded.
    target=Fraction("57.5"),
    sketch="sin(x) ≈ x + x·z·(S0 + S1·z + … + S{n}·z^{n}), z = x²",
)
COS = Kernel(
    name="COS",
    variable="x",
    bound=Fraction("0.786"),
    taylor=_cos_taylor,
    exact=2,
    target=Fraction(58),
    sketch="cos(x) ≈ 1 − z/2 + z²·(C0 + C1·z + … + C{n}·z^{n}), z = x²",
)
ATAN = Kernel(
    name="ATAN",
    variable="u",
    bound=Fraction("0.2501"),
    taylor=_atan_taylor,
    exact=1,
    target=Fraction(60),
    sketch="atan(u) ≈ u + u·z·(A0 + A1·z + … + A{n}·z^{n}), z = u²",
)


def to_decimal(x):
    """A Fraction rounded to the current Decimal context."""
    return Decimal(x.numerator) / Decimal(x.denominator)


def horner(coefficients, z):
    """The polynomial with these coefficients, lowest power first, at z."""
    total = Decimal(0)
    for coefficient in reversed(coefficients):
        total = total * z + coefficient
    return total


def solve(rows):
    """The solution of a square linear system given as augmented rows, by Gaussian elimination with partial pivoting."""
    rows = [list(row) for row in rows]
    size = len(rows)
    for column in range(size):
        pivot = max(range(column, size), key=lambda r: abs(rows[r][column]))
        rows[column], rows[pivot] = rows[pivot], rows[column]
        for r in range(column + 1, size):
            factor = rows[r][column] / rows[column][column]
            rows[r] = [a - factor * b for a, b in zip(rows[r], rows[column])]
    solution = [Decimal(0)] * size
    for r in reversed(range(size)):
        known = sum((rows[r][c] * solution[c] for c in range(r + 1, size)), Decimal(0))
        solution[r] = (rows[r][size] - known) / rows[r][r]
    return solution


class Problem:
    """A kernel's fit in Decimal: g's Taylor series to 60 digits, and the error of a polynomial on a grid of z."""

    @in_context
    def __init__(self, kernel):
        self.kernel = kernel
        self.top = to_decimal(kernel.bound**2)
        # Taylor terms up to the first that is negligible at the top of the interval: each series alternates and its
        # terms shrink, so what is left off is smaller still.
        terms, smallest = [], Fraction(1, 10 ** (DIGITS + 3))
        while len(terms) <= kernel.exact or abs(terms[-1]) * kernel.bound ** (2 * len(terms) - 2) >= smallest:
            terms.append(kernel.taylor(len(terms)))
        self.g = [to_decimal(term) for term in terms]
        self.f = self.g[kernel.exact :]
        zs = (self.top * i * i / GRID**2 for i in range(1, GRID + 1))
        self.grid = [(z, horner(self.f, z), self.weight(z)) for z in zs]

    def weight(self, z):
        return z**self.kernel.exact / horner(self.g, z)

    def error(self, coefficients, z):
        """The relative error of the polynomial with these coefficients at z, positive where it is too large."""
        return self.weight(z) * (horner(coefficients, z) - horner(self.f, z))

    def peak(self, coefficients, sign, low, high):
        """Where sign * error is largest on [low, high], by golden-section search, with that error."""

        def height(z):
            return sign * self.error(coefficients, z)

        c, d = low + GOLDEN * (high - low), high - GOLDEN * (high - low)
        hc, hd = height(c), height(d)
        for _ in range(48):
            if hc < hd:
                low, c, hc = c, d, hd
                d = high - GOLDEN * (high - low)
                hd = height(d)
            else:
                high, d, hd = d, c, hc
                c = low + GOLDEN * (high - low)
                hc = height(c)
        candidates = [(hc, c), (hd, d), (height(low), low), (height(high), high)]
        best, z = max(candidates)
        return z, sign * best

    @in_context
    def extrema(self, coefficients):
        """The error's alternating extrema: the largest of each run of one sign on the grid, refined between its
        neighbours, as (z, error) pairs."""
        errors = [w * (horner(coefficients, z) - f) for z, f, w in self.grid]
        runs = []
        for i, e in enumerate(errors):
            if runs and (e >= 0) == (errors[runs[-1]] >= 0):
                if abs(e) > abs(errors[runs[-1]]):
                    runs[-1] = i
            else:
                runs.append(i)
        zs = [Decimal(0)] + [z for z, _, _ in self.grid]
        return [self.peak(coefficients, 1 if errors[i] >= 0 else -1, zs[i], zs[min(i + 2, GRID)]) for i in runs]

    def level(self, fixed, free, reference):
        """The coefficients whose error is ±h, alternating, at the reference points, and h."""
        size = len(fixed) + len(free)
        rows = []
        for i, z in enumerate(reference):
            powers = [z**j for j in range(size)]
            known = sum((fixed[j] * powers[j] for j in fixed), Decimal(0))
            sign = 1 if i % 2 == 0 else -1
            rows.append([powers[j] for j in free] + [-sign / self.weight(z), horner(self.f, z) - known])
        solution = solve(rows)
        coefficients = [fixed[j] if j in fixed else solution[free.index(j)] for j in range(size)]
        return coefficients, solution[-1]

    @in_context
    def remez(self, size, fixed):
        """The minimax coefficients of a polynomial of `size` terms, holding those in `fixed` (index to value), with
        the error's levelled size and its extrema at the end."""
        free = [j for j in range(size) if j not in fixed]
        if not free:
            coefficients = [fixed[j] for j in range(size)]
            extrema = self.extrema(coefficients)
            return coefficients, max(abs(e) for _, e in extrema), extrema
        count = len(free) + 1
        reference = [self.top * (i + 1) ** 2 / count**2 for i in range(count)]
        for _ in range(60):
            coefficients, level = self.level(fixed, free, reference)
            extrema = self.extrema(coefficients)
            if max(abs(e) for _, e in extrema) <= abs(level) * (1 + TOLERANCE):
                return coefficients, abs(level), extrema
            reference = [z for z, _ in exchange(extrema, count)]
        raise RuntimeError(f"Remez's exchange did not converge for {self.kernel.name} with {size} terms")


def exchange(extrema, count):
    """The `count` alternating extrema to level next: drop whichever end is smaller until that many are left, which
    always keeps the largest."""
    if len(extrema) < count:
        raise RuntimeError(f"the error alternates {len(extrema)} times, and Remez needs {count}")
    extrema = list(extrema)
    while len(extrema) > count:
        extrema.pop(0 if abs(extrema[0][1]) < abs(extrema[-1][1]) else -1)
    return extrema


@dataclasses.dataclass(frozen=True)
class Fit:
    """A kernel's coefficients as exact doubles, lowest power first, with the levelled error of the exact minimax
    polynomial, the error once rounded, and the points where the errors peak, in z."""

    kernel: Kernel
    coefficients: tuple
    levelled: Decimal
    rounded: Decimal
    extrema: tuple


@in_context
def fit(kernel, size, problem=None):
    """The minimax fit of `size` coefficients, rounded to doubles one at a time with those above refitted each time."""
    problem = problem or Problem(kernel)
    coefficients, levelled, first = problem.remez(size, {})
    exact, fixed = [], {}
    for j in range(size):
        if j:
            coefficients = problem.remez(size, fixed)[0]
        exact.append(nearest_double(Fraction(coefficients[j])))
        fixed[j] = to_decimal(exact[j])
    _, rounded, last = problem.remez(size, fixed)
    extrema = tuple(sorted({z for z, _ in first + last}))
    return Fit(kernel, tuple(exact), levelled, rounded, extrema)


@in_context
def smallest_fit(kernel):
    """The fit with the fewest coefficients whose error, once rounded, is within the kernel's target."""
    problem, size = Problem(kernel), 1
    while True:
        if problem.remez(size, {})[1] <= kernel.limit():
            result = fit(kernel, size, problem)
            if result.rounded <= kernel.limit():
                return result
        size += 1


# The Rust file ------------------------------------------------------------------------------------------------------


@dataclasses.dataclass(frozen=True)
class Build:
    """Everything the Rust file holds."""

    constants: dict
    split_error: Fraction
    two_over_pi: list
    fits: tuple


@functools.cache
def build():
    """Computes the constants from both bounds on π, checks they agree, and fits the three kernels."""
    low, high = pi_bounds()
    atan_low, atan_high = arctan_inverse(2, PI_BITS)
    constants = reduction_constants(low, atan_low)
    two_over_pi = two_over_pi_words(low)
    if constants != reduction_constants(high, atan_high) or two_over_pi != two_over_pi_words(high):
        raise RuntimeError("π is not known closely enough to round its constants: raise PI_BITS")
    pieces = sum(constants[f"PIO2_C{i}"] for i in range(1, 5))
    split_error = max(abs(pieces - low / 2), abs(pieces - high / 2))
    fits = (fit(SIN, 6), fit(COS, 6), smallest_fit(ATAN))
    for result in fits:
        if result.rounded > result.kernel.limit():
            raise RuntimeError(f"{result.kernel.name} misses its target: {power_of_two(result.rounded)}")
    return Build(constants, split_error, two_over_pi, fits)


HEADER = """\
//! Source: original
//! The owned trigonometry's constants, fitted and computed by `tools/coefficients/fit.py`. Generated: do not edit; run
//! `python3 -B tools/coefficients/fit.py` to regenerate it."""

DOUBLE_DOUBLES = (("PIO2", "π/2"), ("PIO4", "π/4"), ("PI", "π"), ("ATAN_HALF", "atan(1/2)"))


@in_context
def power_of_two(error):
    """The error as 2^-x, with x rounded down to a tenth so that the power bounds the error."""
    error = to_decimal(Fraction(error))
    tenths = (-10 * error.ln() / Decimal(2).ln()).to_integral_value(decimal.ROUND_FLOOR)
    return f"2^-{tenths.scaleb(-1)}"


def decimal_text(x):
    """An exact Fraction to 20 significant digits, in scientific form below a thousandth."""
    with decimal.localcontext(decimal.Context(prec=20)):
        value = Decimal(x.numerator) / Decimal(x.denominator)
        return format(value, ".19e") if 0 < abs(x) < Fraction(1, 1000) else str(value)


def hex_literal(bits):
    """A 64-bit word as a Rust hex literal grouped in fours."""
    text = f"{bits:016X}"
    return "0x" + "_".join(text[i : i + 4] for i in range(0, 16, 4))


def scalar(name, value, text):
    """A documented f64 constant; `{}` in the text becomes its decimal value."""
    doc = textwrap.wrap(text.format(decimal_text(value)), width=116, initial_indent="/// ", subsequent_indent="/// ")
    return [*doc, f"pub(crate) const {name}: f64 = f64::from_bits({hex_literal(double_bits(value))});"]


def array(name, kind, items, text, notes=None):
    """A documented array constant, one item to a line as rustfmt leaves it, each with an optional trailing note."""
    doc = textwrap.wrap(text, width=116, initial_indent="/// ", subsequent_indent="/// ")
    lines = [f"    {item}," + (f" // {note}" if note else "") for item, note in zip(items, notes or [None] * len(items))]
    return [*doc, f"pub(crate) const {name}: [{kind}; {len(items)}] = [", *lines, "];"]


def render(built):
    """The Rust file's text."""
    c = built.constants
    split = power_of_two(built.split_error)
    blocks = [
        [HEADER],
        scalar(
            "PIO2_C1",
            c["PIO2_C1"],
            "π/2 truncated to 33 significant bits, {}: the first of Cody and Waite's four pieces of π/2. The first"
            " three have 33 bits each, so k times one is exact for k below 2^20.",
        ),
        scalar("PIO2_C2", c["PIO2_C2"], "The next 33 significant bits of π/2, {}: the second piece."),
        scalar("PIO2_C3", c["PIO2_C3"], "The next 33 significant bits of π/2, {}: the third piece."),
        scalar(
            "PIO2_C4",
            c["PIO2_C4"],
            f"The rest of π/2 rounded to the nearest double, {{}}: the four pieces sum to within {split} of π/2.",
        ),
        scalar("INV_PIO2", c["INV_PIO2"], "2/π rounded to the nearest double, {}."),
    ]
    for name, what in DOUBLE_DOUBLES:
        blocks.append(scalar(f"{name}_HI", c[f"{name}_HI"], f"{what} rounded to the nearest double, {{}}."))
        low = f"{what} less `{name}_HI`, rounded to the nearest double, {{}}."
        blocks.append(scalar(f"{name}_LO", c[f"{name}_LO"], low))
        if name == "PI":
            blocks.append(scalar("THREE_PIO4", c["THREE_PIO4"], "3π/4 rounded to the nearest double, {}."))
    words = [hex_literal(word) for word in built.two_over_pi]
    blocks.append(
        array(
            "TWO_OVER_PI",
            "u64",
            words,
            f"The first {64 * TABLE_WORDS} bits of 2/π after the binary point, for Payne and Hanek's reduction. Word 0"
            " holds bits 1 to 64, bit 1 in its most significant place.",
        )
    )
    for result in built.fits:
        kernel = result.kernel
        items = [f"f64::from_bits({hex_literal(double_bits(x))})" for x in result.coefficients]
        blocks.append(
            array(
                kernel.name,
                "f64",
                items,
                f"Remez's minimax fit for {kernel.sketch.format(n=len(items) - 1)}, lowest power first: within"
                f" {power_of_two(result.rounded)} relative for |{kernel.variable}| ≤ {decimal_text(kernel.bound)} as"
                f" doubles, and {power_of_two(result.levelled)} before rounding.",
                [decimal_text(x) for x in result.coefficients],
            )
        )
    text = "\n\n".join("\n".join(block) for block in blocks) + "\n"
    if any(len(line) > 120 for line in text.splitlines()):
        raise RuntimeError("a line of the Rust file is over 120 characters")
    return text


def summary(built):
    """One line per fit: its size, levelled error and rounded error."""
    return [
        f"{r.kernel.name:5} {len(r.coefficients)} coefficients: levelled {power_of_two(r.levelled)},"
        f" rounded {power_of_two(r.rounded)}, target 2^-{float(r.kernel.target):g}"
        for r in built.fits
    ]


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--check", action="store_true", help="write nothing; exit non-zero if the Rust file is stale")
    args = parser.parse_args(argv)
    built = build()
    text = render(built)
    if args.check:
        if not OUT.exists() or OUT.read_text() != text:
            print(f"{OUT.relative_to(ROOT)} is out of date: run python3 -B tools/coefficients/fit.py", file=sys.stderr)
            return 1
        return 0
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(text)
    print("\n".join(summary(built)))
    return 0


if __name__ == "__main__":
    sys.exit(main())
