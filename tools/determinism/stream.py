"""The determinism stream, implemented again in Python so that its record does not come from the code it checks.

`ci/determinism-hashes.txt` holds what `occam-math`'s stream must hash to. This file computes those hashes from the
definition in `docs/DETERMINISM.md` on CPython's IEEE doubles, sharing no code with the crate, so a wrong answer that
every Rust build agrees on still fails. Run it to print the record, or with `--check` to compare it with the file.
"""

import math
import pathlib
import struct
import sys

from atan2 import atan2
from sincos import sin_cos

RECORD = pathlib.Path(__file__).resolve().parents[2] / "ci" / "determinism-hashes.txt"

MASK = (1 << 64) - 1
CANONICAL_NAN = 0x7FF8_0000_0000_0000
FNV_OFFSET = 0xCBF2_9CE4_8422_2325
FNV_PRIME = 0x0000_0100_0000_01B3
SIGN_AND_MANTISSA = 0x800F_FFFF_FFFF_FFFF
BLOCKS = 64
STEPS = 840

INFINITY = math.inf
NAN = math.nan


def bits(x):
    """The bits of a double."""
    return struct.unpack("<Q", struct.pack("<d", x))[0]


def double(word):
    """The double with these bits."""
    return struct.unpack("<d", struct.pack("<Q", word))[0]


def canonical_bits(x):
    """The bits of `x`, with every NaN replaced by one pattern."""
    return CANONICAL_NAN if x != x else bits(x)


class SplitMix64:
    """The stream's pseudo-random numbers."""

    def __init__(self, seed):
        self.state = seed & MASK

    def next_u64(self):
        """The next number in the sequence."""
        self.state = (self.state + 0x9E37_79B9_7F4A_7C15) & MASK
        z = self.state
        z = ((z ^ (z >> 30)) * 0xBF58_476D_1CE4_E5B9) & MASK
        z = ((z ^ (z >> 27)) * 0x94D0_49BB_1331_11EB) & MASK
        return z ^ (z >> 31)


class Digest:
    """64-bit FNV-1a over the little-endian bytes of each word."""

    def __init__(self):
        self.value = FNV_OFFSET

    def write_u64(self, word):
        """Writes a word, as its little-endian bytes."""
        value = self.value
        for shift in range(0, 64, 8):
            value = ((value ^ ((word >> shift) & 0xFF)) * FNV_PRIME) & MASK
        self.value = value

    def write_value(self, x):
        """Writes a number, as its canonical bits."""
        self.write_u64(canonical_bits(x))


SPECIAL = [
    0.0,
    -0.0,
    1.0,
    -1.0,
    INFINITY,
    -INFINITY,
    NAN,
    double(0x7FEF_FFFF_FFFF_FFFF),
    -double(0x7FEF_FFFF_FFFF_FFFF),
    double(0x0010_0000_0000_0000),
    double(1),
    -double(1),
    0.1,
    1.0 / 3.0,
    9_007_199_254_740_992.0,
    1e-300,
]


def length(word):
    """A number of the size a length has in millimetres: an exponent from 2^-15 to 2^16."""
    exponent = 0x3F0 + ((word >> 52) & 0x1F)
    return double(((word & SIGN_AND_MANTISSA) + (exponent << 52)) & MASK)


def draw(generator):
    """The next input: an awkward number, any bit pattern, a subnormal, or a length."""
    kind = generator.next_u64() >> 62
    word = generator.next_u64()
    if kind == 0:
        return SPECIAL[word & 15]
    if kind == 1:
        return double(word)
    if kind == 2:
        return double(word & SIGN_AND_MANTISSA)
    return length(word)


def sign(x):
    """-1.0 or 1.0, from the sign bit, zeros included."""
    return math.copysign(1.0, x)


def div(a, b):
    """IEEE division, which Python refuses for a zero divisor."""
    if b == 0.0:
        if a != a or a == 0.0:
            return NAN
        return math.copysign(INFINITY, sign(a) * sign(b))
    return a / b


def sqrt(a):
    """IEEE square root: NaN below zero, -0 for -0."""
    if a < 0.0:
        return NAN
    return math.sqrt(a)


def select(mask, yes, no):
    """`yes` where the comparison held, `no` elsewhere."""
    return yes if mask else no


def outputs(a, b, c):
    """Every operation on one set of inputs, in the order of the record."""
    sine, cosine = sin_cos(a)
    return [
        ("add", a + b),
        ("sub", a - b),
        ("mul", a * b),
        ("div", div(a, b)),
        ("neg", -a),
        ("abs", abs(a)),
        ("sqrt", sqrt(a)),
        ("mixed", sqrt(abs((a + b) * c - div(a, b)))),
        ("lt", select(a < b, a, b)),
        ("le", select(a <= b, b, c)),
        ("up", math.nextafter(a, INFINITY)),
        ("down", math.nextafter(a, -INFINITY)),
        ("sin", sine),
        ("cos", cosine),
        ("copysign", math.copysign(a, b)),
        ("atan2", atan2(a, b)),
    ]


OPERATIONS = [name for name, _ in outputs(0.0, 0.0, 0.0)]


def block(index):
    """The hashes of one block, whose inputs depend on its index alone."""
    first, second, third = (SplitMix64((index << 2) + operand) for operand in range(3))
    digests = [Digest() for _ in OPERATIONS]
    for _ in range(STEPS):
        a, b, c = draw(first), draw(second), draw(third)
        for digest, (_, value) in zip(digests, outputs(a, b, c)):
            digest.write_value(value)
    return [digest.value for digest in digests]


def run():
    """The stream's hash for each operation, the blocks folded in block order."""
    digests = [Digest() for _ in OPERATIONS]
    for index in range(BLOCKS):
        for digest, value in zip(digests, block(index)):
            digest.write_u64(value)
    return dict(zip(OPERATIONS, (digest.value for digest in digests)))


def recorded():
    """The record's `<operation> <hash>` lines, as a dictionary in file order."""
    lines = RECORD.read_text(encoding="utf-8").splitlines()
    pairs = (line.split() for line in lines if line and not line.startswith("#"))
    return {name: int(value, 16) for name, value in pairs}


def main(args):
    """Prints the record, or with `--check` compares it with the file."""
    hashes = run()
    if args == ["--check"]:
        if hashes != recorded():
            print(f"{RECORD} does not match the separate implementation", file=sys.stderr)
            return 1
        return 0
    for name, value in hashes.items():
        print(f"{name} {value:016x}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
