"""occam's exp, expm1, sinh and cosh, implemented again in Python from the operation sequence the crate promises.

x is reduced to n·ln2/128 + r, with n the integer nearest x·128/ln2. A table of 2^(k/128), for k the low seven bits
of n, and a polynomial in r give e^r·2^(k/128), and the rest of n goes into the exponent. Where that exponent would
leave the normal range, the result is scaled through 2^1009 or 2^-1022. expm1 is a Taylor polynomial near zero and the
same reduction elsewhere. sinh and cosh are Taylor polynomials below 1 and are built from expm1 and exp above it. Each
step is an IEEE double operation on CPython's floats, in the specification's order and grouping, so the results match
the crate's bit for bit.

The table is not copied from anywhere: it is computed here from first principles, in Python's integers.
"""

import math
import struct

from sincos import select, two_sum

MASK = (1 << 64) - 1
MAGNITUDE = 0x7FFF_FFFF_FFFF_FFFF  # every bit but the sign


def bits(x):
    """The bits of a double."""
    return struct.unpack("<Q", struct.pack("<d", x))[0]


def double(word):
    """The double with these bits."""
    return struct.unpack("<d", struct.pack("<Q", word))[0]


INV_LN2_N = double(0x4067_1547_652B_82FE)  # 128/ln 2
NEG_LN2_HI_N = double(0xBF76_2E42_FEFA_0000)  # -ln2/128, its leading part
NEG_LN2_LO_N = double(0xBD0C_F79A_BC9E_3B3A)  # and the rest of it
SHIFT = double(0x4338_0000_0000_0000)  # 1.5·2^52: adding it rounds to an integer, which the low bits then hold
C2 = double(0x3FDF_FFFF_FFFF_FDBD)
C3 = double(0x3FC5_5555_5555_543C)
C4 = double(0x3FA5_5555_CF17_2B91)
C5 = double(0x3F81_1111_67A4_D017)

TINY = 0x3C90_0000_0000_0000  # 2^-54: below this, e^x is 1 + x and expm1 x is x
NEAR = 0x3FA0_0000_0000_0000  # 2^-5: below this, expm1 is a polynomial
LARGE = 0x4080_0000_0000_0000  # 512: from here the scale may leave the normal range
HUGE = 0x4090_0000_0000_0000  # 1024: from here e^x is 0 or infinity

DOWN_1009 = 0x3F10_0000_0000_0000  # 1009 in the exponent field
UP_1022 = 0x3FE0_0000_0000_0000  # 1022 in the exponent field
TWO_1009 = double(0x7F00_0000_0000_0000)
TWO_M1022 = double(0x0010_0000_0000_0000)

T2, T3, T4, T5, T6, T7, T8 = (1.0 / math.factorial(n) for n in range(2, 9))  # each one IEEE division

F18 = 6_402_373_705_728_000.0  # 18!, exact
SINH = [  # 1/3!, 1/5!, ..., 1/19!: each one IEEE division, the last by the IEEE product 18!·19
    1.0 / 6.0,
    1.0 / 120.0,
    1.0 / 5040.0,
    1.0 / 362880.0,
    1.0 / 39916800.0,
    1.0 / 6227020800.0,
    1.0 / 1307674368000.0,
    1.0 / 355687428096000.0,
    1.0 / (F18 * 19.0),
]
COSH = [  # 1/2!, 1/4!, ..., 1/18!: each one IEEE division
    1.0 / 2.0,
    1.0 / 24.0,
    1.0 / 720.0,
    1.0 / 40320.0,
    1.0 / 3628800.0,
    1.0 / 479001600.0,
    1.0 / 87178291200.0,
    1.0 / 20922789888000.0,
    1.0 / F18,
]

STEPS = 128  # entries of the table: 2^(k/128) for k from 0 to 127
GUARD = 200  # bits found below a double's 52 when rounding 2^(k/128)


# The table ----------------------------------------------------------------------------------------------------------


def scaled_power(k):
    """floor(2^(52 + GUARD + k/128)), as seven integer square roots of 2^(128·(52 + GUARD) + k).

    Each root is exact as a floor, because floor(sqrt(floor(y))) = floor(sqrt(y)) for every real y at least 0.
    """
    root = 1 << (STEPS * (52 + GUARD) + k)
    for _ in range(7):
        root = math.isqrt(root)
    return root


def entry(k):
    """(tail, high) for E = 2^(k/128): the double H nearest E, and (E - H)/H rounded to the nearest double."""
    if k == 0:
        return 0.0, 1.0  # E is 1, which a double holds exactly
    low = scaled_power(k)  # E is irrational here, so E·2^(52 + GUARD) lies strictly between low and low + 1
    mantissa = (low + (1 << (GUARD - 1))) >> GUARD  # rounds E·2^52 to nearest: no tie, and the floor loses nothing
    high = math.ldexp(float(mantissa), -52)  # exact: the mantissa has 53 bits
    whole = mantissa << GUARD
    tail = (low - whole) / whole  # an integer quotient, which CPython rounds correctly
    if (low + 1 - whole) / whole != tail:
        raise ArithmeticError(f"2^({k}/128) needs more than {GUARD} guard bits to round its tail")
    return tail, high


def build_table():
    """The 256 words: for each k, the bits of the tail, then the bits of the head less k·2^45."""
    words = []
    for k in range(STEPS):
        tail, high = entry(k)
        words += [bits(tail), (bits(high) - (k << 45)) & MASK]
    return words


TABLE = build_table()


# The steps ----------------------------------------------------------------------------------------------------------


def reduce(x):
    """(tmp, sbits, ki): e^x is near scale·(1 + tmp), scale being the double with bits sbits, and ki holds n."""
    shifted = (INV_LN2_N * x) + SHIFT
    ki = bits(shifted)
    kd = shifted - SHIFT
    r = (x + (kd * NEG_LN2_HI_N)) + (kd * NEG_LN2_LO_N)
    index = ((ki & 0x7F) << 1) & MASK
    tail = double(TABLE[index & 0xFF])
    sbits = (TABLE[(index + 1) & 0xFF] + ((ki << 45) & MASK)) & MASK
    r2 = r * r
    tmp = ((tail + r) + (r2 * (C2 + (r * C3)))) + ((r2 * r2) * (C4 + (r * C5)))
    return tmp, sbits, ki


def combine(tmp, sbits):
    """scale·(1 + tmp), for a scale in the normal range."""
    scale = double(sbits)
    return scale + (scale * tmp)


def above(tmp, sbits):
    """scale·(1 + tmp) for a scale beyond the largest double: formed 2^1009 lower, then multiplied up."""
    return TWO_1009 * combine(tmp, (sbits - DOWN_1009) & MASK)


def below(tmp, sbits):
    """scale·(1 + tmp) for a scale below the normal range: formed 2^1022 higher, then multiplied down.

    A result under 1 there becomes subnormal, so it is first rounded at the subnormal spacing, by adding 1 and taking
    the low part's error along, so that the final multiplication by 2^-1022 rounds only once.
    """
    scale = double((sbits + UP_1022) & MASK)
    y = scale + (scale * tmp)
    lo = (scale - y) + (scale * tmp)
    hi = 1.0 + y
    lo = ((1.0 - hi) + y) + lo
    return TWO_M1022 * select(y < 1.0, (hi + lo) - 1.0, y)


def horner(z, coefficients):
    """The polynomial with these coefficients, lowest power first, at z, by Horner's rule from 0."""
    s = 0.0
    for coefficient in reversed(coefficients):
        s = coefficient + (z * s)
    return s


# The functions ------------------------------------------------------------------------------------------------------


def exp(x):
    """e^x, bit for bit as occam computes it."""
    if x != x:
        return x
    size = bits(x) & MAGNITUDE
    if size >= HUGE:
        return 0.0 if bits(x) >> 63 else math.inf
    if size < TINY:
        return 1.0 + x
    tmp, sbits, ki = reduce(x)
    if size < LARGE:
        return combine(tmp, sbits)
    if ki & 0x8000_0000 == 0:
        return above(tmp, sbits)
    return below(tmp, sbits)


def expm1(x):
    """e^x - 1, bit for bit as occam computes it."""
    size = bits(x) & MAGNITUDE
    if x != x or size < TINY:
        return x
    if size < NEAR:
        p = T6 + (x * (T7 + (x * T8)))
        p = T4 + (x * (T5 + (x * p)))
        p = T2 + (x * (T3 + (x * p)))
        return x + ((x * x) * p)
    if size >= LARGE:
        return exp(x) - 1.0
    tmp, sbits, _ = reduce(x)
    scale = double(sbits)
    hi, lo = two_sum(scale, -1.0)
    return hi + (lo + (scale * tmp))


def sinh_cosh(x):
    """sinh x and cosh x, bit for bit as occam computes them."""
    size = abs(x)
    z = size * size
    em1 = expm1(size)
    e = em1 + 1.0  # at least 1, or infinite, or NaN: never a zero divisor below
    small = size < 1.0
    s = select(small, size + ((size * z) * horner(z, SINH)), 0.5 * (em1 + (em1 / e)))  # infinite e: NaN, replaced
    c = select(small, 1.0 + (z * horner(z, COSH)), 0.5 * (e + (1.0 / e)))
    root = exp(0.5 * size)
    large = (0.5 * root) * root
    s = select(709.0 <= size, large, s)
    c = select(709.0 <= size, large, c)
    return math.copysign(s, x), c


def sinh(x):
    """sinh x, bit for bit as occam computes it."""
    return sinh_cosh(x)[0]


def cosh(x):
    """cosh x, bit for bit as occam computes it."""
    return sinh_cosh(x)[1]
