# `view.timing`: how fast and how good is the real mesher, natively and in the page?

In short: on P9's own parts rebuilt on the kernel, the real mesher keeps every triangle within its deflection, gives the
same bits at every thread count, and keeps the angular limit far better than P9's mesher at coarse deflection, though at
0.2 mm its B-spline faces fill with slivers. It reaches 3.2 to 3.3 times at six threads, against P9's 5.2, because the
watertight check runs on one thread: the faces alone reach 5.1. It is 2.8 to 2.9 times slower per triangle than P9's
throwaway. In the page the pool gains nothing from more workers on these models, because each worker opens its own copy,
measuring every edge against its B-spline faces: 28 s or more against about 2 s of meshing on the 1,002-face model. The
editing worker's longest call on construction's requests is 1.76 s; a run's save reaches the page in 364 ms (median) on
the live demo's plate, and in two minutes on P9's 1,002-face model. The mesher has no lane kernel, so its two new `gates
perf` classes are scalar. A pick on the GTX 1080 Ti takes 248 ms in Chromium and 428 ms in Firefox (medians), against
the 16.7 ms goal, most of it the page's pick computing the model's digest twice; on the laptop the page ran out of
memory before its first pick.

## Question

The `view` milestone's goals ([the plan](../plan/view.md)): P9's quality and scaling tables on the real mesher; the mesh
workers' pool timed in the browser beside [P10's](2026-11-09-p10.md); the editing worker's longest call on
construction's requests, which [the browser topic](../architecture/browser.md) waits on before splitting a boolean
across calls; the time from a run's save to the page's redraw; and a pick on real GPUs within 16.7 ms on P9's
10,001-face model, a [goal](../decisions/2026-11-13-gpu-figures-are-goals.md) no measurement grounds. And the mesher's
classes in `gates perf`, scalar and lanes.

## What was measured

**Machines.** Native timings and the editing worker's ran on the measuring machine, P9's and P10's: an Intel Core
i7-8700K, six physical cores, each run pinned to CPUs 0 to 5 (hyperthreads never), governor `powersave`. It is the build
host, which other jobs share: each measurement waited up to 45 minutes for the one-minute load average to fall under
1.5, and the load it started at is in its raw output. One, the 12,151-face scaling, started after that wait ran out at a
load of 30, though its seven repeats at each count spread under 1%. The pool and the redraw ran on the laptop, an Intel
Core i7-1165G7 with four cores and eight threads, when the measuring machine was taken by others' work and then by a
person's game. Instruction counts come from `just perf` on the laptop, whose record they are. `rustc 1.99.1`, `lto =
"fat"`, `codegen-units = 1`, no `target-cpu`; the page's kernel is `wasm32-unknown-unknown` with simd128; headless
Chromium is Playwright 1.64.1's Chromium 156, on the software adapter `just web-test` asks for, since the workers run on
the CPU.

**Code.** Every figure here but the record's was measured on this step's code over `aa77cf8`, before `view.hardening`
(#77) changed the mesher's repair and #76 the commit's allocations. The record's counts were taken again after both.

**P9's models on the kernel.** `occam::mesh_workloads::synthetic` draws P9's parts from P9's seed, 42, with P9's
generator's random numbers in P9's order, and counts P9's faces against the target, so targets 1,000 and 10,000 give the
parts of P9's 1,002- and 10,001-face models (a test holds those counts). Plates with holes and solids of revolution are
made by one request each run of them, as an agent would make them; panels and quilts, which no request makes yet, are
written through `Edit`; every step is committed and so checked. The kernel lays each part out as it lays out any other:
it cuts every periodic surface into patches so that no face crosses its own seam, and a panel's hole wall into two
halves, so **P9's 1,002 faces are 1,163 here, and its 10,001 are 12,151**. Two things differ from P9's geometry:

- **A panel's walls** are B-splines, its top's sides swept straight down, where P9 had surfaces of extrusion: the same
  surfaces, in a family the native format writes.
- **A panel's hole** is a circle in its top's parameters, as P9's was. The kernel has no curve that is a surface's image
  of another, so the hole's edge is a cubic B-spline through the top at 512 angles round the circle, as an imported
  model's would be. The commit measures how far each edge strays from its faces: no edge of the 1,163-face model strays
  more than 7.1e-12 mm.

**Commands.** `just view-timing build|quality|scale|save|open` natively (`crates/occam/examples/view_timing.rs`), and
`just view-timing-browser pool|editing|redraw` in the browser (`web/test/timing.mjs`), each printing one JSON line a
measurement. The raw lines are at the end.

**Quality,** as P9 measured it, per triangle: how far its surface strays from it, measured to the triangle by the
mesher's own `occam_mesh::deviation` at 45 points (P9: 28); how far its normal turns from the surface's outward normal
at its corners, the middles of its sides and its middle (7 points); and its smallest angle. Meshed on six threads at
P9's angular limit, 0.35 rad.

**Scaling:** the whole mesh (edges, faces, merge, the watertight check) at 1 to 6 threads, seven repeats each, least and
median, the digest of every repeat kept.

**The pool,** in the page `occam view` serves, at 1 to 6 mesh workers: *cold* from a new browser context's navigation
(fresh compile, workers and instances, nothing cached) to the model drawn; *warm* the model opened again in the editing
worker and meshed whole by the live pool, three times. Every call's time from being dealt to being answered is kept; a
worker's first call of a model waits for it to open its copy.

**The editing worker:** its call, from asking to answer, on each of construction's requests `gates perf` times and its
polygon of 1,000 edges, on a fresh page each time, five times.

**Save to redraw:** `occam run` adds a box to the model `occam view` watches; from the file's modification time to the
page's frame showing it, finished on the GPU (`settled()`), by the same clock.

## Results

### The mesher's classes in `gates perf`

| Class | What | Instructions |
|---|---|---|
| `mesh_p9_parts` | P9's first three parts (two plates and a panel, 28 faces) meshed whole on one thread at 0.1 mm: edges, faces, merge and the watertight check | 274,546,750 |
| `mesh_pick` | 227 rays picked on their mesh, one onto every sixteenth triangle, with edges within 0.01 mm | 15,101,133 |

**The mesher has no lane kernel.** It evaluates its surfaces a point at a time through `Surface`, never through the
batched `eval_batch`, so there is no `<kernel>_w<width>` to hold to width 1, and none was made up. Each class is scalar;
in the browser each runs in both builds, which must give the same bits (L23), and they did. In the browser, on the
laptop's headless Chromium, on the merged tree: `mesh_p9_parts` 31.2 ms scalar and 30.7 simd128, simd128 gaining
nothing, as a mesher without lanes would; `mesh_pick` 1.7 ms in both, about 7 µs a pick. The green `just perf` held all
31 classes within 2% of the record.

The red run gave exactly two findings, `perf.unrecorded` for `mesh_p9_parts` and `mesh_pick`, and `just perf --update`
took them. The counts above are the record's after this step merged main's, taken again on the merged tree: there
`mesh_p9_parts` takes 3.4% more than its 265,390,588 at `aa77cf8`, since `view.hardening` changed the mesher's repair,
and five construction and command classes moved by 0.122% at most from main's record.

### Quality

The angular limit is 0.35 rad, as P9's. P9's rows are from [P9](2026-11-09-p9.md), on its own mesher and its own layout
of the same parts.

| Model | Deflection | Triangles | Max deviation / deflection | Over deflection | Max turn (rad) | Over angular | Of which under 1° | Area over angular | Mean smallest angle | Under 20° |
|---|---|---|---|---|---|---|---|---|---|---|
| 1,163 faces | 0.1 | 245,778 | 0.954 | 0 | 3.14 | 1,107 (0.45%) | 4 | 0.0229% | 29.6 | 28% |
| P9, 1,002 faces | 0.1 | 96,576 | 0.815 | 0 | 3.03 | 6,416 (6.6%) | 1,895 | 1.02% | 28.5 | 31% |
| 1,163 faces | 0.02 | 372,222 | 0.956 | 0 | 3.14 | 324 (0.09%) | 28 | 0.0015% | 32.5 | 21% |
| P9, 1,002 faces | 0.02 | 454,558 | 0.806 | 0 | 0.46 | 142 (0.03%) | 0 | 0.0018% | 29.0 | 31% |
| 1,163 faces | 0.005 | 1,189,912 | 0.966 | 0 | 3.14 | 204 (0.02%) | 52 | under 0.0001% | 34.6 | 14% |
| P9, 1,002 faces | 0.005 | 2,055,908 | 0.770 | 0 | 3.14 | 2,178 (0.1%) | 2,178 | under 0.0001% | 26.0 | 40% |
| 12,151 faces | 0.2 | 3,164,576 | 0.954 | 0 | 3.14 | 621,658 (19.6%) | 563,029 | 0.185% | 20.9 | 48% |
| P9, 10,001 faces | 0.2 | 1,179,318 | 0.832 | 0 | 3.14 | 217,149 (18%) | 113,363 | 3.66% | 19.0 | 55% |

**The deflection held everywhere:** no triangle strays past it, on any family, at any deflection, measured to the
triangle at 45 points each. The furthest is 0.954 to 0.956 of the deflection, where P9's mesher kept to about 0.8.

**The angular limit holds far better than P9's at coarse deflection** (0.023% of the area over it at 0.1, against P9's
1.02%), and about as well at fine. What turns by a half turn, as inverted, is only on cones and spheres, at an apex or a
pole: at 0.02, 22 of the cones' 138 triangles over the limit and 6 of the spheres' 34 are slivers under a degree, P9's
"conical points [that] cannot satisfy an angular test at any size". B-splines, 64% of the faces, have none over 0.51 rad
at 0.02 and 0.83 at 0.1.

By family at 0.1 and 0.02:

| Deflection | Family | Faces | Triangles | Max deviation / deflection | Max turn (rad) | Over angular | Area over angular | Smallest angle | Mean smallest |
|---|---|---|---|---|---|---|---|---|---|
| 0.1 | B-spline | 741 | 105,420 | 0.954 | 0.83 | 69 | 0.0135% | 0.71 | 25.3 |
| 0.1 | cone | 12 | 14,356 | 0.933 | 3.14 | 200 | 0.3052% | 0.27 | 26.6 |
| 0.1 | cylinder | 250 | 101,104 | 0.918 | 1.15 | 798 | 0.1150% | 4.82 | 35.9 |
| 0.1 | plane | 134 | 9,220 | 0 | 0 | 0 | 0 | 0.07 | 4.9 |
| 0.1 | sphere | 14 | 5,096 | 0.940 | 3.13 | 36 | 0.0588% | 2.00 | 37.5 |
| 0.1 | torus | 12 | 10,582 | 0.767 | 0.38 | 4 | 0.0213% | 7.66 | 34.5 |
| 0.02 | B-spline | 741 | 167,074 | 0.956 | 0.51 | 8 | 0.0003% | 1.25 | 28.6 |
| 0.02 | cone | 12 | 21,764 | 0.938 | 3.14 | 138 | 0.0055% | 0.004 | 29.0 |
| 0.02 | cylinder | 250 | 131,198 | 0.938 | 0.45 | 144 | 0.0123% | 3.77 | 39.0 |
| 0.02 | plane | 134 | 11,940 | 0 | 0 | 0 | 0 | 0.05 | 3.8 |
| 0.02 | sphere | 14 | 19,338 | 0.942 | 3.14 | 34 | 0.0045% | 0.70 | 37.9 |
| 0.02 | torus | 12 | 20,908 | 0.937 | 0.31 | 0 | 0 | 4.88 | 37.8 |

**At P9's coarse 0.2 on the 12,151-face model, B-spline faces fill with slivers.** 570,751 of their 1,708,024 triangles
turn past the limit and 563,029 of all those over it have an angle under a degree; the B-splines' mean smallest angle is
10.8 degrees, against 25 to 31 at the finer deflections. They cover 0.097% of the B-splines' area, so what is drawn is
close to right, but nearly a fifth of the mesh's triangles are slivers that turn past the limit. On the other families
the coarse mesh behaves as at 0.1. P9's mesher was over the angular limit on 18% of its triangles here too, on 3.66% of
the area. At 0.005 nothing on a B-spline turns past 0.26 rad.

Planes are not refined, so their triangles are poor by design, as P9's were: mean smallest angle 2.0 to 6.3 degrees.

### Parallel scaling

Median ms of the whole mesh, with the speed-up against one thread on medians. Least and median were within 1% of each
other in every row.

| Model | Triangles | 1 | 2 | 3 | 4 | 5 | 6 | Speed-up at 2 | at 4 | at 6 |
|---|---|---|---|---|---|---|---|---|---|---|
| 1,163 faces, 0.02 | 372,222 | 2726 | 1575 | 1212 | 1028 | 912 | 822 | 1.73 | 2.65 | **3.32** |
| P9, 1,002 faces, 0.02 | 454,558 | 1150 | 633 | | 329 | | 223 | 1.82 | 3.49 | **5.17** |
| 12,151 faces, 0.05 | 2,961,572 | 19489 | 11361 | 8821 | 7455 | 6648 | 6054 | 1.72 | 2.61 | **3.22** |
| P9, 10,001 faces, 0.05 | 1,850,878 | 4289 | 2362 | | 1198 | | 819 | 1.82 | 3.58 | **5.24** |

**Bit-identical:** each model gave one digest in all 42 of its runs at 1 to 6 threads: `64e064d05ff878da` for 1,163
faces and `711d993f9113b95e` for 12,151.

**The real mesher is slower than P9's,** per triangle and whole: 7.3 µs a triangle on one thread at 1,163 faces and 6.6
at 12,151, against P9's 2.5 and 2.3. It makes fewer triangles than P9's at 0.02 and more at 0.1 and 0.05.

**The watertight check runs on one thread, and it is what holds the speed-up down.** The four phases timed apart on the
1,163-face model at 0.02 (`phases`, median of three, in ms):

| Threads | Edges sampled | Faces meshed | Merge | Watertight check |
|---|---|---|---|---|
| 1 | 49.2 | 2673.1 | 12.7 | 447.9 |
| 6 | 13.1 | 527.6 | 15.0 | 454.4 |

The faces, 84% of one thread's time, run 5.07 times as fast on six threads, as P9's did (4.8 to 5.3). The watertight
check, 14% of one thread's time, does not get faster at all, and is 45% of the six-thread time. The 12,151-face model
was not timed by phase; its whole speed-up, 3.22, has the same shape.

### Opening a model

Every open measures each edge's deviation from the faces that use it and checks the whole model, trusting nothing from
outside: `occam view`'s server opens each model it reads, natively, and in the page the editing worker and then every
mesh worker opens its own copy. On P9's models that is the biggest cost there is.

On the measuring machine, natively, the 1,163-face model takes **23,390 ms to open on one thread and 7,387 ms on six**,
and building it, every step committed and so checked, 22,945 ms. Meshing it whole at the page's 0.1 mm takes 537 ms on
six threads, and at 0.02 mm 2,726 ms on one.

Stack samples during the open were all inside an edge's deviation being measured against a B-spline face
(`occam_topo::geometric::deviation`, through `SplineSurface::closest`). The 28-face model of the perf class, two plates
and one panel, takes 0.37 s to open on the laptop and 0.04 s to mesh.

### The pool in the browser

The build host was shared with another worker's jobs and then with a person's game while these ran, so the page's
timings are the laptop's: headless Chromium 156 on the Intel Core i7-1165G7, four cores and eight threads, through the
laptop's job queue with nothing else queued, as [P10](2026-11-09-p10.md) timed Chromium. The laptop was not quiet: its
load average was 5.6 when the pool's runs began. The model is P9's 1,002-face model, 1,163 faces here, at the page's 0.1
mm: 245,778 triangles in 62 calls.

| Workers | Cold: navigation to drawn | Warm: the pool's job, least / median | Warm: the editing worker's open first | Warm: each worker's first call, its open in it | Warm: the other calls, summed / median call |
|---|---|---|---|---|---|
| 1 | 61.3 s | 30.3 / 30.4 s | 28.2 to 28.8 s | 28.1 to 28.1 s | 2.16 s / 29 ms |
| 2 | 65.2 s | 34.9 / 39.7 s | 31.4 to 34.8 s | 38.3 to 38.4 s | 2.51 s / 35 ms |
| 3 | 72.7 s | 40.7 / 46.5 s | 32.3 to 35.8 s | 39.6 to 39.6 s | 3.06 s / 41 ms |
| 4 | 86.8 s | 47.9 / 48.1 s | 27.6 to 29.5 s | 46.7 to 47.6 s | 3.45 s / 49 ms |
| 5 | 112.3 s | 62.7 / 67.0 s | 27.9 to 31.4 s | 46.0 to 67.0 s | 3.48 s / 51 ms |
| 6 | 105.9 s | 87.4 / 91.4 s | 32.1 to 34.7 s | 63.5 to 87.4 s | 6.04 s / 91 ms |

**On P9's models the pool's time is opening the model, not meshing it, and more workers make it slower.** Each worker
opens its own copy, measuring every edge and checking the model, before its first call: 28 s alone, 47 s with four
opening at once on four cores, up to 87 s with six. The editing worker opens it first, another 28 to 36 s, before any
worker has it. The meshing itself, the calls after each worker's first, is 2.2 s of calls on one worker, 29 ms each, and
about 1 s of wall time on four, well short of the long calls P10 warned of. So the page takes 61 s to draw this model
cold with one worker and 106 s with six, and every model the editing worker publishes costs its open and then the
pool's, about a minute at best, whatever the number of workers. P10's pool, whose workers each copied 1.57 MB of input
in 1.2 to 1.7 ms, scaled 4.7 times on six; this one cannot scale until a worker's open stops costing many times its
share of the meshing. Five and six workers are more than the laptop's four cores, and the editing worker idles.

### The editing worker's longest call

Construction's requests that `gates perf` times, and its polygon of 1,000 edges, each run by the editing worker in a
fresh page, five times; from the page's asking to its answer. On the measuring machine, which was quiet for these:

| Request | Least | Median |
|---|---|---|
| A polygon of 1,000 edges, its face, extruded | 1,747 ms | **1,759 ms** |
| The transforms: a box copied and moved, a cylinder scaled, one stretched into B-splines | 221 ms | 233 ms |
| Each primitive once | 113 ms | 118 ms |
| The sweeps: a plate with a hole extruded, a rectangle revolved | 104 ms | 109 ms |
| Every primitive measured | 10 ms | 12 ms |

The laptop gave the same order: 1,720 ms for the polygon, 237 for the transforms, 153 for the primitives. **The longest
call any of these makes is 1.76 s**, all of it on the editing worker, while the page's own thread stays free.

### From a run's save to the page's redraw

`occam run` adds a box of 2 mm to the model `occam view` watches; from the file's modification time to the end of the
first frame that shows it, finished on the GPU. Ten runs on the laptop, on the plate with a hole that the live demo
builds:

| Model | Least | Median | Slowest |
|---|---|---|---|
| The plate with a hole, then a box more each run | 171 ms | **364 ms** | 495 ms |
| P9's 1,002-face model, 1,163 faces, then a box more each run (three runs) | 117,441 ms | **120,625 ms** | 155,468 ms |

The server looks at the file every 200 ms, so up to 200 ms of each is waiting for the look; the rest is the server
opening the model, the page fetching it, the editing worker opening it and finding what changed, the mesh workers
opening it and meshing the new box's faces, and the upload. On the plate that is a few tens of milliseconds each. On
P9's model it is three opens one after another, the server's natively and the editing worker's and the six mesh workers'
in the page, each measuring every edge: two minutes for a box whose six faces mesh in milliseconds.

### A pick on a real GPU

`just view-pick-gpu <model> chromium|firefox --headed --workers 2`, run once each on P9's 10,001-face model (12,151
faces here) as the page `occam view` serves it, at its 0.1 mm: a click on each of 50 faces, from the pointer's release
to the answer, and to the end of the first frame that shows the face selected, finished on the GPU. The first pick,
which meshes the model whole in a worker, is timed apart. Headed, on each machine's own screen; two mesh workers, to
shorten the minutes each takes to open the model. The 16.7 ms is a
[goal](../decisions/2026-11-13-gpu-figures-are-goals.md), not a gate.

| Machine's GPU | Browser | Adapter it reported | First pick | Answered, median / p95 / most | Shown, median / p95 / most | Within 16.7 ms |
|---|---|---|---|---|---|---|
| NVIDIA GeForce GTX 1080 Ti, driver 580.196.07 | Chromium 156.0.8078.4, headed | `nvidia`, `pascal`, not a fallback | 26.3 s | 194 / 202 / 211 ms | **248** / 262 / 264 ms | no |
| NVIDIA GeForce GTX 1080 Ti, driver 580.196.07 | Firefox 157.0 (Playwright's build), headed | names hidden; not a fallback, and on the GTX 1080 Ti by `nvidia-smi` | 27.4 s | 382 / 415 / 695 ms | **428** / 536 / 788 ms | no |
| Intel Iris Xe (laptop, beside an Iris Xe MAX) | Chromium 156.0.8078.4, headed | did not finish | | | | |

**Every pick misses the goal, by 15 times in Chromium and 26 in Firefox, and most of it is not the pick.** A pick
against the CPU mesh is about 66,000 instructions natively (`mesh_pick`) and 7 µs in the browser. But each click asks
the worker twice, once to find the depth and once with the edges' tolerance, and `Session::picking` computes the model's
digest every time it is asked, before it finds it can keep the mesh it made for picking: two digests of the whole
12,151-face model a click. One takes 86.6 ms natively on the laptop (`just view-timing digest`), so two are about 173 of
the 194 ms before the answer. The 54 ms after it in Chromium, 46 in Firefox, to a frame finished on the GPU, is the page
selecting the face, uploading its mark and drawing the next frames. Neither part depends on the GPU much.

**The laptop could not draw the model.** Chromium's page with two mesh workers grew to 9.99 GB resident, past the 10G
memory cap of the laptop's job queue (the laptop has 15 GB), and the kernel's OOM killer stopped it before the first
pick; Firefox was not run there, by the rule to stop at the first OOM rather than retry. The build host's runs, in a 16G
cap on a 31 GB machine, were not stopped by it.

**`occam view` cannot serve this model.** Its server opens and checks the model before it says it listens, and `occam
view` waits 30 s for that; P9's 10,001-face model takes longer to open, so `occam view` gives up and stops the server.
The script starts the server as `occam view` does but waits for it, which is how these ran.

The build host's screen was locked through its runs. Other workers' jobs held its load average near 80 while Chromium's
page opened the model, falling to 2.5 before its picks; Firefox ran on a quiet machine.

## What this does not show

- **The pick on the laptop's integrated GPU,** which ran out of memory, nor Firefox there. Each browser ran once a
  machine, on one model.
- **The models are P9's synthetic ones,** rebuilt: their trims are exact lines and circles but for the panels' holes,
  and none comes from a real file.
- **The quality figures are sampled,** at 45 points a triangle for the deviation and 7 for the turn: lower bounds on the
  true maxima, as P9's were.
- **The browser figures are headless Chromium's alone:** the pool's and the redraw's on the laptop, a four-core machine,
  where five and six workers outnumber the cores; the editing worker's on both machines. Firefox, whose long-call
  penalty P10 found, and Safari were not measured, nor was the pool on the 12,151-face model, whose every open would
  take about ten times as long.
- **Where the open's time goes** was found by sampling a few stacks, not by a profiler: every sample was in an edge's
  deviation being measured against a B-spline face.
- **The build host is shared,** and the laptop is not a quiet machine. Each native measurement waited for a quiet build
  host, but another job may have started during one; the load at each start is in the raw output.

## Raw output

<details>
<summary>Raw output, one JSON line a measurement</summary>

Measuring machine, natively (`just view-timing`, pinned to CPUs 0 to 5), each preceded by the load it began at:

```text
# load: 1.37 5.87 12.36 1/511 388594
{"mode":"scale","target":1000,"faces":1163,"deflection":0.02,"threads":1,"triangles":372222,"least_ms":2719.6,"median_ms":2725.8,"ms":[2738.0,2728.7,2720.7,2719.6,2725.5,2728.0,2725.8],"digests":["64e064d05ff878da"]}
{"mode":"scale","target":1000,"faces":1163,"deflection":0.02,"threads":2,"triangles":372222,"least_ms":1572.1,"median_ms":1575.0,"ms":[1578.4,1572.1,1583.6,1575.0,1572.5,1575.2,1574.7],"digests":["64e064d05ff878da"]}
{"mode":"scale","target":1000,"faces":1163,"deflection":0.02,"threads":3,"triangles":372222,"least_ms":1206.8,"median_ms":1212.1,"ms":[1206.8,1220.3,1230.1,1211.3,1229.6,1212.1,1210.3],"digests":["64e064d05ff878da"]}
{"mode":"scale","target":1000,"faces":1163,"deflection":0.02,"threads":4,"triangles":372222,"least_ms":1024.2,"median_ms":1028.0,"ms":[1024.2,1032.1,1035.3,1032.6,1028.0,1027.7,1026.1],"digests":["64e064d05ff878da"]}
{"mode":"scale","target":1000,"faces":1163,"deflection":0.02,"threads":5,"triangles":372222,"least_ms":902.8,"median_ms":911.6,"ms":[918.4,911.6,902.8,916.5,903.2,914.7,904.3],"digests":["64e064d05ff878da"]}
{"mode":"scale","target":1000,"faces":1163,"deflection":0.02,"threads":6,"triangles":372222,"least_ms":820.7,"median_ms":821.5,"ms":[822.1,821.4,823.1,821.5,820.7,823.1,821.5],"digests":["64e064d05ff878da"]}
# load: 1.41 4.47 11.09 1/511 388768
{"mode":"quality","target":1000,"model_faces":1163,"deflection":0.1,"angle":0.35,"family":"all","faces":1163,"triangles":245778,"deviation_over_deflection":0.9535,"over_deflection":0,"turn":3.1416,"over_angle":1107,"over_angle_slivers":4,"area":64025.994,"area_over_angle":14.639386,"smallest":0.067,"mean_smallest":29.645,"under_20":68862,"refused":0,"open":0,"capped":0,"mesh_ms":536.9}
# load: 1.30 4.25 10.87 1/511 388843
{"mode":"quality","target":1000,"model_faces":1163,"deflection":0.02,"angle":0.35,"family":"all","faces":1163,"triangles":372222,"deviation_over_deflection":0.9562,"over_deflection":0,"turn":3.1416,"over_angle":324,"over_angle_slivers":28,"area":64043.578,"area_over_angle":0.969181,"smallest":0.004,"mean_smallest":32.463,"under_20":77998,"refused":0,"open":0,"capped":0,"mesh_ms":1093.9}
# load: 30.56 40.26 40.18 1/521 2067249
{"mode":"scale","target":10000,"faces":12151,"deflection":0.05,"threads":1,"triangles":2961572,"least_ms":19366.8,"median_ms":19489.1,"ms":[19540.2,19504.1,19489.1,19459.1,19550.7,19427.2,19366.8],"digests":["711d993f9113b95e"]}
{"mode":"scale","target":10000,"faces":12151,"deflection":0.05,"threads":2,"triangles":2961572,"least_ms":11337.2,"median_ms":11361.2,"ms":[11407.5,11337.5,11357.7,11361.2,11337.2,11380.2,11369.4],"digests":["711d993f9113b95e"]}
{"mode":"scale","target":10000,"faces":12151,"deflection":0.05,"threads":3,"triangles":2961572,"least_ms":8793.4,"median_ms":8821.2,"ms":[8800.9,8803.9,8793.4,8862.2,8885.7,8829.9,8821.2],"digests":["711d993f9113b95e"]}
{"mode":"scale","target":10000,"faces":12151,"deflection":0.05,"threads":4,"triangles":2961572,"least_ms":7446.3,"median_ms":7455.4,"ms":[7457.4,7446.3,7457.9,7451.8,7465.1,7455.1,7455.4],"digests":["711d993f9113b95e"]}
{"mode":"scale","target":10000,"faces":12151,"deflection":0.05,"threads":5,"triangles":2961572,"least_ms":6637.6,"median_ms":6648.4,"ms":[6637.6,6652.1,6648.4,6639.5,6642.4,6654.5,6649.7],"digests":["711d993f9113b95e"]}
{"mode":"scale","target":10000,"faces":12151,"deflection":0.05,"threads":6,"triangles":2961572,"least_ms":6049.5,"median_ms":6054.1,"ms":[6068.3,6060.7,6049.5,6050.6,6054.1,6050.3,6054.6],"digests":["711d993f9113b95e"]}
# load: 1.46 6.01 20.42 1/493 2069317
{"mode":"quality","target":10000,"model_faces":12151,"deflection":0.2,"angle":0.35,"family":"all","faces":12151,"triangles":3164576,"deviation_over_deflection":0.9539,"over_deflection":0,"turn":3.1416,"over_angle":621658,"over_angle_slivers":563029,"area":700354.140,"area_over_angle":1295.581609,"smallest":0.000,"mean_smallest":20.851,"under_20":1521240,"refused":0,"open":1,"capped":1,"mesh_ms":12322.8}
# load: 1.39 3.44 16.26 1/494 2069579
{"mode":"quality","target":1000,"model_faces":1163,"deflection":0.005,"angle":0.35,"family":"all","faces":1163,"triangles":1189912,"deviation_over_deflection":0.9657,"over_deflection":0,"turn":3.1416,"over_angle":204,"over_angle_slivers":52,"area":64051.057,"area_over_angle":0.017174,"smallest":0.003,"mean_smallest":34.591,"under_20":161003,"refused":0,"open":0,"capped":0,"mesh_ms":2735.7}
# load: 1.48 3.24 15.71 1/494 2069669
{"mode":"build","target":1000,"faces":1163,"ms":22945.3,"digest":"6c02faa25fa97a8d","edges_stray_mm":7.0858605464945376e-12}
{"mode":"open","file":".timing/p9-1k.occam","threads":1,"faces":1163,"ms":23390.3}
{"mode":"open","file":".timing/p9-1k.occam","threads":6,"faces":1163,"ms":7387.1}
{"mode":"phases","target":1000,"deflection":0.02,"threads":1,"edges_ms":49.2,"faces_ms":2673.1,"merge_ms":12.7,"watertight_ms":447.9}
{"mode":"phases","target":1000,"deflection":0.02,"threads":6,"edges_ms":13.1,"faces_ms":527.6,"merge_ms":15.0,"watertight_ms":454.4}
{"mode":"browser","chromium":"156.0.8078.4","cpus":6}
{"mode":"editing","request":"construct_primitives","least_ms":112.59999999403954,"median_ms":118.29999999701977,"ms":[113,131,118,117,122]}
{"mode":"editing","request":"construct_sweeps","least_ms":104.19999999552965,"median_ms":108.5,"ms":[107,104,114,115,109]}
{"mode":"editing","request":"construct_transforms","least_ms":220.70000000298023,"median_ms":232.70000000298023,"ms":[221,241,242,227,233]}
{"mode":"editing","request":"construct_polygon_extrude","least_ms":1746.5,"median_ms":1759,"ms":[1754,1747,1765,1759,1785]}
{"mode":"editing","request":"measure_primitives","least_ms":9.5,"median_ms":11.600000001490116,"ms":[10,10,12,13,12]}
```

Laptop (`just view-timing-browser` and `just view-timing open`, through its queue):

```text
{"mode":"save","target":1000,"faces":1163,"bytes":3348463,"digest":"6c02faa25fa97a8d","file":".timing/p9-1k.occam"}
# load: 1.63 2.23 4.08 2/830 226879
{"mode":"browser","chromium":"156.0.8078.4","cpus":8}
{"mode":"editing","request":"construct_primitives","least_ms":124.69999998807907,"median_ms":153,"ms":[125,151,153,171,167]}
{"mode":"editing","request":"construct_sweeps","least_ms":147,"median_ms":152.40000000596046,"ms":[147,152,159,150,155]}
{"mode":"editing","request":"construct_transforms","least_ms":217.19999998807907,"median_ms":236.5,"ms":[321,217,237,303,220]}
{"mode":"editing","request":"construct_polygon_extrude","least_ms":1692.5,"median_ms":1720.4000000059605,"ms":[1693,1720,1710,1737,1727]}
{"mode":"editing","request":"measure_primitives","least_ms":9.600000023841858,"median_ms":13.099999994039536,"ms":[10,13,13,11,14]}
# load: 1.41 1.89 3.59 1/838 228602
{"mode":"browser","chromium":"156.0.8078.4","cpus":8}
{"mode":"redraw","model":"plate.json","rounds":10,"least_ms":171,"median_ms":364,"settled_ms":[303,171,199,392,363,354,374,364,495,371],"drawn_ms":[268,129,164,348,314,310,332,306,465,317]}
# load: 2.55 2.08 3.57 1/829 229141
{"mode":"browser","chromium":"156.0.8078.4","cpus":8}
{"mode":"pool","model":"p9-1k.occam","workers":1,"faces":1163,"triangles":245778,"cold_ms":61318,"cold_settled_ms":61426,"cold":{"first_calls_ms":[29195],"other_calls_ms_sum":2224,"other_calls_ms_median":30,"calls":62},"warm_ms":[30513,30379,30284],"warm_open_ms":[28825,28272,28205],"warm_least_ms":30284,"warm_median_ms":30379,"warm":{"first_calls_ms":[28122],"other_calls_ms_sum":2157,"other_calls_ms_median":29,"calls":62}}
{"mode":"pool","model":"p9-1k.occam","workers":2,"faces":1163,"triangles":245778,"cold_ms":65214,"cold_settled_ms":65344,"cold":{"first_calls_ms":[33206,33264],"other_calls_ms_sum":2714,"other_calls_ms_median":37,"calls":62},"warm_ms":[34879,42502,39654],"warm_open_ms":[31417,33221,34795],"warm_least_ms":34879,"warm_median_ms":39654,"warm":{"first_calls_ms":[38440,38333],"other_calls_ms_sum":2512,"other_calls_ms_median":35,"calls":62}}
{"mode":"pool","model":"p9-1k.occam","workers":3,"faces":1163,"triangles":245778,"cold_ms":72730,"cold_settled_ms":72905,"cold":{"first_calls_ms":[42238,42248,42008],"other_calls_ms_sum":3616,"other_calls_ms_median":57,"calls":62},"warm_ms":[46495,47112,40656],"warm_open_ms":[32329,35800,32440],"warm_least_ms":40656,"warm_median_ms":46495,"warm":{"first_calls_ms":[39642,39562,39639],"other_calls_ms_sum":3056,"other_calls_ms_median":41,"calls":62}}
{"mode":"pool","model":"p9-1k.occam","workers":4,"faces":1163,"triangles":245778,"cold_ms":86774,"cold_settled_ms":86930,"cold":{"first_calls_ms":[57626,57478,48333,58331],"other_calls_ms_sum":3611,"other_calls_ms_median":52,"calls":62},"warm_ms":[47883,48054,48062],"warm_open_ms":[29530,27559,27550],"warm_least_ms":47883,"warm_median_ms":48054,"warm":{"first_calls_ms":[46738,47637,47093,47208],"other_calls_ms_sum":3454,"other_calls_ms_median":49,"calls":62}}
{"mode":"pool","model":"p9-1k.occam","workers":5,"faces":1163,"triangles":245778,"cold_ms":112271,"cold_settled_ms":112381,"cold":{"first_calls_ms":[46414,47046,46735,66086,66922],"other_calls_ms_sum":5022,"other_calls_ms_median":79,"calls":62},"warm_ms":[88846,62652,67019],"warm_open_ms":[27909,29125,31382],"warm_least_ms":62652,"warm_median_ms":67019,"warm":{"first_calls_ms":[49120,45998,66947,47271,67012],"other_calls_ms_sum":3479,"other_calls_ms_median":51,"calls":62}}
{"mode":"pool","model":"p9-1k.occam","workers":6,"faces":1163,"triangles":245778,"cold_ms":105894,"cold_settled_ms":106003,"cold":{"first_calls_ms":[69343,76252,53097,61008,73971,77689],"other_calls_ms_sum":6201,"other_calls_ms_median":88,"calls":62},"warm_ms":[92883,91416,87409],"warm_open_ms":[32066,33299,34703],"warm_least_ms":87409,"warm_median_ms":91416,"warm":{"first_calls_ms":[85247,87238,77703,75368,87401,63461],"other_calls_ms_sum":6038,"other_calls_ms_median":91,"calls":62}}
# load: 5.68 5.15 4.26 2/838 243730
{"mode":"browser","chromium":"156.0.8078.4","cpus":8}
{"mode":"redraw","model":"p9-1k.occam","rounds":3,"least_ms":117441,"median_ms":120625,"settled_ms":[155468,120625,117441],"drawn_ms":[155357,120538,117354]}
# load: 5.58 5.59 4.94 1/832 249537
{"mode":"open","file":".timing/p9-1k.occam","threads":1,"faces":1163,"ms":21900.6}
{"mode":"open","file":".timing/p9-1k.occam","threads":6,"faces":1163,"ms":10421.3}
# load: 4.51 5.31 4.87 1/829 250325
```

The per-family quality rows are in the tables above.

</details>

## Open questions for the user

1. **Should a mesh worker trust the model the editing worker publishes?** Each worker opens its own copy as `occam view`
   and the editing worker do, measuring every edge and checking the whole model, though the editing worker has just done
   so. On P9's 1,002-face model that open is most of a cold draw and of every redraw in the page. Recommended: open the
   published twin without measuring or checking it again in the mesh workers, a decision about trust that is the user's.
