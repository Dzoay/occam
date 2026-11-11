"""occam's atan2, implemented again in Python from the operation sequence the crate promises.

The smaller of |y| and |x| is divided by the larger, the quotient is moved near zero by subtracting one of three
anchors (0, atan(1/2) or π/4), and a polynomial in what is left gives the angle, which the octant then places. Each
step is an IEEE double operation on CPython's floats, in the specification's order, so the result matches the crate's
bit for bit. C99's special cases come out of the same steps and two final selections.
"""

import math

from constants import ATAN, ATAN_HALF_HI, ATAN_HALF_LO, PI_HI, PI_LO, PIO2_HI, PIO2_LO, PIO4_HI, PIO4_LO, THREE_PIO4_HI
from sincos import horner, select, two_sum

TINY_DIVISOR = 2.0**-1000  # below this the pair is scaled up, so the quotient keeps its bits
HUGE_DIVISOR = 2.0**1021  # from here the pair is halved, so the denominator cannot overflow


def div(a, b):
    """IEEE division, which Python refuses for a zero divisor."""
    if b == 0.0:
        if a != a or a == 0.0:
            return math.nan
        return math.copysign(math.inf, math.copysign(1.0, a) * math.copysign(1.0, b))
    return a / b


def atan2(y, x):
    """The angle of the point (x, y) from the positive x axis, in [-π, π], bit for bit as occam computes it."""
    ax = abs(x)
    ay = abs(y)
    neg_x = math.copysign(1.0, x) < 0.0
    swap = ax < ay
    n = select(swap, ax, ay)
    d = select(swap, ay, ax)
    scale = select(d < TINY_DIVISOR, 2.0**100, select(HUGE_DIVISOR <= d, 0.5, 1.0))
    n = n * scale
    d = d * scale
    first = n < 0.25 * d
    v, th, tl = select(
        first,
        (0.0, 0.0, 0.0),
        select(n < 0.75 * d, (0.5, ATAN_HALF_HI, ATAN_HALF_LO), (1.0, PIO4_HI, PIO4_LO)),
    )
    num = select(first, n, n - v * d)  # selected, not multiplied by v = 0, so an infinite d gives no 0 * inf
    den = select(first, d, d + v * n)
    u = div(num, den)
    z = u * u
    a = u + (u * z) * horner(ATAN, z)
    oh, ol = select(swap, (PIO2_HI, PIO2_LO), select(neg_x, (PI_HI, PI_LO), (0.0, 0.0)))
    sgn = select(neg_x, -1.0, 1.0) * select(swap, -1.0, 1.0)
    h, e = two_sum(oh, sgn * th)
    angle = h + (e + (ol + sgn * (tl + a)))
    both_zero = select(d <= 0.0, n, 1.0) <= 0.0
    both_inf = math.inf <= select(math.inf <= d, n, 0.0)
    angle = select(both_zero, select(neg_x, PI_HI, 0.0), angle)
    angle = select(both_inf, select(neg_x, THREE_PIO4_HI, PIO4_HI), angle)
    return math.copysign(angle, y)
