#!/usr/bin/env python3
"""Times a list of cases through the pinned wheel at one thread count, one process for the whole list.

Started by runner.py in a scrubbed environment. Reuses the oracle harness's worker for every engine call, so that this
file adds only the timer and the measurement after it: the case's inputs are built by the oracle's own operations.
One process serves a whole list so that the slow import of the binding is not paid per case, and so that the thread pool
is configured exactly once, before any work. Each case is reported on one line of the results file as soon as it finishes,
and the id in progress is written to the progress file, so a crash loses at most one case.
"""

import argparse
import importlib.util
import json
import os
import pathlib
import sys
import time

HERE = pathlib.Path(__file__).resolve().parent
ORACLE_WORKER = HERE.parents[1] / "oracle" / "occam_oracle" / "occt_worker.py"


def _load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


TIMING = _load("timing", HERE / "timing.py")

BOOLEANS = ("fuse", "cut", "common")


class Counters:
    """CPU seconds against wall seconds for the timed calls, to show the parallel switch really engaged."""

    def __init__(self):
        self.cpu = 0.0
        self.wall = 0.0


class Runner:
    """Builds one repetition of a case through the oracle's operations; `execute` decides what is timed."""

    def __init__(self, oracle, threads, kind, result_id, counters):
        self.oracle = oracle
        self.threads = threads
        self.kind = kind
        self.result_id = result_id
        self.counters = counters
        self.shapes = {}
        self.meshed = None

    def input(self, op):
        self.shapes[op["id"]] = self.oracle.OPERATIONS[op["op"]](op, self.shapes, self.threads)

    def prepare(self, op):
        oracle = self.oracle
        kind = op["op"]
        if kind in BOOLEANS:
            builders = {"fuse": oracle.BRepAlgoAPI_Fuse, "cut": oracle.BRepAlgoAPI_Cut, "common": oracle.BRepAlgoAPI_Common}
            perform = lambda: oracle.boolean(builders[kind], op, self.shapes)
        elif kind == "fillet":
            edges = oracle.select_edges(self.shapes[op["target"]], op["edges"])  # outside the timer
            if not edges:
                raise oracle.OpFailure("the edge selector matched no edge")
            perform = lambda: self._fillet(op, edges)
        elif kind == "tessellate":
            perform = lambda: self._mesh(op)
        else:
            raise ValueError(f"not a timed operation: {kind}")

        def timed():
            cpu, wall = time.process_time(), time.perf_counter()
            outcome = perform()
            self.counters.cpu += time.process_time() - cpu
            self.counters.wall += time.perf_counter() - wall
            return outcome

        return timed

    def _fillet(self, op, edges):
        oracle = self.oracle
        fillet = oracle.BRepFilletAPI_MakeFillet(self.shapes[op["target"]])
        for edge in edges:
            fillet.Add(op["radius"], oracle.TopoDS.Edge(edge))
        fillet.Build()
        if not fillet.IsDone():
            raise oracle.OpFailure("the fillet did not finish")
        return fillet.Shape()

    def _mesh(self, op):
        oracle = self.oracle
        target = self.shapes[op["target"]]
        mesher = oracle.BRepMesh_IncrementalMesh(
            target, op["linear_deflection"], op.get("relative", False), op["angular_deflection"], self.threads > 1
        )
        if not mesher.IsDone():
            raise oracle.OpFailure("the mesher did not finish")
        return target

    def finish(self, op, outcome):
        if op["op"] == "tessellate":
            self.meshed = outcome
        else:
            self.shapes[op["id"]] = outcome

    def measure(self):
        oracle = self.oracle
        if self.kind == "triangles":
            total = 0
            for face in oracle.distinct(self.meshed, oracle.FACE):
                triangulation = oracle.BRep_Tool.Triangulation_s(oracle.TopoDS.Face(face), oracle.TopLoc_Location())
                if triangulation is None:
                    raise TIMING.RefusedResult("a face has no triangulation")
                total += triangulation.NbTriangles()
            return total
        properties = oracle.GProp_GProps()
        oracle.BRepGProp.VolumeProperties_s(self.shapes[self.result_id], properties)
        return properties.Mass()


def expect_status(case, kind, measured):
    """True or False against a closed-form volume, None when the case has none. Never reported as a number."""
    spec = case.get("expect", {}).get("volume")
    if kind != "volume" or spec is None or not measured:
        return None
    value = spec["value"]
    tolerance = max(spec.get("rel_tol", 0.0) * abs(value), spec.get("abs_tol", 0.0))
    return abs(measured[0] - value) <= tolerance


def time_case(oracle, entry, threads, budget_ms, least, most, pool):
    case = entry["case"]
    chosen = entry["chosen"]
    timed_kinds = frozenset(chosen["timed"])
    kind = chosen["kind"]
    counters = Counters()

    def make_runner():
        return Runner(oracle, threads, kind, case["result"], counters)

    base = {
        "id": case["id"],
        "family": chosen["family"],
        "group": chosen["group"],
        "kind": kind,
        "threads": threads,
        "pool_threads": pool,
    }
    try:
        record = TIMING.measure_case(case["ops"], timed_kinds, make_runner, kind, budget_ms, least, most)
    except TIMING.RefusedResult as error:
        return {**base, "status": "refused", "message": str(error), "ms": [], "measured": []}
    except Exception as error:  # pylint: disable=broad-except
        return {**base, "status": "error", "message": oracle.message_of(error), "ms": [], "measured": []}
    return {
        **base,
        "status": "ok",
        "ms": record["ms"],
        "measured": record["measured"],
        "warmups": record["warmups"],
        "repetitions": record["repetitions"],
        "cpu_wall_ratio": counters.cpu / counters.wall if counters.wall > 0 else None,
        "expect_ok": expect_status(case, kind, record["measured"]),
    }


def main():
    parser = argparse.ArgumentParser(description="Time cases through the OCCT engine.")
    parser.add_argument("--cases", required=True, type=pathlib.Path)
    parser.add_argument("--results", required=True, type=pathlib.Path)
    parser.add_argument("--progress", required=True, type=pathlib.Path)
    parser.add_argument("--threads", required=True, type=int)
    parser.add_argument("--cores", required=True, help="logical CPUs to pin to, comma separated")
    parser.add_argument("--budget-ms", type=float, default=100.0)
    parser.add_argument("--least", type=int, default=5)
    parser.add_argument("--most", type=int, default=30)
    args = parser.parse_args()

    os.sched_setaffinity(0, {int(cpu) for cpu in args.cores.split(",")})
    oracle = _load("occt_worker", ORACLE_WORKER)
    oracle.fix_parallelism(args.threads)
    pool = oracle.OSD_ThreadPool.DefaultPool_s(args.threads if args.threads > 1 else 1).NbThreads()

    entries = json.loads(args.cases.read_text(encoding="utf-8"))
    with args.results.open("a", encoding="utf-8") as results:
        for entry in entries:
            args.progress.write_text(f"{entry['case']['id']}\n{time.time()}\n", encoding="utf-8")
            record = time_case(oracle, entry, args.threads, args.budget_ms, args.least, args.most, pool)
            results.write(json.dumps(record, allow_nan=False) + "\n")
            results.flush()
    args.progress.write_text("", encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main())
