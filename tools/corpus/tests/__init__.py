"""Puts the harness's case-format validator on the path for these tests."""

import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[2] / "oracle"))
