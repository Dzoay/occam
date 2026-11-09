#!/usr/bin/env python3
"""Imports the arris test recipes that pass a per-case licence and origin gate.

Reads only each fixture's `fixture.json`, the recipe with its closed-form answers, which arris's fixture README says
people or an agent wrote for its corpus. `expected.json` (an oracle's answer) and the `dump` files are never opened.
A case is taken only if nothing marks it as coming from elsewhere and its answers are closed-form expressions, so that
no number from another kernel can come along; everything else is excluded and reported with its reason.

The output is committed under `tools/corpus/imported/`. `just corpus-import-arris <repo>` regenerates it from a clone at
the commit pinned in `sources.lock.json`.
"""

import argparse
import ast
import json
import math
import pathlib
import re
import subprocess
import sys

import generate

ORACLE = pathlib.Path(__file__).resolve().parents[1] / "oracle"
sys.path.insert(0, str(ORACLE))
from occam_oracle import caseformat  # noqa: E402

LOCK = pathlib.Path(__file__).resolve().parent / "sources.lock.json"
OUT = pathlib.Path(__file__).resolve().parent / "imported"
LICENCE = "MIT OR Apache-2.0"
COPYRIGHT = "Copyright (c) 2026 Sergei Sergienko"

_KERNEL = re.compile(r"open\s*cascade|\bocct\b|\boracles?\b|freecad|parasolid|\bacis\b|catia|solidworks|\bnx\b|\bcreo\b|solidcore", re.I)
# A "consumer" is a downstream user of arris: such a probe, and any recipe built on one, does not state where it came from.
_UNCLEAR_ORIGIN = re.compile(r"\bconsumer\b|parallel-cylinders-cut", re.I)
_FUNCTIONS = {
    "sin": math.sin,
    "cos": math.cos,
    "tan": math.tan,
    "atan": math.atan,
    "sqrt": math.sqrt,
    "radians": math.radians,
    "degrees": math.degrees,
    "abs": abs,
}
_MAX_POWER = 64
_SHAPE_OPS = {"box", "cylinder"}
_BOOLEAN_OPS = {"fuse", "cut", "common"}


class Excluded(Exception):
    """This fixture stays out. `category` groups reasons for the report."""

    def __init__(self, category, detail=""):
        super().__init__(f"{category}{': ' + detail if detail else ''}")
        self.category = category


def _evaluate_node(node, names):
    if isinstance(node, ast.Expression):
        return _evaluate_node(node.body, names)
    if isinstance(node, ast.Constant) and isinstance(node.value, (int, float)) and not isinstance(node.value, bool):
        return node.value
    if isinstance(node, ast.Name) and node.id in names:
        return names[node.id]
    if isinstance(node, ast.UnaryOp) and isinstance(node.op, (ast.USub, ast.UAdd)):
        value = _evaluate_node(node.operand, names)
        return -value if isinstance(node.op, ast.USub) else value
    if isinstance(node, ast.BinOp) and isinstance(node.op, (ast.Add, ast.Sub, ast.Mult, ast.Div, ast.Pow)):
        left, right = _evaluate_node(node.left, names), _evaluate_node(node.right, names)
        if isinstance(node.op, ast.Add):
            return left + right
        if isinstance(node.op, ast.Sub):
            return left - right
        if isinstance(node.op, ast.Mult):
            return left * right
        if isinstance(node.op, ast.Div):
            return left / right
        if abs(right) > _MAX_POWER or abs(left) > 1e6:
            raise Excluded("bad expression", "power out of range")
        return left**right
    if isinstance(node, ast.Call) and isinstance(node.func, ast.Name) and node.func.id in _FUNCTIONS and len(node.args) == 1 and not node.keywords:
        return _FUNCTIONS[node.func.id](_evaluate_node(node.args[0], names))
    raise Excluded("bad expression", f"disallowed construct {type(node).__name__}")


def evaluate(text, params):
    """The fixtures' expression grammar: + - * / ^ (power), parentheses, pi and a few functions, over `params`."""
    if "**" in text:
        raise Excluded("bad expression", "'**' is not in the grammar")
    try:
        tree = ast.parse(text.replace("^", "**"), mode="eval")
    except SyntaxError as error:
        raise Excluded("bad expression", str(error)) from error
    try:
        return _evaluate_node(tree, {"pi": math.pi, **params})
    except (ValueError, ZeroDivisionError, OverflowError, TypeError) as error:
        raise Excluded("bad expression", str(error)) from error


def _number(value, params):
    if isinstance(value, bool):
        raise Excluded("bad value", repr(value))
    if isinstance(value, (int, float)):
        return value
    if isinstance(value, str):
        return evaluate(value, params)
    raise Excluded("bad value", repr(value))


def _vector(value, params):
    if not isinstance(value, list) or len(value) != 3:
        raise Excluded("bad value", repr(value))
    return [_number(item, params) for item in value]


def _fold(kind, name, target, tools):
    """An operation on many operands, as a chain of two-operand ones; the last op keeps the step's name."""
    ops = []
    previous = target
    for number, tool in enumerate(tools, start=1):
        op_id = name if number == len(tools) else f"{name}.{number}"
        ops.append({"op": kind, "id": op_id, "target": previous, "tool": tool})
        previous = op_id
    return ops


def _convert_step(step, params):
    kind, name = step.get("op"), step.get("name")
    if kind == "box":
        low, high = _vector(step["min"], params), _vector(step["max"], params)
        return [{"op": "box", "id": name, "size": [h - l for h, l in zip(high, low)], "origin": low}]
    if kind == "cylinder":
        return [
            {
                "op": "cylinder",
                "id": name,
                "radius": _number(step["radius"], params),
                "height": _number(step["height"], params),
                "origin": _vector(step["base"], params),
                "axis": _vector(step["axis"], params),
            }
        ]
    if kind == "fuse" and "bodies" in step:
        return _fold("fuse", name, step["bodies"][0], step["bodies"][1:])
    if kind in ("fuse", "common"):
        return [{"op": kind, "id": name, "target": step["a"], "tool": step["b"]}]
    if kind == "cut" and "tools" in step:
        return _fold("cut", name, step["target"], step["tools"])
    if kind == "cut":
        return [{"op": "cut", "id": name, "target": step["target"], "tool": step["tool"]}]
    raise Excluded("unsupported operation", str(kind))


def _closed_form(value, params, measure):
    if isinstance(value, bool) or isinstance(value, (int, float)):
        raise Excluded("bare number answer", measure)
    if not isinstance(value, str):
        raise Excluded("bad value", repr(value))
    return _number(value, params)


def _expect(analytic, params):
    expect = {"volume": {"value": generate.sig12(_closed_form(analytic["volume"], params, "volume")), "rel_tol": generate.REL_TOL}}
    if "area" in analytic:
        expect["area"] = {"value": generate.sig12(_closed_form(analytic["area"], params, "area")), "rel_tol": generate.REL_TOL}
    shells = (analytic.get("counts") or {}).get("shells")
    if shells is not None and "genus" in analytic:
        expect["euler"] = {"value": 2 * (shells - analytic["genus"])}
    return expect


def _gate(fixture, slug):
    """Raises Excluded with the reason a fixture stays out; returns None when it may be imported."""
    if any(key in fixture for key in ("source", "licence", "file", "sha256")):
        raise Excluded("third-party or separately licensed part")
    if slug.startswith("regression/"):
        raise Excluded("regression fixture: an arris failure waiting for its fix")
    if fixture.get("kind") in ("part", "geometry", "query"):
        raise Excluded("not a solid recipe", fixture["kind"])
    steps = fixture.get("steps") or []
    if not steps:
        raise Excluded("no recipe")
    for step in steps:
        kind = step.get("op")
        if kind == "step":
            raise Excluded("step file operand")
        if kind not in _SHAPE_OPS | _BOOLEAN_OPS:
            raise Excluded("unsupported operation", str(kind))
    analytic = fixture.get("analytic") or {}
    if "expect_error" in analytic or "degenerate" in analytic:
        raise Excluded("must fail by design")
    if analytic.get("volume") is None:
        raise Excluded("no closed-form volume")
    if isinstance(analytic["volume"], (int, float)) or isinstance(analytic.get("area"), (int, float)):
        raise Excluded("bare number answer")
    description = fixture.get("description", "")
    if _KERNEL.search(description):
        raise Excluded("names another kernel")
    if _UNCLEAR_ORIGIN.search(description):
        raise Excluded("unclear origin")


def _sanitise(name):
    return re.sub(r"[^A-Za-z0-9._-]", "-", name)


def _case(fixture, slug, commit, params, variant):
    ops = []
    for step in fixture["steps"]:
        ops.extend(_convert_step(step, params))
    case_id = "arris-" + slug.replace("/", "-") + (f"-{_sanitise(variant)}" if variant else "")
    note = f" variant {variant}" if variant else ""
    case = {
        "schema": generate.CASE_SCHEMA,
        "id": case_id,
        "origin": (
            f"arris-labs/arris@{commit} tests/fixtures/{slug}/fixture.json{note}: the recipe and its closed-form answer, "
            f"converted by tools/corpus/import_arris.py; {COPYRIGHT}, under {LICENCE} (LICENSE-MIT, LICENSE-APACHE at that commit)"
        ),
        "licence": LICENCE,
        "ops": ops,
        "result": fixture["result"],
        "expect": _expect(fixture["analytic"], params),
    }
    try:
        caseformat.validate_case(case)
    except caseformat.CaseError as error:
        raise Excluded("invalid after conversion", str(error)) from error
    return case


def convert_fixture(fixture, slug, commit):
    """The cases one fixture becomes (one per variant), or Excluded."""
    _gate(fixture, slug)
    params = fixture.get("params") or {}
    if not all(isinstance(value, (int, float)) and not isinstance(value, bool) for value in params.values()):
        raise Excluded("bad value", "non-numeric params")
    cases = [_case(fixture, slug, commit, params, None)]
    for name, overrides in sorted((fixture.get("variants") or {}).items()):
        cases.append(_case(fixture, slug, commit, {**params, **overrides}, name))
    return cases


def survey(fixtures, commit):
    """Every fixture under `fixtures`: the cases taken, and what was excluded by reason."""
    cases, exclusions = [], {}
    root = pathlib.Path(fixtures)
    for path in sorted(root.rglob("fixture.json")):
        slug = path.parent.relative_to(root).as_posix()
        try:
            cases.extend(convert_fixture(json.loads(path.read_text(encoding="utf-8")), slug, commit))
        except Excluded as excluded:
            exclusions.setdefault(excluded.category, []).append(slug)
    return cases, exclusions


def _locked_commit():
    document = json.loads(LOCK.read_text(encoding="utf-8"))
    return next(entry["commit"] for entry in document["git_sources"] if entry["name"] == "arris")


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("repo", type=pathlib.Path, help="a clone of arris at the commit pinned in sources.lock.json")
    parser.add_argument("--out", default=str(OUT))
    args = parser.parse_args(argv)
    commit = subprocess.run(["git", "-C", str(args.repo), "rev-parse", "HEAD"], check=True, capture_output=True, text=True).stdout.strip()
    if commit != _locked_commit():
        print(f"clone is at {commit}, sources.lock.json pins {_locked_commit()}", file=sys.stderr)
        return 1
    cases, exclusions = survey(args.repo / "tests" / "fixtures", commit)
    generate.write_cases(cases, args.out)
    print(f"imported {len(cases)} cases from {commit}")
    for category, slugs in sorted(exclusions.items()):
        print(f"excluded {len(slugs):>4}  {category}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
