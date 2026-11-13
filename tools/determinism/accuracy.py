"""How close the separate sin, cos, atan2, exp, expm1, sinh and cosh come to the true values, measured in units in the
last place.

Each reference is computed in Decimal to 130 significant digits from π found here, by Gauss's arctangent formula in
integers: sin and cos by Taylor's series after reducing the argument by π/2 at 480 digits, which leaves over 170 after
the point for an argument as large as 1.8e308, and atan by Gregory's series after halving the angle. e^x is Decimal's
own exponential, which it rounds correctly. e^x - 1, sinh and cosh come from it for |x| of 1 or more, and from their
Taylor series below that, where subtracting from e^x would cancel the leading digits. The error of a result is
|computed - exact| / ulp(exact), with ulp(v) = 2^(floor(log2 |v|) - 52), and never below the subnormal spacing
2^-1074. The inputs come from SplitMix64, seeded per set, so every run measures the same numbers.

`python3 -B tools/determinism/accuracy.py [--count N]` prints, for each function and set of inputs, how many were
measured, the share correctly rounded (within half an ulp), and the largest error.
"""

import argparse
import decimal
import hashlib
import math
import sys
from decimal import Decimal

from atan2 import atan2
from exp import exp, expm1, sinh_cosh
from sincos import sin_cos
from stream import SplitMix64, bits, double

DIGITS = 130  # significant digits of every reference
WIDE = 480  # digits for reducing by π/2: an argument below 2^1024 has at most 309 before the point
PI_DIGITS = 520  # digits of π, more than the widest reduction uses
GUARD = 10  # extra decimal places carried while summing π's series in integers
COUNT = 2000  # inputs per set, by default
LN_LARGEST = 709.78  # just below the logarithm of the largest double, 709.7827: e^x is infinite above it

NARROW = decimal.Context(prec=DIGITS, rounding=decimal.ROUND_HALF_EVEN, Emin=-99999, Emax=99999)
REDUCING = decimal.Context(prec=WIDE, rounding=decimal.ROUND_HALF_EVEN, Emin=-99999, Emax=99999)


def arctan_inverse(n, one):
    """arctan(1/n) times `one`, in integers, by Gregory's series."""
    total, power, index, square = 0, one // n, 0, n * n
    while power:
        term = power // (2 * index + 1)
        total += -term if index % 2 else term
        power //= square
        index += 1
    return total


def gauss_pi(digits):
    """π to `digits` decimal places, by Gauss's π/4 = 12 arctan(1/18) + 8 arctan(1/57) - 5 arctan(1/239)."""
    one = 10 ** (digits + GUARD)
    quarter = 12 * arctan_inverse(18, one) + 8 * arctan_inverse(57, one) - 5 * arctan_inverse(239, one)
    with decimal.localcontext(decimal.Context(prec=digits + 1)):
        return +Decimal(4 * quarter).scaleb(-(digits + GUARD))


PI = gauss_pi(PI_DIGITS)
with decimal.localcontext(decimal.Context(prec=PI_DIGITS + 2)):
    HALF_PI = PI / 2  # exact: halving needs one digit more at most
    TWO_PI = float(PI * 2)


# References ---------------------------------------------------------------------------------------------------------


def reduce(x):
    """(k mod 4, r) with x = k·π/2 + r, k the nearest integer to x/(π/2), and r to 130 digits."""
    with decimal.localcontext(REDUCING):
        exact = Decimal(x)
        k = (exact / HALF_PI).to_integral_value(rounding=decimal.ROUND_HALF_EVEN)
        r = exact - k * HALF_PI
    with decimal.localcontext(NARROW):
        return int(k) % 4, +r


def taylor_sin_cos(r):
    """sin r and cos r by their Taylor series, for |r| up to about π/4, in the current context."""
    z = r * r
    sine, term, index = r, r, 1
    while True:
        term = -term * z / ((index + 1) * (index + 2))
        index += 2
        if sine + term == sine:
            break
        sine += term
    cosine, term, index = Decimal(1), Decimal(1), 0
    while True:
        term = -term * z / ((index + 1) * (index + 2))
        index += 2
        if cosine + term == cosine:
            break
        cosine += term
    return sine, cosine


def reference_sin_cos(x):
    """sin x and cos x for a finite double x, to 130 significant digits."""
    quadrant, r = reduce(x)
    with decimal.localcontext(NARROW):
        sine, cosine = taylor_sin_cos(r)
    minus_sine, minus_cosine = sine.copy_negate(), cosine.copy_negate()
    return [(sine, cosine), (cosine, minus_sine), (minus_sine, minus_cosine), (minus_cosine, sine)][quadrant]


def reference_atan(t):
    """atan t for t >= 0, by halving the angle until t is below 0.05 and then Gregory's series."""
    with decimal.localcontext(NARROW):
        halvings = 0
        while t > Decimal("0.05"):
            t = t / (1 + (1 + t * t).sqrt())
            halvings += 1
        square, power, total, index = t * t, t, t, 0
        while True:
            index += 1
            power = power * square
            term = power / (2 * index + 1)
            step = total - term if index % 2 else total + term
            if step == total:
                break
            total = step
        return total * 2**halvings


def reference_atan2(y, x):
    """atan2(y, x) for finite doubles, not both zero, to 130 significant digits."""
    ay, ax = Decimal(y).copy_abs(), Decimal(x).copy_abs()
    with decimal.localcontext(NARROW):
        angle = HALF_PI - reference_atan(ax / ay) if ay > ax else reference_atan(ay / ax)
        if math.copysign(1.0, x) < 0.0:
            angle = PI - angle
        return -angle if math.copysign(1.0, y) < 0.0 else +angle


def series(first, z, index):
    """The sum of first·z^j·index!/(index + 2j)! over j from 0, in the current context: sinh and cosh's series from
    their term in x^index, with z = x·x, summed until a term no longer changes the total.
    """
    total, term = first, first
    while True:
        term = term * z / ((index + 1) * (index + 2))
        index += 2
        if total + term == total:
            return total
        total += term


def reference_exp(x):
    """e^x for a finite double x, to 130 significant digits."""
    with decimal.localcontext(NARROW):
        return Decimal(x).exp()


def reference_expm1(x):
    """e^x - 1 for a finite double x, to 130 significant digits: below 1, as sinh x + (cosh x - 1), neither of which
    cancels.
    """
    with decimal.localcontext(NARROW):
        exact = Decimal(x)
        if exact.copy_abs() < 1:
            square = exact * exact
            return series(exact, square, 1) + series(square / 2, square, 2)
        return exact.exp() - 1


def reference_sinh_cosh(x):
    """sinh x and cosh x for a finite double x, to 130 significant digits."""
    with decimal.localcontext(NARROW):
        exact = Decimal(x)
        if exact.copy_abs() < 1:
            square = exact * exact
            return series(exact, square, 1), 1 + series(square / 2, square, 2)
        grow = exact.copy_abs().exp()
        shrink = 1 / grow
        sinh = (grow - shrink) / 2
        return (sinh if exact > 0 else -sinh), (grow + shrink) / 2


def ulp(v):
    """The spacing of doubles at a real v: 2^(floor(log2 |v|) - 52), and 2^-1074 among the subnormals."""
    magnitude = v.copy_abs()
    approximate = float(magnitude)
    if approximate < 2.0**-1022:
        return Decimal(2.0**-1074)
    exponent = math.frexp(approximate)[1] - 1
    if Decimal(math.ldexp(1.0, exponent)) > magnitude:
        exponent -= 1
    return Decimal(math.ldexp(1.0, exponent - 52))


def ulps(computed, exact):
    """The error of a computed double, in units in the last place of the exact value."""
    with decimal.localcontext(NARROW):
        return float(abs(Decimal(computed) - exact) / ulp(exact))


# Inputs -------------------------------------------------------------------------------------------------------------


def unit(generator):
    """A double uniform in [0, 1), from the top 53 bits of the next number."""
    return (generator.next_u64() >> 11) * 2.0**-53


def uniform(generator, low, high):
    """A non-zero double uniform in [low, high)."""
    while True:
        x = low + (high - low) * unit(generator)
        if x != 0.0:
            return x


def spread(generator, low, high):
    """A double of either sign with magnitude in [low, high], its binary exponent uniform over that range."""
    first, last = math.frexp(low)[1] - 1, math.frexp(high)[1] - 1
    while True:
        exponent = first + generator.next_u64() % (last - first + 1)
        word = generator.next_u64()
        x = double(((exponent + 1023) << 52) | (word >> 12))
        if low <= x <= high:
            return -x if word & 1 else x


def nearest(value):
    """The double nearest a Decimal: CPython converts through the decimal string, rounding correctly."""
    return float(value)


def offset(x, generator, ulps_away=1024):
    """A positive double x moved by up to `ulps_away` places either way."""
    return double(bits(x) + generator.next_u64() % (2 * ulps_away + 1) - ulps_away)


def near_quarter_turns(generator):
    """A double within 1024 places of the double nearest k·π/2, for k from 1 to 1000."""
    k = 1 + generator.next_u64() % 1000
    with decimal.localcontext(NARROW):
        return offset(nearest(k * HALF_PI), generator)


def pattern_angle(generator):
    """The double nearest i·2π/n, for n from 2 to 720 and i from 0 to n - 1: the angles of a circular pattern."""
    n = 2 + generator.next_u64() % 719
    i = generator.next_u64() % n
    with decimal.localcontext(NARROW):
        return nearest(i * 2 * PI / n)


def small_integer(generator):
    """A non-zero integer from -20 to 20, as a double."""
    magnitude = 1 + generator.next_u64() % 20
    return float(-magnitude if generator.next_u64() & 1 else magnitude)


def near_one_thirty_second(generator):
    """A double of either sign within 1024 places of 2^-5, where expm1 leaves its polynomial for the reduction."""
    x = offset(2.0**-5, generator)
    return -x if generator.next_u64() & 1 else x


def near_axes(generator):
    """A point whose smaller coordinate is between 2^-60 and 2^-30 of the larger, on either axis."""
    large = uniform(generator, -1.0, 1.0)
    small = large * spread(generator, 2.0**-60, 2.0**-30)
    return (small, large) if generator.next_u64() & 1 else (large, small)


def near_diagonal(generator):
    """A point with |y| within 1024 places of |x|, in any quadrant."""
    x = uniform(generator, -1.0, 1.0)
    y = offset(abs(x), generator)
    return (-y if generator.next_u64() & 1 else y), x


SIN_COS_SETS = {
    "uniform": lambda g: uniform(g, -TWO_PI, TWO_PI),
    "near k·π/2": near_quarter_turns,
    "pattern": pattern_angle,
    "tiny": lambda g: spread(g, 2.0**-60, 2.0**-20),
    "large": lambda g: spread(g, 1e3, 1e6),
    "huge": lambda g: spread(g, 1e6, 1e300),
}

ATAN2_SETS = {
    "uniform": lambda g: (uniform(g, -1.0, 1.0), uniform(g, -1.0, 1.0)),
    "integer ratios": lambda g: (small_integer(g), small_integer(g)),
    "near the axes": near_axes,
    "near the diagonal": near_diagonal,
    "huge and tiny": lambda g: (spread(g, 1e-300, 1e300), spread(g, 1e-300, 1e300)),
}

EXP_SETS = {
    "uniform": lambda g: uniform(g, -745.0, LN_LARGEST),
    "near 0": lambda g: spread(g, 2.0**-60, 2.0**-20),
    "subnormal results": lambda g: uniform(g, -745.13, -708.4),
}

EXPM1_SETS = {
    "uniform": lambda g: uniform(g, -40.0, LN_LARGEST),
    "near 0": lambda g: uniform(g, -0.05, 0.05),
    "around 2^-5": near_one_thirty_second,
    "tiny": lambda g: spread(g, 2.0**-60, 2.0**-20),
}

SINH_COSH_SETS = {
    "uniform": lambda g: uniform(g, -710.0, 710.0),
    "within 1": lambda g: uniform(g, -1.0, 1.0),
    "near 0": lambda g: spread(g, 2.0**-60, 1e-4),
}


def seed(label):
    """A set's seed, from its function and name, so adding a set changes no other's inputs."""
    return int.from_bytes(hashlib.blake2b(label.encode("utf-8"), digest_size=8).digest(), "little")


def inputs(sets, function, name, count):
    """The first `count` inputs of one of a function's sets."""
    generator = SplitMix64(seed(f"{function} {name}"))
    return [sets[name](generator) for _ in range(count)]


# Measurement --------------------------------------------------------------------------------------------------------


def measure_sin_cos(name, count):
    """The errors of sin and cos, in ulps, on a set's inputs."""
    errors = {"sin": [], "cos": []}
    for x in inputs(SIN_COS_SETS, "sin_cos", name, count):
        s, c = sin_cos(x)
        exact_sin, exact_cos = reference_sin_cos(x)
        errors["sin"].append(ulps(s, exact_sin))
        errors["cos"].append(ulps(c, exact_cos))
    return errors


def measure_atan2(name, count):
    """The errors of atan2, in ulps, on a set's inputs."""
    return [ulps(atan2(y, x), reference_atan2(y, x)) for y, x in inputs(ATAN2_SETS, "atan2", name, count)]


def measure_exp(name, count):
    """The errors of exp, in ulps, on a set's inputs."""
    return [ulps(exp(x), reference_exp(x)) for x in inputs(EXP_SETS, "exp", name, count)]


def measure_expm1(name, count):
    """The errors of expm1, in ulps, on a set's inputs."""
    return [ulps(expm1(x), reference_expm1(x)) for x in inputs(EXPM1_SETS, "expm1", name, count)]


def measure_sinh_cosh(name, count):
    """The errors of sinh and cosh, in ulps, on a set's inputs."""
    errors = {"sinh": [], "cosh": []}
    for x in inputs(SINH_COSH_SETS, "sinh_cosh", name, count):
        s, c = sinh_cosh(x)
        exact_sinh, exact_cosh = reference_sinh_cosh(x)
        errors["sinh"].append(ulps(s, exact_sinh))
        errors["cosh"].append(ulps(c, exact_cosh))
    return errors


def summary(errors):
    """(count, share correctly rounded, largest error) of a list of errors."""
    return len(errors), sum(error <= 0.5 for error in errors) / len(errors), max(errors)


def rows(count):
    """(function, set, count, share correctly rounded, largest error) for every function and set."""
    found = []
    for name in SIN_COS_SETS:
        errors = measure_sin_cos(name, count)
        found += [(function, name, *summary(errors[function])) for function in ("sin", "cos")]
    for name in ATAN2_SETS:
        found.append(("atan2", name, *summary(measure_atan2(name, count))))
    for name in EXP_SETS:
        found.append(("exp", name, *summary(measure_exp(name, count))))
    for name in EXPM1_SETS:
        found.append(("expm1", name, *summary(measure_expm1(name, count))))
    for name in SINH_COSH_SETS:
        errors = measure_sinh_cosh(name, count)
        found += [(function, name, *summary(errors[function])) for function in ("sinh", "cosh")]
    return found


def main(argv=None):
    """Prints the accuracy table."""
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("--count", type=int, default=COUNT, help=f"inputs per set (default {COUNT})")
    args = parser.parse_args(argv)
    print(f"{'function':<9}{'set':<19}{'count':>7}{'correctly rounded':>19}{'largest error (ulp)':>21}")
    for function, name, count, share, largest in rows(args.count):
        print(f"{function:<9}{name:<19}{count:>7}{share:>19.2%}{largest:>21.4f}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
