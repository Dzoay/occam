"""The case format, `occam-oracle-case/1`, as the corpus uses it.

`tools/oracle/occam_oracle/caseformat.py` is the definition. This is a smaller check that exists so the corpus tests run
on a branch without the harness; once the harness is merged, the corpus tests also run its validator over every case.
"""

import math
import re

SCHEMA = "occam-oracle-case/1"
ROLES = ("a", "b", "union", "intersection")
_ID = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]*$")
_COPYLEFT = re.compile(r"(GPL|MPL|EPL|EUPL|CDDL|CC-BY-SA)", re.IGNORECASE)


class CaseError(ValueError):
    """A case is not well-formed; the message names the case or the op."""


def _number(value, where):
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(value):
        raise CaseError(f"{where}: expected a finite number, got {value!r}")


def _vector(value, where):
    if not isinstance(value, list) or len(value) != 3:
        raise CaseError(f"{where}: expected three numbers")
    for item in value:
        _number(item, where)


def _positive(value, where):
    _number(value, where)
    if value <= 0:
        raise CaseError(f"{where}: must be greater than zero")


def _non_negative(value, where):
    _number(value, where)
    if value < 0:
        raise CaseError(f"{where}: must not be negative")


def _size(value, where):
    _vector(value, where)
    if any(item <= 0 for item in value):
        raise CaseError(f"{where}: every component must be greater than zero")


def _direction(value, where):
    _vector(value, where)
    if not any(value):
        raise CaseError(f"{where}: must not be zero")


def _edges(value, where):
    if value == "all":
        return
    if not isinstance(value, dict) or set(value) != {"in_box"} or set(value["in_box"]) != {"min", "max"}:
        raise CaseError(f"{where}: expected 'all' or an in_box")
    _vector(value["in_box"]["min"], where)
    _vector(value["in_box"]["max"], where)


def _reference(value, where):
    if not isinstance(value, str):
        raise CaseError(f"{where}: expected an op id")


_BOOLEAN = {"target": _reference, "tool": _reference}
OPS = {
    "box": {"size": _size, "origin": _vector},
    "cylinder": {"radius": _positive, "height": _positive, "origin": _vector, "axis": _direction},
    "sphere": {"radius": _positive, "origin": _vector},
    "cone": {"radius1": _non_negative, "radius2": _non_negative, "height": _positive, "origin": _vector, "axis": _direction},
    "torus": {"major_radius": _positive, "minor_radius": _positive, "origin": _vector, "axis": _direction},
    "translate": {"target": _reference, "vector": _vector},
    "rotate": {"target": _reference, "axis_origin": _vector, "axis": _direction, "angle_degrees": _number},
    "fuse": _BOOLEAN,
    "cut": _BOOLEAN,
    "common": _BOOLEAN,
    "fillet": {"target": _reference, "radius": _positive, "edges": _edges},
    "tessellate": {"target": _reference, "linear_deflection": _positive, "angular_deflection": _positive},
}
OPTIONAL = {"tessellate": {"relative"}}


def _check_op(op):
    name = f"op {op.get('id')!r}"
    kind = op.get("op")
    if kind not in OPS:
        raise CaseError(f"{name}: unknown operation {kind!r}")
    fields = OPS[kind]
    unknown = set(op) - set(fields) - OPTIONAL.get(kind, set()) - {"op", "id"}
    if unknown:
        raise CaseError(f"{name}: unknown field(s) {', '.join(sorted(unknown))}")
    for field, check in fields.items():
        if field not in op:
            raise CaseError(f"{name}: missing field {field}")
        check(op[field], f"{name}.{field}")


def _toleranced(spec, measure):
    if not isinstance(spec, dict) or "value" not in spec or not {"rel_tol", "abs_tol"} & set(spec):
        raise CaseError(f"expect.{measure}: needs a value and a tolerance")
    _number(spec["value"], f"expect.{measure}.value")


def _check_expect(expect):
    for measure, spec in expect.items():
        if measure in ("volume", "area"):
            _toleranced(spec, measure)
        elif measure == "bbox":
            if set(spec) != {"min", "max", "abs_tol"}:
                raise CaseError("expect.bbox: expected min, max and abs_tol")
            _vector(spec["min"], "expect.bbox.min")
            _vector(spec["max"], "expect.bbox.max")
        elif measure == "euler":
            if not isinstance(spec.get("value"), int) or isinstance(spec.get("value"), bool):
                raise CaseError("expect.euler: expected an integer value")
        elif measure in ("valid", "closed", "watertight"):
            if not isinstance(spec.get("value"), bool):
                raise CaseError(f"expect.{measure}: expected true or false")
        elif measure != "error":
            raise CaseError(f"expect: unknown or disallowed measure {measure}")


def validate_case(case):
    """Raises CaseError if `case` is not a well-formed case."""
    unknown = set(case) - {"schema", "id", "origin", "licence", "ops", "result", "expect", "volume_identity"}
    if unknown:
        raise CaseError(f"unknown field(s) {', '.join(sorted(unknown))}")
    if case.get("schema") != SCHEMA:
        raise CaseError(f"schema: expected {SCHEMA}, got {case.get('schema')!r}")
    if not isinstance(case.get("id"), str) or not _ID.match(case["id"]):
        raise CaseError(f"id: not a file-name-safe identifier: {case.get('id')!r}")
    for key in ("origin", "licence"):
        if not isinstance(case.get(key), str) or not case[key]:
            raise CaseError(f"{key}: required")
    if _COPYLEFT.search(case["licence"]):
        raise CaseError(f"licence: {case['licence']!r} is copyleft and may not enter the repository")
    shapes = set()
    seen = set()
    for op in case.get("ops", []):
        _check_op(op)
        if op["id"] in seen:
            raise CaseError(f"op {op['id']!r}: duplicate id")
        seen.add(op["id"])
        for field in ("target", "tool"):
            if field in op and op[field] not in shapes:
                raise CaseError(f"op {op['id']!r}: {field} {op[field]!r} is not the id of an earlier shape")
        if op["op"] != "tessellate":
            shapes.add(op["id"])
    if case.get("result") not in shapes:
        raise CaseError(f"result: {case.get('result')!r} is not the id of a shape")
    if "expect" in case:
        _check_expect(case["expect"])
    if "volume_identity" in case:
        tag = case["volume_identity"]
        if not isinstance(tag, dict) or set(tag) != {"group", "role"} or tag["role"] not in ROLES:
            raise CaseError(f"volume_identity: role must be one of {', '.join(ROLES)}: {tag!r}")
