"""The owned trigonometry's constants, read from the Rust file that holds them.

`crates/occam-math/src/trig/constants.rs` is the one home of these numbers. This module parses its `f64::from_bits`
literals and its `u64` words with a small regular-expression reader, so the separate sin, cos and atan2 use the very
bits the crate does and never a second copy of them.
"""

import pathlib
import re
import struct

SOURCE = pathlib.Path(__file__).resolve().parents[2] / "crates" / "occam-math" / "src" / "trig" / "constants.rs"

SCALAR = re.compile(r"const (\w+): f64 = f64::from_bits\((0x[0-9A-Fa-f_]+)\);")
ARRAY = re.compile(r"const (\w+): \[(f64|u64); (\d+)\] = \[(.*?)\];", re.DOTALL)
WORD = re.compile(r"0x[0-9A-Fa-f_]+")


def double(word):
    """The double with these bits."""
    return struct.unpack("<d", struct.pack("<Q", word))[0]


def read(path=SOURCE):
    """Every constant in the file: a double, a list of doubles, or a list of words, by name."""
    text = path.read_text(encoding="utf-8")
    found = {name: double(int(literal, 16)) for name, literal in SCALAR.findall(text)}
    for name, kind, size, body in ARRAY.findall(text):
        words = [int(literal, 16) for literal in WORD.findall(body)]
        if len(words) != int(size):
            raise ValueError(f"{name} declares {size} items and holds {len(words)}")
        found[name] = [double(word) for word in words] if kind == "f64" else words
    return found


_FOUND = read()

PIO2_C1 = _FOUND["PIO2_C1"]
PIO2_C2 = _FOUND["PIO2_C2"]
PIO2_C3 = _FOUND["PIO2_C3"]
PIO2_C4 = _FOUND["PIO2_C4"]
INV_PIO2 = _FOUND["INV_PIO2"]
PIO2_HI = _FOUND["PIO2_HI"]
PIO2_LO = _FOUND["PIO2_LO"]
PIO4_HI = _FOUND["PIO4_HI"]
PIO4_LO = _FOUND["PIO4_LO"]
PI_HI = _FOUND["PI_HI"]
PI_LO = _FOUND["PI_LO"]
THREE_PIO4_HI = _FOUND["THREE_PIO4_HI"]
ATAN_HALF_HI = _FOUND["ATAN_HALF_HI"]
ATAN_HALF_LO = _FOUND["ATAN_HALF_LO"]
TWO_OVER_PI = _FOUND["TWO_OVER_PI"]
SIN = _FOUND["SIN"]
COS = _FOUND["COS"]
ATAN = _FOUND["ATAN"]
