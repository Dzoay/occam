"""Comparison and invariants (L11, L12): geometric, with tolerances, never by topology or bits.

`check_case` compares a result with a case's `expect` and applies the invariants that need no engine. The invariants
decide disagreements: a closed-form answer or an invariant, not whichever engine spoke last.
"""

IDENTITY_REL_TOL = 1e-6


def _report(case_id, measures, invariants):
    checks = [*measures, *invariants]
    return {"id": case_id, "pass": all(entry["pass"] for entry in checks), "measures": measures, "invariants": invariants}


def _measure(name, passed, expected, actual, **detail):
    return {"measure": name, "pass": bool(passed), "expected": expected, "actual": actual, **detail}


def _invariant(name, passed, detail):
    return {"invariant": name, "pass": bool(passed), "detail": detail}


def _compare_number(name, spec, actual):
    allowed = max(spec.get("rel_tol", 0.0) * abs(spec["value"]), spec.get("abs_tol", 0.0))
    delta = abs(actual - spec["value"])
    return _measure(name, delta <= allowed, spec["value"], actual, delta=delta, tolerance=allowed)


def _compare_bbox(spec, actual):
    if actual is None:
        return _measure("bbox", False, spec, None, detail="the result has no bounding box")
    deltas = [abs(got - want) for got, want in zip(actual["min"] + actual["max"], spec["min"] + spec["max"])]
    return _measure(
        "bbox",
        max(deltas) <= spec["abs_tol"],
        {"min": spec["min"], "max": spec["max"]},
        actual,
        delta=max(deltas),
        tolerance=spec["abs_tol"],
    )


def _compare_watertight(spec, tessellations):
    if not tessellations:
        return _measure("watertight", False, spec["value"], None, detail="the case tessellates nothing")
    actual = all(stats["watertight"] for stats in tessellations.values())
    return _measure("watertight", actual == spec["value"], spec["value"], actual)


def _compare_expectations(expect, result):
    shape, measures = result["shape"], []
    for name in ("volume", "area"):
        if name in expect:
            measures.append(_compare_number(name, expect[name], shape[name]))
    if "bbox" in expect:
        measures.append(_compare_bbox(expect["bbox"], shape["bbox"]))
    for name in ("euler", "valid", "closed"):
        if name in expect:
            measures.append(_measure(name, shape[name] == expect[name]["value"], expect[name]["value"], shape[name]))
    if "watertight" in expect:
        measures.append(_compare_watertight(expect["watertight"], result["tessellations"]))
    return measures


def _expects_invalid(expect):
    """A case waives the shape invariants only by declaring that it expects an invalid shape (`"valid": false`)."""
    return expect.get("valid", {}).get("value", True) is False


def _shape_invariants(expect, shape):
    if _expects_invalid(expect):
        return []
    counts = shape["counts"]
    computed = counts["vertices"] - counts["edges"] + 2 * counts["faces"] - counts["wires"]
    invariants = [
        _invariant("valid", shape["valid"], "the engine's checker accepts the result"),
        _invariant("closed", shape["closed"], "every non-degenerate edge bounds an even number of face sides"),
        _invariant("euler_counts", shape["euler"] == computed, f"V - E + 2F - L is {computed}, the result says {shape['euler']}"),
    ]
    if expect.get("volume", {}).get("value", 1.0) != 0.0:
        invariants.append(_invariant("volume_positive", shape["volume"] > 0.0, f"volume is {shape['volume']}"))
    return invariants


def check_case(case, result):
    """Compares `result` with `case`'s `expect` and applies the invariants. Reports each one."""
    case_id = case["id"]
    expect = case.get("expect", {})
    if result.get("id") != case_id:
        wrong = _measure("status", False, case_id, result.get("id"), detail="the result belongs to another case")
        return _report(case_id, [wrong], [])
    if "error" in expect:
        got = result.get("error", {}).get("op") if result["status"] == "error" else None
        return _report(case_id, [_measure("error", got == expect["error"]["op"], expect["error"]["op"], got)], [])
    if result["status"] != "ok":
        failed = _measure("status", False, "ok", result["status"], detail=result["error"])
        return _report(case_id, [failed], [])
    return _report(case_id, _compare_expectations(expect, result), _shape_invariants(expect, result["shape"]))


def check_volume_identity(group, members):
    """vol(A) + vol(B) = vol(A union B) + vol(A intersect B), when a case set provides all four results.

    `members` maps a role (`a`, `b`, `union`, `intersection`) to a result. A group that is missing roles is skipped
    (`pass` is None); a group with a failed member fails.
    """
    roles = ("a", "b", "union", "intersection")
    missing = [role for role in roles if role not in members]
    if missing:
        return {"group": group, "pass": None, "detail": f"skipped: the case set does not provide {', '.join(missing)}"}
    failed = [role for role in roles if members[role].get("status") != "ok"]
    if failed:
        return {"group": group, "pass": False, "detail": f"no measurement for {', '.join(failed)}"}
    volume = {role: members[role]["shape"]["volume"] for role in roles}
    left, right = volume["a"] + volume["b"], volume["union"] + volume["intersection"]
    scale = max(abs(item) for item in volume.values()) or 1.0
    allowed = IDENTITY_REL_TOL * scale
    return {
        "group": group,
        "pass": abs(left - right) <= allowed,
        "detail": f"vol(a)+vol(b)={left}, vol(union)+vol(intersection)={right}",
        "delta": abs(left - right),
        "tolerance": allowed,
    }


def compare_results(left, right, rel_tol=1e-6, bbox_tol=1e-3):
    """Whether two engines' results for one case agree: volume, area and box within tolerance, and the same validity.

    Never topology (L12). `bbox_tol` is relative to the box diagonal, because a second opinion's box may be measured on
    a mesh. `agree` is None when an engine does not offer the operation at all: nothing was compared. A case one engine
    could do and the other failed is a disagreement to look at, not a pass.
    """
    if left["status"] != "ok" or right["status"] != "ok":
        states = f"{left['oracle']['engine']}: {_status(left)}; {right['oracle']['engine']}: {_status(right)}"
        unsupported = "op_unsupported" in (left.get("error", {}).get("code"), right.get("error", {}).get("code"))
        return {"agree": None if unsupported else left["status"] == right["status"], "detail": states}
    a, b = left["shape"], right["shape"]
    problems = []
    for name in ("volume", "area"):
        if abs(a[name] - b[name]) > rel_tol * max(abs(a[name]), abs(b[name]), 1e-12):
            problems.append(f"{name} {a[name]} vs {b[name]}")
    if a["valid"] != b["valid"]:
        problems.append(f"valid {a['valid']} vs {b['valid']}")
    if a["bbox"] and b["bbox"]:
        corners = list(zip(a["bbox"]["min"] + a["bbox"]["max"], b["bbox"]["min"] + b["bbox"]["max"]))
        diagonal = sum((high - low) ** 2 for low, high in zip(a["bbox"]["min"], a["bbox"]["max"])) ** 0.5 or 1.0
        if max(abs(p - q) for p, q in corners) > bbox_tol * diagonal:
            problems.append(f"bbox {a['bbox']} vs {b['bbox']}")
    return {"agree": not problems, "detail": "; ".join(problems) or f"volume {a['volume']}, area {a['area']}"}


def _status(result):
    if result["status"] == "ok":
        return "ok"
    return f"{result['error']['code']} at {result['error']['op']}: {result['error']['message']}"
