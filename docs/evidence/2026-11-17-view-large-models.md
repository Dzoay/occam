# The view on large models: before and after the mesh workers trust the editing worker

In short: on P9's 1,002-face model the mesh workers' pool now does in 2.6 s on one worker, and 0.50 s on six, what took
35 s whatever the number of workers, since a worker no longer opens and checks its own copy; the page draws the model
cold in 32 to 34 s instead of 65 to 66, and a run's save reaches it in 39 s instead of 74. A pick through a session on
P9's 10,001-face model takes 0.036 ms instead of 57, the digest it no longer computes each time. The page's peak memory
on that model with two mesh workers is unchanged, 1.63 GB headless, and `occam view` now serves it.

## Question

What do the three changes that follow [the view's timing](2026-11-16-view-timing.md) do to its figures on P9's models:
the page's mesh workers opening the twin the editing worker checked without checking it again
([decision](../decisions/2026-11-16-mesh-workers-trust-the-editing-worker.md)); a pick computing the model's digest only
when it makes its picker, not on every pick; and `occam view` waiting while its server opens the model
([decision](../decisions/2026-11-17-occam-view-waits-while-its-server-opens.md))? The pool, the cold draw and the time
from a run's save to the page's redraw on the 1,002-face model, a pick natively, and the page's peak memory with two
mesh workers on the 10,001-face model, which the laptop's page ran out of memory on.

## What was measured

**Where and how.** Each figure was taken twice on the build host, the timing's measuring machine, on 2026-11-17, with
the same commands and the same saved models (P9's 1,163 and 12,151 faces, as `just view-timing save` writes them), each
run pinned to CPUs 0 to 5. *Before* is the code with `occam view`'s wait alone, so that it serves the 10,001-face model;
*after* has all three. Both were measured on this branch before it was replayed onto main, so without #82 and #84,
whose faster check of plates would shorten the opens of both alike. Another session's tests shared the machine for part
of each run; the load each step began at is in the raw output. These are not the laptop's figures in
[the view's timing](2026-11-16-view-timing.md), so compare before with after here.

**Tools.** `just view-timing digest <file>` and `just view-timing pick <file>` natively: the model's digest, and picks
through a session as a mesh worker makes them, on one thread at the page's limits, the first and then twenty one at a
time, down onto the model's triangles as `gates perf` picks. `just view-timing-browser pool`, `redraw` and `memory` in
headless Chromium 156 on SwiftShader; `memory` reads the most each of the browser's processes held resident (Linux's
`VmHWM`) from navigation to the model drawn and picked once.

## Results

### The pool on P9's 1,002-face model (1,163 faces, 245,778 triangles, 62 calls)

| Workers | | Cold: navigation to drawn | Warm: the pool's job, least / median | Warm: each worker's first call | Warm: the other calls, summed / median |
|---|---|---|---|---|---|
| 1 | before | 65.1 s | 35.0 / 35.6 s | 32.4 s | 2.58 s / 35 ms |
| 1 | after | **34.1 s** | 2.59 / **2.59 s** | 35 ms | 2.55 s / 34 ms |
| 6 | before | 66.1 s | 34.5 / 34.5 s | 33.8 to 34.5 s | 2.57 s / 37 ms |
| 6 | after | **32.0 s** | 0.50 / **0.50 s** | 40 to 72 ms | 2.59 s / 38 ms |

**The pool's job is now its meshing, and it scales:** a worker's first call no longer opens the model, so the warm job
fell from 35.6 s to 2.59 s on one worker and from 34.5 s to 0.50 s on six, 5.2 times one worker's. What is left of a
cold draw is the editing worker's open, the page's one check, 31 to 33 s before and after alike.

### From a run's save to the page's redraw, on the same model, three runs

| | Least | Median | Slowest |
|---|---|---|---|
| before | 72.3 s | 74.3 s | 76.2 s |
| after | 37.9 s | **38.8 s** | 38.9 s |

What is left is the two checked opens one after the other: the server's, natively, and the editing worker's.

### A pick, natively, on P9's 10,001-face model (12,151 faces)

| | The digest, median | A pick through a session, median (least) of 20 | The first pick, which meshes the model whole |
|---|---|---|---|
| before | 56.9 ms | 56.8 ms (56.4) | 18.5 s |
| after | 58.6 ms | **0.036 ms** (0.028) | 19.8 s |

**A pick no longer costs a digest:** each pick took as long as the model's digest, which it computed and dropped; now
only the first, which makes the picker, computes one. The digest itself is unchanged code, 57 to 59 ms on this machine
(86.6 on the laptop). A click asks two picks, so this is about 115 ms of each click's answer on this machine; the picks
on a real GPU were not timed again, those being headed runs the user schedules.

### The page's peak memory, P9's 10,001-face model, two mesh workers, headless

| | Renderer: the page and its workers | GPU process | Navigation to drawn | First pick |
|---|---|---|---|---|
| before | 1,634 MB | 428 MB | 595 s | 24.3 s |
| after | 1,633 MB | 428 MB | **302 s** | 25.9 s |

**The peak did not move,** though the page draws the model in half the time: the opens' measuring is not where it is.
Watched as they ran, the renderer held under 0.6 GB while the workers opened the model, and about 1.1 GB partway
through the first pick, in which one worker meshes the model whole and keeps the mesh and its box trees, beside the
copies of the model the editing worker and both mesh workers hold and the mesh the page laid end to end. The browser's
processes together held at most 2.3 GB in either run, their peaks summed, so headless Chromium on SwiftShader does not
come near the 9.99 GB at which the laptop's headed page was stopped; what made that, headed on the Iris Xe in a cgroup
that held the native server too, these runs do not show.

## What this does not show

- **The pick on a real GPU,** headed, which [the view's timing](2026-11-16-view-timing.md) timed: those runs are the
  user's to schedule. Natively a click's two picks lose about 115 ms between them on this machine.
- **What stopped the laptop's headed page at 9.99 GB.** Headless here the browser's processes together peak at no more
  than 2.3 GB, before and after alike.
- **Firefox, Safari, and other machines.** One run of each, on one machine shared with others' tests.

## Raw output

<details>
<summary>Raw output, one JSON line a measurement</summary>

Before, with `occam view`'s wait alone:

```text
# load: 0.12 0.37 0.40 1/1108 1338531
{"mode":"save","target":1000,"faces":1163,"bytes":3348463,"digest":"6c02faa25fa97a8d","file":"~/.cache/occam-pick/p9-1k.occam"}
# load: 1.22 0.77 0.54 1/1114 1338884
{"mode":"digest","faces":12151,"least_ms":56.74,"median_ms":56.88,"digest":"2ec9a4c48b4a56b4"}
# load: 3.54 1.41 0.77 2/1114 1338968
{"mode":"pick","faces":12151,"first_ms":18504.9,"picks":20,"least_ms":56.373,"median_ms":56.780}
# load: 3.27 1.86 0.98 1/1114 1339076
{"mode":"browser","chromium":"156.0.8078.4","cpus":6}
{"mode":"pool","model":"p9-1k.occam","workers":1,"faces":1163,"triangles":245778,"cold_ms":65144,"cold_settled_ms":65234,"cold":{"first_calls_ms":[31138],"other_calls_ms_sum":2593,"other_calls_ms_median":35,"calls":62},"warm_ms":[35632,35628,35012],"warm_open_ms":[33144,32974,32622],"warm_least_ms":35012,"warm_median_ms":35628,"warm":{"first_calls_ms":[32425],"other_calls_ms_sum":2580,"other_calls_ms_median":35,"calls":62}}
{"mode":"pool","model":"p9-1k.occam","workers":6,"faces":1163,"triangles":245778,"cold_ms":66064,"cold_settled_ms":66160,"cold":{"first_calls_ms":[33773,34454,34126,33958,33891,34380],"other_calls_ms_sum":2679,"other_calls_ms_median":40,"calls":62},"warm_ms":[34547,34539,34529],"warm_open_ms":[31355,31311,31274],"warm_least_ms":34529,"warm_median_ms":34539,"warm":{"first_calls_ms":[34522,33912,34371,33793,33797,34104],"other_calls_ms_sum":2569,"other_calls_ms_median":37,"calls":62}}
# load: 4.38 3.16 2.02 1/1111 1341964
{"mode":"browser","chromium":"156.0.8078.4","cpus":6}
{"mode":"redraw","model":"p9-1k.occam","rounds":3,"least_ms":72334,"median_ms":74318,"settled_ms":[72334,74318,76171],"drawn_ms":[72267,74239,76099]}
# load: 4.66 3.64 2.54 2/1148 1343394
{"mode":"browser","chromium":"156.0.8078.4","cpus":6}
{"mode":"memory","model":"p9-10k.occam","workers":2,"faces":12151,"drawn_ms":595477,"first_pick_ms":24289,"renderer_peak_mb":1634,"gpu_peak_mb":428,"browser_peak_mb":103,"processes":[{"kind":"browser","peak_mb":103},{"kind":"zygote","peak_mb":44},{"kind":"zygote","peak_mb":44},{"kind":"gpu-process","peak_mb":428},{"kind":"utility","peak_mb":77},{"kind":"renderer","peak_mb":1634}]}
# load: 1.68 2.01 2.19 1/1114 1344542
```

After, with all three; the first digest and pick ran beside this branch's own wasm build, so they were taken again on a quieter machine, the second pair:

```text
# load: 27.43 19.67 10.06 2/1267 1483731
{"mode":"digest","faces":12151,"least_ms":59.72,"median_ms":59.93,"digest":"2ec9a4c48b4a56b4"}
# load: 15.18 18.08 10.83 2/1155 1490263
{"mode":"pick","faces":12151,"first_ms":19414.6,"picks":20,"least_ms":0.028,"median_ms":0.036}
# load: 7.66 15.31 10.40 2/1154 1491005
{"mode":"browser","chromium":"156.0.8078.4","cpus":6}
{"mode":"pool","model":"p9-1k.occam","workers":1,"faces":1163,"triangles":245778,"cold_ms":34112,"cold_settled_ms":34201,"cold":{"first_calls_ms":[63],"other_calls_ms_sum":2575,"other_calls_ms_median":35,"calls":62},"warm_ms":[2585,2591,2595],"warm_open_ms":[31341,31151,31287],"warm_least_ms":2585,"warm_median_ms":2591,"warm":{"first_calls_ms":[35],"other_calls_ms_sum":2553,"other_calls_ms_median":34,"calls":62}}
{"mode":"pool","model":"p9-1k.occam","workers":6,"faces":1163,"triangles":245778,"cold_ms":32033,"cold_settled_ms":32133,"cold":{"first_calls_ms":[98,104,128,96,96,111],"other_calls_ms_sum":2784,"other_calls_ms_median":42,"calls":62},"warm_ms":[503,503,502],"warm_open_ms":[31415,31184,31193],"warm_least_ms":502,"warm_median_ms":503,"warm":{"first_calls_ms":[40,53,72,56,55,59],"other_calls_ms_sum":2585,"other_calls_ms_median":38,"calls":62}}
# load: 1.17 5.40 7.46 1/1109 1492325
{"mode":"browser","chromium":"156.0.8078.4","cpus":6}
{"mode":"redraw","model":"p9-1k.occam","rounds":3,"least_ms":37874,"median_ms":38849,"settled_ms":[37874,38849,38861],"drawn_ms":[37807,38781,38794]}
# load: 2.00 3.65 6.32 1/1117 1492796
{"mode":"browser","chromium":"156.0.8078.4","cpus":6}
{"mode":"memory","model":"p9-10k.occam","workers":2,"faces":12151,"drawn_ms":302472,"first_pick_ms":25884,"renderer_peak_mb":1633,"gpu_peak_mb":428,"browser_peak_mb":103,"processes":[{"kind":"browser","peak_mb":103},{"kind":"zygote","peak_mb":43},{"kind":"zygote","peak_mb":43},{"kind":"gpu-process","peak_mb":428},{"kind":"utility","peak_mb":75},{"kind":"renderer","peak_mb":1633}]}
# load: 3.64 2.65 4.84 2/1138 1494749
# the digest and the pick taken again:
# load: 3.06 2.57 4.78 3/1138 1494925
{"mode":"digest","faces":12151,"least_ms":58.40,"median_ms":58.61,"digest":"2ec9a4c48b4a56b4"}
# load: 5.41 3.27 4.91 2/1143 1495563
{"mode":"pick","faces":12151,"first_ms":19758.5,"picks":20,"least_ms":0.028,"median_ms":0.036}
# load: 5.04 3.71 4.95 2/1145 1496307
```

</details>
