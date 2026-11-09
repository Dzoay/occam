"""The case format (`occam-oracle-case/1`): what a case may say, checked before any engine runs.

A case is data: the operation list, the result to measure, and optional expected measures. README.md documents every
field, and this module enforces it.
"""

import json
import math
import pathlib
import re

SCHEMA = "occam-oracle-case/1"

_ID = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]*$")
_COPYLEFT = re.compile(r"(GPL|MPL|EPL|EUPL|CDDL|CC-BY-SA)", re.IGNORECASE)

ROLES = ("a", "b", "union", "intersection")


class CaseError(ValueError):
    """A case is not well-formed; the message names the case or the op."""


def _number(value, where):
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(value):
        raise CaseError(f"{where}: expected a finite number, got {value!r}")
    return value


def _positive(value, where):
    if _number(value, where) <= 0:
        raise CaseError(f"{where}: must be greater than zero")


def _non_negative(value, where):
    if _number(value, where) < 0:
        raise CaseError(f"{where}: must not be negative")


def _vector(value, where):
    if not isinstance(value, list) or len(value) != 3:
        raise CaseError(f"{where}: expected a list of three numbers")
    return [_number(item, where) for item in value]


def _positive_vector(value, where):
    for item in _vector(value, where):
        if item <= 0:
            raise CaseError(f"{where}: every component must be greater than zero")


def _direction(value, where):
    if not any(_vector(value, where)):
        raise CaseError(f"{where}: must not be the zero vector")


def _boolean(value, where):
    if not isinstance(value, bool):
        raise CaseError(f"{where}: expected true or false")


def _text(value, where):
    if not isinstance(value, str) or not value:
        raise CaseError(f"{where}: expected a non-empty string")


def _box(value, where):
    if not isinstance(value, dict) or set(value) != {"min", "max"}:
        raise CaseError(f"{where}: expected an object with min and max")
    low, high = _vector(value["min"], f"{where}.min"), _vector(value["max"], f"{where}.max")
    if any(a > b for a, b in zip(low, high)):
        raise CaseError(f"{where}: min is above max")


def _edge_selector(value, where):
    if value == "all":
        return
    if isinstance(value, dict) and set(value) == {"in_box"}:
        _box(value["in_box"], f"{where}.in_box")
        return
    raise CaseError(f'{where}: expected "all" or {{"in_box": {{"min": ..., "max": ...}}}}')


def _reference(_value, _where):
    """Checked in order against the ops before it; see _check_references."""


_PLACED = {"origin": _vector}
_AXIS = {"origin": _vector, "axis": _direction}
_TWO = {"target": _reference, "tool": _reference}

# Required fields of each operation: name to checker. Operations that make no shape are listed in _NO_SHAPE.
OPERATIONS = {
    "box": {"size": _positive_vector, **_PLACED},
    "cylinder": {"radius": _positive, "height": _positive, **_AXIS},
    "sphere": {"radius": _positive, **_PLACED},
    "cone": {"radius1": _non_negative, "radius2": _non_negative, "height": _positive, **_AXIS},
    "torus": {"major_radius": _positive, "minor_radius": _positive, **_AXIS},
    "translate": {"target": _reference, "vector": _vector},
    "rotate": {"target": _reference, "axis_origin": _vector, "axis": _direction, "angle_degrees": _number},
    "fuse": _TWO,
    "cut": _TWO,
    "common": _TWO,
    "fillet": {"target": _reference, "radius": _positive, "edges": _edge_selector},
    "tessellate": {"target": _reference, "linear_deflection": _positive, "angular_deflection": _positive},
    "remove_face": {"target": _reference, "in_box": _box},
}
_OPTIONAL = {"tessellate": {"relative": _boolean}}
_NO_SHAPE = {"tessellate"}
_REFERENCE_FIELDS = ("target", "tool")


def _check_op(op, index):
    if not isinstance(op, dict):
        raise CaseError(f"ops[{index}]: expected an object")
    op_id = op.get("id")
    if not isinstance(op_id, str) or not _ID.match(op_id):
        raise CaseError(f"ops[{index}]: id must be letters, digits, '.', '_' or '-', got {op_id!r}")
    name = f"op {op_id!r}"
    kind = op.get("op")
    if kind not in OPERATIONS:
        raise CaseError(f"{name}: unknown operation {kind!r}")
    required = OPERATIONS[kind]
    optional = _OPTIONAL.get(kind, {})
    allowed = {"op", "id", *required, *optional}
    unknown = sorted(set(op) - allowed)
    if unknown:
        raise CaseError(f"{name}: unknown field(s) {', '.join(unknown)}")
    for field, checker in required.items():
        if field not in op:
            raise CaseError(f"{name}: missing field {field}")
        checker(op[field], f"{name}.{field}")
    for field, checker in optional.items():
        if field in op:
            checker(op[field], f"{name}.{field}")


def _check_references(ops):
    """Every reference names an earlier op, and that op made a shape."""
    shapes = set()
    seen = set()
    for op in ops:
        if op["id"] in seen:
            raise CaseError(f"op {op['id']!r}: duplicate id")
        seen.add(op["id"])
        for field in _REFERENCE_FIELDS:
            if field in op and op[field] not in shapes:
                raise CaseError(f"op {op['id']!r}: {field} {op[field]!r} is not the id of an earlier shape")
        if op["op"] not in _NO_SHAPE:
            shapes.add(op["id"])
    return shapes


def _toleranced(spec, where):
    if not isinstance(spec, dict):
        raise CaseError(f"{where}: expected an object")
    if "rel_tol" not in spec and "abs_tol" not in spec:
        raise CaseError(f"{where}: needs rel_tol or abs_tol")
    for key in ("rel_tol", "abs_tol"):
        if key in spec:
            _non_negative(spec[key], f"{where}.{key}")


def _expect_number(spec, where):
    _toleranced(spec, where)
    unknown = sorted(set(spec) - {"value", "rel_tol", "abs_tol"})
    if unknown or "value" not in spec:
        raise CaseError(f"{where}: expected value and a tolerance, got {sorted(spec)}")
    _number(spec["value"], f"{where}.value")


def _expect_box(spec, where):
    if not isinstance(spec, dict) or set(spec) != {"min", "max", "abs_tol"}:
        raise CaseError(f"{where}: expected min, max and abs_tol")
    _box({"min": spec["min"], "max": spec["max"]}, where)
    _non_negative(spec["abs_tol"], f"{where}.abs_tol")


def _expect_integer(spec, where):
    if not isinstance(spec, dict) or set(spec) != {"value"} or isinstance(spec["value"], bool) or not isinstance(spec["value"], int):
        raise CaseError(f'{where}: expected {{"value": <integer>}}')


def _expect_boolean(spec, where):
    if not isinstance(spec, dict) or set(spec) != {"value"} or not isinstance(spec["value"], bool):
        raise CaseError(f'{where}: expected {{"value": true|false}}')


def _expect_error(spec, where):
    if not isinstance(spec, dict) or set(spec) != {"op"}:
        raise CaseError(f'{where}: expected {{"op": <op id>}}')
    _text(spec["op"], f"{where}.op")


_EXPECT = {
    "volume": _expect_number,
    "area": _expect_number,
    "bbox": _expect_box,
    "euler": _expect_integer,
    "valid": _expect_boolean,
    "closed": _expect_boolean,
    "watertight": _expect_boolean,
    "error": _expect_error,
}


def _check_expect(expect):
    if not isinstance(expect, dict):
        raise CaseError("expect: expected an object")
    unknown = sorted(set(expect) - set(_EXPECT))
    if unknown:
        raise CaseError(f"expect: unknown or disallowed measure(s) {', '.join(unknown)}")
    for measure, checker in _EXPECT.items():
        if measure in expect:
            checker(expect[measure], f"expect.{measure}")
    if "error" in expect and len(expect) > 1:
        raise CaseError("expect.error stands alone: a case that must fail cannot also expect measures")


def _check_volume_identity(spec):
    if not isinstance(spec, dict) or set(spec) != {"group", "role"}:
        raise CaseError('volume_identity: expected {"group": ..., "role": ...}')
    _text(spec["group"], "volume_identity.group")
    if spec["role"] not in ROLES:
        raise CaseError(f"volume_identity.role: expected one of {', '.join(ROLES)}, got {spec['role']!r}")


def validate_case(case):
    """Raises CaseError if `case` is not a well-formed case."""
    if not isinstance(case, dict):
        raise CaseError("a case must be a JSON object")
    allowed = {"schema", "id", "origin", "licence", "ops", "result", "expect", "volume_identity"}
    unknown = sorted(set(case) - allowed)
    if unknown:
        raise CaseError(f"unknown field(s) {', '.join(unknown)}")
    if case.get("schema") != SCHEMA:
        raise CaseError(f"schema: expected {SCHEMA}, got {case.get('schema')!r}")
    case_id = case.get("id")
    if not isinstance(case_id, str) or not _ID.match(case_id):
        raise CaseError(f"id: expected a file-name-safe identifier, got {case_id!r}")
    for key in ("origin", "licence"):
        _text(case.get(key), key)
    if _COPYLEFT.search(case["licence"]):
        raise CaseError(f"licence: {case['licence']!r} is copyleft and may not enter the repository")
    ops = case.get("ops")
    if not isinstance(ops, list) or not ops:
        raise CaseError("ops: expected a non-empty list")
    for index, op in enumerate(ops):
        _check_op(op, index)
    shapes = _check_references(ops)
    result = case.get("result")
    if result not in shapes:
        raise CaseError(f"result: {result!r} is not the id of a shape made by an op")
    if "expect" in case:
        _check_expect(case["expect"])
    if "volume_identity" in case:
        _check_volume_identity(case["volume_identity"])


def load_case(path):
    """Reads and validates a case file."""
    path = pathlib.Path(path)
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise CaseError(f"{path}: {error}") from error
    try:
        validate_case(document)
    except CaseError as error:
        raise CaseError(f"{path}: {error}") from error
    return document


def case_files(paths):
    """The case files named by a list of files and directories, directories searched for *.json, sorted."""
    found = []
    for raw in paths:
        path = pathlib.Path(raw)
        found.extend(sorted(path.rglob("*.json")) if path.is_dir() else [path])
    return found
