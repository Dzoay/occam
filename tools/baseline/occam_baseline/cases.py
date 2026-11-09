"""Which corpus cases are timed, and how they are grouped. Standard library only."""

import dataclasses
import re

_BOOLEANS = ("fuse", "cut", "common")
_NEAR_TANGENT = ("near-crossed-", "near-sphere-box-")
_FAMILY = re.compile(r"-\d{4}(?:-[a-z]+)?$")

GROUPS = ("booleans", "fillets", "tessellation")


@dataclasses.dataclass(frozen=True)
class Chosen:
    """What is timed for one case and where its numbers go. `group` is a table row: `booleans-fuse`, `tessellation`, ..."""

    group: str
    family: str
    timed: frozenset
    kind: str  # "volume" or "triangles"

    @property
    def run_group(self):
        """The run a case belongs to: booleans (near-tangent included), fillets or tessellation."""
        if self.group.startswith(("booleans-", "near-tangent-")):
            return "booleans"
        return self.group


def _result_kind(case):
    return next(op["op"] for op in case["ops"] if op["id"] == case["result"])


def classify(case):
    """The `Chosen` for a case, or None if it has no operation in scope (a primitive alone, for example).

    Tessellation is checked first so a plate cut and then meshed times only the mesher. Booleans after inputs are summed
    across the case, because the chain of cuts in a plate with holes is one model-making step.
    """
    kinds = [op["op"] for op in case["ops"]]
    base = _FAMILY.sub("", case["id"])
    if "tessellate" in kinds:
        return Chosen("tessellation", base, frozenset({"tessellate"}), "triangles")
    if "fillet" in kinds:
        return Chosen("fillets", base, frozenset({"fillet"}), "volume")
    booleans = [kind for kind in kinds if kind in _BOOLEANS]
    if not booleans:
        return None
    kind = _result_kind(case)
    if kind not in _BOOLEANS:
        kind = booleans[-1]
    near = case["id"].startswith(_NEAR_TANGENT)
    group = ("near-tangent-" if near else "booleans-") + kind
    family = f"{base}-{kind}" if near else base
    return Chosen(group, family, frozenset(booleans), "volume")
