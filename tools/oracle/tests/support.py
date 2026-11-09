"""Builders for cases and fabricated results, shared by the harness's tests."""

import math

CASE_SCHEMA = "occam-oracle-case/1"
RESULT_SCHEMA = "occam-oracle-result/1"


def case(case_id, ops, result, expect=None, **extra):
    """A well-formed case."""
    document = {
        "schema": CASE_SCHEMA,
        "id": case_id,
        "origin": "written by hand for the oracle harness's own tests",
        "licence": "MIT OR Apache-2.0",
        "ops": ops,
        "result": result,
    }
    if expect is not None:
        document["expect"] = expect
    document.update(extra)
    return document


def box(op_id, size, origin=(0, 0, 0)):
    return {"op": "box", "id": op_id, "size": list(size), "origin": list(origin)}


def cylinder(op_id, radius, height, origin=(0, 0, 0), axis=(0, 0, 1)):
    return {"op": "cylinder", "id": op_id, "radius": radius, "height": height, "origin": list(origin), "axis": list(axis)}


def sphere(op_id, radius, origin=(0, 0, 0)):
    return {"op": "sphere", "id": op_id, "radius": radius, "origin": list(origin)}


def boolean(kind, op_id, target, tool):
    return {"op": kind, "id": op_id, "target": target, "tool": tool}


def box_case(case_id="box", size=(10, 20, 30), expect=None):
    return case(case_id, [box("a", size)], "a", expect)


def cut_case(case_id="box-cut-cylinder"):
    """A 10 x 20 x 30 box with a radius-2 cylinder through its full height."""
    ops = [
        box("a", (10, 20, 30)),
        cylinder("b", 2, 40, origin=(5, 10, -5)),
        boolean("cut", "c", "a", "b"),
    ]
    return case(case_id, ops, "c")


CUT_VOLUME = 6000.0 - math.pi * 4.0 * 30.0


def oracle():
    return {
        "engine": "opencascade",
        "engine_version": "8.0.1",
        "package": "cadquery-ocp",
        "package_version": "8.0.1.1.0",
    }


def shape(**overrides):
    """A fabricated measurement block: the result shape of a unit cube, valid and closed."""
    block = {
        "volume": 1.0,
        "area": 6.0,
        "bbox": {"min": [0.0, 0.0, 0.0], "max": [1.0, 1.0, 1.0]},
        "euler": 2,
        "counts": {"vertices": 8, "edges": 12, "degenerate_edges": 0, "wires": 6, "faces": 6, "shells": 1, "solids": 1},
        "valid": True,
        "closed": True,
    }
    block.update(overrides)
    return block


def ok_result(case_id="box", tessellations=None, **shape_overrides):
    return {
        "schema": RESULT_SCHEMA,
        "id": case_id,
        "oracle": oracle(),
        "threads": 1,
        "status": "ok",
        "shape": shape(**shape_overrides),
        "tessellations": tessellations or {},
    }


def error_result(case_id="box", op="f", kind="fillet", code="op_failed", message="StdFail_NotDone: command not done"):
    return {
        "schema": RESULT_SCHEMA,
        "id": case_id,
        "oracle": oracle(),
        "threads": 1,
        "status": "error",
        "error": {"code": code, "op": op, "kind": kind, "message": message},
    }
