"""The divergence register: every disagreement between oracles or a closed-form answer, and what settled it."""

import re

SCHEMA = "occam-oracle-divergences/1"

VERDICTS = {"occt-wrong", "closed-form-wrong", "second-opinion-wrong", "undecided"}
# The invariant or arbiter that decided the entry; `none` only with the verdict `undecided`.
DECIDED_BY = {"closed-form", "valid", "closed", "volume-positive", "euler", "volume-identity", "watertight", "none"}

_SOURCE = re.compile(r"^[a-z][a-z0-9-]*$")
_FIELDS = {"case", "measure", "values", "decided_by", "verdict", "note"}


def _number_or_flag(value):
    return isinstance(value, (bool, int, float))


def _check_entry(entry, where):
    if not isinstance(entry, dict):
        return [f"{where}: expected an object"]
    errors = []
    missing = sorted(_FIELDS - set(entry))
    extra = sorted(set(entry) - _FIELDS)
    if missing:
        errors.append(f"{where}: missing {', '.join(missing)}")
    if extra:
        errors.append(f"{where}: unknown field(s) {', '.join(extra)}")
    if missing or extra:
        return errors
    for key in ("case", "measure", "note"):
        if not isinstance(entry[key], str) or not entry[key].strip():
            errors.append(f"{where}.{key}: expected a non-empty string")
    values = entry["values"]
    if not isinstance(values, dict) or len(values) < 2:
        errors.append(f"{where}.values: expected at least two sources")
    else:
        if not all(_SOURCE.match(source) for source in values):
            errors.append(f"{where}.values: a source name is not a lower-case slug")
        if not all(_number_or_flag(value) for value in values.values()):
            errors.append(f"{where}.values: every value must be a number or a boolean")
    if entry["decided_by"] not in DECIDED_BY:
        errors.append(f"{where}.decided_by: unknown {entry['decided_by']!r}")
    if entry["verdict"] not in VERDICTS:
        errors.append(f"{where}.verdict: unknown {entry['verdict']!r}")
    if (entry["verdict"] == "undecided") != (entry["decided_by"] == "none"):
        errors.append(f"{where}: `decided_by: none` and the verdict `undecided` go together")
    return errors


def check_register(document):
    """The problems with a parsed register, or an empty list if it is well-formed."""
    if not isinstance(document, dict):
        return ["the register must be a JSON object"]
    errors = []
    if document.get("schema") != SCHEMA:
        errors.append(f"schema: expected {SCHEMA}")
    entries = document.get("entries")
    if not isinstance(entries, list):
        return [*errors, "entries: expected a list"]
    keys = []
    for index, entry in enumerate(entries):
        where = f"entries[{index}]"
        problems = _check_entry(entry, where)
        errors.extend(problems)
        if not problems:
            keys.append((entry["case"], entry["measure"]))
    if len(set(keys)) != len(keys):
        errors.append("entries: a case and measure appears twice")
    if keys != sorted(keys):
        errors.append("entries: must be sorted by case, then measure")
    return errors
