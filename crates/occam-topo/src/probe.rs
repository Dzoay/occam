//! Source: original
//! What `gates determinism` hashes of the topology (docs/DETERMINISM.md): the digests of models built through
//! [`Edit`](crate::Edit), and the reports [`Snapshot::check_all`] gives on models corrupted as no edit could, each report
//! hashed as its JSON. Corrupting a model takes this crate's own tables, so the probe is built only into its tests:
//! `gates determinism` runs one of them as every build and reads the hashes it prints, and `just determinism-topology`
//! another, which prints the record.
//!
//! The record is `ci/determinism-topology.txt`. There is no second topology to take it from, as the maths record is
//! taken from `tools/determinism/stream.py`, so it is what the x86-64 build prints on one thread: every other build and
//! thread count is held to that one, and the topology to its answers, but an answer every build gets wrong alike would
//! pass.

use occam_math::{Digest, Exec};
use occam_report::{DisplayUnits, Report};
use occam_units::{Angle, Length, Tolerance};

use crate::kind::sealed::Sealed;
use crate::kind::{Edge, EdgeUse, Id, Kind, Vertex};
use crate::models::{Built, Spec, build, cube, cylinder, frame, sheet, wire};
use crate::snapshot::Snapshot;

/// How many operations the probe hashes.
const OPERATION_COUNT: usize = 8;

/// The operations, in the order [`hashes`] gives them and the record lists them.
const OPERATIONS: [&str; OPERATION_COUNT] = [
    "topo-cube",
    "topo-frame",
    "topo-cylinder",
    "topo-sheet",
    "topo-wire",
    "topo-steps",
    "topo-check-few",
    "topo-check-many",
];

/// The cubes of `topo-check-many`: enough that its edge uses fill more than one chunk of their table.
const CUBES: usize = 50;

/// The tolerance the probe's models are built to: the default for a new model (O12).
fn tolerance() -> Option<Tolerance> {
    Tolerance::new(Length::from_millimetres(1e-7), Angle::from_radians(1e-9)).ok()
}

/// Writes how many bytes, then the bytes.
fn text(digest: &mut Digest, bytes: &[u8]) {
    digest.write_u64(u64::try_from(bytes.len()).unwrap_or(u64::MAX));
    digest.write_bytes(bytes);
}

/// Writes how many reports, then each as its JSON, or its code if it cannot be written.
fn reports(digest: &mut Digest, reports: &[Report]) {
    digest.write_u64(u64::try_from(reports.len()).unwrap_or(u64::MAX));
    for report in reports {
        match report.to_json(DisplayUnits::default()) {
            Ok(json) => text(digest, json.as_bytes()),
            Err(failed) => text(digest, failed.code.text().as_bytes()),
        }
    }
}

/// The snapshot `steps` build, one edit a step, with the ids each spec was given; or the reports of the commit that
/// failed.
fn built(steps: &[&[Spec]], exec: &Exec) -> Result<(Snapshot, Vec<Built>), Vec<Report>> {
    let mut snapshot = Snapshot::empty(tolerance().ok_or_else(Vec::new)?);
    let mut ids = Vec::new();
    for specs in steps {
        let mut edit = snapshot.edit();
        ids.extend(specs.iter().map(|spec| build(&mut edit, spec)));
        snapshot = edit.commit(exec)?.0;
    }
    Ok((snapshot, ids))
}

/// The hash of what `steps` build, corrupted by `corrupt`: the snapshot's digest and every report `check_all` gives,
/// or, if a commit failed, its reports.
fn hash(steps: &[&[Spec]], corrupt: fn(&mut Snapshot, &[Built]), exec: &Exec) -> u64 {
    let mut digest = Digest::EMPTY;
    match built(steps, exec) {
        Ok((mut snapshot, ids)) => {
            corrupt(&mut snapshot, &ids);
            digest.write_u64(snapshot.digest());
            reports(&mut digest, &snapshot.check_all(exec));
        }
        Err(failed) => {
            text(&mut digest, b"refused");
            reports(&mut digest, &failed);
        }
    }
    digest.finish()
}

/// Frees `id` straight from its table, as no edit could, leaving whatever links to it.
fn free<K: Kind>(snapshot: &mut Snapshot, id: Option<&Id<K>>) {
    if let Some(id) = id {
        K::table_mut(&mut snapshot.model).free(id.slot(), id.generation());
    }
}

/// Leaves a model as it was built.
const fn clean(_: &mut Snapshot, _: &[Built]) {}

/// Frees a vertex of the cube, a face of the frame and the curve of one of the cylinder's edges.
fn few(snapshot: &mut Snapshot, ids: &[Built]) {
    let mut ids = ids.iter();
    let (cube, frame, cylinder) = (ids.next(), ids.next(), ids.next());
    let edge = cylinder.and_then(|built| built.edges.get(2));
    let curve = edge.and_then(|e| Edge::table(&snapshot.model).live(e.slot(), e.generation())).map(|e| e.curve);
    free(snapshot, frame.and_then(|built| built.faces.get(2)));
    free::<Vertex>(snapshot, cube.and_then(|built| built.vertices.first()));
    free(snapshot, curve.as_ref());
}

/// Frees every seventh vertex and every eleventh edge use of every cube.
fn many(snapshot: &mut Snapshot, ids: &[Built]) {
    for built in ids {
        for vertex in built.vertices.iter().step_by(7) {
            free::<Vertex>(snapshot, Some(vertex));
        }
        for used in built.uses.iter().step_by(11) {
            free::<EdgeUse>(snapshot, Some(used));
        }
    }
}

/// Every operation's hash on `exec`, in the order of [`OPERATIONS`].
fn hashes(exec: &Exec) -> [u64; OPERATION_COUNT] {
    let cubes: Vec<Spec> = (0..CUBES).map(|_| cube()).collect();
    [
        hash(&[&[cube()]], clean, exec),
        hash(&[&[frame()]], clean, exec),
        hash(&[&[cylinder()]], clean, exec),
        hash(&[&[sheet()]], clean, exec),
        hash(&[&[wire()]], clean, exec),
        hash(&[&[cube()], &[frame(), wire()], &[cylinder()]], clean, exec),
        hash(&[&[cube(), frame(), cylinder(), sheet(), wire()]], few, exec),
        hash(&[&cubes], many, exec),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What the probe must give. The file is the one home of these numbers, and `gates determinism` reads it too.
    const RECORDED: &str = include_str!("../../../ci/determinism-topology.txt");

    fn want() -> [u64; OPERATION_COUNT] {
        let mut lines = RECORDED.lines().filter(|line| !line.starts_with('#'));
        OPERATIONS.map(|name| {
            let (operation, hash) = lines.next().unwrap().split_once(' ').unwrap();
            assert_eq!(operation, name, "the record is in the order of the operations");
            u64::from_str_radix(hash, 16).unwrap()
        })
    }

    #[test]
    fn the_probe_gives_the_recorded_hashes_on_one_thread_and_on_four() {
        assert_eq!(hashes(&Exec::serial()), want());
        #[cfg(not(target_family = "wasm"))]
        assert_eq!(hashes(&Exec::threads(core::num::NonZeroUsize::new(4).unwrap()).unwrap()), want());
    }

    /// What goes before each line `gates determinism` reads, among the lines the test harness prints.
    const MARK: &str = "determinism: ";

    /// The build line, then every hash on `exec`, as the gate reads them, each printed after the mark.
    fn print(exec: &Exec) -> usize {
        let simd128 = cfg!(target_feature = "simd128");
        let build = format!("build {} simd128={simd128} threads={}", std::env::consts::ARCH, exec.thread_count());
        let rows =
            OPERATIONS.iter().zip(hashes(exec)).map(|(operation, hash)| format!("{operation} scalar {hash:016x}"));
        let lines: Vec<String> = std::iter::once(build).chain(rows).collect();
        // A line of its own for the first, whatever the harness has printed on this one.
        println!();
        for line in &lines {
            println!("{MARK}{line}");
        }
        lines.len()
    }

    /// As many threads as `gates determinism` asks of the builds that have them: the machine's, and two at least.
    #[cfg(not(target_family = "wasm"))]
    fn every_thread() -> Exec {
        let threads = std::thread::available_parallelism().map_or(2, |count| count.get().max(2));
        Exec::threads(core::num::NonZeroUsize::new(threads).unwrap()).unwrap()
    }

    #[test]
    #[ignore = "what `gates determinism` runs as every build, to compare what each prints; the test above holds this one"]
    fn the_gate_reads_every_hash_on_one_thread_and_on_every_thread() {
        assert_eq!(print(&Exec::serial()), OPERATION_COUNT + 1);
        #[cfg(not(target_family = "wasm"))]
        assert_eq!(print(&every_thread()), OPERATION_COUNT + 1);
    }

    /// The record's header. The record is what this build prints on one thread: see the probe's documentation.
    const HEADER: &str = "\
# `gates determinism` and occam-topo's probe test: what the topology's probe must hash to, `<operation> <hash>`, in the
# order of `OPERATIONS` in crates/occam-topo/src/probe.rs. Unlike ci/determinism-hashes.txt, these do not come from a
# separate implementation, as there is no second topology: they are what the x86-64 build printed on one thread
# (`just determinism-topology`). Every other build and thread count is held to them. See docs/DETERMINISM.md before
# changing one.";

    #[test]
    #[ignore = "what `just determinism-topology` runs, to print the record as this build gives it"]
    fn the_record_as_this_build_gives_it_on_one_thread() {
        let rows =
            OPERATIONS.iter().zip(hashes(&Exec::serial())).map(|(operation, hash)| format!("{operation} {hash:016x}"));
        let lines: Vec<String> = HEADER.lines().map(str::to_string).chain(rows).collect();
        println!();
        for line in &lines {
            println!("record: {line}");
        }
        assert_eq!(lines.len(), RECORDED.lines().count(), "as many lines as the record has");
    }

    #[test]
    fn the_many_cubes_fill_more_than_one_chunk_of_edge_uses_and_lose_some_of_each() {
        let cubes: Vec<Spec> = (0..CUBES).map(|_| cube()).collect();
        let (snapshot, ids) = built(&[&cubes], &Exec::serial()).unwrap();
        let uses = snapshot.ids::<EdgeUse>().len();
        assert!(uses > crate::table::CHUNK, "{uses} edge uses");
        let mut corrupted = snapshot.clone();
        many(&mut corrupted, &ids);
        assert_eq!(corrupted.ids::<Vertex>().len(), CUBES * 6, "two of each cube's eight vertices");
        assert_eq!(corrupted.ids::<EdgeUse>().len(), CUBES * 21, "three of each cube's twenty-four uses");
        assert!(corrupted.check_all(&Exec::serial()).len() > CUBES);
    }

    #[test]
    fn every_operation_is_distinct_and_a_refused_build_is_hashed_apart() {
        let found = hashes(&Exec::serial());
        let mut sorted = found.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), OPERATION_COUNT, "{found:x?}");
        let mut digest = Digest::EMPTY;
        text(&mut digest, b"refused");
        reports(&mut digest, &[]);
        assert_ne!(digest.finish(), Digest::EMPTY.finish());
    }
}
