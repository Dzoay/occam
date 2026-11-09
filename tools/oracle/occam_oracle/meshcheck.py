"""Judges a triangle mesh by what it is in space, the same way for every engine."""

import collections
import math


def _weld(points, tolerance):
    """One id per point, merging points within `tolerance`, found by scanning the neighbouring cells of a uniform grid."""
    cells = collections.defaultdict(list)
    ids = [0] * len(points)
    next_id = 0
    limit = tolerance * tolerance
    for index, (x, y, z) in enumerate(points):
        cell = (math.floor(x / tolerance), math.floor(y / tolerance), math.floor(z / tolerance))
        match = next(
            (
                identity
                for i in (cell[0] - 1, cell[0], cell[0] + 1)
                for j in (cell[1] - 1, cell[1], cell[1] + 1)
                for k in (cell[2] - 1, cell[2], cell[2] + 1)
                for (px, py, pz), identity in cells.get((i, j, k), ())
                if (px - x) ** 2 + (py - y) ** 2 + (pz - z) ** 2 <= limit
            ),
            None,
        )
        if match is None:
            match = next_id
            next_id += 1
            cells[cell].append(((x, y, z), match))
        ids[index] = match
    return ids


def summarize(points, triangles, tolerance):
    """`triangles` and `vertices` after welding, and `watertight`: every welded edge used exactly twice, opposed."""
    ids = _weld(points, tolerance)
    kept = []
    for a, b, c in triangles:
        triple = ids[a], ids[b], ids[c]
        if len(set(triple)) == 3:
            kept.append(triple)
    directed = collections.Counter()
    for a, b, c in kept:
        directed.update(((a, b), (b, c), (c, a)))
    watertight = bool(directed) and all(count == 1 and directed[(v, u)] == 1 for (u, v), count in directed.items())
    return {"triangles": len(kept), "vertices": len({v for triple in kept for v in triple}), "watertight": watertight}
