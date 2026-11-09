# Corpus inventory, 2026-11-09

What `census.corpus` assembled, and how each piece was checked. Evidence, not instruction. Numbers measured by the OCCT
oracle are reported only as agreement counts and qualitative statements: whether oracle output may be committed is still
open as O6 in [the plan](../PLAN.md#7-decisions-the-user-still-owns). The oracle results themselves live in the machine's
cache and are not in the repository.

Everything is run through the harness in `tools/oracle` (`opencascade` 8.0.1 from the `cadquery-ocp` 8.0.1.1.0 wheel, one
thread), and the run is reproducible from the seeds and the locked sources below.

## Generated cases

`tools/corpus/generate.py`, standard library only, one `random.Random(seed)` per case with `seed = family base * 1000 +
index`. Parameters are rounded to 3 decimals before any closed form is computed; derived placements and near-tangent
offsets keep 12 decimals. Tolerances were stated before any run: relative 1e-6 on volume and area, absolute 1e-5 on bounding
box coordinates. Every case records `origin` (`tools/corpus/generate.py family <family> seed <seed>`) and licence (`MIT OR
Apache-2.0`). The files are committed under `tools/corpus/generated/`, 546 of them, about 500 KiB. The last column counts
cases the oracle agreed with, meaning every expected measure matched and the oracle's invariants held.

| Family | Cases | Parameters | Closed form | Agreed with oracle |
|---|---|---|---|---|
| `prim-box` | 12 | sizes 0.5-50, any position | V, A, Euler 2, bbox | 12 of 12 |
| `prim-cylinder` | 12 | r 0.5-20, h 0.5-50, world and arbitrary axes | V, A, Euler 2, bbox | 12 of 12 |
| `prim-cone` | 12 | apex cones and frustums, world and arbitrary axes | V, A, Euler 2, bbox | 12 of 12 |
| `prim-sphere` | 10 | r 0.5-50, any position | V, A, Euler 2, bbox | 10 of 10 |
| `prim-torus` | 10 | R/r 1.5-5, world and arbitrary axes | V, A, Euler 0, bbox | 10 of 10 |
| `box-corner-fuse` | 8 | overlap on every axis, generic position | V, A, Euler 2, bbox | 8 of 8 |
| `box-corner-cut` | 8 | overlap on every axis, generic position | V, A, Euler 2, bbox | 8 of 8 |
| `box-corner-common` | 8 | overlap on every axis, generic position | V, A, Euler 2, bbox | 8 of 8 |
| `box-slab-fuse` | 4 | same footprint, coplanar side faces | V, A, Euler 2, bbox | 4 of 4 |
| `box-slab-cut` | 4 | same footprint, coplanar side faces | V, A, Euler 2, bbox | 4 of 4 |
| `box-slab-common` | 4 | same footprint, coplanar side faces | V, A, Euler 2, bbox | 4 of 4 |
| `box-stacked-fuse` | 4 | b on a's top face, footprint inside | V, A, Euler 2, bbox | 4 of 4 |
| `box-stacked-cut` | 4 | b on a's top face, no overlap | V, A, Euler 2, bbox | 4 of 4 |
| `plate-hole` | 8 | one through-hole | V, A, Euler 0, bbox | 8 of 8 |
| `plate-holes` | 8 | 2 to 6 through-holes | V, A, Euler 2-2g, bbox | 8 of 8 |
| `sphere-cap-cut` | 8 | plane through the sphere, either side, three axes | V, A, Euler 2, bbox | 5 of 8 |
| `sphere-cap-common` | 8 | plane through the sphere, either side, three axes | V, A, Euler 2, bbox | 5 of 8 |
| `steinmetz-common` | 8 | equal orthogonal cylinders, half rotated | V = 16r^3/3, A = 16r^2, Euler 2, bbox unrotated | 8 of 8 |
| `steinmetz-fuse` | 4 | equal orthogonal cylinders, half rotated | V, A, Euler 2, bbox unrotated | 4 of 4 |
| `steinmetz-cut` | 4 | equal orthogonal cylinders, half rotated | V, A, bbox unrotated; two pieces at pinch points | 4 of 4 |
| `coaxial-equal-fuse` | 4 | one radius, overlapping along one axis | V, A, Euler 2, bbox | 4 of 4 |
| `coaxial-equal-common` | 4 | one radius, overlapping along one axis | V, A, Euler 2, bbox | 4 of 4 |
| `coaxial-equal-cut` | 4 | one radius, overlapping along one axis | V, A, Euler 2, bbox | 4 of 4 |
| `coaxial-stepped-fuse` | 4 | a thinner cylinder entering the wider one's end | V, A, Euler 2, bbox | 4 of 4 |
| `coaxial-stepped-common` | 4 | a thinner cylinder entering the wider one's end | V, A, Euler 2, bbox | 4 of 4 |
| `coaxial-stepped-cut` | 4 | a thinner cylinder entering the wider one's end | V, A, Euler 2, bbox | 4 of 4 |
| `coaxial-tube` | 4 | concentric cylinders, inner through the full length | V, A, Euler 0, bbox | 4 of 4 |
| `sphere-lens-fuse` | 8 | two spheres overlapping away from tangency | V, A, Euler 2, bbox | 6 of 8 |
| `sphere-lens-common` | 8 | two spheres overlapping away from tangency | V, A, Euler 2 | 5 of 8 |
| `sphere-lens-cut` | 8 | two spheres overlapping away from tangency | V, A, Euler 2 | 8 of 8 |
| `fillet-box-edge` | 8 | one edge of a box, r up to 40% of the narrower adjacent face | V = abc - (1 - pi/4)r^2L, A, Euler 2, bbox | 8 of 8 |
| `fillet-box-all` | 8 | all edges and corners, r up to 30% of the shortest side | rounded-box V and A, Euler 2, bbox | 8 of 8 |
| `tess-sphere` | 4 | four deflection settings | V, A, Euler 2, watertight; triangle count reported | 4 of 4 |
| `tess-cylinder` | 4 | four deflection settings | V, A, Euler 2, watertight; triangle count reported | 4 of 4 |
| `tess-torus` | 4 | four deflection settings | V, A, Euler 0, watertight; triangle count reported | 4 of 4 |
| `tess-box` | 4 | four deflection settings | V, A, Euler 2, watertight; triangle count reported | 4 of 4 |
| `tess-cone` | 4 | four deflection settings | V, A, Euler 2, watertight; triangle count reported | 4 of 4 |
| `tess-plate` | 4 | four deflection settings | V, A, Euler 0, watertight; triangle count reported | 4 of 4 |
| `near-crossed-external` | 72 | axes r1 + r2 - e apart, e 1e-9 to 0.5, half rotated | none: volume identity across the group | 67 of 72; identity 18 of 18 groups |
| `near-crossed-internal` | 72 | axes r1 - r2 - e apart, e 1e-9 to 0.5, half rotated | none: volume identity across the group | 67 of 72; identity 2 of 18 groups |
| `near-crossed-steinmetz` | 72 | equal radii, axes e apart, e 1e-9 to 0.5, half rotated | none: volume identity across the group | 72 of 72; identity 5 of 18 groups |
| `near-sphere-box` | 90 | box face e inside the tangent plane, e 1e-9 to 0.5, half rotated; plus a cut | none: volume identity across the group | 85 of 90; identity 18 of 18 groups |
| **Total** | **546** | | | **520 of 546** |

Each near-tangent group has four cases (A, B, the union and the intersection, tagged for `volume_identity`), and each
sphere-box group adds a cut with no tag. Only the side of tangency where the bodies overlap is covered; the side with a
gap has a trivial closed form and an empty intersection, and is not included.

Tessellation families carry `watertight` and the shape's closed forms. A mesh count cannot be stated in `expect`, by L12,
so the oracle reports triangle and vertex counts per `tessellate` op in its result file.

## What the oracle disagreed with

No case failed to run. Twenty-six cases did not agree, in three kinds. These counts were reproduced on the merged harness
at commit `6251c56`, 520 of 546 generated cases and 32 of 32 imported ones passing.

**Spherical faces (11 cases: the disagreement is OCCT's measure, not the closed form).** `sphere-cap-common-0002`,
`-0005`, `-0008`, `sphere-cap-cut-0002`, `-0004`, `-0008` report a bounding box extending to the full sphere along the
axis of the plane, past where the tool box ends, which is impossible geometry: the harness's box is not tight for
trimmed spherical faces. `sphere-cap-common-0005`, `sphere-cap-cut-0002`, `-0004`, `sphere-lens-common-0002`, `-0006`,
`-0007`, `sphere-lens-fuse-0004`, `-0006` also disagree on volume and area. In each of those the intersection circle
passes close to a pole of one of the spheres (within 0.17R for the caps, within 0.09R for the lenses) and no case further
from a pole disagrees on volume or area; two lens cases this close agree. The closed forms of these eight cases were
checked by numerical quadrature, independently of the formulae, and agree to better than 3e-12. The same formulae agree
with the oracle in the other 30 cap and lens cases, including all eight `sphere-lens-cut`.

**Near-tangent boolean results.** In 15 of the 306 near-tangent cases the oracle's own invariants fail. Eleven report an
intersection of zero volume (`near-crossed-external` 5, `near-crossed-internal` 1, `near-sphere-box` 5), at offsets of
about 3e-7 or less, where the true overlap is positive but tiny, so the engine reports no solid where the exact answer
is a very small one. Four report an invalid or open union (`near-crossed-internal-0007`, `-0009`, `-0016`, `-0017`), at
offsets of about 1.3e-7 or less.

**Volume identity.** Twenty-nine groups fail `vol(A) + vol(B) = vol(A union B) + vol(A intersect B)` at 1e-6 of the
largest volume: 16 of the 18 `near-crossed-internal` groups and 13 of the 18 `near-crossed-steinmetz` groups. The 13
failing Steinmetz groups have offsets of about 2.2e-7 or more, in both the world and rotated placements, and the 5
passing ones have offsets of about 2.6e-7 or less. The identity is what demonstrates these errors, independent of any
closed form. All 18 groups pass in both `near-crossed-external` and `near-sphere-box`.

## NIST CAD test parts

Terms, from the [NIST MBE PMI downloads page](https://www.nist.gov/ctl/smart-connected-systems-division/smart-connected-manufacturing-systems-group/mbe-pmi-0),
retrieved 2026-11-09: "The test cases, CAD models, and STEP files can be used without any restrictions." The same page says
their use does not imply NIST endorsement, asks for acknowledgement, and does not allow the NIST logo in promotional
materials. The page gives no separate copyright statement for the models; NIST's general
[disclaimer](https://www.nist.gov/disclaimer) treats NIST-published information as public unless marked as copyrighted.
These terms are unrestricted as the page states them; a lawyer's view has not been taken.

Locked in `tools/corpus/sources.lock.json`: `NIST-PMI-STEP-Files.zip`, sha256
`8fa78429e6d8d9b0d7681d223b6aa9ec98c3772185c55b1a0e3679b21c181911`, 13976599 bytes, from
`https://www.nist.gov/document/nist-pmi-step-files`, with its 45 member files (the CTC, FTC and STC parts in AP203 and
AP242, the PDF drawings and a README) each named with a sha256. `tools/corpus/fetch.py` (`just corpus-fetch`) downloads it
into `~/.cache/occam-corpus/nist/` with the generic User-Agent `occam-census` and verifies the archive and every member,
reading members in memory without extracting. Nothing is converted or run yet: these parts join the tests once STEP
reading exists. The archive's own README says these are not error-free reference STEP files.

In: the STEP archive above, under the terms quoted. Out, not locked: the native CAD models (CATIA, NX, Inventor, Creo,
SolidWorks), which are of no use to a STEP-reading pipeline, and the HTC, MTC and D2MI models, which were not assessed
here.

## arris

Scope: the user approved `arris` cases joining the corpus with a per-case licence and origin check, unclear origin
staying out. Commit `1a7bc1e660b9d20a677ac06a317de83200ffd639` of `https://github.com/arris-labs/arris`, cloned
anonymously into `~/.cache/occam-corpus/arris/repo`. Licence: `MIT OR Apache-2.0`, as stated in `Cargo.toml` and by the
two licence files at the repository root; the MIT file's notice reads `Copyright (c) 2026 Sergei Sergienko`.

Read: the first lines of the two licence files, the licence field of `Cargo.toml`, the opening of
`tests/fixtures/README.md` (which states who writes each fixture file), and `fixture.json` data. Not read: its Rust
source, tests, implementation, `expected.json` (an oracle's answer), `second_oracle.json`, `dump.*.txt` or any STEP file's
content.

The gate, in `tools/corpus/import_arris.py`, applied per fixture. A recipe stays out if it carries `source`, `licence`,
`file` or `sha256` (a separately licensed part); is a `regression/` fixture; is not a solid recipe; reads a STEP operand;
uses an operation outside box, cylinder, fuse, cut and common; expects an error or degeneracy; has no closed-form volume or
gives a bare number where an expression should be (such a number could be a measured value from another kernel); names
another kernel in its description; or describes a downstream consumer's probe or a recipe built on one, whose origin the
repository does not state. Excluded, of 652 fixtures: 389 unsupported operation, 72 regression, 49 not a solid recipe, 43
STEP operand, 42 separately licensed part, 9 bare number, 9 must fail, 7 names another kernel, 7 unclear origin, 1 no
closed-form volume.

Taken: 32 cases from 24 recipes, committed under `tools/corpus/imported/`, each recording the commit, the path and the
notice in its `origin`, and declared in `REUSE.toml`. The oracle agreed with 32 of 32. Attribution, with the path under
`tests/fixtures/` at that commit:

| Recipe | Cases |
|---|---|
| `boolean/adjacent-pockets-cut-many` | `arris-boolean-adjacent-pockets-cut-many` |
| `boolean/adjoining-cavities-cut-many` | `arris-boolean-adjoining-cavities-cut-many` |
| `boolean/blind-hole` | `arris-boolean-blind-hole` |
| `boolean/bolt-pattern-8` | `arris-boolean-bolt-pattern-8` |
| `boolean/boss` | `arris-boolean-boss` |
| `boolean/boss-flush` | `arris-boolean-boss-flush` |
| `boolean/cavity-cylinder` | `arris-boolean-cavity-cylinder` |
| `boolean/coaxial-cut` | `arris-boolean-coaxial-cut` |
| `boolean/counterbore-cut-many` | `arris-boolean-counterbore-cut-many` |
| `boolean/crossing-pockets-cut-many` | `arris-boolean-crossing-pockets-cut-many` |
| `boolean/oblique-hole` | `arris-boolean-oblique-hole` |
| `boolean/overlapping-bosses-fuse-many` | `arris-boolean-overlapping-bosses-fuse-many` |
| `boolean/overlapping-pockets-cut-many` | `arris-boolean-overlapping-pockets-cut-many` |
| `boolean/repeated-hole-cut-many` | `arris-boolean-repeated-hole-cut-many` |
| `boolean/sliver-common` | `arris-boolean-sliver-common` |
| `boolean/stacked-bosses-fuse-many` | `arris-boolean-stacked-bosses-fuse-many` |
| `boolean/stacked-pockets-cut-many` | `arris-boolean-stacked-pockets-cut-many` |
| `boolean/three-bores-one-point-cut-many` | `arris-boolean-three-bores-one-point-cut-many` |
| `boolean/through-hole` | `arris-boolean-through-hole` |
| `boolean/tripod-fuse-many` | `arris-boolean-tripod-fuse-many` |
| `primitive/cylinder` | `arris-primitive-cylinder` |
| `provenance/bolt-pattern-rebuild` | `arris-provenance-bolt-pattern-rebuild`, `-thicker-wider`, `-tighter` |
| `provenance/split-bar-cut` | `arris-provenance-split-bar-cut`, `-left`, `-narrow`, `-right` |
| `provenance/split-frame-cut` | `arris-provenance-split-frame-cut`, `-left`, `-narrow`, `-right` |

Only the recipe and its closed-form expressions are taken. Their descriptions and tolerances are not.

Arris's fixtures that cite the NIST archive, or that are shrunk from NIST parts, are excluded by the rule on separately
licensed parts. The NIST parts themselves come through the lock above.

## solidcore

Still external, as the plan says. The user has approved it only as a second opinion; no case of it enters this corpus.

## Regenerating

- `just corpus-generate` rewrites `tools/corpus/generated/` from its seeds and prints the family table. Two runs write
  identical files, and a test holds the committed files to the generator.
- `just corpus-fetch` downloads and verifies the locked sources.
- `just corpus-import-arris <repo>` rewrites `tools/corpus/imported/` from a clone at the pinned commit, and refuses any
  other commit.
- `just corpus-test` runs the pipeline's own tests, which never touch the network.
- `just oracle run tools/corpus/generated` and `just oracle check tools/corpus/generated` run the corpus through the
  harness.
