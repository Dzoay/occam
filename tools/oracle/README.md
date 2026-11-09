# The oracle harness

`tools/oracle/` runs operation lists through a pinned Open CASCADE (OCCT) binary and reports what comes out, so occam's
behaviour can be compared with it from outside. It is the [census milestone's](../../docs/plan/census.md) oracle step.

OCCT is a black box here. Only the public names of the binding are used, and the wheel is never committed
([the licence and provenance decision](../../docs/decisions/2026-11-08-licence-and-provenance.md),
[the OCCT oracle decision](../../docs/decisions/2026-11-09-occt-oracle.md)). The harness never writes to `crates/`.

## What is pinned

`cadquery-ocp==8.0.1.1.0` (OCCT 8.0.1), named in [`pyproject.toml`](pyproject.toml) and locked in [`uv.lock`](uv.lock), on
CPython 3.13 (`.python-version`; `requires-python` allows 3.11 to 3.14). The wheel and everything it brings
(VTK and its dependencies, which it declares itself) install into a virtualenv outside the repository:
`${XDG_CACHE_HOME:-$HOME/.cache}/occam-oracle/venv`. The harness's own code imports only the Python standard library and
that wheel; only [`occam_oracle/occt_worker.py`](occam_oracle/occt_worker.py) imports the wheel.

The second opinions are pinned separately, by commit, in [`occam_oracle/second.py`](occam_oracle/second.py); see
[Second opinions](#second-opinions).

## How to run

```text
just oracle run tools/oracle/examples       # write one result file per case
just oracle check tools/oracle/examples     # compare them with each case's expect, apply the invariants
just oracle register                        # check that divergences.json is well-formed
just oracle build-second                    # clone and build the second opinions under the cache
just oracle run --engine arris tools/oracle/examples
just oracle compare --engines occt arris tools/oracle/examples
just oracle-test                            # the harness's own tests
```

Without `just`:

```text
UV_PROJECT_ENVIRONMENT=${XDG_CACHE_HOME:-$HOME/.cache}/occam-oracle/venv \
  uv run --frozen --project tools/oracle python tools/oracle/oracle.py run tools/oracle/examples
```

`run` and `check` take case files or directories (searched recursively for `*.json`), and `--threads N` (default 1).
`check` exits 1 if anything fails and 2 for a malformed case.

The first run downloads a large wheel. `just oracle` and `just oracle-test` are not part of `just gates`; the
[nightly workflow](../../.github/workflows/nightly.yml) runs `just oracle-test`.

## The case format

A case is one JSON file, schema `occam-oracle-case/1`:

```json
{
  "schema": "occam-oracle-case/1",
  "id": "box-cut-cylinder-0001",
  "origin": "where the case came from",
  "licence": "MIT OR Apache-2.0",
  "ops": [
    {"op": "box", "id": "a", "size": [10, 20, 30], "origin": [0, 0, 0]},
    {"op": "cylinder", "id": "b", "radius": 2, "height": 40, "origin": [5, 10, -5], "axis": [0, 0, 1]},
    {"op": "cut", "id": "c", "target": "a", "tool": "b"}
  ],
  "result": "c",
  "expect": {"volume": {"value": 5623.008881569225, "rel_tol": 1e-6}},
  "volume_identity": {"group": "pair-0001", "role": "a"}
}
```

Rules, checked before anything runs:

- `id` becomes a file name, so it may contain only letters, digits, `.`, `_` and `-`, starting with a letter or digit.
- `origin` and `licence` are required for every case. A copyleft `licence` is refused.
- Ops run in order. Each has a unique `id` drawn from the same alphabet as the case's, and a `target` or `tool` must name
  the `id` of an earlier op that made a shape. `result` names the shape to measure.
- Lengths and radii are finite and greater than zero, and `axis` is non-zero (a magnitude other than 1 is fine).

| `op` | Fields |
| --- | --- |
| `box` | `size` `[dx, dy, dz]`, `origin` (the minimum corner) |
| `cylinder` | `radius`, `height`, `origin` (centre of one end), `axis` |
| `sphere` | `radius`, `origin` (the centre) |
| `cone` | `radius1` (at `origin`), `radius2` (at the far end), `height`, `origin`, `axis` |
| `torus` | `major_radius`, `minor_radius`, `origin` (the centre), `axis` |
| `translate` | `target`, `vector` |
| `rotate` | `target`, `axis_origin`, `axis`, `angle_degrees` |
| `fuse`, `cut`, `common` | `target`, `tool` |
| `fillet` | `target`, `radius` (constant), `edges` |
| `tessellate` | `target`, `linear_deflection`, `angular_deflection`, `relative` (optional, default false) |
| `remove_face` | `target`, `in_box`: fault injection, see below |

`fillet` selects its edges by a rule that depends only on the geometry, never on the engine's internal order. `edges`
is either `"all"`, or `{"in_box": {"min": [x, y, z], "max": [x, y, z]}}`, which takes every edge whose tight bounding box
lies inside the region, inclusive. Degenerate edges (which collapse to a point) and seams (where a face closes on itself)
are never selected. Selected edges are added in order of their bounding boxes. Pad an `in_box` by about 1e-6: a curved
edge's tight box can carry that much numerical error.

`tessellate` makes no shape, so it cannot be a `target` or the `result`. It meshes a copy of its target, so meshing
changes nothing that is measured afterwards.

`remove_face` is the one way to make an invalid shape from a case. It rebuilds the target as a solid without the single
face that lies inside `in_box`, leaving a deliberately open shell, and exists so the validity check can itself be
tested. Selecting anything other than exactly one face is an error.

### `expect`

`expect` is optional, and so is each measure in it:

| Measure | Form | Passes when |
| --- | --- | --- |
| `volume`, `area` | `{"value": x, "rel_tol": r, "abs_tol": a}` (at least one tolerance) | `abs(actual - x) <= max(r * abs(x), a)` |
| `bbox` | `{"min": [...], "max": [...], "abs_tol": a}` | every coordinate is within `a` |
| `euler` | `{"value": n}` | exact |
| `valid`, `closed` | `{"value": true}` or `false` | exact |
| `watertight` | `{"value": true}` or `false` | every `tessellate` in the case agrees |
| `error` | `{"op": "<op id>"}` | the case failed at that op |

`expect.error` stands alone. Entity counts are not an expectation: a case may not assert a face or edge count, because
occam is compared with OCCT geometrically, never by topology (L12).

`expect` values are closed-form or otherwise oracle-independent answers. Expected numbers are not copied from a run of
OCCT.

`volume_identity` is optional, with `group` (any string) and `role` (`a`, `b`, `union` or `intersection`). When a case
set provides all four roles of a group, `check` applies the volume identity below.

## The result format

`run` writes one result file per case. Files live outside the repository, because whether oracle output may be committed is
[not settled](../../docs/PLAN.md#7-decisions-the-user-still-owns) (O6). The default root is
`${XDG_CACHE_HOME:-$HOME/.cache}/occam-oracle/results`, or `--out`/`--results` to choose one. Under the root the path is:

```text
<engine>-<engine-version>/threads-<n>/<case-id>.json
```

for example `opencascade-8.0.1/threads-1/box-cut-cylinder-0001.json`.

The file is JSON, written deterministically: UTF-8/ASCII, keys sorted, two-space indent, LF line endings, a final newline,
floats in shortest round-trip form (Python's `repr`), `-0.0` normalised to `0.0`, and no timing, hostname, path or date. Two
runs produce the same bytes.

Schema `occam-oracle-result/1`, a successful case:

```json
{
  "id": "box-cut-cylinder-0001",
  "oracle": {
    "engine": "opencascade",
    "engine_version": "8.0.1",
    "package": "cadquery-ocp",
    "package_version": "8.0.1.1.0"
  },
  "schema": "occam-oracle-result/1",
  "shape": {
    "area": 2551.858377202057,
    "bbox": {"max": [10.0, 20.0, 30.0], "min": [0.0, 0.0, 0.0]},
    "closed": true,
    "counts": {"degenerate_edges": 0, "edges": 15, "faces": 7, "shells": 1, "solids": 1, "vertices": 10, "wires": 9},
    "euler": 0,
    "valid": true,
    "volume": 5623.008881569224
  },
  "status": "ok",
  "tessellations": {},
  "threads": 1
}
```

| Field | Meaning |
| --- | --- |
| `oracle` | Which engine produced this: `engine`, `engine_version` (for OCCT, the OCCT version), and the package it came from with `package_version`. Compare it with the pin you expect. |
| `threads` | The parallelism the case ran under. |
| `status` | `ok` or `error`. |
| `shape.volume`, `shape.area` | The engine's mass properties of the result. |
| `shape.bbox` | `{"min": [x, y, z], "max": [x, y, z]}`, OCCT's optimal box with no tolerance added; `null` for an empty result. It is not always tight: on trimmed spherical faces it has been seen to reach past the shape, and it inflated a torus by 1e-7. Compare with an `abs_tol` of at least 1e-6, and treat an excess on a spherical face as a weakness of the oracle rather than of the result (L11). |
| `shape.euler` | `V - E + 2F - L`, from `counts`: vertices, non-degenerate edges, faces and the wires of all faces. Degenerate edges collapse to a point and are not counted; a sphere gives 2, a cylinder 2, a plate with one through-hole 0. |
| `shape.counts` | `vertices`, `edges` (non-degenerate only), `degenerate_edges`, `wires`, `faces`, `shells`, `solids`. Reported, never compared. |
| `shape.valid` | The engine's checker's verdict. |
| `shape.closed` | Every non-degenerate edge lies on an even number of face sides, so the boundary has no free edge. |
| `tessellations` | One entry per `tessellate` op, by op id: `target`, the deflections and `relative` echoed back, `triangles`, `vertices` (after welding coincident points), and `watertight` (every edge of the welded mesh used exactly twice, in opposite directions). |

A case that fails has `status: "error"` and neither `shape` nor `tessellations`:

```json
{
  "error": {"code": "op_failed", "kind": "fillet", "message": "StdFail_NotDone: BRep_API: command not done", "op": "too-big"},
  "status": "error"
}
```

| `error.code` | Meaning |
| --- | --- |
| `op_failed` | The engine refused or failed the op named by `op` (its id) and `kind`. |
| `op_unsupported` | This engine does not offer that operation. Nothing was attempted. |
| `measure_failed` | Every op ran but measuring the result failed. |
| `worker_crashed` | The child died; `op`/`kind` name the op in progress if any. |
| `worker_timeout` | The child ran past `--timeout`. |
| `worker_malformed` | The child wrote something that is not a result. |
| `identity_mismatch` | The child's reported engine differs from the pin. |

## Comparison and invariants

`check` reports, per case, each measure in `expect` (pass or fail, with the values and the tolerance), and these
invariants, which need no oracle at all (L11):

- `valid`: the checker accepts the result.
- `closed`: the boundary has no free edge.
- `volume_positive`: volume is greater than zero. A case expecting a volume of exactly 0 waives this one.
- `euler_counts`: `euler` equals `V - E + 2F - L` from the counts. The expected Euler value is the `euler` measure.

A case that expects `valid: false` waives all of them, since it set out to make a broken shape. Nothing else waives an
invariant.

Across a case set, `check` also applies the volume identity `vol(A) + vol(B) = vol(A union B) + vol(A intersect B)`, for
every `volume_identity` group that provides all four results (relative tolerance 1e-6 of the largest volume). A group that
is missing a role is skipped and reported as skipped; a group with a failed member fails.

Comparison is geometric, with tolerances, never by topology or bits (L12).

## The divergence register

[`divergences.json`](divergences.json) records each disagreement between OCCT, a second opinion, or a closed-form answer.
It is empty until there is one. `just oracle register` checks it. Schema `occam-oracle-divergences/1`:

```json
{
  "schema": "occam-oracle-divergences/1",
  "entries": [
    {
      "case": "box-cut-cylinder-0001",
      "measure": "volume",
      "values": {"occt": 5623.0, "closed-form": 5623.1},
      "decided_by": "closed-form",
      "verdict": "occt-wrong",
      "note": "One sentence on what happened and why this verdict stands."
    }
  ]
}
```

`values` names at least two sources (lower-case slugs: `occt`, `closed-form`, or an engine name) with their numbers or
booleans. `decided_by` is the invariant or arbiter that decided it (`closed-form`, `valid`, `closed`, `volume-positive`,
`euler`, `volume-identity`, `watertight`), or `none`. `verdict` is `occt-wrong`, `closed-form-wrong`,
`second-opinion-wrong` or `undecided`; `none` and `undecided` go together, since an undecided entry has no arbiter yet.
Entries are sorted by case then measure, and a case and measure appear once.

Whether oracle output carries licence terms is O6 in [the plan](../../docs/PLAN.md#7-decisions-the-user-still-owns), still
open. Record only the numbers an entry needs, and ask before the first entry lands that holds a value from OCCT.

## Scrubbed environment and threads

The engine's work happens only in a child process (L16). The child's whole environment is `PATH` (fixed to
`/usr/bin:/bin`), `HOME` (a fresh temporary directory, also its working directory), `LC_ALL=C` and `PYTHONHASHSEED=0`.
No `CSF_*` or other variable leaks in. Each case gets its own child, so one case cannot disturb the next.

Parallelism is fixed by `--threads N` (default 1), recorded as `threads` in the result. At 1 the booleans' and the mesher's
parallel switches are off. Above 1 they are on and the engine's default thread pool is set to `N`.

## Second opinions

`arris` at release 0.10.0 (commit `f6860e379ba054888b087b8e141f47cd1e631eab`) is wired up. `just oracle build-second`
clones that commit anonymously over https under `${XDG_CACHE_HOME:-$HOME/.cache}/occam-oracle/second/`, generates a small
Rust project there from [`second/Cargo.toml.in`](second/Cargo.toml.in) and
[`second/arris_driver.rs.in`](second/arris_driver.rs.in), and compiles it. Nothing of theirs enters the repository. Results
go to `arris-0.10.0/threads-<n>/<case-id>.json` in the same format, with `oracle.package` the repository and
`oracle.package_version` the commit. `just oracle compare --engines occt arris <cases>` then reports where two engines
disagree, geometrically: volume and area within `--rel-tol`, the box within `--bbox-tol` of its diagonal, and validity. It
exits 1 on a disagreement, and `SKIP` means an engine does not offer the operation, so nothing was compared.

The driver is written only from arris's README and [its published API docs](https://docs.rs/arris/0.10.0). Its source
is not to be read, and that holds for anyone changing the driver.

Where arris answers differently from the description above:

- `bbox` is the extent of a mesh at a chord of 1e-4 of the diagonal, because arris documents no bounding-box query at this
  release. It is exact for planar shapes and otherwise inside the true box by at most that chord.
- A fillet's `in_box` uses the same meshed edges. `all` means every edge that is neither degenerate nor a seam.
- `tessellate` passes `linear_deflection` as arris's chord, scaled by the diagonal when `relative` is true.
  `angular_deflection` has no counterpart and is ignored.
- `threads` is recorded but changes nothing: arris documents no parallel mode at this release.
- `remove_face` is `op_unsupported`.
- Sphere, cone and torus are built by a full revolve of a profile, since arris documents no such primitive at this
  release. Arris's booleans at this release are documented for planes and cylinders; anything else is its own typed
  refusal, reported as `op_failed`.

`solidcore` is not wired up yet, and no result comes from it. Its release 1.0.1 is commit
`e83425f7942c10ca806242af220e5a2901a9db60`. A driver still has to be written for volume (`brep_volume` needs a
`PropertyAttributes`), for placing shapes off the Z axis and rotating them (`transform` with an `Axis2Placement`), and for
entity counts, fillets and tessellation.

### Adding an engine

An engine is an `Engine` in [`occam_oracle/runner.py`](occam_oracle/runner.py) or
[`occam_oracle/second.py`](occam_oracle/second.py), and `ENGINES` maps its name to it. Its worker is started as
`python -B -s -P <worker> <worker_args> --case FILE --result FILE --progress FILE --threads N` in the scrubbed environment.
It reads the case, writes the op it is about to run to `--progress` so that a crash can be blamed on an op, and writes one
result document to `--result`.

A compiled kernel can skip JSON. [`occam_oracle/second_worker.py`](occam_oracle/second_worker.py) is a worker that
translates for a driver started as `<driver> <progress-file>`. Its stdin has one op per line, `<op> <id> <fields...>`, in
the order the case format lists them, then `result <id>`: floats in round-trip form, booleans as `0` or `1`, a fillet's
`edges` as `all` or `box` and six numbers. Its stdout is one of:

```text
error <op-id> <message>              an op failed; a message ending "is not supported by this engine" is op_unsupported,
                                     and one starting "measure:" against the result id is measure_failed
ok                                   followed by these lines, in any order
volume <f>
area <f>
bbox <minx> <miny> <minz> <maxx> <maxy> <maxz>
counts <vertices> <edges> <degenerate> <wires> <faces> <shells> <solids>
valid <0|1>
closed <0|1>
mesh <op-id> <points> <triangles>    then one `x y z` line per point and one `a b c` line per triangle, zero-based
```

Euler is computed from `counts`, and a mesh is judged by [`occam_oracle/meshcheck.py`](occam_oracle/meshcheck.py), the same
rule as OCCT's.

## How `occam-verify` should consume this

`occam-verify`, the Rust side, should:

1. Take a results root, defaulting to `${XDG_CACHE_HOME:-$HOME/.cache}/occam-oracle/results`, and find each case at
   `<root>/<engine>-<engine-version>/threads-<n>/<case-id>.json`, with `opencascade-8.0.1` as the reference directory and
   `arris-0.10.0` as the second opinion.
2. Parse the file as JSON and require `schema` to be `occam-oracle-result/1` and `oracle` to match the pin it was built
   against; treat anything else as "no result".
3. Compare its own measurements with `shape` the way `check` does: with a tolerance per measure, and geometrically. Never
   compare bytes, never compare `counts`, and treat `euler` as a number that must match exactly and is derived from counts
   which themselves are not compared.
4. Read `status: "error"` as "the oracle could not do this", not as a test failure in occam, unless the case expects it.
5. Never write back into `tools/oracle/`. Record a disagreement it finds as an entry in the divergence register.

The one document that defines both formats is this file, and the code that enforces them is
[`occam_oracle/caseformat.py`](occam_oracle/caseformat.py) and
[`occam_oracle/resultformat.py`](occam_oracle/resultformat.py).
