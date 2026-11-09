#!/usr/bin/env python3
"""The worker for the OCCT engine: runs one case through the pinned wheel and writes one result.

This is the only file that imports the binding. Standard library and that binding only, and only its public names.
Open CASCADE here is a black box: this file says what to build and measures what comes out.
"""

import argparse
import collections
import importlib.metadata
import importlib.util
import json
import math
import pathlib
import re
import sys

import OCP
from OCP.BOPAlgo import BOPAlgo_Options
from OCP.Bnd import Bnd_Box
from OCP.BRep import BRep_Builder, BRep_Tool
from OCP.BRepAlgoAPI import BRepAlgoAPI_Common, BRepAlgoAPI_Cut, BRepAlgoAPI_Fuse
from OCP.BRepBndLib import BRepBndLib
from OCP.BRepBuilderAPI import BRepBuilderAPI_Copy, BRepBuilderAPI_Transform
from OCP.BRepCheck import BRepCheck_Analyzer
from OCP.BRepFilletAPI import BRepFilletAPI_MakeFillet
from OCP.BRepGProp import BRepGProp
from OCP.BRepMesh import BRepMesh_IncrementalMesh
from OCP.BRepPrimAPI import (
    BRepPrimAPI_MakeBox,
    BRepPrimAPI_MakeCone,
    BRepPrimAPI_MakeCylinder,
    BRepPrimAPI_MakeSphere,
    BRepPrimAPI_MakeTorus,
)
from OCP.GProp import GProp_GProps
from OCP.OSD import OSD_ThreadPool
from OCP.TopAbs import TopAbs_Orientation, TopAbs_ShapeEnum
from OCP.TopExp import TopExp_Explorer
from OCP.TopLoc import TopLoc_Location
from OCP.TopoDS import TopoDS, TopoDS_Shell, TopoDS_Solid
from OCP.gp import gp_Ax1, gp_Ax2, gp_Dir, gp_Pnt, gp_Trsf, gp_Vec

# The worker runs with -P, so its siblings are loaded by path rather than through sys.path.
_SPEC = importlib.util.spec_from_file_location("meshcheck", pathlib.Path(__file__).with_name("meshcheck.py"))
MESHCHECK = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(MESHCHECK)

ENGINE = "opencascade"
PACKAGE = "cadquery-ocp"
RESULT_SCHEMA = "occam-oracle-result/1"
_ADDRESS = re.compile(r"0x[0-9a-fA-F]+")

VERTEX = TopAbs_ShapeEnum.TopAbs_VERTEX
EDGE = TopAbs_ShapeEnum.TopAbs_EDGE
WIRE = TopAbs_ShapeEnum.TopAbs_WIRE
FACE = TopAbs_ShapeEnum.TopAbs_FACE
SHELL = TopAbs_ShapeEnum.TopAbs_SHELL
SOLID = TopAbs_ShapeEnum.TopAbs_SOLID


class OpFailure(Exception):
    """An operation that finished without producing what it was asked to."""


def oracle_identity():
    # The OCP binding is versioned as <OCCT version>.<binding build>, so the OCCT version is its first three fields.
    return {
        "engine": ENGINE,
        "engine_version": ".".join(OCP.__version__.split(".")[:3]),
        "package": PACKAGE,
        "package_version": importlib.metadata.version(PACKAGE),
    }


def fix_parallelism(threads):
    """Parallel switches off at one thread; otherwise the default pool is fixed to that size."""
    parallel = threads > 1
    BOPAlgo_Options.SetParallelMode_s(parallel)
    BRepMesh_IncrementalMesh.SetParallelDefault_s(parallel)
    if parallel:
        OSD_ThreadPool.DefaultPool_s(threads)


def message_of(error):
    """Exception text without anything that changes between runs."""
    return _ADDRESS.sub("0x?", f"{type(error).__name__}: {error}")


def point(values):
    return gp_Pnt(values[0], values[1], values[2])


def direction(values):
    return gp_Dir(values[0], values[1], values[2])


def frame(op):
    return gp_Ax2(point(op["origin"]), direction(op["axis"]))


def explore(shape, kind):
    explorer = TopExp_Explorer(shape, kind)
    while explorer.More():
        yield explorer.Current()
        explorer.Next()


class Distinct:
    """Sub-shapes without repeats, identified as the engine identifies them: same underlying shape, same location."""

    def __init__(self):
        self._buckets = {}
        self.items = []

    def index(self, shape):
        bucket = self._buckets.setdefault(hash(shape), [])
        for index, other in bucket:
            if shape.IsSame(other):
                return index
        index = len(self.items)
        bucket.append((index, shape))
        self.items.append(shape)
        return index


def distinct(shape, kind):
    found = Distinct()
    for item in explore(shape, kind):
        found.index(item)
    return found.items


def tight_box(shape):
    """The optimal bounding box, with no tolerance added; None for an empty shape."""
    box = Bnd_Box()
    BRepBndLib.AddOptimal_s(shape, box, False, False)
    if box.IsVoid():
        return None
    low, high = box.CornerMin(), box.CornerMax()
    return (low.X(), low.Y(), low.Z()), (high.X(), high.Y(), high.Z())


def inside(bounds, region):
    low, high = bounds
    return all(region["min"][axis] <= low[axis] and high[axis] <= region["max"][axis] for axis in range(3))


class EdgeUses:
    """How often each edge of a shape appears on its faces, overall and within a single face.

    A seam, where a face closes on itself, appears twice on the one face; a shared edge appears once on each of its faces.
    """

    def __init__(self, shape):
        self.edges = Distinct()
        self.count = collections.Counter()
        self.seam = set()
        for face in explore(shape, FACE):
            within_face = collections.Counter()
            for occurrence in explore(face, EDGE):
                index = self.edges.index(occurrence)
                self.count[index] += 1
                within_face[index] += 1
            self.seam.update(index for index, uses in within_face.items() if uses >= 2)


def degenerate(edge):
    return BRep_Tool.Degenerated_s(TopoDS.Edge(edge))


# Operations. Each returns the shape it makes; tessellate reports statistics instead.
def op_box(op, _shapes, _threads):
    return BRepPrimAPI_MakeBox(point(op["origin"]), *op["size"]).Shape()


def op_cylinder(op, _shapes, _threads):
    return BRepPrimAPI_MakeCylinder(frame(op), op["radius"], op["height"]).Shape()


def op_sphere(op, _shapes, _threads):
    return BRepPrimAPI_MakeSphere(point(op["origin"]), op["radius"]).Shape()


def op_cone(op, _shapes, _threads):
    return BRepPrimAPI_MakeCone(frame(op), op["radius1"], op["radius2"], op["height"]).Shape()


def op_torus(op, _shapes, _threads):
    return BRepPrimAPI_MakeTorus(frame(op), op["major_radius"], op["minor_radius"]).Shape()


def transformed(shape, transform):
    built = BRepBuilderAPI_Transform(shape, transform, True)
    if not built.IsDone():
        raise OpFailure("the transform did not finish")
    return built.Shape()


def op_translate(op, shapes, _threads):
    transform = gp_Trsf()
    transform.SetTranslation(gp_Vec(*op["vector"]))
    return transformed(shapes[op["target"]], transform)


def op_rotate(op, shapes, _threads):
    transform = gp_Trsf()
    transform.SetRotation(gp_Ax1(point(op["axis_origin"]), direction(op["axis"])), math.radians(op["angle_degrees"]))
    return transformed(shapes[op["target"]], transform)


def boolean(builder, op, shapes):
    algorithm = builder(shapes[op["target"]], shapes[op["tool"]])
    if not algorithm.IsDone():
        raise OpFailure("the boolean did not finish")
    shape = algorithm.Shape()
    if shape.IsNull():
        raise OpFailure("the boolean returned nothing")
    return shape


def op_fuse(op, shapes, _threads):
    return boolean(BRepAlgoAPI_Fuse, op, shapes)


def op_cut(op, shapes, _threads):
    return boolean(BRepAlgoAPI_Cut, op, shapes)


def op_common(op, shapes, _threads):
    return boolean(BRepAlgoAPI_Common, op, shapes)


def select_edges(shape, selector):
    """The edges a fillet applies to, in geometric order, so the choice owes nothing to the kernel's internal order.

    Degenerate edges and seams are never candidates. "all" takes every other edge; `in_box` takes those whose tight
    bounding box lies inside the region (inclusive).
    """
    uses = EdgeUses(shape)
    chosen = []
    for index, edge in enumerate(uses.edges.items):
        if degenerate(edge) or index in uses.seam:
            continue
        bounds = tight_box(edge)
        if selector == "all" or inside(bounds, selector["in_box"]):
            chosen.append((bounds[0] + bounds[1], edge))
    chosen.sort(key=lambda item: item[0])
    return [edge for _bounds, edge in chosen]


def op_fillet(op, shapes, _threads):
    edges = select_edges(shapes[op["target"]], op["edges"])
    if not edges:
        raise OpFailure("the edge selector matched no edge")
    fillet = BRepFilletAPI_MakeFillet(shapes[op["target"]])
    for edge in edges:
        fillet.Add(op["radius"], TopoDS.Edge(edge))
    fillet.Build()
    if not fillet.IsDone():
        raise OpFailure("the fillet did not finish")
    return fillet.Shape()


def op_remove_face(op, shapes, _threads):
    """Fault injection: rebuilds a solid without the one face inside `in_box`, leaving a deliberately open shell."""
    builder = BRep_Builder()
    shell, solid = TopoDS_Shell(), TopoDS_Solid()
    builder.MakeShell(shell)
    removed = 0
    for face in distinct(shapes[op["target"]], FACE):
        if inside(tight_box(face), op["in_box"]):
            removed += 1
        else:
            builder.Add(shell, face)
    if removed != 1:
        raise OpFailure(f"in_box selected {removed} faces, expected exactly one")
    builder.MakeSolid(solid)
    builder.Add(solid, shell)
    return solid


def weld_tolerance(shape, bounds):
    """Coincident within the larger of the shape's own edge and vertex tolerances and 1e-9 of its size."""
    largest = 0.0
    for vertex in distinct(shape, VERTEX):
        largest = max(largest, BRep_Tool.Tolerance_s(TopoDS.Vertex(vertex)))
    for edge in distinct(shape, EDGE):
        largest = max(largest, BRep_Tool.Tolerance_s(TopoDS.Edge(edge)))
    diagonal = math.dist(bounds[0], bounds[1])
    return max(largest, 1e-9 * diagonal)


def mesh_triangles(shape):
    """All triangles of a meshed shape in space, consistently wound, as point indices into one list."""
    points, triangles = [], []
    for face in distinct(shape, FACE):
        location = TopLoc_Location()
        triangulation = BRep_Tool.Triangulation_s(TopoDS.Face(face), location)
        if triangulation is None:
            raise OpFailure("a face has no triangulation")
        transform = location.Transformation()
        base = len(points)
        for index in range(1, triangulation.NbNodes() + 1):
            node = triangulation.Node(index).Transformed(transform)
            points.append((node.X(), node.Y(), node.Z()))
        flip = face.Orientation() == TopAbs_Orientation.TopAbs_REVERSED
        for index in range(1, triangulation.NbTriangles() + 1):
            triangle = triangulation.Triangle(index)
            a, b, c = triangle.Value(1), triangle.Value(2), triangle.Value(3)
            triangles.append((base + a - 1, base + c - 1, base + b - 1) if flip else (base + a - 1, base + b - 1, base + c - 1))
    return points, triangles


def op_tessellate(op, shapes, threads):
    """Meshes a copy, so the case's own shapes never carry a triangulation into a later op or a measurement."""
    copy = BRepBuilderAPI_Copy(shapes[op["target"]], True, False).Shape()
    relative = op.get("relative", False)
    mesher = BRepMesh_IncrementalMesh(copy, op["linear_deflection"], relative, op["angular_deflection"], threads > 1)
    if not mesher.IsDone():
        raise OpFailure("the mesher did not finish")
    bounds = tight_box(copy)
    if bounds is None:
        raise OpFailure("there is nothing to tessellate")
    points, triangles = mesh_triangles(copy)
    stats = MESHCHECK.summarize(points, triangles, weld_tolerance(copy, bounds))
    return {
        "target": op["target"],
        "linear_deflection": op["linear_deflection"],
        "angular_deflection": op["angular_deflection"],
        "relative": relative,
        **stats,
    }


OPERATIONS = {
    "box": op_box,
    "cylinder": op_cylinder,
    "sphere": op_sphere,
    "cone": op_cone,
    "torus": op_torus,
    "translate": op_translate,
    "rotate": op_rotate,
    "fuse": op_fuse,
    "cut": op_cut,
    "common": op_common,
    "fillet": op_fillet,
    "remove_face": op_remove_face,
    "tessellate": op_tessellate,
}


def finite(value, what):
    if not math.isfinite(value):
        raise OpFailure(f"{what} is not finite")
    return value


def measure(shape):
    volume, area = GProp_GProps(), GProp_GProps()
    BRepGProp.VolumeProperties_s(shape, volume)
    BRepGProp.SurfaceProperties_s(shape, area)
    bounds = tight_box(shape)
    uses = EdgeUses(shape)
    counted = [index for index, edge in enumerate(uses.edges.items) if not degenerate(edge)]
    vertices = len(distinct(shape, VERTEX))
    all_faces = distinct(shape, FACE)
    faces = len(all_faces)
    wires = sum(len(distinct(face, WIRE)) for face in all_faces)
    return {
        "volume": finite(volume.Mass(), "the volume"),
        "area": finite(area.Mass(), "the area"),
        "bbox": None if bounds is None else {"min": [finite(v, "the box") for v in bounds[0]], "max": [finite(v, "the box") for v in bounds[1]]},
        "euler": vertices - len(counted) + 2 * faces - wires,
        "counts": {
            "vertices": vertices,
            "edges": len(counted),
            "degenerate_edges": len(uses.edges.items) - len(counted),
            "wires": wires,
            "faces": faces,
            "shells": len(distinct(shape, SHELL)),
            "solids": len(distinct(shape, SOLID)),
        },
        "valid": bool(BRepCheck_Analyzer(shape).IsValid()),
        "closed": all(uses.count[index] % 2 == 0 for index in counted),
    }


def run(case, threads, progress):
    identity = oracle_identity()

    def failure(code, op_id, kind, message):
        return {
            "schema": RESULT_SCHEMA,
            "id": case["id"],
            "oracle": identity,
            "threads": threads,
            "status": "error",
            "error": {"code": code, "op": op_id, "kind": kind, "message": message},
        }

    fix_parallelism(threads)
    shapes = {}
    tessellations = {}
    for op in case["ops"]:
        progress.write_text(op["id"], encoding="utf-8")
        try:
            outcome = OPERATIONS[op["op"]](op, shapes, threads)
        except Exception as error:  # pylint: disable=broad-except
            return failure("op_failed", op["id"], op["op"], message_of(error))
        if op["op"] == "tessellate":
            tessellations[op["id"]] = outcome
        else:
            shapes[op["id"]] = outcome
    result_id = case["result"]
    kind = next(op["op"] for op in case["ops"] if op["id"] == result_id)
    progress.write_text(result_id, encoding="utf-8")
    try:
        shape = measure(shapes[result_id])
    except Exception as error:  # pylint: disable=broad-except
        return failure("measure_failed", result_id, kind, message_of(error))
    return {
        "schema": RESULT_SCHEMA,
        "id": case["id"],
        "oracle": identity,
        "threads": threads,
        "status": "ok",
        "shape": shape,
        "tessellations": tessellations,
    }


def main():
    parser = argparse.ArgumentParser(description="One case through the OCCT engine.")
    parser.add_argument("--case", required=True, type=pathlib.Path)
    parser.add_argument("--result", required=True, type=pathlib.Path)
    parser.add_argument("--progress", required=True, type=pathlib.Path)
    parser.add_argument("--threads", required=True, type=int)
    args = parser.parse_args()
    case = json.loads(args.case.read_text(encoding="utf-8"))
    result = run(case, args.threads, args.progress)
    args.result.write_text(json.dumps(result, allow_nan=False), encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main())
