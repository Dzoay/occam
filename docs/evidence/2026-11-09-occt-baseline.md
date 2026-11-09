# OCCT whole-operation baseline

What occam has to beat, measured from outside on the generated corpus. Every target stated from these numbers is a
**goal, not a gate** (L46). OCCT is run as a black box through a pinned wheel outside the workspace; none of its
measured volumes or areas appear here (O6).

## Method and limits

- Scope: 346 distinct cases timed; 200 corpus cases left out because they have no boolean, fillet or
  tessellate operation to time (primitives alone, and the single-solid roles of the near-tangent groups).
- Per case: 1 warm-up run excluded, then between 5 and 30 timed repetitions (median 17).
- Whole operations only (L32): the timer wraps the entire boolean, fillet or tessellate call, forced to completion;
  the result (volume or triangle count) is measured after the timer stops, and a repetition whose result is empty is
  refused. Inputs (primitives, transforms, earlier booleans that are inputs) are rebuilt fresh, outside the timer, for
  every repetition. Times are wall clock (`perf_counter` for native runs, `performance.now()` under Node), in a scrubbed
  environment, one process per run of a group at a thread count, restarted only after a crash or hang, so the cost of
  importing the binding is not counted or repeated.
- Each native timing process is pinned to as many physical cores as it has threads, one logical CPU per core, by CPU
  affinity. The Node process is not pinned, and V8 may use helper threads for compilation and garbage collection.
- Per-case time is the median of its timed repetitions. Family statistics are over the per-case times: count, median,
  minimum, 90th percentile (linear interpolation) and total. "Spread" is the median across cases of (slowest - fastest
  repetition) / median repetition.

## The recorded machine

| | |
| --- | --- |
| CPU | Intel(R) Core(TM) i7-8700K CPU @ 3.70GHz |
| Physical cores | 6 |
| Logical cores | 12 |
| Governor | powersave |
| Kernel | 7.3.1-1-cachyos |
| Memory (GiB) | 31.3 |
| OCCT wheel | cadquery-ocp 8.0.1.1.0 |
| CPython | 3.13.18 |
| Node | v26.13.0 |

| Run | Threads | Load before (1/5/15 min) | Load after (1/5/15 min) |
| --- | --- | --- | --- |
| occt-native booleans | 1 | 1.48 1.99 1.76 | 1.02 1.53 1.62 |
| occt-native booleans | 2 | 2.61 1.81 1.69 | 1.07 1.41 1.54 |
| occt-native booleans | 4 | 1.07 1.41 1.54 | 1.04 1.24 1.44 |
| occt-native booleans | 6 | 1.04 1.24 1.44 | 1.08 1.19 1.38 |
| occt-native fillets | 1 | 1.02 1.53 1.62 | 1.02 1.51 1.61 |
| occt-native tessellation | 1 | 1.02 1.51 1.61 | 1.01 1.48 1.60 |
| occt-native tessellation | 2 | 1.46 1.18 1.23 | 1.33 1.17 1.23 |
| occt-native tessellation | 4 | 1.33 1.17 1.23 | 1.23 1.16 1.22 |
| occt-native tessellation | 6 | 1.23 1.16 1.22 | 1.23 1.16 1.22 |
| ocjs-node booleans (near-tangent) | 1 | 0.68 0.55 0.88 | 0.96 0.70 0.90 |
| ocjs-node booleans (regular) | 1 | 0.26 0.47 0.87 | 0.68 0.55 0.88 |
| ocjs-node fillets | 1 | 0.13 0.45 0.86 | 0.26 0.47 0.87 |
| ocjs-node tessellation | 1 | 0.05 0.44 0.86 | 0.13 0.45 0.86 |

## Per-family tables (OCCT native, one thread)

| Family | Cases | Median (ms) | Min (ms) | P90 (ms) | Total (ms) | Spread of repeats (median) |
| --- | --- | --- | --- | --- | --- | --- |
| booleans-common | 44 | 2.946 | 1.469 | 9.222 | 198.966 | 0.040 |
| booleans-cut | 64 | 2.827 | 1.164 | 8.451 | 256.043 | 0.031 |
| booleans-fuse | 36 | 3.215 | 1.392 | 9.198 | 150.703 | 0.033 |
| fillets | 16 | 3.264 | 0.936 | 5.592 | 52.234 | 0.040 |
| near-tangent-common | 61 | 15.086 | 1.459 | 57.036 | 1650.980 | 0.008 |
| near-tangent-cut | 18 | 1.691 | 0.815 | 2.568 | 32.800 | 0.035 |
| near-tangent-fuse | 72 | 14.137 | 1.011 | 54.984 | 1940.291 | 0.017 |
| tessellation | 24 | 1.504 | 0.557 | 6.194 | 107.653 | 0.036 |

Not counted in near-tangent-common: `near-crossed-external-0008-intersection`, `near-crossed-external-0009-intersection`, `near-crossed-external-0016-intersection`, `near-crossed-external-0017-intersection`, `near-crossed-external-0018-intersection`, `near-crossed-internal-0009-intersection`, `near-sphere-box-0008-intersection`, `near-sphere-box-0009-intersection`, `near-sphere-box-0016-intersection`, `near-sphere-box-0017-intersection`, `near-sphere-box-0018-intersection`.
Timed, but disagreed with their closed-form volume: `sphere-cap-common-0005`, `sphere-cap-cut-0002`, `sphere-cap-cut-0004`, `sphere-lens-common-0002`, `sphere-lens-common-0006`, `sphere-lens-common-0007`, `sphere-lens-fuse-0004`, `sphere-lens-fuse-0006`.

By corpus family:

| Family | Cases | Median (ms) | Min (ms) | P90 (ms) | Total (ms) | Spread of repeats (median) |
| --- | --- | --- | --- | --- | --- | --- |
| box-corner-common | 8 | 2.919 | 2.877 | 2.962 | 23.366 | 0.041 |
| box-corner-cut | 8 | 2.979 | 2.953 | 3.021 | 23.870 | 0.043 |
| box-corner-fuse | 8 | 3.277 | 3.201 | 3.301 | 26.122 | 0.037 |
| box-slab-common | 4 | 2.693 | 2.681 | 2.711 | 10.785 | 0.055 |
| box-slab-cut | 4 | 2.672 | 2.657 | 2.696 | 10.703 | 0.032 |
| box-slab-fuse | 4 | 2.989 | 2.986 | 3.014 | 11.987 | 0.065 |
| box-stacked-cut | 4 | 1.180 | 1.164 | 1.193 | 4.718 | 0.039 |
| box-stacked-fuse | 4 | 1.392 | 1.392 | 1.410 | 5.594 | 0.038 |
| coaxial-equal-common | 4 | 1.546 | 1.534 | 1.559 | 6.186 | 0.049 |
| coaxial-equal-cut | 4 | 1.536 | 1.530 | 1.539 | 6.141 | 0.031 |
| coaxial-equal-fuse | 4 | 1.656 | 1.655 | 1.658 | 6.626 | 0.048 |
| coaxial-stepped-common | 4 | 1.559 | 1.487 | 1.562 | 6.167 | 0.052 |
| coaxial-stepped-cut | 4 | 1.526 | 1.508 | 1.572 | 6.151 | 0.042 |
| coaxial-stepped-fuse | 4 | 1.617 | 1.585 | 1.657 | 6.491 | 0.038 |
| coaxial-tube | 4 | 1.938 | 1.929 | 1.944 | 7.750 | 0.033 |
| fillet-box-all | 8 | 5.589 | 5.583 | 5.594 | 44.713 | 0.017 |
| fillet-box-edge | 8 | 0.940 | 0.936 | 0.944 | 7.521 | 0.046 |
| near-crossed-external-common | 13 | 12.563 | 8.456 | 15.061 | 157.132 | 0.007 |
| near-crossed-external-fuse | 18 | 10.529 | 1.011 | 15.534 | 169.239 | 0.022 |
| near-crossed-internal-common | 17 | 37.575 | 24.755 | 42.919 | 649.846 | 0.005 |
| near-crossed-internal-fuse | 18 | 38.364 | 25.469 | 44.323 | 895.034 | 0.008 |
| near-crossed-steinmetz-common | 18 | 47.993 | 6.022 | 78.924 | 818.737 | 0.007 |
| near-crossed-steinmetz-fuse | 18 | 49.222 | 6.452 | 80.338 | 836.921 | 0.008 |
| near-sphere-box-common | 13 | 1.696 | 1.459 | 2.561 | 25.265 | 0.037 |
| near-sphere-box-cut | 18 | 1.691 | 0.815 | 2.568 | 32.800 | 0.035 |
| near-sphere-box-fuse | 18 | 2.103 | 1.011 | 2.972 | 39.097 | 0.035 |
| plate-hole | 8 | 2.018 | 2.014 | 2.050 | 16.230 | 0.039 |
| plate-holes | 8 | 7.316 | 3.986 | 11.851 | 62.145 | 0.013 |
| sphere-cap-common | 8 | 4.107 | 1.469 | 7.169 | 34.381 | 0.031 |
| sphere-cap-cut | 8 | 4.874 | 1.455 | 9.490 | 39.932 | 0.027 |
| sphere-lens-common | 8 | 9.529 | 1.543 | 11.786 | 70.911 | 0.019 |
| sphere-lens-cut | 8 | 6.989 | 1.506 | 9.148 | 54.124 | 0.019 |
| sphere-lens-fuse | 8 | 9.198 | 1.604 | 11.515 | 69.080 | 0.016 |
| steinmetz-common | 8 | 6.131 | 5.002 | 6.343 | 47.171 | 0.017 |
| steinmetz-cut | 4 | 6.088 | 5.566 | 6.490 | 24.280 | 0.016 |
| steinmetz-fuse | 4 | 6.358 | 5.271 | 6.679 | 24.803 | 0.026 |
| tess-box | 4 | 0.558 | 0.557 | 0.569 | 2.245 | 0.079 |
| tess-cone | 4 | 1.811 | 1.430 | 4.605 | 10.755 | 0.035 |
| tess-cylinder | 4 | 0.997 | 0.899 | 2.141 | 5.521 | 0.060 |
| tess-plate | 4 | 1.251 | 1.200 | 2.027 | 6.062 | 0.023 |
| tess-sphere | 4 | 2.406 | 1.198 | 11.081 | 20.578 | 0.024 |
| tess-torus | 4 | 6.134 | 5.846 | 32.950 | 62.493 | 0.030 |

## Thread scaling (OCCT native)

OCCT's own parallel switches, through the oracle harness's `fix_parallelism`. At one thread the booleans' global
parallel mode (`BOPAlgo_Options.SetParallelMode_s`) and the mesher's default (`BRepMesh_IncrementalMesh.SetParallelDefault_s`)
are off. Above one thread both are on, the mesher's `isInParallel` argument is true, and the size of the default pool is
fixed once, before any work, with `OSD_ThreadPool.DefaultPool_s(N)`; every record states the size the pool reported. Each
process is pinned to N physical cores. Fillets are left out here: the fillet API has no parallel switch.

"CPU/wall" is the median across cases of process CPU time over wall time inside the timed calls. A value near 1 means
the switch did not turn into parallel work for those cases, whatever the thread count says. Speed-ups are over the
cases that succeeded at every thread count. They include any change of clock speed with the number of busy cores.

### booleans

| Threads | Cases | Median (ms) | Total (ms) | Speed-up of median | Speed-up of total | Median per-case speed-up | CPU/wall |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | 144 | 2.975 | 605.713 | 1.00 | 1.00 | 1.00 | 1.00 |
| 2 | 144 | 2.715 | 514.951 | 1.10 | 1.18 | 1.10 | 1.29 |
| 4 | 144 | 3.134 | 547.830 | 0.95 | 1.11 | 0.95 | 1.36 |
| 6 | 144 | 3.337 | 558.309 | 0.89 | 1.08 | 0.92 | 1.44 |

Determinism across repetitions and thread counts:

No difference in the measured result between repetitions or thread counts.

### near-tangent booleans

| Threads | Cases | Median (ms) | Total (ms) | Speed-up of median | Speed-up of total | Median per-case speed-up | CPU/wall |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | 151 | 12.563 | 3624.071 | 1.00 | 1.00 | 1.00 | 1.00 |
| 2 | 151 | 10.161 | 3160.869 | 1.24 | 1.15 | 1.15 | 1.21 |
| 4 | 151 | 9.707 | 3024.802 | 1.29 | 1.20 | 1.16 | 1.32 |
| 6 | 151 | 9.794 | 2979.585 | 1.28 | 1.22 | 1.18 | 1.38 |

Determinism across repetitions and thread counts:

No difference in the measured result between repetitions or thread counts.

### tessellation

| Threads | Cases | Median (ms) | Total (ms) | Speed-up of median | Speed-up of total | Median per-case speed-up | CPU/wall |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | 24 | 1.504 | 107.653 | 1.00 | 1.00 | 1.00 | 1.00 |
| 2 | 24 | 1.220 | 101.607 | 1.23 | 1.06 | 1.17 | 1.38 |
| 4 | 24 | 1.175 | 101.218 | 1.28 | 1.06 | 1.08 | 1.53 |
| 6 | 24 | 1.283 | 102.646 | 1.17 | 1.05 | 1.03 | 1.42 |

Determinism across repetitions and thread counts:

No difference in the measured result between repetitions or thread counts.

### fillets (one thread only)

Determinism across repetitions:

No difference in the measured result between repetitions or thread counts.

## Node WebAssembly (`opencascade.js`)

These are WebAssembly numbers under Node's V8, **not a browser**: the host has no Chromium. The build embeds its own
OCCT (7.4), which differs from the native wheel's 8.0.1, so the two are not strictly the same code.
`opencascade.js` 1.1.1.

| Family | Cases | Median (ms) | Min (ms) | P90 (ms) | Total (ms) | Spread of repeats (median) |
| --- | --- | --- | --- | --- | --- | --- |
| booleans-common | 44 | 36.866 | 19.657 | 73.867 | 1952.246 | 0.008 |
| booleans-cut | 64 | 35.706 | 15.434 | 75.265 | 2839.259 | 0.007 |
| booleans-fuse | 36 | 39.329 | 18.453 | 72.186 | 1534.388 | 0.006 |
| fillets | 16 | 40.340 | 9.110 | 72.562 | 650.367 | 0.022 |
| near-tangent-common | 61 | 120.524 | 19.070 | 235.944 | 7411.797 | 0.007 |
| near-tangent-cut | 18 | 19.842 | 10.139 | 26.327 | 375.744 | 0.008 |
| near-tangent-fuse | 72 | 86.084 | 12.581 | 231.825 | 8070.637 | 0.007 |
| tessellation | 24 | 4.804 | 2.464 | 15.251 | 251.396 | 0.044 |

Not counted in near-tangent-common: `near-crossed-external-0008-intersection`, `near-crossed-external-0009-intersection`, `near-crossed-external-0016-intersection`, `near-crossed-external-0017-intersection`, `near-crossed-external-0018-intersection`, `near-crossed-internal-0009-intersection`, `near-sphere-box-0008-intersection`, `near-sphere-box-0009-intersection`, `near-sphere-box-0016-intersection`, `near-sphere-box-0017-intersection`, `near-sphere-box-0018-intersection`.
Timed, but disagreed with their closed-form volume: `sphere-cap-common-0005`, `sphere-cap-cut-0002`, `sphere-cap-cut-0004`, `sphere-lens-common-0002`, `sphere-lens-common-0006`, `sphere-lens-common-0007`, `sphere-lens-fuse-0004`, `sphere-lens-fuse-0006`.

By corpus family:

| Family | Cases | Median (ms) | Min (ms) | P90 (ms) | Total (ms) | Spread of repeats (median) |
| --- | --- | --- | --- | --- | --- | --- |
| box-corner-common | 8 | 35.327 | 34.879 | 35.525 | 282.194 | 0.010 |
| box-corner-cut | 8 | 35.706 | 35.363 | 35.804 | 285.423 | 0.012 |
| box-corner-fuse | 8 | 39.177 | 39.084 | 39.686 | 314.584 | 0.006 |
| box-slab-common | 4 | 37.202 | 36.653 | 37.553 | 148.706 | 0.013 |
| box-slab-cut | 4 | 37.061 | 36.478 | 37.068 | 147.670 | 0.006 |
| box-slab-fuse | 4 | 41.037 | 40.878 | 41.344 | 164.386 | 0.004 |
| box-stacked-cut | 4 | 15.505 | 15.434 | 15.548 | 61.994 | 0.005 |
| box-stacked-fuse | 4 | 18.493 | 18.453 | 18.612 | 74.098 | 0.009 |
| coaxial-equal-common | 4 | 23.546 | 23.334 | 23.714 | 94.183 | 0.016 |
| coaxial-equal-cut | 4 | 23.605 | 23.456 | 23.686 | 94.381 | 0.014 |
| coaxial-equal-fuse | 4 | 25.113 | 24.896 | 25.164 | 100.292 | 0.011 |
| coaxial-stepped-common | 4 | 23.844 | 20.931 | 24.001 | 92.678 | 0.013 |
| coaxial-stepped-cut | 4 | 21.054 | 21.018 | 23.215 | 87.261 | 0.016 |
| coaxial-stepped-fuse | 4 | 22.089 | 22.036 | 24.456 | 91.664 | 0.011 |
| coaxial-tube | 4 | 24.544 | 24.248 | 24.614 | 97.972 | 0.021 |
| fillet-box-all | 8 | 72.191 | 71.467 | 72.728 | 577.121 | 0.016 |
| fillet-box-edge | 8 | 9.141 | 9.110 | 9.208 | 73.246 | 0.030 |
| near-crossed-external-common | 13 | 81.092 | 62.702 | 88.921 | 1000.487 | 0.009 |
| near-crossed-external-fuse | 18 | 73.967 | 13.553 | 90.982 | 1125.105 | 0.009 |
| near-crossed-internal-common | 17 | 153.527 | 120.524 | 175.763 | 2640.162 | 0.004 |
| near-crossed-internal-fuse | 18 | 158.484 | 125.422 | 179.021 | 2891.345 | 0.005 |
| near-crossed-steinmetz-common | 18 | 204.142 | 55.837 | 273.341 | 3477.885 | 0.006 |
| near-crossed-steinmetz-fuse | 18 | 211.840 | 60.006 | 281.984 | 3604.669 | 0.005 |
| near-sphere-box-common | 13 | 21.630 | 19.070 | 26.945 | 293.263 | 0.011 |
| near-sphere-box-cut | 18 | 19.842 | 10.139 | 26.327 | 375.744 | 0.008 |
| near-sphere-box-fuse | 18 | 23.049 | 12.581 | 31.404 | 449.519 | 0.010 |
| plate-hole | 8 | 25.875 | 25.464 | 26.093 | 206.976 | 0.007 |
| plate-holes | 8 | 98.160 | 52.953 | 156.690 | 826.520 | 0.007 |
| sphere-cap-common | 8 | 38.241 | 19.657 | 57.733 | 314.099 | 0.005 |
| sphere-cap-cut | 8 | 41.599 | 19.317 | 73.482 | 347.888 | 0.005 |
| sphere-lens-common | 8 | 75.640 | 20.935 | 90.810 | 568.200 | 0.007 |
| sphere-lens-cut | 8 | 59.120 | 20.219 | 72.677 | 452.122 | 0.008 |
| sphere-lens-fuse | 8 | 72.186 | 21.335 | 88.007 | 550.640 | 0.003 |
| steinmetz-common | 8 | 57.772 | 50.002 | 60.137 | 452.187 | 0.011 |
| steinmetz-cut | 4 | 57.987 | 52.976 | 61.647 | 231.051 | 0.006 |
| steinmetz-fuse | 4 | 60.958 | 53.815 | 62.446 | 238.723 | 0.006 |
| tess-box | 4 | 2.646 | 2.548 | 2.741 | 10.598 | 0.098 |
| tess-cone | 4 | 7.626 | 4.222 | 14.482 | 35.621 | 0.193 |
| tess-cylinder | 4 | 2.656 | 2.464 | 4.806 | 13.465 | 0.038 |
| tess-plate | 4 | 4.931 | 4.700 | 6.873 | 22.257 | 0.064 |
| tess-sphere | 4 | 5.748 | 2.972 | 23.298 | 44.743 | 0.015 |
| tess-torus | 4 | 13.106 | 12.562 | 64.104 | 124.711 | 0.005 |

Determinism across repetitions:

No difference in the measured result between repetitions or thread counts.

## What occam has to beat

- **booleans-common**: median per-case 2.946 ms, total 198.966 ms, one thread, native. Goal, not a gate. Node WebAssembly: median 36.866 ms. Goal, not a gate.
- **booleans-cut**: median per-case 2.827 ms, total 256.043 ms, one thread, native. Goal, not a gate. Node WebAssembly: median 35.706 ms. Goal, not a gate.
- **booleans-fuse**: median per-case 3.215 ms, total 150.703 ms, one thread, native. Goal, not a gate. Node WebAssembly: median 39.329 ms. Goal, not a gate.
- **fillets**: median per-case 3.264 ms, total 52.234 ms, one thread, native. Goal, not a gate. Node WebAssembly: median 40.340 ms. Goal, not a gate.
- **near-tangent-common**: median per-case 15.086 ms, total 1650.980 ms, one thread, native. Goal, not a gate. Node WebAssembly: median 120.524 ms. Goal, not a gate.
- **near-tangent-cut**: median per-case 1.691 ms, total 32.800 ms, one thread, native. Goal, not a gate. Node WebAssembly: median 19.842 ms. Goal, not a gate.
- **near-tangent-fuse**: median per-case 14.137 ms, total 1940.291 ms, one thread, native. Goal, not a gate. Node WebAssembly: median 86.084 ms. Goal, not a gate.
- **tessellation**: median per-case 1.504 ms, total 107.653 ms, one thread, native. Goal, not a gate. Node WebAssembly: median 4.804 ms. Goal, not a gate.

## Run notes

- **Order and interference.** The native runs took place between 12:09 and 12:56 UTC on 2026-11-09 and the Node runs
  between 13:06 and 13:09. Every stretch held a lock shared with other timing work on the machine, so no two timings
  overlapped. The load averages above include each stretch's own work and the stretch before it.
- **One stretch was repeated.** The first attempt at the Node runs stopped when the terminal driving it crashed, before
  it had written a result. All three Node stretches were then run from the beginning, so nothing from that attempt is in
  these numbers.
- **Cases not counted, or disagreeing.** The 11 cases not counted are in
  [the corpus inventory](2026-11-09-corpus-inventory.md): intersections where the engine reports no solid, and both
  builds do so on the same cases. The eight volume disagreements are there too. The inventory found the error is in
  OCCT's measure of trimmed spherical faces and not in the closed form. Both builds show the same eight, and they are
  timed and counted.
- **Native and Node are different code.** The two builds give the same status on every case, and the same triangle count
  in 20 of the 24 tessellation cases. The four `tess-cone` cases mesh differently, so their times are not like for like.
  As medians of the per-case ratios, Node takes 12.5 times the native time on regular booleans, 11.3 on fillets and 2.8
  on tessellation. Near-tangent booleans have a median of 6.6 and a range from 0.8 to 44, which suggests the two versions
  take different paths there.
- **Thread scaling says little about large models.** Every case here is small, a few milliseconds to tens of
  milliseconds. The best speed-up of total time is 1.22 at six threads, and regular booleans have a slower median at
  four and six threads than at one. These numbers show that OCCT's parallel switches do little for small inputs. They
  are not a scaling target for large models, which this corpus does not contain.
