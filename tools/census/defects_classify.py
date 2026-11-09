#!/usr/bin/env python3
"""Rule-based failure classes over stripped issue reports, for `census.defects`.

Works only on the output of `defects_strip.py`. Each class is a set of topic patterns. An issue belongs to a class when
a topic appears in its title, or when a topic and a failure word appear in one sentence of its body. Deterministic: no
randomness, no learning, no model. Counts are multi-label, because one report can have several symptoms.

Python standard library only.
"""

import argparse
import hashlib
import json
import re
import sys
from dataclasses import dataclass
from pathlib import Path

DEFAULT_IN = Path.home() / ".cache" / "occam-census" / "issues-stripped" / "issues.jsonl"


def ci(*patterns):
    return [re.compile(pattern, re.IGNORECASE) for pattern in patterns]


def cs(*patterns):
    return [re.compile(pattern) for pattern in patterns]


@dataclass(frozen=True)
class FailureClass:
    id: str
    label: str
    topics: tuple
    exclude: tuple = ()
    title_needs_failure: bool = False


FAILURE_WORDS = ci(
    r"\bfail(?:s|ed|ure|ures|ing)?\b",
    r"\berrors?\b",
    r"\bcrash(?:es|ed|ing)?\b",
    r"\bwrong(?:ly)?\b",
    r"\bincorrect(?:ly)?\b",
    r"\binvalid\b",
    r"\bbroken\b",
    r"\bbreaks?\b",
    r"\bbug(?:s|gy)?\b",
    r"\bdoes(?:n'?t| not)\b",
    r"\bdo(?:n'?t| not)\b",
    r"\bcan(?:'?t|not)\b",
    r"\bunable\b",
    r"\bnot working\b",
    r"\bno longer\b",
    r"\bhangs?\b",
    r"\bexception\b",
    r"\bproblems?\b",
    r"\bissues?\b",
    r"\bunexpected(?:ly)?\b",
    r"\bmissing\b",
    r"\bcorrupt(?:ed|ion)?\b",
    r"\bgarbage\b",
    r"\bbad\b",
    r"\bslow\b",
    r"\bloses?\b",
    r"\blost\b",
    r"\bdisappears?\b",
    r"\bempty\b",
)

CLASSES = (
    FailureClass(
        "boolean",
        "Boolean operations fail or give wrong results (fuse, cut, common)",
        ci(
            r"\bbooleans?\b(?!\s+(?:propert|value|flag|type|parameter|expression|variable|field|attribute|option|setting|logic))",
            r"\bfuse[sd]?\b",
            r"\bfusing\b",
            r"\bfusion\s+of\b",
            r"\bunion\b",
            r"(?<!section )\bcut\s+(?:of|between|with|operation|result|feature|fails?|failed|gives?|returns?|produces?|crash(?:es)?|tool|body|bodies|shape|solids?)\b",
            r"\bboolean\s+cut\b",
            r"\b(?:can(?:'?t|not)|fails? to|unable to)\s+cut\b",
            r"\bcommon\s+(?:of|between|operation|result|fails?|failed|gives?|returns?|produces?)\b",
            r"\bboolean\s+common\b",
            r"\bgeneral(?:\s+fuse)\b",
        ),
    ),
    FailureClass("fillet-chamfer", "Fillet and chamfer failures", ci(r"\bfillet(?:s|ed|ing)?\b", r"\bchamfer(?:s|ed|ing)?\b")),
    FailureClass(
        "tolerance-gap",
        "Tolerance, gap and near-coincidence trouble",
        ci(
            r"\btolerances?\b",
            r"\bgaps?\b",
            r"\bnot closed\b",
            r"\bopen (?:wire|shell)\b",
            r"\bnot watertight\b",
            r"\bcoincid\w+\b",
            r"\bsliver\b",
            r"\bcoplanar\b",
            r"\bnear[- ]?tangent\b",
        ),
    ),
    FailureClass(
        "tessellation",
        "Tessellation and meshing defects",
        ci(r"\btessellat\w+\b", r"\btriangulat\w+\b", r"\bmesh(?:es|ing|ed)?\b", r"\bdegenerate triangles?\b"),
    ),
    FailureClass(
        "shape-healing",
        "Shape healing and repair (sewing, fixing shapes)",
        ci(r"\bheal\w*\b", r"\bsew(?:ing|ed|n)?\b", r"\bfix\s+shape\b", r"\bshape[- ]?fix\w*\b", r"\brepair(?:s|ed|ing)?\b", r"\brefine\s+shape\b", r"\bremove\s+splitter\b"),
    ),
    FailureClass(
        "invalid-shape",
        "Operations return invalid shapes",
        ci(
            r"\binvalid\s+(?:shape|solid|face|wire|shell|result|geometry|body)\b",
            r"\bself[- ]?intersect\w*\b",
            r"\bnon[- ]?manifold\b",
            r"\bcheck geometry\b",
            r"\bnot a (?:valid )?solid\b",
            r"\bbad (?:solid|shape|face|geometry)\b",
        ),
    ),
    FailureClass(
        "topo-naming",
        "Topological naming: references to faces and edges break after an edit",
        ci(
            r"\btopo(?:logical)?\s*-?\s*naming\b",
            r"\btoponaming\b",
            r"\btnp\b",
            r"\battached to (?:the )?wrong\b",
            r"\bwrong (?:face|edge|vertex) after\b",
            r"\bloses?\s+(?:its\s+)?(?:attachment|references?)\b",
            r"\breferences?\s+(?:is |are |got |get )?(?:lost|broken|wrong)\b",
            r"\belement names?\b",
        ),
    ),
    FailureClass(
        "exchange",
        "STEP, IGES and BREP import and export",
        cs(r"\bSTEP\b", r"\bIGES\b", r"\bSTP\b")
        + ci(r"\bstep\s+(?:files?|import|export|model|format|reader|writer)\b", r"\.(?:step|stp|iges|igs)\b", r"\biges\b", r"(?<!\.)\bbrep\s+files?\b"),
    ),
    FailureClass(
        "exchange-other",
        "Other formats (STL, OBJ, DXF, 3MF, SVG, glTF, IFC)",
        cs(r"\bSTL\b", r"\bOBJ\b", r"\bDXF\b", r"\bDWG\b", r"\b3MF\b", r"\bIFC\b", r"\bSVG\b") + ci(r"\bgltf\b", r"\bstl\s+(?:files?|export|import)\b"),
    ),
    FailureClass(
        "sweep-loft",
        "Sweep, pipe and loft",
        ci(r"\bsweep(?:s|ing)?\b", r"\bloft(?:s|ed|ing)?\b", r"\bpipe\s*shell\b", r"\bpipe\b", r"\bthru\s*sections?\b"),
    ),
    FailureClass(
        "offset-shell",
        "Offset, shell and thickness",
        ci(
            r"\boffset\s+(?:of|fails?|failed|operation|shape|face|surface|curve|wire|distance|mode|produces?)\b",
            r"\b(?:2d|3d|negative|fill)\s+offset\b",
            r"\boffsetting\b",
            r"\bthick(?:ness|en|ening)\b",
            r"(?<!python )(?<!pipe )(?<!command )\bshell(?:ing)?\b(?!\s+(?:script|command))",
        ),
        exclude=tuple(cs(r"\bIGES\b", r"\bSTEP\b") + ci(r"\bstep\s+files?\b", r"\bentity\b", r"\blevels?\s+of\b")),
    ),
    FailureClass("draft", "Draft angles", ci(r"\bdraft\s+(?:angle|feature|operation|tool)\b", r"\bdrafted\b", r"\bapply(?:ing)?\s+draft\b")),
    FailureClass(
        "extrude-revolve",
        "Extrude, revolve, pad and pocket",
        ci(r"\bextru(?:de|des|ded|ding|sion)s?\b", r"\brevol(?:ve|ves|ved|ving|ution)s?\b", r"\bpad(?:s|ding)?\b", r"\bpockets?\b", r"\bgrooves?\b"),
        title_needs_failure=True,
    ),
    FailureClass(
        "sketch",
        "Sketcher and constraint solving",
        ci(r"\bsketcher\b", r"\bsolver\b", r"\bredundant\b", r"\bconflicting\b", r"\b(?:over|under)[- ]?constrained\b", r"\bconstraints?\b"),
    ),
    FailureClass(
        "pattern-transform",
        "Pattern, mirror and transformation features",
        ci(r"\bpatterns?\b", r"\bmirror(?:s|ed|ing)?\b", r"\bmulti\s*-?\s*transform\b"),
        title_needs_failure=True,
    ),
    FailureClass(
        "intersection",
        "Curve and surface intersection, section and projection",
        ci(
            r"(?<!self-)(?<!self )\bintersect(?:s|ion|ions|ing)?\b",
            r"\bsection\s+of\b",
            r"\bcross[- ]section\w*\b",
            r"\bsectioning\b",
            r"\bsection\s+(?:fails?|plane|cut)\b",
            r"\bproject(?:ing|ion|s|ed)?\b[^.]{0,30}\b(?:curve|edge|wire|point|shape|sketch|surface|face)\b",
        ),
    ),
    FailureClass(
        "curves-nurbs",
        "Curves, B-splines and NURBS",
        ci(r"\bb-?splines?\b", r"\bnurbs\b", r"\bbezier\b", r"\bapproximat(?:e|ion|ing)\b", r"\binterpolat(?:e|ion|ing)\b"),
    ),
    FailureClass(
        "measure",
        "Wrong measurements: volume, area, inertia, bounding box",
        ci(r"\bvolume\b", r"\bsurface area\b", r"\binertia\b", r"\bcent(?:er|re) of (?:mass|gravity)\b", r"\bcentroid\b", r"\bbounding\s*box\b", r"\bbbox\b", r"\bquick\s+measure\b", r"\bmeasure\s+tool\b"),
    ),
    FailureClass(
        "unhelpful-failure",
        "Silent or unhelpful failure: empty result, no reason given",
        ci(
            r"\bno error message\b",
            r"\bfails? (?:silently|without (?:any )?(?:error|message|reason))\b",
            r"\bsilent(?:ly)?\s+fail\w*\b",
            r"\bempty (?:result|shape|solid|compound)\b",
            r"\bnull\s+(?:shape|result)\b",
            r"\bunhelpful\b",
            r"\berror message\b[^.]{0,40}\b(?:unclear|misleading|wrong|cryptic|confusing)\b",
        ),
    ),
    FailureClass(
        "crash",
        "Crashes (segfault, abort, access violation)",
        ci(r"\bcrash(?:es|ed|ing)?\b", r"\bsegfault(?:s|ed)?\b", r"\bsegmentation fault\b", r"\baccess violation\b", r"\bsigsegv\b", r"\bsigabrt\b", r"\bcore dump(?:ed)?\b", r"\babort(?:s|ed)\b", r"\bstack overflow\b"),
    ),
    FailureClass(
        "hang",
        "Hangs, freezes and infinite loops",
        ci(r"\bhang(?:s|ing)?\b", r"\bfreez(?:e|es|ing)\b", r"\binfinite loop\b", r"\bnever (?:finishes|returns|completes)\b", r"\bunresponsive\b", r"\bnot responding\b", r"\bdeadlock\b"),
    ),
    FailureClass(
        "performance",
        "Performance: slow operations",
        ci(r"\bslow(?:er|ly|ness|down|s)?\b", r"\bperformance\b", r"\btakes? (?:hours|minutes|forever|ages|a long time)\b", r"\bbottleneck\b", r"\bsluggish\b"),
    ),
    FailureClass(
        "memory",
        "Memory leaks and exhaustion",
        ci(
            r"\bmemory\s+(?:leak\w*|usage|consumption|error|corruption|overrun|overflow|footprint|access|exhaust\w*)\b",
            r"\b(?:huge|large|excessive|high)\s+memory\b",
            r"\bleak(?:s|ing|age)?\s+(?:of\s+)?memory\b",
            r"\boom\b",
            r"\bout[- ]of[- ]memory\b",
        ),
    ),
    FailureClass(
        "nondeterminism",
        "Non-deterministic results and thread-safety",
        ci(r"\bnon[- ]?determinis\w+\b", r"\bnot determinis\w+\b", r"\bdifferent results?\b", r"\binconsistent results?\b", r"\brace conditions?\b", r"\bdata races?\b", r"\bthread[- ]?safe\w*\b", r"\bmulti-?thread\w*\b"),
    ),
    FailureClass(
        "persistence",
        "Native file save and load failures",
        ci(r"\b(?:file|document)s?\s+(?:is\s+|are\s+)?corrupt\w*\b", r"\bcorrupt\w*\s+(?:file|document)s?\b", r"\b(?:cannot|can'?t|fails? to|unable to)\s+(?:open|load|save)\b", r"\bsaving\s+(?:the\s+)?(?:file|document|model)\s+(?:fails?|failed)\b"),
        exclude=tuple(cs(r"\bSTEP\b", r"\bIGES\b", r"\bSTL\b", r"\bOBJ\b", r"\bBREP\b") + ci(r"\bstep\s+files?\b", r"\.(?:step|stp|iges|igs|stl|obj|brep)\b", r"\bexported\b")),
    ),
    FailureClass(
        "units",
        "Units and unit conversion",
        ci(r"(?<!texture )\bunits?\b(?!\s+tests?)", r"\bunit conversion\b", r"\bmillimet\w+\b", r"\binch(?:es)?\b"),
    ),
    FailureClass("drawing", "Hidden-line drawings and TechDraw", ci(r"\btechdraw\b", r"\bhidden[- ]line\b", r"\bhlr\b")),
    FailureClass("placement-attachment", "Placement and attachment of features", ci(r"\bplacements?\b", r"\battachment\s+(?:editor|modes?|offset|support)\b", r"\battaching\b")),
    FailureClass("recompute", "Feature-tree recompute failures", ci(r"\brecompute\s+(?:fails?|failed|error)\b", r"\bfail(?:s|ed|ure)?\s+to\s+recompute\b", r"\brecomput\w+\s+failed\b")),
    FailureClass(
        "visualization",
        "3D view and rendering",
        ci(r"\b3d view\b", r"\bviewport\b", r"\brender(?:ing|ed|er|s)?\b", r"\bdisplay(?:s|ed|ing)?\b", r"\bshading\b", r"\bopengl\b", r"\bvisuali[sz]ation\b", r"\bcoin3?d\b"),
    ),
    FailureClass(
        "gui",
        "Application interface: dialogs, panels, menus",
        ci(r"\bdialogs?\b", r"\bbuttons?\b", r"\btoolbars?\b", r"\bmenus?\b", r"\bwidgets?\b", r"\btask panel\b", r"\bpreferences?\b", r"\bcombo ?box\b", r"\btooltips?\b", r"\bdock\b", r"\bgui\b", r"\bworkbenches?\b", r"\bui\b"),
    ),
    FailureClass(
        "build",
        "Build, compile and packaging",
        ci(
            r"\bbuild\s+(?:fails?|failed|failure|errors?|system|breaks?)\b",
            r"\b(?:fails?|failed|failure|unable|cannot|can'?t|not)\s+(?:to\s+)?build\b",
            r"\bcompil(?:e|es|ed|er|ing|ation)\b",
            r"\bcmake\b",
            r"\blink(?:er|ing)\b",
            r"\bpackag(?:e|es|ing)\s+(?:fails?|failed|errors?)\b",
            r"\binstall(?:er|ation)?\s+(?:fails?|failed|errors?)\b",
        ),
    ),
    FailureClass(
        "python-api",
        "Python bindings, macros and the scripting API",
        ci(r"\bpython\b", r"\bbindings?\b", r"\bmacros?\b", r"\bapi\b", r"\bscripting\b"),
    ),
    FailureClass("out-assembly", "Assembly workbench (not a kernel concern)", ci(r"\bassembl(?:y|ies)\b", r"\bjoints?\b")),
    FailureClass("out-fem", "Finite element analysis (not a kernel concern)", ci(r"\bfem\b", r"\bfinite element\b", r"\bcalculix\b", r"\bgmsh\b", r"\bnetgen\b")),
    FailureClass("out-cam", "CAM and toolpaths (not a kernel concern)", ci(r"\bcam\b", r"\bg-?code\b", r"\btoolpaths?\b")),
    FailureClass("out-bim", "Architecture and BIM (not a kernel concern)", ci(r"\bbim\b", r"\barch\b", r"\bifc\b")),
)

CLASS_LABELS = {klass.id: klass.label for klass in CLASSES}

# A title that says it is about the interface should not count against the modelling classes it merely names.
GUI_TITLE = re.compile(r"(?i)^\W*(?:ui|gui)\b\s*:|\btask\s*panel\b")
MODELLING = frozenset(
    {
        "boolean", "fillet-chamfer", "tolerance-gap", "tessellation", "shape-healing", "invalid-shape", "topo-naming",
        "exchange", "sweep-loft", "offset-shell", "draft", "extrude-revolve", "pattern-transform", "intersection",
        "curves-nurbs", "measure",
    }
)

SENTENCE_SPLIT = re.compile(r"(?<=[.!?])\s+|\n+")
NOT_DEFECT_LABELS = re.compile(r"\b(?:feature|enhancement|question|documentation|task|discussion|proposal)\b", re.IGNORECASE)
NOT_DEFECT_TITLE = re.compile(r"^\W*(?:feature\s*request|fr|rfe|enhancement|proposal|question)\b", re.IGNORECASE)


def _has_failure_word(text):
    return any(word.search(text) for word in FAILURE_WORDS)


def _has_topic(klass, text):
    return any(pattern.search(text) for pattern in klass.topics) and not any(pattern.search(text) for pattern in klass.exclude)


def _title_counts(klass, title):
    if not _has_topic(klass, title):
        return False
    if klass.id in MODELLING and GUI_TITLE.search(title):
        return False
    return not klass.title_needs_failure or _has_failure_word(title)


def classes_of(title, body):
    """The sorted class ids of one stripped issue."""
    matched = {klass.id for klass in CLASSES if _title_counts(klass, title)}
    for sentence in SENTENCE_SPLIT.split(body or ""):
        if not _has_failure_word(sentence):
            continue
        matched.update(klass.id for klass in CLASSES if klass.id not in matched and _has_topic(klass, sentence))
    return sorted(matched)


BROAD_LABEL_SETS = {
    "FreeCAD/FreeCAD": (frozenset({"Mod: Part", "Type: Bug"}), frozenset({"Mod: Part Design", "Type: Bug"}), frozenset({"3rd party: OCC"})),
    "Open-Cascade-SAS/OCCT": (frozenset({"2. Bug"}),),
}


def in_broad_sample(issue):
    """True for reports reached without searching for a symptom: the ranking uses only these, so a topical page cannot inflate its own class."""
    wanted = BROAD_LABEL_SETS.get(issue.get("repo"))
    if wanted is None:
        return True
    labels = set(issue.get("labels") or [])
    return any(required <= labels for required in wanted)


def explain(issue, class_id):
    """What matched: the title, or the sentences that hold a topic and a failure word. For the precision check."""
    klass = next(klass for klass in CLASSES if klass.id == class_id)
    title = issue.get("title") or ""
    if _title_counts(klass, title):
        return [f"title: {title}"]
    reasons = []
    for sentence in SENTENCE_SPLIT.split(issue.get("body") or ""):
        if _has_topic(klass, sentence) and _has_failure_word(sentence):
            reasons.append(sentence.strip())
    return reasons


def is_defect(issue):
    """False for feature requests, questions and documentation. A report with no such sign counts as a defect."""
    if any(NOT_DEFECT_LABELS.search(label) for label in issue.get("labels") or []):
        return False
    return not NOT_DEFECT_TITLE.match(issue.get("title") or "")


def tally(issues):
    """Counts per class and per source, ranked by total, then by name."""
    sources = {}
    excluded = 0
    unclassified = 0
    per_class = {}
    for issue in issues:
        if not is_defect(issue):
            excluded += 1
            continue
        repo = issue["repo"]
        sources[repo] = sources.get(repo, 0) + 1
        classes = classes_of(issue.get("title") or "", issue.get("body") or "")
        if not classes:
            unclassified += 1
        for class_id in classes:
            row = per_class.setdefault(class_id, {})
            row[repo] = row.get(repo, 0) + 1
    rows = [
        {"class": class_id, "label": CLASS_LABELS[class_id], "total": sum(row.values()), "per_source": dict(sorted(row.items()))}
        for class_id, row in per_class.items()
    ]
    rows.sort(key=lambda row: (-row["total"], row["class"]))
    return {"sources": dict(sorted(sources.items())), "excluded_not_defects": excluded, "unclassified": unclassified, "rows": rows}


def sample(issues, class_id, count, seed):
    """A deterministic sample of a class's defects. The seed decides which ones."""
    members = [issue for issue in issues if is_defect(issue) and class_id in classes_of(issue.get("title") or "", issue.get("body") or "")]
    members.sort(key=lambda issue: hashlib.sha256(f"{seed}/{issue['repo']}/{issue['number']}".encode()).hexdigest())
    return members[:count]


def load_issues(path):
    with Path(path).open(encoding="utf-8") as stream:
        return [json.loads(line) for line in stream if line.strip()]


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--in", dest="input", type=Path, default=DEFAULT_IN)
    parser.add_argument("--sample", metavar="CLASS", help="print a sample of a class's stripped text instead of the table")
    parser.add_argument("--count", type=int, default=8)
    parser.add_argument("--seed", default="census")
    parser.add_argument("--chars", type=int, default=240)
    parser.add_argument("--broad-only", action="store_true", help="sample from the broad sample only")
    options = parser.parse_args(argv)
    issues = load_issues(options.input)
    if options.sample:
        pool = [issue for issue in issues if in_broad_sample(issue)] if options.broad_only else issues
        for issue in sample(pool, options.sample, options.count, options.seed):
            print(f"--- {issue['repo']}#{issue['number']} {issue['labels']}")
            print(issue["title"])
            for reason in explain(issue, options.sample)[:3]:
                print("  >", reason[: options.chars])
        return 0
    broad = [issue for issue in issues if in_broad_sample(issue)]
    print(json.dumps({"broad_sample": tally(broad), "all_fetched": tally(issues)}, indent=1))
    return 0


if __name__ == "__main__":
    sys.exit(main())
