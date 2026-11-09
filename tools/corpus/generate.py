#!/usr/bin/env python3
"""Generates the parametric corpus: operation lists with closed-form answers where they exist.

Everything here is the project's own work. Each case records its family and seed in `origin` and is licensed
MIT OR Apache-2.0. Parameters are rounded before any closed form is computed, so a printed case and its answer agree;
derived placement coordinates and near-tangent offsets keep 12 decimals, which leaves the closed forms accurate to far
better than the tolerances they state.

`just corpus-generate` runs this. Run with --table to print the family table without writing anything.
"""

import argparse
import dataclasses
import itertools
import json
import math
import pathlib
import random
import re
import sys

CASE_SCHEMA = "occam-oracle-case/1"
LICENCE = "MIT OR Apache-2.0"
OUT = pathlib.Path(__file__).resolve().parent / "generated"

# Parameters carry DECIMALS places; placements derived from them, and near-tangent offsets, carry PLACEMENT_DECIMALS.
DECIMALS = 3
PLACEMENT_DECIMALS = 12

# A priori tolerances, stated before any oracle run: relative for volume and area, absolute (model units) for the box.
REL_TOL = 1e-6
BBOX_TOL = 1e-5

PI = math.pi
AXES = ((1.0, 0.0, 0.0), (0.0, 1.0, 0.0), (0.0, 0.0, 1.0))


@dataclasses.dataclass(frozen=True)
class Body:
    """An operation list, the id of the shape to measure, and the closed-form answer for it, if there is one."""

    ops: list
    result: str
    expect: dict | None = None


def _clean(value):
    """Removes -0.0, so a file never differs by the sign of a zero."""
    return value + 0.0


def _placed(vector):
    return [_clean(round(item, PLACEMENT_DECIMALS)) for item in vector]


def sig12(value):
    """Closed-form numbers to 12 significant digits: far inside every tolerance, and the same on every libm."""
    return float(f"{value:.12g}")


def _vecsig12(vector):
    return [_clean(sig12(item)) for item in vector]


def answer(volume=None, area=None, euler=None, bbox=None):
    """The `expect` block of a case; only the measures with a closed form are given."""
    expect = {}
    if volume is not None:
        expect["volume"] = {"value": sig12(volume), "rel_tol": REL_TOL}
    if area is not None:
        expect["area"] = {"value": sig12(area), "rel_tol": REL_TOL}
    if euler is not None:
        expect["euler"] = {"value": euler}
    if bbox is not None:
        expect["bbox"] = {"min": _vecsig12(bbox[0]), "max": _vecsig12(bbox[1]), "abs_tol": BBOX_TOL}
    return expect


def _unit(axis):
    length = math.sqrt(sum(item * item for item in axis))
    return tuple(item / length for item in axis)


def _box_op(op_id, size, origin):
    return {"op": "box", "id": op_id, "size": list(size), "origin": list(origin)}


def _cylinder_op(op_id, radius, height, origin, axis):
    return {"op": "cylinder", "id": op_id, "radius": radius, "height": height, "origin": list(origin), "axis": list(axis)}


def _sphere_op(op_id, radius, origin):
    return {"op": "sphere", "id": op_id, "radius": radius, "origin": list(origin)}


def _boolean_op(kind, op_id, target, tool):
    return {"op": kind, "id": op_id, "target": target, "tool": tool}


def _rotate_op(op_id, target, rotation):
    axis_origin, axis, angle = rotation
    return {"op": "rotate", "id": op_id, "target": target, "axis_origin": list(axis_origin), "axis": list(axis), "angle_degrees": angle}


def _box_bbox(low, size):
    return list(low), [a + b for a, b in zip(low, size)]


def _bbox_union(*boxes):
    return ([min(box[0][i] for box in boxes) for i in range(3)], [max(box[1][i] for box in boxes) for i in range(3)])


def _disc_extent(radius, unit, index):
    """Half the extent, along coordinate `index`, of a disc of `radius` with unit normal `unit`."""
    return radius * math.sqrt(max(0.0, 1.0 - unit[index] * unit[index]))


def _frustum_bbox(radius1, radius2, height, origin, axis):
    """The box of the convex hull of two discs; a cylinder is the case radius1 == radius2."""
    unit = _unit(axis)
    far = [origin[i] + height * unit[i] for i in range(3)]
    low = [min(origin[i] - _disc_extent(radius1, unit, i), far[i] - _disc_extent(radius2, unit, i)) for i in range(3)]
    high = [max(origin[i] + _disc_extent(radius1, unit, i), far[i] + _disc_extent(radius2, unit, i)) for i in range(3)]
    return low, high


def _cylinder_bbox(radius, height, origin, axis):
    return _frustum_bbox(radius, radius, height, origin, axis)


# Primitives ---------------------------------------------------------------------------------------------------------


def build_box(size, origin):
    a, b, c = size
    return Body([_box_op("a", size, origin)], "a", answer(a * b * c, 2 * (a * b + b * c + c * a), 2, _box_bbox(origin, size)))


def build_cylinder(radius, height, origin, axis):
    volume = PI * radius * radius * height
    area = 2 * PI * radius * (radius + height)
    ops = [_cylinder_op("a", radius, height, origin, axis)]
    return Body(ops, "a", answer(volume, area, 2, _cylinder_bbox(radius, height, origin, axis)))


def build_frustum(radius1, radius2, height, origin, axis):
    """A cone or frustum; `radius2` of zero makes the true apex cone."""
    slant = math.sqrt(height * height + (radius1 - radius2) ** 2)
    volume = PI * height * (radius1 * radius1 + radius1 * radius2 + radius2 * radius2) / 3
    area = PI * (radius1 * radius1 + radius2 * radius2) + PI * (radius1 + radius2) * slant
    ops = [{"op": "cone", "id": "a", "radius1": radius1, "radius2": radius2, "height": height, "origin": list(origin), "axis": list(axis)}]
    return Body(ops, "a", answer(volume, area, 2, _frustum_bbox(radius1, radius2, height, origin, axis)))


def build_sphere(radius, centre):
    bbox = ([c - radius for c in centre], [c + radius for c in centre])
    return Body([_sphere_op("a", radius, centre)], "a", answer(4 * PI * radius**3 / 3, 4 * PI * radius**2, 2, bbox))


def build_torus(major, minor, centre, axis):
    unit = _unit(axis)
    extent = [_disc_extent(major, unit, i) + minor for i in range(3)]
    bbox = ([c - e for c, e in zip(centre, extent)], [c + e for c, e in zip(centre, extent)])
    op = {"op": "torus", "id": "a", "major_radius": major, "minor_radius": minor, "origin": list(centre), "axis": list(axis)}
    return Body([op], "a", answer(2 * PI * PI * major * minor * minor, 4 * PI * PI * major * minor, 0, bbox))


# Booleans with closed forms ---------------------------------------------------------------------------------------------


def _box_volume_area(size):
    a, b, c = size
    return a * b * c, 2 * (a * b + b * c + c * a)


def build_box_pair(mode, op, low_a, size_a, low_b, size_b):
    """Two axis-aligned boxes by inclusion-exclusion.

    `corner`: b overlaps a's high corner on every axis, sticking out on all three.
    `slab`: same footprint in x and y, overlapping in z, so the side faces are coplanar.
    `stacked`: b sits on a's top with its footprint strictly inside, so the boxes touch along a face and nothing overlaps.
    """
    high_a = [low + size for low, size in zip(low_a, size_a)]
    high_b = [low + size for low, size in zip(low_b, size_b)]
    volume_a, area_a = _box_volume_area(size_a)
    volume_b, area_b = _box_volume_area(size_b)
    ops = [_box_op("a", size_a, low_a), _box_op("b", size_b, low_b), _boolean_op(op, "c", "a", "b")]
    if mode == "corner":
        low_i, size_i = list(low_b), [high - low for high, low in zip(high_a, low_b)]
        volume_i, area_i = _box_volume_area(size_i)
        if op == "fuse":
            expect = answer(volume_a + volume_b - volume_i, area_a + area_b - area_i, 2, (list(low_a), high_b))
        elif op == "cut":
            expect = answer(volume_a - volume_i, area_a, 2, _box_bbox(low_a, size_a))
        else:
            expect = answer(volume_i, area_i, 2, _box_bbox(low_i, size_i))
    elif mode == "slab":
        if op == "fuse":
            low, high = list(low_a), high_b
        elif op == "cut":
            low, high = list(low_a), [high_a[0], high_a[1], low_b[2]]
        else:
            low, high = [low_a[0], low_a[1], low_b[2]], high_a
        size = [h - l for h, l in zip(high, low)]
        volume, area = _box_volume_area(size)
        expect = answer(volume, area, 2, (low, high))
    elif mode == "stacked":
        if op == "fuse":
            contact = size_b[0] * size_b[1]
            expect = answer(volume_a + volume_b, area_a + area_b - 2 * contact, 2, (list(low_a), [high_a[0], high_a[1], high_b[2]]))
        else:
            expect = answer(volume_a, area_a, 2, _box_bbox(low_a, size_a))
    else:
        raise ValueError(f"unknown box pair mode {mode!r}")
    return Body(ops, "c", expect)


def build_plate(size, origin, holes):
    """A plate with through-holes, cut one by one. `holes` are (x, y, radius) in world coordinates."""
    length, width, thickness = size
    pad = 1.0
    ops = [_box_op("a", size, origin)]
    previous = "a"
    for number, (x, y, radius) in enumerate(holes):
        tool, cut = f"h{number}", f"c{number}"
        ops.append(_cylinder_op(tool, radius, thickness + 2 * pad, [x, y, origin[2] - pad], [0, 0, 1]))
        ops.append(_boolean_op("cut", cut, previous, tool))
        previous = cut
    squares = sum(radius * radius for _x, _y, radius in holes)
    rims = sum(radius for _x, _y, radius in holes)
    volume = length * width * thickness - PI * thickness * squares
    area = 2 * length * width + 2 * (length + width) * thickness - 2 * PI * squares + 2 * PI * thickness * rims
    return Body(ops, previous, answer(volume, area, 2 - 2 * len(holes), _box_bbox(origin, size)))


def _cap_volume(radius, height):
    return PI * height * height * (3 * radius - height) / 3


def _cap_area(radius, height):
    return 2 * PI * radius * height


def build_sphere_cap(mode, radius, centre, axis_index, sign, offset):
    """A sphere cut by, or meeting, a box whose face is the plane sign*(x - centre) = offset along one axis."""
    height = radius - offset
    disc_squared = radius * radius - offset * offset
    disc = math.sqrt(disc_squared)
    thickness = 3 * radius
    low = [c - 2 * radius for c in centre]
    size = [4 * radius] * 3
    low[axis_index] = centre[axis_index] + offset if sign > 0 else centre[axis_index] - offset - thickness
    size[axis_index] = thickness
    ops = [_sphere_op("a", radius, centre), _box_op("b", size, _placed(low)), _boolean_op(mode, "c", "a", "b")]

    sphere_volume, sphere_area = 4 * PI * radius**3 / 3, 4 * PI * radius**2
    cap_volume, cap_area = _cap_volume(radius, height), _cap_area(radius, height)
    if mode == "cut":
        volume, area = sphere_volume - cap_volume, sphere_area - cap_area + PI * disc_squared
        along = (centre[axis_index] - radius, centre[axis_index] + offset) if sign > 0 else (centre[axis_index] - offset, centre[axis_index] + radius)
        across = radius if offset >= 0 else disc
    else:
        volume, area = cap_volume, cap_area + PI * disc_squared
        along = (centre[axis_index] + offset, centre[axis_index] + radius) if sign > 0 else (centre[axis_index] - radius, centre[axis_index] - offset)
        across = radius if offset <= 0 else disc
    box_low = [c - across for c in centre]
    box_high = [c + across for c in centre]
    box_low[axis_index], box_high[axis_index] = along
    return Body(ops, "c", answer(volume, area, 2, (box_low, box_high)))


def _rotated(ops, ids, rotation):
    """Rotates each named shape by the same rigid rotation; the rotated ids are `r<id>`."""
    if rotation is None:
        return ops, {shape: shape for shape in ids}
    extra = [_rotate_op(f"r{shape}", shape, rotation) for shape in ids]
    return ops + extra, {shape: f"r{shape}" for shape in ids}


def build_steinmetz(op, radius, length, rotation=None):
    """Two equal cylinders on orthogonal axes through the origin: common volume 16r^3/3, surface 16r^2."""
    ops = [
        _cylinder_op("a", radius, length, _placed([-length / 2, 0, 0]), [1, 0, 0]),
        _cylinder_op("b", radius, length, _placed([0, -length / 2, 0]), [0, 1, 0]),
    ]
    ops, named = _rotated(ops, ("a", "b"), rotation)
    ops.append(_boolean_op(op, "c", named["a"], named["b"]))

    half = length / 2
    steinmetz_volume = 16 * radius**3 / 3
    steinmetz_area = 16 * radius * radius
    cylinder_volume = PI * radius * radius * length
    cylinder_area = 2 * PI * radius * length + 2 * PI * radius * radius
    bbox = None
    euler = 2
    if op == "common":
        volume, area = steinmetz_volume, steinmetz_area
        bbox = ([-radius, -radius, -radius], [radius, radius, radius])
    elif op == "fuse":
        volume, area = 2 * cylinder_volume - steinmetz_volume, 2 * cylinder_area - steinmetz_area
        bbox = ([-half, -half, -radius], [half, half, radius])
    else:
        # a cut b leaves two pieces meeting at the two pinch points of the intersection curve, so no Euler number is stated.
        volume, area = cylinder_volume - steinmetz_volume, cylinder_area
        bbox = ([-half, -radius, -radius], [half, radius, radius])
        euler = None
    return Body(ops, "c", answer(volume, area, euler, None if rotation else bbox))


def _along(origin, axis, distance):
    return _placed([origin[i] + distance * axis[i] for i in range(3)])


def build_coaxial_equal(op, radius, height_a, start, height_b, origin, axis):
    """Two cylinders of one radius on one axis: a over [0, height_a], b over [start, start+height_b] with overlap."""
    ops = [
        _cylinder_op("a", radius, height_a, origin, axis),
        _cylinder_op("b", radius, height_b, _along(origin, axis, start), axis),
        _boolean_op(op, "c", "a", "b"),
    ]
    if op == "fuse":
        low, length = 0.0, start + height_b
    elif op == "common":
        low, length = start, height_a - start
    else:
        low, length = 0.0, start
    volume, area = PI * radius * radius * length, 2 * PI * radius * radius + 2 * PI * radius * length
    bbox = _cylinder_bbox(radius, length, _along(origin, axis, low), axis)
    return Body(ops, "c", answer(volume, area, 2, bbox))


def build_coaxial_stepped(op, radius_a, radius_b, height_a, start, height_b, origin, axis):
    """A thinner cylinder b on a's axis, entering a's top and ending inside it."""
    ops = [
        _cylinder_op("a", radius_a, height_a, origin, axis),
        _cylinder_op("b", radius_b, height_b, _along(origin, axis, start), axis),
        _boolean_op(op, "c", "a", "b"),
    ]
    overlap = height_a - start
    protrusion = start + height_b - height_a
    annulus = PI * (radius_a * radius_a - radius_b * radius_b)
    if op == "fuse":
        volume = PI * radius_a * radius_a * height_a + PI * radius_b * radius_b * protrusion
        area = PI * radius_a * radius_a + 2 * PI * radius_a * height_a + annulus + 2 * PI * radius_b * protrusion + PI * radius_b * radius_b
        bbox = _bbox_union(_cylinder_bbox(radius_a, height_a, origin, axis), _cylinder_bbox(radius_b, start + height_b, origin, axis))
    elif op == "common":
        volume, area = PI * radius_b * radius_b * overlap, 2 * PI * radius_b * radius_b + 2 * PI * radius_b * overlap
        bbox = _cylinder_bbox(radius_b, overlap, _along(origin, axis, start), axis)
    else:
        volume = PI * radius_a * radius_a * height_a - PI * radius_b * radius_b * overlap
        area = PI * radius_a * radius_a + 2 * PI * radius_a * height_a + annulus + 2 * PI * radius_b * overlap + PI * radius_b * radius_b
        bbox = _cylinder_bbox(radius_a, height_a, origin, axis)
    return Body(ops, "c", answer(volume, area, 2, bbox))


def build_tube(radius_outer, radius_inner, height, origin, axis):
    """A tube: a coaxial cylinder cut through the full length of a wider one. Its surface is a torus, so Euler is 0."""
    pad = 1.0
    ops = [
        _cylinder_op("a", radius_outer, height, origin, axis),
        _cylinder_op("b", radius_inner, height + 2 * pad, _along(origin, axis, -pad), axis),
        _boolean_op("cut", "c", "a", "b"),
    ]
    squares = radius_outer * radius_outer - radius_inner * radius_inner
    volume = PI * squares * height
    area = 2 * PI * squares + 2 * PI * (radius_outer + radius_inner) * height
    return Body(ops, "c", answer(volume, area, 0, _cylinder_bbox(radius_outer, height, origin, axis)))


def build_sphere_lens(op, radius_a, radius_b, distance, centre_a, direction):
    """Two overlapping spheres; the lens they share is two spherical caps on the plane of their intersection circle."""
    unit = _unit(direction)
    centre_b = _along(centre_a, unit, distance)
    plane = (distance * distance + radius_a * radius_a - radius_b * radius_b) / (2 * distance)
    height_a = radius_a - plane
    height_b = radius_b - (distance - plane)
    lens_volume = _cap_volume(radius_a, height_a) + _cap_volume(radius_b, height_b)
    lens_area = _cap_area(radius_a, height_a) + _cap_area(radius_b, height_b)
    volume_a, area_a = 4 * PI * radius_a**3 / 3, 4 * PI * radius_a**2
    volume_b, area_b = 4 * PI * radius_b**3 / 3, 4 * PI * radius_b**2
    ops = [_sphere_op("a", radius_a, centre_a), _sphere_op("b", radius_b, centre_b), _boolean_op(op, "c", "a", "b")]
    bbox = None
    if op == "fuse":
        volume, area = volume_a + volume_b - lens_volume, area_a + area_b - lens_area
        boxes = [([c - r for c in centre], [c + r for c in centre]) for centre, r in ((centre_a, radius_a), (centre_b, radius_b))]
        bbox = _bbox_union(*boxes)
    elif op == "common":
        volume, area = lens_volume, lens_area
    else:
        volume = volume_a - lens_volume
        area = area_a - _cap_area(radius_a, height_a) + _cap_area(radius_b, height_b)
    return Body(ops, "c", answer(volume, area, 2, bbox))


def _edge_selector(size, origin, axis_index, corner, pad):
    others = [i for i in range(3) if i != axis_index]
    low = [origin[i] + size[i] * corner[others.index(i)] - pad if i in others else origin[i] - pad for i in range(3)]
    high = [origin[i] + size[i] * corner[others.index(i)] + pad if i in others else origin[i] + size[i] + pad for i in range(3)]
    return {"in_box": {"min": _placed(low), "max": _placed(high)}}


def build_fillet_edge(size, origin, axis_index, corner, radius):
    """One box edge along `axis_index`, at `corner` (low or high, for the two other axes in order), filleted."""
    length = size[axis_index]
    selector = _edge_selector(size, origin, axis_index, corner, pad=1e-3)
    ops = [_box_op("a", size, origin), {"op": "fillet", "id": "f", "target": "a", "radius": radius, "edges": selector}]
    a, b, c = size
    volume = a * b * c - (1 - PI / 4) * radius * radius * length
    area = 2 * (a * b + b * c + c * a) - 2 * radius * length + (PI / 2) * radius * length - 2 * radius * radius * (1 - PI / 4)
    return Body(ops, "f", answer(volume, area, 2, _box_bbox(origin, size)))


def build_fillet_all(size, origin, radius):
    """Every edge and corner of a box rounded: the Minkowski sum of the inner box with a ball."""
    a, b, c = (edge - 2 * radius for edge in size)
    ops = [_box_op("a", size, origin), {"op": "fillet", "id": "f", "target": "a", "radius": radius, "edges": "all"}]
    faces = a * b + b * c + c * a
    edges = a + b + c
    volume = a * b * c + 2 * radius * faces + PI * radius * radius * edges + 4 * PI * radius**3 / 3
    area = 2 * faces + 2 * PI * radius * edges + 4 * PI * radius * radius
    return Body(ops, "f", answer(volume, area, 2, _box_bbox(origin, size)))


def build_tessellated(shape, params, tessellation):
    """A primitive and its meshing. The count of triangles is reported by the run; `expect` cannot state topology (L12)."""
    linear, angular, relative = tessellation
    origin, axis = (0, 0, 0), (0, 0, 1)
    if shape == "sphere":
        body = build_sphere(*params, origin)
    elif shape == "cylinder":
        body = build_cylinder(*params, origin, axis)
    elif shape == "torus":
        body = build_torus(*params, origin, axis)
    elif shape == "box":
        body = build_box(params, origin)
    elif shape == "cone":
        body = build_frustum(*params, origin, axis)
    elif shape == "plate":
        length, width, thickness, radius = params
        body = build_plate((length, width, thickness), origin, [(length / 2, width / 2, radius)])
    else:
        raise ValueError(f"unknown shape {shape!r}")
    mesh = {"op": "tessellate", "id": "m", "target": body.result, "linear_deflection": linear, "angular_deflection": angular}
    if relative:
        mesh["relative"] = True
    return Body(body.ops + [mesh], body.result, {**body.expect, "watertight": {"value": True}})


# Near-tangent placements: no closed form, judged by the volume identity across a group's four results. --------------------


def _roles(base_ops, rotation):
    """The four results of the volume identity for two shapes with ids a and b, rotated rigidly when asked."""
    ops, named = _rotated(base_ops, ("a", "b"), rotation)
    a, b = named["a"], named["b"]
    single = {shape: [op for op in ops if op["id"] in {shape, f"r{shape}"}] for shape in ("a", "b")}
    return {
        "a": Body(single["a"], a),
        "b": Body(single["b"], b),
        "union": Body(ops + [_boolean_op("fuse", "u", a, b)], "u"),
        "intersection": Body(ops + [_boolean_op("common", "i", a, b)], "i"),
    }


def build_near_crossed(kind, radius_a, radius_b, offset, rotation=None):
    """Crossed cylinders, a along x and b along y, their axes `offset` from a tangent configuration.

    `external`: axes at radius_a + radius_b - offset, so the surfaces just overlap near one point.
    `internal`: axes at radius_a - radius_b - offset, so b's top generator lies just inside a's.
    `steinmetz`: equal radii and axes `offset` apart, so the two intersection curves nearly cross.
    """
    distance = {
        "external": radius_a + radius_b - offset,
        "internal": radius_a - radius_b - offset,
        "steinmetz": offset,
    }[kind]
    length = 8 * max(radius_a, radius_b)
    a = _cylinder_op("a", radius_a, length, _placed([-length / 2, 0, 0]), [1, 0, 0])
    b = _cylinder_op("b", radius_b, length, _placed([0, -length / 2, distance]), [0, 1, 0])
    return _roles([a, b], rotation)


def build_near_sphere_box(radius, axis_index, sign, offset, rotation=None):
    """A sphere at the origin and a box whose face lies `offset` inside the tangent plane at `radius` along one axis."""
    low = [-2 * radius] * 3
    size = [4 * radius] * 3
    depth = 2 * radius
    low[axis_index] = radius - offset if sign > 0 else -(radius - offset) - depth
    size[axis_index] = depth
    a = _sphere_op("a", radius, (0, 0, 0))
    b = _box_op("b", size, _placed(low))
    roles = _roles([a, b], rotation)
    named = ("ra", "rb") if rotation else ("a", "b")
    roles["cut"] = Body(roles["union"].ops[:-1] + [_boolean_op("cut", "x", *named)], "x")
    return roles


# Sampling ------------------------------------------------------------------------------------------------------------


def _uniform(rng, low, high):
    return _clean(round(rng.uniform(low, high), DECIMALS))


def _log(rng, low, high):
    return _clean(round(10 ** rng.uniform(math.log10(low), math.log10(high)), DECIMALS))


def _scaled(value, rng, low, high):
    return _clean(round(value * rng.uniform(low, high), DECIMALS))


def _point(rng, span=20.0):
    return [_uniform(rng, -span, span) for _ in range(3)]


def _direction(rng):
    while True:
        candidate = [_uniform(rng, -1.0, 1.0) for _ in range(3)]
        if math.sqrt(sum(item * item for item in candidate)) > 0.2:
            return candidate


def _axis(rng, index):
    """Cycles through the world axes; every fourth case takes an arbitrary direction."""
    return list(AXES[index % 3]) if index % 4 != 3 else _direction(rng)


def _rotation(rng):
    return (_point(rng, 5.0), _direction(rng), _uniform(rng, 5.0, 175.0))


def _sample_box(index, rng):
    return build_box([_log(rng, 0.5, 50.0) for _ in range(3)], _point(rng))


def _sample_cylinder(index, rng):
    return build_cylinder(_log(rng, 0.5, 20.0), _log(rng, 0.5, 50.0), _point(rng), _axis(rng, index))


def _sample_cone(index, rng):
    # The first two are true cones with an apex; the rest are frustums with two distinct radii.
    radius1 = _log(rng, 1.0, 20.0)
    radius2 = 0 if index <= 2 else _scaled(radius1, rng, 0.2, 0.8)
    return build_frustum(radius1, radius2, _log(rng, 1.0, 40.0), _point(rng), _axis(rng, index))


def _sample_sphere(index, rng):
    return build_sphere(_log(rng, 0.5, 50.0), _point(rng))


def _sample_torus(index, rng):
    minor = _log(rng, 0.2, 5.0)
    return build_torus(_scaled(minor, rng, 1.5, 5.0), minor, _point(rng), _axis(rng, index))


def _pair(mode, op):
    def make(index, rng):
        low_a = _point(rng, 10.0)
        size_a = [_log(rng, 2.0, 30.0) for _ in range(3)]
        if mode == "corner":
            overlaps = [_scaled(size, rng, 0.2, 0.8) for size in size_a]
            low_b = [_clean(round(low + size - overlap, DECIMALS)) for low, size, overlap in zip(low_a, size_a, overlaps)]
            size_b = [_clean(round(overlap + rng.uniform(0.5, 20.0), DECIMALS)) for overlap in overlaps]
        elif mode == "slab":
            overlap = _scaled(size_a[2], rng, 0.2, 0.8)
            low_b = [low_a[0], low_a[1], _clean(round(low_a[2] + size_a[2] - overlap, DECIMALS))]
            size_b = [size_a[0], size_a[1], _clean(round(overlap + rng.uniform(0.5, 20.0), DECIMALS))]
        else:
            size_b = [_scaled(size_a[0], rng, 0.2, 0.7), _scaled(size_a[1], rng, 0.2, 0.7), _log(rng, 1.0, 20.0)]
            low_b = [
                _clean(round(low_a[0] + rng.uniform(0.1, 0.9) * (size_a[0] - size_b[0]), DECIMALS)),
                _clean(round(low_a[1] + rng.uniform(0.1, 0.9) * (size_a[1] - size_b[1]), DECIMALS)),
                _clean(round(low_a[2] + size_a[2], DECIMALS)),
            ]
        return build_box_pair(mode, op, low_a, size_a, low_b, size_b)

    return make


def _sample_plate_hole(index, rng):
    origin = _point(rng, 10.0)
    length, width, thickness = _log(rng, 5.0, 100.0), _log(rng, 5.0, 100.0), _log(rng, 0.5, 10.0)
    radius = _scaled(min(length, width), rng, 0.1, 0.35)
    x = _clean(round(origin[0] + length / 2 + rng.uniform(-0.1, 0.1) * length, DECIMALS))
    y = _clean(round(origin[1] + width / 2 + rng.uniform(-0.1, 0.1) * width, DECIMALS))
    return build_plate((length, width, thickness), origin, [(x, y, radius)])


def _sample_plate_holes(index, rng):
    origin = _point(rng, 10.0)
    length, width, thickness = _log(rng, 30.0, 120.0), _log(rng, 30.0, 120.0), _log(rng, 0.5, 10.0)
    count = 2 + (index - 1) % 5
    columns = math.ceil(math.sqrt(count))
    rows = math.ceil(count / columns)
    cell_x, cell_y = length / columns, width / rows
    holes = []
    for row, column in itertools.islice(itertools.product(range(rows), range(columns)), count):
        radius = _scaled(min(cell_x, cell_y), rng, 0.15, 0.3)
        x = _clean(round(origin[0] + (column + 0.5 + rng.uniform(-0.1, 0.1)) * cell_x, DECIMALS))
        y = _clean(round(origin[1] + (row + 0.5 + rng.uniform(-0.1, 0.1)) * cell_y, DECIMALS))
        holes.append((x, y, radius))
    return build_plate((length, width, thickness), origin, holes)


def _sphere_cap(mode):
    def make(index, rng):
        radius = _log(rng, 0.5, 30.0)
        offset = 0 if index == 1 else _scaled(radius, rng, -0.85, 0.85)
        return build_sphere_cap(mode, radius, _point(rng), (index - 1) % 3, 1 if index % 2 else -1, offset)

    return make


def _steinmetz(op):
    def make(index, rng):
        radius = _log(rng, 0.5, 20.0)
        return build_steinmetz(op, radius, _scaled(radius, rng, 2.5, 5.0), None if index % 2 else _rotation(rng))

    return make


def _coaxial_axis(index):
    return list(AXES[index % 3])


def _coaxial_equal(op):
    def make(index, rng):
        height_a = _log(rng, 2.0, 40.0)
        start = _scaled(height_a, rng, 0.2, 0.8)
        height_b = _clean(round(height_a - start + rng.uniform(0.5, 20.0), DECIMALS))
        return build_coaxial_equal(op, _log(rng, 0.5, 20.0), height_a, start, height_b, _point(rng), _coaxial_axis(index))

    return make


def _coaxial_stepped(op):
    def make(index, rng):
        radius_a = _log(rng, 1.0, 20.0)
        radius_b = _scaled(radius_a, rng, 0.3, 0.8)
        height_a = _log(rng, 2.0, 40.0)
        start = _scaled(height_a, rng, 0.2, 0.8)
        height_b = _clean(round(height_a - start + rng.uniform(0.5, 20.0), DECIMALS))
        return build_coaxial_stepped(op, radius_a, radius_b, height_a, start, height_b, _point(rng), _coaxial_axis(index))

    return make


def _sample_tube(index, rng):
    radius_outer = _log(rng, 1.0, 20.0)
    return build_tube(radius_outer, _scaled(radius_outer, rng, 0.3, 0.8), _log(rng, 1.0, 40.0), _point(rng), _coaxial_axis(index))


def _sphere_lens(op):
    def make(index, rng):
        radius_a = _log(rng, 1.0, 20.0)
        radius_b = radius_a if index == 1 else _scaled(radius_a, rng, 0.4, 1.6)
        spread = abs(radius_a - radius_b)
        distance = _clean(round(spread + rng.uniform(0.15, 0.85) * (radius_a + radius_b - spread), DECIMALS))
        direction = list(AXES[index % 3]) if index % 2 else _direction(rng)
        return build_sphere_lens(op, radius_a, radius_b, distance, _point(rng), direction)

    return make


def _sample_fillet_edge(index, rng):
    size = [_log(rng, 4.0, 40.0) for _ in range(3)]
    axis_index = (index - 1) % 3
    across = min(size[i] for i in range(3) if i != axis_index)
    radius = _scaled(across, rng, 0.05, 0.4)
    corner = (rng.randrange(2), rng.randrange(2))
    return build_fillet_edge(size, _point(rng, 10.0), axis_index, corner, radius)


def _sample_fillet_all(index, rng):
    size = [_log(rng, 4.0, 40.0) for _ in range(3)]
    return build_fillet_all(size, _point(rng, 10.0), _scaled(min(size), rng, 0.05, 0.3))


TESSELLATIONS = ((0.5, 0.5, False), (0.05, 0.5, False), (0.01, 0.2, False), (0.02, 0.5, True))


def _tessellated(shape):
    def make(index, rng):
        if shape == "sphere":
            params = (_log(rng, 1.0, 20.0),)
        elif shape == "cylinder":
            params = (_log(rng, 1.0, 10.0), _log(rng, 1.0, 40.0))
        elif shape == "torus":
            minor = _log(rng, 0.5, 3.0)
            params = (_scaled(minor, rng, 1.5, 4.0), minor)
        elif shape == "box":
            params = tuple(_log(rng, 1.0, 30.0) for _ in range(3))
        elif shape == "cone":
            radius1 = _log(rng, 1.0, 10.0)
            params = (radius1, _scaled(radius1, rng, 0.2, 0.8), _log(rng, 1.0, 30.0))
        else:
            params = (_log(rng, 10.0, 40.0), _log(rng, 10.0, 40.0), _log(rng, 0.5, 5.0), _uniform(rng, 1.0, 4.0))
        return build_tessellated(shape, params, TESSELLATIONS[(index - 1) % len(TESSELLATIONS)])

    return make


def _offset(rng, exponent):
    return _clean(round(round(rng.uniform(1.0, 5.0), DECIMALS) * 10.0**-exponent, PLACEMENT_DECIMALS))


def _crossed_spec(kind, sample, exponent, rng):
    offset = _offset(rng, exponent)
    radius_a = _uniform(rng, 2.0, 20.0)
    if kind == "external":
        radius_b = _scaled(radius_a, rng, 0.4, 0.9)
    elif kind == "internal":
        radius_b = _scaled(radius_a, rng, 0.3, 0.8)
    else:
        radius_b = radius_a
    return {"offset": offset, "radius_a": radius_a, "radius_b": radius_b, "rotation": _rotation(rng) if sample else None}


def _near_crossed(kind):
    specs = list(itertools.product((0, 1), range(1, 10)))

    def make(index, rng):
        spec = _crossed_spec(kind, *specs[index - 1], rng)
        return build_near_crossed(kind, spec["radius_a"], spec["radius_b"], spec["offset"], spec["rotation"])

    make.count = len(specs)
    make.offset = lambda index, rng: _crossed_spec(kind, *specs[index - 1], rng)["offset"]
    return make


def _sphere_box_spec(sample, exponent, rng):
    return {
        "offset": _offset(rng, exponent),
        "radius": _uniform(rng, 2.0, 20.0),
        "axis_index": rng.randrange(3),
        "sign": 1 if rng.randrange(2) else -1,
        "rotation": _rotation(rng) if sample else None,
    }


def _near_sphere_box():
    specs = list(itertools.product((0, 1), range(1, 10)))

    def make(index, rng):
        spec = _sphere_box_spec(*specs[index - 1], rng)
        return build_near_sphere_box(spec["radius"], spec["axis_index"], spec["sign"], spec["offset"], spec["rotation"])

    make.count = len(specs)
    make.offset = lambda index, rng: _sphere_box_spec(*specs[index - 1], rng)["offset"]
    return make


@dataclasses.dataclass(frozen=True)
class Family:
    name: str
    base: int  # seed = base * 1000 + index, so a family's seeds stay put whatever is added around it
    count: int
    make: object
    parameters: str
    closed_form: str


def _near(name, base, make, parameters):
    return Family(name, base, make.count, make, parameters, "none: volume identity across the group")


FAMILIES = [
    Family("prim-box", 1, 12, _sample_box, "sizes 0.5-50, any position", "V, A, Euler 2, bbox"),
    Family("prim-cylinder", 2, 12, _sample_cylinder, "r 0.5-20, h 0.5-50, world and arbitrary axes", "V, A, Euler 2, bbox"),
    Family("prim-cone", 3, 12, _sample_cone, "apex cones and frustums, world and arbitrary axes", "V, A, Euler 2, bbox"),
    Family("prim-sphere", 4, 10, _sample_sphere, "r 0.5-50, any position", "V, A, Euler 2, bbox"),
    Family("prim-torus", 5, 10, _sample_torus, "R/r 1.5-5, world and arbitrary axes", "V, A, Euler 0, bbox"),
    Family("box-corner-fuse", 6, 8, _pair("corner", "fuse"), "overlap on every axis, generic position", "V, A, Euler 2, bbox"),
    Family("box-corner-cut", 7, 8, _pair("corner", "cut"), "overlap on every axis, generic position", "V, A, Euler 2, bbox"),
    Family("box-corner-common", 8, 8, _pair("corner", "common"), "overlap on every axis, generic position", "V, A, Euler 2, bbox"),
    Family("box-slab-fuse", 9, 4, _pair("slab", "fuse"), "same footprint, coplanar side faces", "V, A, Euler 2, bbox"),
    Family("box-slab-cut", 10, 4, _pair("slab", "cut"), "same footprint, coplanar side faces", "V, A, Euler 2, bbox"),
    Family("box-slab-common", 11, 4, _pair("slab", "common"), "same footprint, coplanar side faces", "V, A, Euler 2, bbox"),
    Family("box-stacked-fuse", 12, 4, _pair("stacked", "fuse"), "b on a's top face, footprint inside", "V, A, Euler 2, bbox"),
    Family("box-stacked-cut", 13, 4, _pair("stacked", "cut"), "b on a's top face, no overlap", "V, A, Euler 2, bbox"),
    Family("plate-hole", 14, 8, _sample_plate_hole, "one through-hole", "V, A, Euler 0, bbox"),
    Family("plate-holes", 15, 8, _sample_plate_holes, "2 to 6 through-holes", "V, A, Euler 2-2g, bbox"),
    Family("sphere-cap-cut", 16, 8, _sphere_cap("cut"), "plane through the sphere, either side, three axes", "V, A, Euler 2, bbox"),
    Family("sphere-cap-common", 17, 8, _sphere_cap("common"), "plane through the sphere, either side, three axes", "V, A, Euler 2, bbox"),
    Family("steinmetz-common", 18, 8, _steinmetz("common"), "equal orthogonal cylinders, half rotated", "V=16r^3/3, A=16r^2, Euler 2, bbox unrotated"),
    Family("steinmetz-fuse", 19, 4, _steinmetz("fuse"), "equal orthogonal cylinders, half rotated", "V, A, Euler 2, bbox unrotated"),
    Family("steinmetz-cut", 20, 4, _steinmetz("cut"), "equal orthogonal cylinders, half rotated", "V, A, bbox unrotated; two pieces at pinch points"),
    Family("coaxial-equal-fuse", 21, 4, _coaxial_equal("fuse"), "one radius, overlapping along one axis", "V, A, Euler 2, bbox"),
    Family("coaxial-equal-common", 22, 4, _coaxial_equal("common"), "one radius, overlapping along one axis", "V, A, Euler 2, bbox"),
    Family("coaxial-equal-cut", 23, 4, _coaxial_equal("cut"), "one radius, overlapping along one axis", "V, A, Euler 2, bbox"),
    Family("coaxial-stepped-fuse", 24, 4, _coaxial_stepped("fuse"), "a thinner cylinder entering the wider one's end", "V, A, Euler 2, bbox"),
    Family("coaxial-stepped-common", 25, 4, _coaxial_stepped("common"), "a thinner cylinder entering the wider one's end", "V, A, Euler 2, bbox"),
    Family("coaxial-stepped-cut", 26, 4, _coaxial_stepped("cut"), "a thinner cylinder entering the wider one's end", "V, A, Euler 2, bbox"),
    Family("coaxial-tube", 27, 4, _sample_tube, "concentric cylinders, inner through the full length", "V, A, Euler 0, bbox"),
    Family("sphere-lens-fuse", 28, 8, _sphere_lens("fuse"), "two spheres overlapping away from tangency", "V, A, Euler 2, bbox"),
    Family("sphere-lens-common", 29, 8, _sphere_lens("common"), "two spheres overlapping away from tangency", "V, A, Euler 2"),
    Family("sphere-lens-cut", 30, 8, _sphere_lens("cut"), "two spheres overlapping away from tangency", "V, A, Euler 2"),
    Family("fillet-box-edge", 31, 8, _sample_fillet_edge, "one edge of a box, r up to 40% of the narrower adjacent face", "V, A, Euler 2, bbox"),
    Family("fillet-box-all", 32, 8, _sample_fillet_all, "all edges and corners, r up to 30% of the shortest side", "V, A, Euler 2, bbox"),
    Family("tess-sphere", 33, 4, _tessellated("sphere"), "four deflection settings", "V, A, Euler 2, watertight; triangle count reported"),
    Family("tess-cylinder", 34, 4, _tessellated("cylinder"), "four deflection settings", "V, A, Euler 2, watertight; triangle count reported"),
    Family("tess-torus", 35, 4, _tessellated("torus"), "four deflection settings", "V, A, Euler 0, watertight; triangle count reported"),
    Family("tess-box", 36, 4, _tessellated("box"), "four deflection settings", "V, A, Euler 2, watertight; triangle count reported"),
    Family("tess-cone", 37, 4, _tessellated("cone"), "four deflection settings", "V, A, Euler 2, watertight; triangle count reported"),
    Family("tess-plate", 38, 4, _tessellated("plate"), "four deflection settings", "V, A, Euler 0, watertight; triangle count reported"),
    _near("near-crossed-external", 39, _near_crossed("external"), "axes r1+r2-e apart, e 1e-9 to 0.5, half rotated"),
    _near("near-crossed-internal", 40, _near_crossed("internal"), "axes r1-r2-e apart, e 1e-9 to 0.5, half rotated"),
    _near("near-crossed-steinmetz", 41, _near_crossed("steinmetz"), "equal radii, axes e apart, e 1e-9 to 0.5, half rotated"),
    _near("near-sphere-box", 42, _near_sphere_box(), "box face e inside the tangent plane, e 1e-9 to 0.5, half rotated; plus a cut"),
]

_FAMILY_NAMES = tuple(family.name for family in FAMILIES)
_ID = re.compile(r"^(?P<family>.+)-\d{4}(?:-[a-z]+)?$")


def _document(case_id, family, seed, body, identity=None):
    document = {
        "schema": CASE_SCHEMA,
        "id": case_id,
        "origin": f"tools/corpus/generate.py family {family} seed {seed}",
        "licence": LICENCE,
        "ops": body.ops,
        "result": body.result,
    }
    if body.expect is not None:
        document["expect"] = body.expect
    if identity is not None:
        document["volume_identity"] = identity
    return document


def generate_all():
    """Every case, as documents in family order. The same seeds give the same documents."""
    cases = []
    for family in FAMILIES:
        for index in range(1, family.count + 1):
            seed = family.base * 1000 + index
            case_id = f"{family.name}-{index:04d}"
            outcome = family.make(index, random.Random(seed))
            if isinstance(outcome, Body):
                cases.append(_document(case_id, family.name, seed, outcome))
                continue
            for role, body in outcome.items():
                identity = {"group": case_id, "role": role} if role in ("a", "b", "union", "intersection") else None
                cases.append(_document(f"{case_id}-{role}", family.name, seed, body, identity))
    return cases


def write_cases(cases, out):
    """Writes one file per case and removes any other .json already there, so the directory is exactly the corpus."""
    out = pathlib.Path(out)
    out.mkdir(parents=True, exist_ok=True)
    wanted = set()
    for case in cases:
        path = out / f"{case['id']}.json"
        path.write_text(json.dumps(case, indent=2) + "\n", encoding="ascii")
        wanted.add(path.name)
    for path in out.glob("*.json"):
        if path.name not in wanted:
            path.unlink()


def family_of(case):
    return _ID.match(case["id"]).group("family")


def family_counts(cases):
    counts = {name: 0 for name in _FAMILY_NAMES}
    for case in cases:
        counts[family_of(case)] += 1
    return {name: count for name, count in counts.items() if count}


def near_tangent_catalogue():
    """Which offsets the near-tangent families use, for the inventory and the tests."""
    entries = []
    for family in FAMILIES:
        if hasattr(family.make, "offset"):
            for index in range(1, family.count + 1):
                seed = family.base * 1000 + index
                entries.append({"family": family.name, "group": f"{family.name}-{index:04d}", "offset": family.make.offset(index, random.Random(seed))})
    return entries


def print_table(cases, stream=sys.stdout):
    counts = family_counts(cases)
    groups = {family.name: family.count for family in FAMILIES if hasattr(family.make, "offset")}
    name_width = max(len(name) for name in counts)
    print(f"{'family':<{name_width}}  {'cases':>5}  answers", file=stream)
    for family in FAMILIES:
        note = f" ({groups[family.name]} groups)" if family.name in groups else ""
        print(f"{family.name:<{name_width}}  {counts[family.name]:>5}  {family.closed_form}{note}", file=stream)
    print(f"{'total':<{name_width}}  {len(cases):>5}", file=stream)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--out", default=str(OUT), help="directory to write the cases to")
    parser.add_argument("--table", action="store_true", help="print the family table and write nothing")
    args = parser.parse_args(argv)
    cases = generate_all()
    if not args.table:
        write_cases(cases, args.out)
    print_table(cases)
    return 0


if __name__ == "__main__":
    sys.exit(main())
