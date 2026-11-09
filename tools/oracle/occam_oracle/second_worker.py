#!/usr/bin/env python3
"""The worker for a second-opinion engine: speaks the worker protocol and translates for its compiled driver.

The driver is a small Rust program with no JSON in it. It reads one operation per line on stdin and answers in lines on
stdout. This worker turns a case into those lines and the answer into a result document. README.md has the protocol.
Standard library only.
"""

import argparse
import importlib.util
import json
import math
import pathlib
import subprocess
import sys

_SPEC = importlib.util.spec_from_file_location("meshcheck", pathlib.Path(__file__).with_name("meshcheck.py"))
MESHCHECK = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(MESHCHECK)

RESULT_SCHEMA = "occam-oracle-result/1"
_UNSUPPORTED = "is not supported by this engine"
_FIELDS = {
    "box": ("size", "origin"),
    "cylinder": ("radius", "height", "origin", "axis"),
    "sphere": ("radius", "origin"),
    "cone": ("radius1", "radius2", "height", "origin", "axis"),
    "torus": ("major_radius", "minor_radius", "origin", "axis"),
    "translate": ("target", "vector"),
    "rotate": ("target", "axis_origin", "axis", "angle_degrees"),
    "fuse": ("target", "tool"),
    "cut": ("target", "tool"),
    "common": ("target", "tool"),
    "fillet": ("target", "radius", "edges"),
    "tessellate": ("target", "linear_deflection", "angular_deflection", "relative"),
    "remove_face": ("target", "in_box"),
}


def _tokens(value):
    if isinstance(value, bool):
        return ["1" if value else "0"]
    if isinstance(value, (int, float)):
        return [repr(float(value))]
    if isinstance(value, str):
        return [value]
    if isinstance(value, list):
        return [token for item in value for token in _tokens(item)]
    if isinstance(value, dict) and set(value) == {"in_box"}:
        return ["box", *_tokens(value["in_box"])]
    if isinstance(value, dict) and set(value) == {"min", "max"}:
        return [*_tokens(value["min"]), *_tokens(value["max"])]
    raise ValueError(f"cannot pass {value!r} to a driver")


def _line(op):
    fields = _FIELDS[op["op"]]
    values = {**{"relative": False}, **op}
    return " ".join([op["op"], op["id"], *(token for field in fields for token in _tokens(values[field]))])


def request(case):
    """The driver's input: one op per line, then which shape to measure. Floats in round-trip form."""
    return "\n".join([*map(_line, case["ops"]), f"result {case['result']}"]) + "\n"


def _shape(lines):
    numbers = {}
    for line in lines:
        key, *rest = line.split()
        numbers[key] = rest
    vertices, edges, degenerate, wires, faces, shells, solids = map(int, numbers["counts"])
    bbox = [float(value) for value in numbers["bbox"]]
    return {
        "volume": float(numbers["volume"][0]),
        "area": float(numbers["area"][0]),
        "bbox": {"min": bbox[:3], "max": bbox[3:]},
        "euler": vertices - edges + 2 * faces - wires,
        "counts": {
            "vertices": vertices,
            "edges": edges,
            "degenerate_edges": degenerate,
            "wires": wires,
            "faces": faces,
            "shells": shells,
            "solids": solids,
        },
        "valid": numbers["valid"][0] == "1",
        "closed": numbers["closed"][0] == "1",
    }


def _tessellation(op, mesh_lines):
    header = mesh_lines[0].split()
    point_count, triangle_count = int(header[2]), int(header[3])
    points = [tuple(map(float, line.split())) for line in mesh_lines[1 : 1 + point_count]]
    triangles = [tuple(map(int, line.split())) for line in mesh_lines[1 + point_count : 1 + point_count + triangle_count]]
    diagonal = math.dist(min(points), max(points)) if points else 1.0
    return {
        "target": op["target"],
        "linear_deflection": op["linear_deflection"],
        "angular_deflection": op["angular_deflection"],
        "relative": op.get("relative", False),
        **MESHCHECK.summarize(points, triangles, 1e-9 * max(diagonal, 1e-12)),
    }


def parse(case, identity, threads, output):
    """Turns the driver's answer into a result document."""
    lines = [line for line in output.splitlines() if line.strip()]
    common = {"schema": RESULT_SCHEMA, "id": case["id"], "oracle": identity, "threads": threads}
    if lines and lines[0].split()[0] == "error":
        _keyword, op_id, *words = lines[0].split(" ")
        kind = next((op["op"] for op in case["ops"] if op["id"] == op_id), None)
        message = " ".join(words)
        code = "op_failed"
        if message.endswith(_UNSUPPORTED):
            code = "op_unsupported"
        elif op_id == case["result"] and len(case["ops"]) and lines[0].startswith(f"error {op_id} measure:"):
            code = "measure_failed"
        return {**common, "status": "error", "error": {"code": code, "op": op_id, "kind": kind, "message": message}}
    if not lines or lines[0] != "ok":
        raise ValueError("the driver's answer does not start with ok or error")
    ops = {op["id"]: op for op in case["ops"]}
    tessellations, shape_lines, index = {}, [], 1
    while index < len(lines):
        words = lines[index].split()
        if words[0] == "mesh":
            count = 1 + int(words[2]) + int(words[3])
            tessellations[words[1]] = _tessellation(ops[words[1]], lines[index : index + count])
            index += count
        else:
            shape_lines.append(lines[index])
            index += 1
    return {**common, "status": "ok", "shape": _shape(shape_lines), "tessellations": tessellations}


def main():
    parser = argparse.ArgumentParser(description="One case through a second-opinion engine.")
    parser.add_argument("--driver", required=True)
    parser.add_argument("--engine-name", required=True)
    parser.add_argument("--engine-version", required=True)
    parser.add_argument("--package", required=True)
    parser.add_argument("--package-version", required=True)
    parser.add_argument("--case", required=True, type=pathlib.Path)
    parser.add_argument("--result", required=True, type=pathlib.Path)
    parser.add_argument("--progress", required=True, type=pathlib.Path)
    parser.add_argument("--threads", required=True, type=int)
    args = parser.parse_args()
    case = json.loads(args.case.read_text(encoding="utf-8"))
    identity = {
        "engine": args.engine_name,
        "engine_version": args.engine_version,
        "package": args.package,
        "package_version": args.package_version,
    }
    answer = subprocess.run([args.driver, str(args.progress)], input=request(case), capture_output=True, text=True, check=False)
    if answer.returncode != 0:
        print(answer.stderr, file=sys.stderr)
        return answer.returncode if answer.returncode > 0 else 128 - answer.returncode
    result = parse(case, identity, args.threads, answer.stdout)
    args.result.write_text(json.dumps(result, allow_nan=False), encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main())
