"""Count the Open CASCADE names FreeCAD uses, per FreeCAD module, at a pinned commit.

Survey tool: see docs/decisions/2026-11-08-licence-and-provenance.md. It is developed on synthetic fixtures, so its author
never reads FreeCAD's code, and only the resulting table is read. It prints nothing from a source file but names and counts.

    uvx --python 3.13 --from cadquery-ocp==8.0.1.1.0 python tools/census/occt_names.py <names file>
    python3 tools/census/freecad_occt_usage.py --root <FreeCAD checkout> --commit <sha> --tag <tag> \
        --names <names file> --out docs/evidence/<date>-freecad-occt-usage.md

The names file lists what the pinned OCCT wheel exports, which is how a name is told apart from a same-styled name of
another project, such as the bundled SMESH or FreeCAD's own `*Py` wrappers. It is generated, not committed.

Class level only, by tokens: no parse, so a name in a disabled `#if` branch counts and a typedef is not followed.
"""

import argparse
import re
import sys
from collections import Counter, defaultdict
from dataclasses import dataclass, field
from pathlib import Path

SOURCE_SUFFIXES = {".c", ".cc", ".cpp", ".cxx", ".h", ".hh", ".hpp", ".hxx", ".inl", ".ipp", ".tpp", ".mm"}
SKIPPED_DIRS = {".git", "build", "node_modules"}

INCLUDE = re.compile(r'^[ \t]*#[ \t]*include[ \t]*[<"]([^>"\n]+)[>"]', re.MULTILINE)
INCLUDE_LINE = re.compile(r"^[ \t]*#[ \t]*include.*$", re.MULTILINE)
NAME = re.compile(r"\b[A-Za-z][A-Za-z0-9]*(?:_[A-Za-z0-9]+)+\b")
STATIC_CALL = re.compile(r"\b([A-Za-z][A-Za-z0-9_]*)\s*::\s*([A-Za-z_][A-Za-z0-9_]*)\s*\(")
RAW_STRING_OPEN = re.compile(r'R"([^ ()\\\t\n]{0,16})\(')


@dataclass
class Census:
    files_scanned: int = 0
    uses: dict = field(default_factory=lambda: defaultdict(Counter))
    includes: dict = field(default_factory=lambda: defaultdict(Counter))
    static_calls: dict = field(default_factory=lambda: defaultdict(Counter))
    unrecognised_includes: Counter = field(default_factory=Counter)
    not_in_wheel: dict = field(default_factory=lambda: defaultdict(Counter))


def _is_identifier_char(char):
    return char.isalnum() or char == "_"


def _skip_string(text, start, quote):
    """Index just past the literal opening at `start`, stopping at an unterminated line."""
    i = start + 1
    while i < len(text) and text[i] != "\n":
        if text[i] == "\\":
            i += 2
            continue
        if text[i] == quote:
            return i + 1
        i += 1
    return i


def _blank(out, text, start, end):
    out.extend("\n" if char == "\n" else " " for char in text[start:end])


def strip_source(text, keep_strings=False):
    """`text` with comments blanked, and string and character literals too unless `keep_strings`. Newlines are kept."""
    out = []
    i = 0
    n = len(text)
    while i < n:
        char = text[i]
        pair = text[i : i + 2]
        if pair == "//":
            end = text.find("\n", i)
            end = n if end < 0 else end
            _blank(out, text, i, end)
            i = end
        elif pair == "/*":
            close = text.find("*/", i + 2)
            end = n if close < 0 else close + 2
            _blank(out, text, i, end)
            i = end
        elif char == '"' or (char == "R" and text[i + 1 : i + 2] == '"' and (i == 0 or not _is_identifier_char(text[i - 1]))):
            raw = RAW_STRING_OPEN.match(text, i) if char == "R" else None
            if raw:
                terminator = ")" + raw.group(1) + '"'
                close = text.find(terminator, raw.end())
                end = n if close < 0 else close + len(terminator)
            elif char == "R":
                end = _skip_string(text, i + 1, '"')
            else:
                end = _skip_string(text, i, '"')
            if keep_strings:
                out.append(text[i:end])
            else:
                _blank(out, text, i, end)
            i = end
        elif char == "'" and (i == 0 or not _is_identifier_char(text[i - 1])):
            end = _skip_string(text, i, "'")
            if keep_strings:
                out.append(text[i:end])
            else:
                _blank(out, text, i, end)
            i = end
        else:
            out.append(char)
            i += 1
    return "".join(out)


def module_of(relative):
    """The FreeCAD module a path under the checkout belongs to."""
    parts = relative.parts
    if parts[0] == "tests":
        return "tests"
    if parts[0] != "src" or len(parts) < 3:
        return parts[0]
    if parts[1] == "Mod":
        return parts[2] if len(parts) > 3 else "Mod"
    if parts[1] == "3rdParty":
        return f"3rdParty/{parts[2]}"
    return parts[1]


def is_counted(module):
    """Whether a module's uses are FreeCAD's own: tests and bundled third-party code are reported but not ranked."""
    return module != "tests" and not module.startswith("3rdParty/")


def _package(name):
    return name.split("_", 1)[0]


def _normalise(name):
    return name.removeprefix("Handle_")


def _scan_text(text, module, names, packages, census):
    kept = strip_source(text, keep_strings=True)
    for include in INCLUDE.findall(kept):
        base = include.rsplit("/", 1)[-1]
        if not base.endswith(".hxx"):
            continue
        name = _normalise(base.removesuffix(".hxx"))
        if _package(name) in packages:
            census.includes[name][module] += 1
        else:
            census.unrecognised_includes[_package(name)] += 1
    code = INCLUDE_LINE.sub("", strip_source(text))
    for match in NAME.finditer(code):
        name = _normalise(match.group(0))
        if _package(name) not in packages:
            continue
        (census.uses if name in names else census.not_in_wheel)[name][module] += 1
    for match in STATIC_CALL.finditer(code):
        owner = _normalise(match.group(1))
        if code[max(0, match.start() - 2) : match.start()] == "::" or owner not in names:
            continue
        census.static_calls[f"{owner}::{match.group(2)}"][module] += 1
        if "_" not in owner:
            census.uses[owner][module] += 1


def scan(root, names):
    """Count every OCCT name under `root`, per module. `names` is what the pinned wheel exports."""
    census = Census()
    root = Path(root)
    packages = {_package(name) for name in names}
    for path in sorted(root.rglob("*")):
        relative = path.relative_to(root)
        if SKIPPED_DIRS.intersection(relative.parts) or path.suffix.lower() not in SOURCE_SUFFIXES or not path.is_file():
            continue
        census.files_scanned += 1
        text = path.read_text(encoding="utf-8", errors="replace")
        _scan_text(text, module_of(relative), names, packages, census)
    return census


def ranked(uses):
    """(name, uses in FreeCAD's own modules), most used first, leaving out names used only by tests or third-party code."""
    totals = ((name, sum(n for module, n in counts.items() if is_counted(module))) for name, counts in uses.items())
    return sorted(((name, total) for name, total in totals if total > 0), key=lambda item: (-item[1], item[0]))


def _spread(counts):
    """`Part 2, PartDesign 2; tests 2`: FreeCAD's own modules, then the rest after a semicolon."""
    def join(items):
        return ", ".join(f"{m} {n}" for m, n in sorted(items, key=lambda item: (-item[1], item[0])))

    own = [(m, n) for m, n in counts.items() if is_counted(m)]
    rest = [(m, n) for m, n in counts.items() if not is_counted(m)]
    return join(own) + (f"; {join(rest)}" if rest else "")


def _names_table(census):
    lines = ["| Name | Uses | Includes | Per module |", "|---|---:|---:|---|"]
    for name, total in ranked(census.uses):
        included = sum(census.includes[name].values()) if name in census.includes else 0
        lines.append(f"| `{name}` | {total} | {included} | {_spread(census.uses[name])} |")
    return lines


def _static_table(census):
    lines = ["| Call | Uses | Per module |", "|---|---:|---|"]
    for name, total in ranked(census.static_calls):
        lines.append(f"| `{name}` | {total} | {_spread(census.static_calls[name])} |")
    return lines


def _modules_table(census):
    names = Counter()
    uses = Counter()
    for counts in census.uses.values():
        for module, n in counts.items():
            names[module] += 1
            uses[module] += n
    lines = ["| Module | Distinct names | Uses | Counted in the ranking |", "|---|---:|---:|---|"]
    for module, total in sorted(uses.items(), key=lambda item: (-item[1], item[0])):
        lines.append(f"| {module} | {names[module]} | {total} | {'yes' if is_counted(module) else 'no'} |")
    return lines


def _not_in_wheel_table(census):
    lines = ["| Name | Uses | Per module |", "|---|---:|---|"]
    for name, total in ranked(census.not_in_wheel):
        lines.append(f"| `{name}` | {total} | {_spread(census.not_in_wheel[name])} |")
    return lines


def _unrecognised_table(census):
    lines = ["| Prefix | Includes |", "|---|---:|"]
    for prefix, n in sorted(census.unrecognised_includes.items(), key=lambda item: (-item[1], item[0])):
        lines.append(f"| `{prefix}` | {n} |")
    return lines


def render(census, commit, tag):
    """The evidence file."""
    ranking = ranked(census.uses)
    lines = [
        "# Open CASCADE names FreeCAD uses",
        "",
        f"FreeCAD {tag}, commit `{commit}`. Made by `tools/census/freecad_occt_usage.py`, a script developed on synthetic",
        "fixtures, so that only this table was read. Input to `census.parity`; class-level detail only.",
        "",
        "## Method and limits",
        "",
        f"- Scanned {census.files_scanned} C and C++ source files. Comments and string literals are not counted. `#include` lines count as includes, not uses.",
        "- A name counts as OCCT only if the pinned OCCT wheel exports it, which includes enumerators. A name in a disabled `#if` branch counts, since nothing is parsed.",
        "- Static calls count `Class::method(` where the class is named at the call site, not calls through an object.",
        "- Ranking and totals count FreeCAD's own modules. `tests` and bundled `3rdParty` code appear after the semicolon and are not ranked.",
        "",
        f"## Names by use ({len(ranking)} names)",
        "",
        *_names_table(census),
        "",
        "## Static calls",
        "",
        *_static_table(census),
        "",
        "## Modules",
        "",
        *_modules_table(census),
        "",
        "## Names in OCCT's style that the wheel does not export",
        "",
        "Set aside rather than counted: FreeCAD's own wrappers such as `*Py` classes, foundation typedefs and macros, container typedefs the bindings do not export under that name, and any name since removed from OCCT.",
        "",
        *_not_in_wheel_table(census),
        "",
        "## `.hxx` includes outside the package list",
        "",
        "Not OCCT by this list, so not counted above. Prefixes only.",
        "",
        *_unrecognised_table(census),
        "",
    ]
    return "\n".join(lines)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--root", required=True, help="the FreeCAD checkout")
    parser.add_argument("--commit", required=True, help="the commit it is at")
    parser.add_argument("--tag", required=True, help="the release tag")
    parser.add_argument("--names", required=True, help="the names the pinned OCCT wheel exports, one per line")
    parser.add_argument("--out", required=True, help="the Markdown file to write")
    args = parser.parse_args(argv)
    names = {line.strip() for line in Path(args.names).read_text(encoding="utf-8").splitlines() if line.strip()}
    census = scan(args.root, names)
    Path(args.out).write_text(render(census, args.commit, args.tag), encoding="utf-8")
    print(f"{census.files_scanned} files, {len(ranked(census.uses))} names, {len(ranked(census.static_calls))} static calls")
    return 0


if __name__ == "__main__":
    sys.exit(main())
