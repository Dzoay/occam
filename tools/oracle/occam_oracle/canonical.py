"""One way of writing a JSON document, so that two runs produce the same bytes."""

import json
import math


def _normalised(value):
    if isinstance(value, float):
        if not math.isfinite(value):
            raise ValueError(f"not a finite number: {value!r}")
        return value + 0.0  # -0.0 becomes 0.0; every other value is unchanged
    if isinstance(value, dict):
        return {key: _normalised(item) for key, item in value.items()}
    if isinstance(value, (list, tuple)):
        return [_normalised(item) for item in value]
    return value


def dumps(value):
    """Sorted keys, LF line endings, ASCII, and floats in shortest round-trip form (`repr`)."""
    return json.dumps(_normalised(value), sort_keys=True, indent=2, ensure_ascii=True, allow_nan=False) + "\n"
