"""occam's sin and cos, implemented again in Python from the operation sequence the crate promises.

Each step is an IEEE double operation on CPython's floats, in the order and grouping the specification gives, so the
results match the crate's bit for bit and the determinism stream can record them from code that shares nothing with
it. Arguments up to 2^20 are reduced by Cody and Waite's four pieces of π/2; larger ones by Payne and Hanek's method,
done here in Python's integers.
"""

import math
import struct

from constants import COS, INV_PIO2, PIO2_C1, PIO2_C2, PIO2_C3, PIO2_C4, PIO2_HI, PIO2_LO, SIN, TWO_OVER_PI

MAGIC = 6_755_399_441_055_744.0  # 1.5 * 2^52: adding and subtracting it rounds to an integer
LIMIT = 1_048_576.0  # 2^20: the largest argument the fast reduction takes
TINY = 2.0**-27  # below this, sin x is x and cos x is 1 to the last bit
SPLITTER = 134_217_729.0  # 2^27 + 1, for Veltkamp's split
TABLE = sum(word << (64 * (len(TWO_OVER_PI) - 1 - index)) for index, word in enumerate(TWO_OVER_PI))
TABLE_BITS = 64 * len(TWO_OVER_PI)
WINDOW = 192  # bits of 2/π multiplied by the mantissa
KEPT = 117  # bits of the reduced fraction carried into the double-double


def bits(x):
    """The bits of a double."""
    return struct.unpack("<Q", struct.pack("<d", x))[0]


def select(mask, yes, no):
    """`yes` where the comparison held, `no` elsewhere."""
    return yes if mask else no


def two_sum(a, b):
    """The rounded sum and its exact error, by Knuth's branch-free method."""
    s = a + b
    bv = s - a
    av = s - bv
    br = b - bv
    ar = a - av
    return s, ar + br


def fast_two_sum(a, b):
    """The rounded sum and its exact error, for |a| at least |b|."""
    s = a + b
    return s, b - (s - a)


def split(a):
    """Veltkamp's split of a double into two halves of 26 bits or fewer."""
    c = SPLITTER * a
    big = c - a
    hi = c - big
    return hi, a - hi


def two_product(a, b):
    """The rounded product and its exact error, by Dekker's method without fused multiply-add."""
    p = a * b
    ah, al = split(a)
    bh, bl = split(b)
    e1 = p - ah * bh
    e2 = e1 - al * bh
    e3 = e2 - ah * bl
    return p, al * bl - e3


def horner(coefficients, z):
    """The polynomial with these coefficients, lowest power first, at z, by Horner's rule."""
    total = coefficients[-1]
    for coefficient in reversed(coefficients[:-1]):
        total = coefficient + z * total
    return total


def fast_reduction(x):
    """(r, rr, q) with x = q·π/2 + r + rr, by Cody and Waite: valid for |x| up to 2^20."""
    t = x * INV_PIO2
    k = (t + MAGIC) - MAGIC
    hi = x - k * PIO2_C1
    s1, e1 = two_sum(hi, -(k * PIO2_C2))
    s2, e2 = two_sum(s1, -(k * PIO2_C3))
    lo = (e1 + e2) - k * PIO2_C4
    r, rr = two_sum(s2, lo)
    m = (k * 0.25 + MAGIC) - MAGIC
    q = k - 4.0 * m
    return r, rr, q


def large_reduction(x):
    """(r, rr, q) with x = q·π/2 + r + rr, by Payne and Hanek: for a normal or non-finite x."""
    if not math.isfinite(x):
        return math.nan, math.nan, 0.0
    word = bits(abs(x))
    exponent = ((word >> 52) & 0x7FF) - 1075
    mantissa = (word & ((1 << 52) - 1)) | (1 << 52)
    first = max(1, exponent - 1)
    window = (TABLE >> (TABLE_BITS - (first + WINDOW - 1))) & ((1 << WINDOW) - 1)
    product = mantissa * window
    shift = first + WINDOW - 1 - exponent
    quadrant = (product >> shift) & 3
    fraction = product & ((1 << shift) - 1)
    if fraction >= 1 << (shift - 1):
        fraction -= 1 << shift
        quadrant = (quadrant + 1) & 3
    if x < 0.0:
        fraction = -fraction
        quadrant = (4 - quadrant) & 3
    if fraction == 0:
        f_hi = f_lo = 0.0
    else:
        magnitude = abs(fraction)
        length = magnitude.bit_length()
        top = magnitude >> (length - KEPT) if length >= KEPT else magnitude << (KEPT - length)
        scale = length - KEPT - shift
        f_hi = math.ldexp(float(top >> 64), scale + 64)
        f_lo = math.ldexp(float(top & ((1 << 64) - 1)), scale)  # float() of an integer rounds to nearest, ties even
        if fraction < 0:
            f_hi, f_lo = -f_hi, -f_lo
    p, pe = two_product(f_hi, PIO2_HI)
    tail = pe + (f_hi * PIO2_LO + f_lo * PIO2_HI)
    r, rr = fast_two_sum(p, tail)
    return r, rr, select(quadrant == 3, -1.0, float(quadrant))


def reduction(x):
    """(r, rr, q) by whichever reduction the argument's size calls for."""
    if abs(x) <= LIMIT:
        return fast_reduction(x)
    return large_reduction(x)


def finish(x, r, rr, q):
    """sin x and cos x from a reduction of x: the two kernels, then the quadrant, then the tiny arguments."""
    z, zl = two_product(r, r)
    ps = horner(SIN, z)
    sin_r = r + ((r * z) * ps + rr * (1.0 - 0.5 * z))
    pc = horner(COS, z)
    h = 0.5 * z
    w = 1.0 - h
    err = (1.0 - w) - h
    cos_r = w + ((err - 0.5 * zl) + ((z * z) * pc - r * rr))  # half of zl undoes the rounding of z in h
    swap = abs(abs(q) - 1.0) < 0.5
    sin_neg = 1.0 < abs(q - 0.5)
    cos_neg = 1.0 < abs(q + 0.5)
    sv = select(swap, cos_r, sin_r)
    cv = select(swap, sin_r, cos_r)
    s = select(sin_neg, -sv, sv)
    c = select(cos_neg, -cv, cv)
    small = abs(x) < TINY
    return select(small, x, s), select(small, 1.0, c)


def sin_cos(x):
    """sin x and cos x, bit for bit as occam computes them."""
    return finish(x, *reduction(x))


def sin(x):
    """sin x, bit for bit as occam computes it."""
    return sin_cos(x)[0]


def cos(x):
    """cos x, bit for bit as occam computes it."""
    return sin_cos(x)[1]
