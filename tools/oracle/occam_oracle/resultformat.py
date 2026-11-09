"""The result format (`occam-oracle-result/1`): one file per case, written deterministically.

The worker writes a result to a scratch file and the runner checks it here before anything keeps it. README.md documents
every field.
"""

from .canonical import dumps

SCHEMA = "occam-oracle-result/1"

COUNTS = ("vertices", "edges", "degenerate_edges", "wires", "faces", "shells", "solids")
ERROR_CODES = {
    "op_failed",
    "measure_failed",
    "worker_crashed",
    "worker_timeout",
    "worker_malformed",
    "identity_mismatch",
}


class ResultError(ValueError):
    """A result does not have the documented shape."""


def error_result(case_id, identity, threads, code, op, kind, message):
    """A result recording that the case could not be measured."""
    return {
        "schema": SCHEMA,
        "id": case_id,
        "oracle": identity,
        "threads": threads,
        "status": "error",
        "error": {"code": code, "op": op, "kind": kind, "message": message},
    }


def _number(value):
    return isinstance(value, (int, float)) and not isinstance(value, bool)


def _require(condition, message):
    if not condition:
        raise ResultError(message)


def _check_bbox(bbox):
    if bbox is None:
        return
    _require(isinstance(bbox, dict) and set(bbox) == {"min", "max"}, "bbox: expected min and max")
    for key in ("min", "max"):
        _require(isinstance(bbox[key], list) and len(bbox[key]) == 3 and all(map(_number, bbox[key])), f"bbox.{key}")


def _check_shape(shape):
    expected = {"volume", "area", "bbox", "euler", "counts", "valid", "closed"}
    _require(isinstance(shape, dict) and set(shape) == expected, f"shape: expected the fields {sorted(expected)}")
    _require(_number(shape["volume"]), "shape.volume")
    _require(_number(shape["area"]), "shape.area")
    _check_bbox(shape["bbox"])
    _require(isinstance(shape["euler"], int) and not isinstance(shape["euler"], bool), "shape.euler")
    counts = shape["counts"]
    _require(isinstance(counts, dict) and set(counts) == set(COUNTS), "shape.counts")
    _require(all(isinstance(counts[key], int) and counts[key] >= 0 for key in COUNTS), "shape.counts")
    _require(isinstance(shape["valid"], bool), "shape.valid")
    _require(isinstance(shape["closed"], bool), "shape.closed")


def _check_tessellation(stats):
    expected = {"target", "linear_deflection", "angular_deflection", "relative", "triangles", "vertices", "watertight"}
    _require(isinstance(stats, dict) and set(stats) == expected, f"tessellation: expected the fields {sorted(expected)}")


def validate_result(result, case_id=None):
    """Raises ResultError if `result` is not a well-formed result (for `case_id`, when given)."""
    _require(isinstance(result, dict), "a result must be a JSON object")
    _require(result.get("schema") == SCHEMA, f"schema: expected {SCHEMA}")
    _require(isinstance(result.get("id"), str) and result["id"], "id")
    if case_id is not None:
        _require(result["id"] == case_id, f"id: expected {case_id!r}, got {result['id']!r}")
    oracle = result.get("oracle")
    _require(isinstance(oracle, dict) and set(oracle) == {"engine", "engine_version", "package", "package_version"}, "oracle")
    _require(isinstance(result.get("threads"), int) and result["threads"] >= 1, "threads")
    status = result.get("status")
    if status == "ok":
        _require(set(result) == {"schema", "id", "oracle", "threads", "status", "shape", "tessellations"}, "ok result fields")
        _check_shape(result["shape"])
        _require(isinstance(result["tessellations"], dict), "tessellations")
    elif status == "error":
        _require(set(result) == {"schema", "id", "oracle", "threads", "status", "error"}, "error result fields")
        error = result["error"]
        _require(isinstance(error, dict) and set(error) == {"code", "op", "kind", "message"}, "error")
        _require(error["code"] in ERROR_CODES, f"error.code: {error['code']!r}")
    else:
        raise ResultError(f"status: expected ok or error, got {status!r}")
    return result


def dumps_result(result):
    """The bytes of a result file, as text."""
    validate_result(result)
    return dumps(result)
