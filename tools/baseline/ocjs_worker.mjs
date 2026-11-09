#!/usr/bin/env node
// Times a list of cases through the WebAssembly build of OCCT under Node's V8, one process for the whole list. Started by
// runner.py. Only the build's public class and method names are used, and the build itself stays outside the repository:
// its directory is an argument. These numbers are WebAssembly under Node, not a browser.
//
// The same rules as timed_worker.py apply: inputs rebuilt outside the timer for each repetition, the timer around the
// whole operation, the measured result checked after the timer and an empty one refused, one record per case written as
// soon as the case finishes. A warm-up pass over every case is discarded first, because V8 compiles WebAssembly in tiers.

import fs from "node:fs";
import path from "node:path";
import { createRequire } from "node:module";

class Refused extends Error {}
class OpFailure extends Error {}

function parseArgs(argv) {
  const args = {};
  for (let index = 0; index < argv.length; index += 2) {
    args[argv[index].replace(/^--/, "")] = argv[index + 1];
  }
  return args;
}

async function loadOpenCascade(directory) {
  const dist = path.join(directory, "node_modules", "opencascade.js", "dist");
  const file = path.join(dist, "opencascade.wasm.js");
  // The build ships as an ES module that also expects Node's CommonJS names, so it is evaluated with them supplied.
  const source = fs.readFileSync(file, "utf8").replace(/export default opencascade;\s*$/, "");
  const factory = new Function("require", "__dirname", "__filename", `${source}\nreturn opencascade;`)(createRequire(file), dist, file);
  return factory({ wasmBinary: fs.readFileSync(path.join(dist, "opencascade.wasm.wasm")), print: () => {}, printErr: () => {} });
}

function makeRunner(oc) {
  const created = [];
  const track = (object) => {
    created.push(object);
    return object;
  };
  const shapes = {};
  const point = (v) => track(new oc.gp_Pnt_3(v[0], v[1], v[2]));
  const direction = (v) => track(new oc.gp_Dir_4(v[0], v[1], v[2]));
  const frame = (op) => track(new oc.gp_Ax2_3(point(op.origin), direction(op.axis)));

  const facesOf = (shape) => {
    const found = [];
    const explorer = track(new oc.TopExp_Explorer_2(shape, oc.TopAbs_ShapeEnum.TopAbs_FACE, oc.TopAbs_ShapeEnum.TopAbs_SHAPE));
    for (; explorer.More(); explorer.Next()) {
      const face = track(oc.TopoDS.Face_1(explorer.Current()));
      if (!found.some((other) => other.IsSame(face))) {
        found.push(face);
      }
    }
    return found;
  };

  const tightBox = (shape) => {
    const box = track(new oc.Bnd_Box_1());
    oc.BRepBndLib.AddOptimal(shape, box, false, false);
    if (box.IsVoid()) {
      return null;
    }
    const low = track(box.CornerMin());
    const high = track(box.CornerMax());
    return [
      [low.X(), low.Y(), low.Z()],
      [high.X(), high.Y(), high.Z()],
    ];
  };

  const selectEdges = (shape, selector) => {
    const edges = [];
    for (const face of facesOf(shape)) {
      const withinFace = [];
      const explorer = track(new oc.TopExp_Explorer_2(face, oc.TopAbs_ShapeEnum.TopAbs_EDGE, oc.TopAbs_ShapeEnum.TopAbs_SHAPE));
      for (; explorer.More(); explorer.Next()) {
        const edge = track(oc.TopoDS.Edge_1(explorer.Current()));
        let entry = edges.find((other) => other.edge.IsSame(edge));
        if (!entry) {
          entry = { edge, seam: false };
          edges.push(entry);
        }
        if (withinFace.includes(entry)) {
          entry.seam = true;
        }
        withinFace.push(entry);
      }
    }
    const chosen = [];
    for (const { edge, seam } of edges) {
      if (seam || oc.BRep_Tool.Degenerated(edge)) {
        continue;
      }
      const bounds = tightBox(edge);
      if (bounds === null) {
        continue;
      }
      const inside = (region) => [0, 1, 2].every((axis) => region.min[axis] <= bounds[0][axis] && bounds[1][axis] <= region.max[axis]);
      if (selector === "all" || inside(selector.in_box)) {
        chosen.push({ edge, order: [...bounds[0], ...bounds[1]] });
      }
    }
    chosen.sort((a, b) => {
      for (let i = 0; i < 6; i += 1) {
        if (a.order[i] !== b.order[i]) {
          return a.order[i] - b.order[i];
        }
      }
      return 0;
    });
    return chosen.map((item) => item.edge);
  };

  const transformed = (shape, transform) => {
    const built = track(new oc.BRepBuilderAPI_Transform_2(shape, transform, true));
    if (!built.IsDone()) {
      throw new OpFailure("the transform did not finish");
    }
    return track(built.Shape());
  };

  const inputs = {
    box: (op) => track(new oc.BRepPrimAPI_MakeBox_2(point(op.origin), op.size[0], op.size[1], op.size[2])).Shape(),
    cylinder: (op) => track(new oc.BRepPrimAPI_MakeCylinder_3(frame(op), op.radius, op.height)).Shape(),
    sphere: (op) => track(new oc.BRepPrimAPI_MakeSphere_5(point(op.origin), op.radius)).Shape(),
    cone: (op) => track(new oc.BRepPrimAPI_MakeCone_3(frame(op), op.radius1, op.radius2, op.height)).Shape(),
    torus: (op) => track(new oc.BRepPrimAPI_MakeTorus_5(frame(op), op.major_radius, op.minor_radius)).Shape(),
    translate: (op) => {
      const transform = track(new oc.gp_Trsf_1());
      transform.SetTranslation_1(track(new oc.gp_Vec_4(op.vector[0], op.vector[1], op.vector[2])));
      return transformed(shapes[op.target], transform);
    },
    rotate: (op) => {
      const transform = track(new oc.gp_Trsf_1());
      transform.SetRotation_1(track(new oc.gp_Ax1_2(point(op.axis_origin), direction(op.axis))), (op.angle_degrees * Math.PI) / 180);
      return transformed(shapes[op.target], transform);
    },
  };
  const booleanBuilders = { fuse: "BRepAlgoAPI_Fuse_3", cut: "BRepAlgoAPI_Cut_3", common: "BRepAlgoAPI_Common_3" };

  const boolean = (op) => {
    const algorithm = track(new oc[booleanBuilders[op.op]](shapes[op.target], shapes[op.tool]));
    if (!algorithm.IsDone()) {
      throw new OpFailure("the boolean did not finish");
    }
    const shape = track(algorithm.Shape());
    if (shape.IsNull()) {
      throw new OpFailure("the boolean returned nothing");
    }
    return shape;
  };

  return {
    created,
    shapes,
    input(op) {
      if (op.op in booleanBuilders) {
        shapes[op.id] = boolean(op);
      } else if (op.op in inputs) {
        shapes[op.id] = track(inputs[op.op](op));
      } else {
        throw new Error(`no input builder for ${op.op}`);
      }
    },
    prepare(op) {
      if (op.op in booleanBuilders) {
        return () => boolean(op);
      }
      if (op.op === "fillet") {
        const edges = selectEdges(shapes[op.target], op.edges);
        if (edges.length === 0) {
          throw new OpFailure("the edge selector matched no edge");
        }
        return () => {
          const fillet = track(new oc.BRepFilletAPI_MakeFillet(shapes[op.target], oc.ChFi3d_FilletShape.ChFi3d_Rational));
          for (const edge of edges) {
            fillet.Add_2(op.radius, edge);
          }
          fillet.Build();
          if (!fillet.IsDone()) {
            throw new OpFailure("the fillet did not finish");
          }
          return track(fillet.Shape());
        };
      }
      if (op.op === "tessellate") {
        return () => {
          const mesher = track(
            new oc.BRepMesh_IncrementalMesh_2(shapes[op.target], op.linear_deflection, Boolean(op.relative), op.angular_deflection, false),
          );
          if (!mesher.IsDone()) {
            throw new OpFailure("the mesher did not finish");
          }
          return shapes[op.target];
        };
      }
      throw new Error(`not a timed operation: ${op.op}`);
    },
    finish(op, outcome) {
      if (op.op === "tessellate") {
        this.meshed = outcome;
      } else {
        shapes[op.id] = outcome;
      }
    },
    measure(kind, resultId) {
      if (kind === "triangles") {
        let total = 0;
        for (const face of facesOf(this.meshed)) {
          const location = track(new oc.TopLoc_Location_1());
          const triangulation = track(oc.BRep_Tool.Triangulation(face, location));
          if (triangulation.IsNull()) {
            throw new Refused("a face has no triangulation");
          }
          total += triangulation.get().NbTriangles();
        }
        return total;
      }
      const properties = track(new oc.GProp_GProps_1());
      oc.BRepGProp.VolumeProperties_1(shapes[resultId], properties, false, false, false);
      return properties.Mass();
    },
    release() {
      for (const object of created) {
        try {
          object.delete();
        } catch {
          // Already released through a shared wrapper; nothing more to do.
        }
      }
    },
  };
}

function checkMeasured(kind, value) {
  const good = kind === "triangles" ? Number.isInteger(value) && value > 0 : typeof value === "number" && Number.isFinite(value) && value > 0;
  if (!good) {
    throw new Refused(`the measured ${kind} is ${value}, so the operation computed nothing measurable`);
  }
}

function runOnce(oc, ops, timedKinds, kind, resultId) {
  const runner = makeRunner(oc);
  try {
    let elapsed = 0;
    for (const op of ops) {
      if (!timedKinds.has(op.op)) {
        runner.input(op);
        continue;
      }
      const operation = runner.prepare(op);
      const start = performance.now();
      const outcome = operation();
      elapsed += performance.now() - start;
      runner.finish(op, outcome);
    }
    const value = runner.measure(kind, resultId);
    checkMeasured(kind, value);
    return { ms: elapsed, value };
  } finally {
    runner.release();
  }
}

function repetitionsFor(firstMs, budgetMs, least, most) {
  if (firstMs <= 0) {
    return most;
  }
  return Math.max(least, Math.min(most, Math.ceil(budgetMs / firstMs)));
}

function expectStatus(testCase, kind, measured) {
  const spec = testCase.expect?.volume;
  if (kind !== "volume" || !spec || measured.length === 0) {
    return null;
  }
  const tolerance = Math.max((spec.rel_tol ?? 0) * Math.abs(spec.value), spec.abs_tol ?? 0);
  return Math.abs(measured[0] - spec.value) <= tolerance;
}

function timeCase(oc, entry, budgetMs, least, most) {
  const testCase = entry.case;
  const chosen = entry.chosen;
  const timedKinds = new Set(chosen.timed);
  const base = { id: testCase.id, family: chosen.family, group: chosen.group, kind: chosen.kind, threads: 1 };
  try {
    runOnce(oc, testCase.ops, timedKinds, chosen.kind, testCase.result); // warm-up, discarded
    const first = runOnce(oc, testCase.ops, timedKinds, chosen.kind, testCase.result);
    const repetitions = repetitionsFor(first.ms, budgetMs, least, most);
    const ms = [first.ms];
    const measured = [first.value];
    for (let index = 1; index < repetitions; index += 1) {
      const next = runOnce(oc, testCase.ops, timedKinds, chosen.kind, testCase.result);
      ms.push(next.ms);
      measured.push(next.value);
    }
    return { ...base, status: "ok", ms, measured, warmups: 1, repetitions, expect_ok: expectStatus(testCase, chosen.kind, measured) };
  } catch (error) {
    const status = error instanceof Refused ? "refused" : "error";
    return { ...base, status, message: String(error?.message ?? error).slice(0, 300), ms: [], measured: [] };
  }
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const budgetMs = Number(args["budget-ms"] ?? 100);
  const least = Number(args.least ?? 5);
  const most = Number(args.most ?? 30);
  const entries = JSON.parse(fs.readFileSync(args.cases, "utf8"));
  const oc = await loadOpenCascade(args["ocjs-dir"]);

  // One warm-up pass over every case, discarded, so V8's tiers settle before anything is timed. A case that aborts the
  // module here is recorded and the worker exits so that a fresh process carries on with the next one.
  for (const entry of entries) {
    fs.writeFileSync(args.progress, `${entry.case.id}\n${Date.now() / 1000}\n`);
    try {
      runOnce(oc, entry.case.ops, new Set(entry.chosen.timed), entry.chosen.kind, entry.case.result);
    } catch (error) {
      if (String(error?.message ?? error).startsWith("abort")) {
        fs.appendFileSync(
          args.results,
          `${JSON.stringify({ id: entry.case.id, family: entry.chosen.family, group: entry.chosen.group, kind: entry.chosen.kind, threads: 1, status: "error", message: String(error?.message ?? error).slice(0, 300), ms: [], measured: [] })}\n`,
        );
        process.exit(3);
      }
      // Any other failure of this case is reported by the timed pass below.
    }
  }
  for (const entry of entries) {
    fs.writeFileSync(args.progress, `${entry.case.id}\n${Date.now() / 1000}\n`);
    const record = timeCase(oc, entry, budgetMs, least, most);
    fs.appendFileSync(args.results, `${JSON.stringify(record)}\n`);
    if (record.status === "error" && record.message.startsWith("abort")) {
      process.exit(3);
    }
  }
  fs.writeFileSync(args.progress, "");
  process.exit(0);
}

main().catch((error) => {
  console.error(String(error?.message ?? error).slice(0, 500));
  process.exit(1);
});
