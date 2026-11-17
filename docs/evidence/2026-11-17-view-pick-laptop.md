# A pick on the laptop's GPU, headed, in Chromium and Firefox

In short: on the laptop, both browsers' WebGPU ran on the Intel Iris Xe MAX, and the page drew P9's 10,001-face model and
answered 50 picks of 50 in each; the laptop's page no longer runs out of memory. In Chromium a pick's answer took 9 ms
(median) and the frame showing it 81 ms; in Firefox 212 and 262 ms. Neither meets the 16.7 ms goal for the frame. The
page's own process held the memory, 3.1 GB in Chromium and 4.1 GB in Firefox at its peak, while the GPU process, the
compositor and `occam view`'s server stayed small: the earlier 9.99 GB is not reproduced, and these samples do not
show what made it.

## Question

[The view's timing](2026-11-16-view-timing.md) timed a pick on a GTX 1080 Ti, and on the laptop Chromium's headed page
grew to 9.99 GB, past its job's 10G cap, and was stopped before its first pick, so Firefox was not run there. Since
then the mesh workers trust the editing worker's check and a pick keeps its digest
([the view on large models](2026-11-17-view-large-models.md)). On the laptop's own GPUs, headed: which adapter does each
browser get when the page asks for high performance, how long does a pick take against the
[16.7 ms goal](../decisions/2026-11-13-gpu-figures-are-goals.md), and how much memory does the page take, and where?

## What was measured

**The machine.** The laptop: an Intel Core i7-1165G7, four cores and eight threads, 15 GB, with two GPUs, the Intel
Iris Xe (TigerLake-LP GT2, PCI 8086:9a49, the integrated one, `renderD128`) and the Intel Iris Xe MAX (DG1, PCI
8086:4905, discrete, `renderD129`). Linux 7.3 with i915, Mesa 26.2.6. Its own screen, which the user freed for the run.
The load average before each run is below.

**The run.** `web/test/pick-timing.mjs` on main `ce33395`, as `just view-pick-gpu` runs it, on P9's 10,001-face model
(12,151 faces), headed from the start, two mesh workers, 50 picks after the first: Chromium 156.0.8078.4 and then
Firefox 157.0 (Playwright's build 1555). Each pick is a click on a face, timed from the pointer's release to the answer
and to the first frame showing the face selected, finished on the GPU; the first pick, which meshes the model whole in
a worker, is timed apart. `occam view` started the server, and waited for it to open the model, 50 to 55 s.

**Memory.** Each browser ran in a systemd scope of its own, capped at 8 GB, and the server in another, so that each
is counted apart; a sampler read once a second the browser scope's anonymous memory and the anonymous memory and
high-water mark (`RssAnon`, `VmHWM`) of each process in it and of the server, and would have stopped the browser at
once past 6 GB. Neither came near. Chromium's main process moves itself into a scope of its own as it starts, so it
was sampled apart; the zygotes it had already started, and with them the GPU process and the page, stayed in the
browser's scope.

**The adapter.** Chromium reports `intel`, `gen-12lp` and no device, and Firefox hides the names, so which GPU did
the work was read from the kernel's counters for each DRM client (`drm-pdev` and `drm-engine-render` in
`/proc/<pid>/fdinfo`). Firefox's WebGPU runs in its parent process, which the sampler read during the run. Chromium's
GPU process holds both GPUs open, and its counters were read in a short run of its own after the measurement: a
headed page asking for a high-performance adapter as the viewer's does, with the same flags, kept busy for six
seconds.

## Results

### The pick

| Browser | GPU the WebGPU work ran on | First pick | Answered, median / p95 / most | Shown, median / p95 / most | Within 16.7 ms | Load before |
|---|---|---|---|---|---|---|
| Chromium 156.0.8078.4, headed | Iris Xe MAX: 3.4 s of render engine time, against 8 ms on the Iris Xe | 31.9 s | **9** / 10 / 14 ms | **81** / 82 / 88 ms | no | 0.85 |
| Firefox 157.0, headed | Iris Xe MAX: 4.9 s of render engine time; the Iris Xe 1.0 s, its compositor | 30.3 s | **212** / 240 / 395 ms | **262** / 450 / 511 ms | no | 0.29 |

Every one of the 50 clicks hit a face in both. **In Chromium the pick itself is now within the goal,** 9 ms from the
pointer's release to the answer, against 194 ms on the GTX 1080 Ti before a pick kept its digest; the frame that
shows the face selected takes about 70 ms more. **In Firefox the answer takes 212 ms,** though the
pick in its worker is the same: where that time goes, between the click and the answer reaching the page, was not
measured. The code differs from the GTX 1080 Ti's run by the digest's fix, so the two machines' figures are not a
comparison of GPUs.

### The page's memory

| Browser | The page's process, anonymous at its peak (high-water) | GPU side | Browser's scope, anonymous at its peak | Everything charged to it | Server, in its own scope |
|---|---|---|---|---|---|
| Chromium | renderer **3,103 MB** (3,251) | GPU process 89 MB (231); main process 59 MB (279), sampled apart | 3,334 MB | 4,124 MB | 122 MB (173) |
| Firefox | content process **4,114 MB** (4,254) | parent process, which runs WebGPU and the compositor, 316 MB (602) | 4,470 MB | 5,199 MB | 157 MB (173) |

**The page's own process holds it:** its main thread and its workers. In both browsers it held about 0.2 GB for the
three to four minutes the editing worker opened the model, then rose within a minute, as the workers meshed it, the
page drew it and the first pick meshed it whole in a worker, to its peak, and fell as the window closed. The GPU
process and Firefox's parent process stayed under 0.32 GB anonymous; by the kernel's counters, Firefox's parent held
0.19 GB of GPU buffers on the Iris Xe MAX and 0.33 GB on the Iris Xe. The server held 0.17 GB at most. Everything charged to the scopes, the GPU buffers in system memory and the browsers' files
included, stayed under 5.2 GB.

**The earlier 9.99 GB is not reproduced, and these samples do not explain it.** That run was on the code before the
mesh workers trusted the editing worker, with the server in the same 10G scope as the browser. Today
the server and the GPU side are small, and the page's own process is what grows: on this laptop, headed, to 3.1 and
4.1 GB, where headless Chromium on the build host's SwiftShader held 1.63 GB
([the view on large models](2026-11-17-view-large-models.md)). Whether that difference is the headed page on a real GPU
or the machine was not separated.

## What this does not show

- **What the page's process holds,** at its peak: the samples count its memory, not what it is.
- **Where Firefox's answer spends 212 ms.**
- **Other GPUs and browsers on this machine:** each browser ran once, on the GPU it chose, never on the Iris Xe alone.

## Raw output

<details>
<summary>The timing script's lines, and the samples' peaks</summary>

```text
# load before: 0.85 0.61 0.30
{"mode":"pick-gpu","browser":"chromium","version":"156.0.8078.4","headless":false,"adapter":{"vendor":"intel","architecture":"gen-12lp","device":"","description":"","isFallbackAdapter":false},"model":"p9-10k.occam","faces":12151,"workers":2,"first_pick_ms":31900.19999998808,"picks":50,"hit":50,"answered_ms":{"median":9,"p95":10.199999988079071,"max":13.699999988079071},"shown_ms":{"median":80.7000000178814,"p95":82.30000001192093,"max":88.2999999821186},"goal_ms":16.7,"within_goal":false}
# samples, peaks in MB: scope anon 3334, memory.peak 4124; renderer anon 3103, hwm 3251; gpu-process anon 89, hwm 231;
# node anon 88, hwm 139; zygote anon 19, hwm 78; utility anon 18, hwm 122; main process (own scope) anon 59, hwm 279;
# server (own scope) anon 122, hwm 173
# load before: 0.29 1.12 0.81
{"mode":"pick-gpu","browser":"firefox","version":"157.0","headless":false,"adapter":{"vendor":"","architecture":"","device":"","description":"","isFallbackAdapter":false},"model":"p9-10k.occam","faces":12151,"workers":2,"first_pick_ms":30296,"picks":50,"hit":50,"answered_ms":{"median":212,"p95":240,"max":395},"shown_ms":{"median":262,"p95":450,"max":511},"goal_ms":16.7,"within_goal":false}
# samples, peaks in MB: scope anon 4470, memory.peak 5199, shmem 409; content process anon 4114, hwm 4254; parent
# anon 316, hwm 602; node anon 87, hwm 137; others under 15 anon; server (own scope) anon 157, hwm 173
# DRM clients of Firefox's parent at the end: 0000:00:02.0 render 1013 ms, resident 331 MB; 0000:03:00.0 render
# 4901 ms, resident 194 MB
# load before the Chromium adapter run: 0.77 1.45 1.16
{"vendor":"intel","architecture":"gen-12lp","device":"","description":"","submits":1551}
# DRM clients of its GPU process at the end: 0000:00:02.0 render 8 ms, resident 203 MB; 0000:03:00.0 render 3439 ms,
# resident 28 MB
```

</details>
