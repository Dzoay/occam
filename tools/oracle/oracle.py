#!/usr/bin/env python3
"""Command-line entry point of the oracle harness: `run`, `check` and `register`. See README.md."""

import sys

from occam_oracle.cli import main

if __name__ == "__main__":
    sys.exit(main())
