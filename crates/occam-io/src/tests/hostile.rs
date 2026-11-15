//! Source: original
//! Hostile files (docs/decisions/2026-11-13-binary-twin-checksum.md). A binary twin cut short anywhere, lengthened, or
//! with any one bit flipped is refused; beneath a checksum made good again, records that lie about their lengths, link
//! to what is not there or not of their kind, hold numbers no family takes, or join into no closed model are refused
//! with a code, as are thousands of bytes changed at random. None may panic, hang, or allocate more than the file's own
//! size pays for: a length claiming 2^60 elements must be refused, not believed.

use serde_json::json;

use occam_report::EntityKind;

use super::geometry::SurfaceFamily;
use super::*;

/// The cube, and its binary twin.
fn cube() -> (Snapshot, Vec<u8>) {
    let (_, cube) = models(&serial()).swap_remove(0);
    let twin = save_binary(&cube).unwrap();
    (cube, twin)
}

/// The codes loading `bytes` is refused with, none if it loads: a model that loads has passed every check.
fn codes(bytes: &[u8]) -> Vec<Code> {
    match load(bytes, &serial()) {
        Ok(_) => Vec::new(),
        Err(reports) => {
            assert!(!reports.is_empty(), "a refusal names at least one failure");
            reports.iter().map(|r| r.code).collect()
        }
    }
}

/// The codes parsing `bytes` is refused with, none if it parses.
fn parse_codes(bytes: &[u8]) -> Vec<Code> {
    parse(bytes).map_or_else(|reports| reports.iter().map(|r| r.code).collect(), |_| Vec::new())
}

/// `bytes`, the records of a twin after its magic and version, as a whole twin with a good checksum.
fn twin_of(records: &[u8]) -> Vec<u8> {
    sealed([MAGIC.as_slice(), &VERSION.to_le_bytes(), records].concat())
}

/// A twin's records, between its version and its checksum.
fn records(twin: &[u8]) -> &[u8] {
    twin.get(MAGIC.len().saturating_add(4)..twin.len().saturating_sub(CHECKSUM)).unwrap()
}

/// `n` written as postcard writes a length or a whole number: seven bits a byte, least significant first.
fn varint(n: u64) -> Vec<u8> {
    encode(&n)
}

#[test]
fn an_empty_file_or_a_magic_alone_is_not_the_format() {
    assert_eq!(codes(b""), [Code::NotReadable]);
    assert_eq!(codes(MAGIC), [Code::NotReadable]);
    assert_eq!(codes(&[MAGIC.as_slice(), &VERSION.to_le_bytes()].concat()), [Code::NotReadable]);
    assert_eq!(codes(&twin_of(&[])), [Code::NotReadable], "a good checksum over no records");
}

#[test]
fn a_twin_cut_short_anywhere_or_lengthened_is_refused() {
    let (_, twin) = cube();
    for length in 0..twin.len() {
        let want = if length < MAGIC.len() + 4 + CHECKSUM { Code::NotReadable } else { Code::Damaged };
        assert_eq!(codes(twin.get(..length).unwrap()), [want], "cut to {length} of {} bytes", twin.len());
    }
    // Beneath a checksum made good again, records cut short anywhere are not the format: each is read to its end.
    let whole = records(&twin);
    for length in 0..whole.len() {
        assert_eq!(parse_codes(&twin_of(whole.get(..length).unwrap())), [Code::NotReadable], "records cut to {length}");
    }
    for extra in [&[0_u8][..], &[1, 2, 3], &twin] {
        assert_eq!(codes(&[twin.as_slice(), extra].concat()), [Code::Damaged], "{} bytes after it", extra.len());
        let report = refusal(&twin_of(&[whole, extra].concat()));
        assert_eq!(report.code, Code::NotReadable);
        assert_eq!(place(&report).1, Some(f64::from(u32::try_from(twin.len() - CHECKSUM + 1).unwrap())), "the first");
    }
}

#[test]
fn every_single_bit_flipped_in_a_twin_is_caught() {
    // In the magic it is no longer a twin, in the version another version, and anywhere else the checksum fails: a
    // CRC-32 catches every error of one bit.
    let (_, twin) = cube();
    for byte in 0..twin.len() {
        let want = match byte {
            b if b < MAGIC.len() => Code::NotReadable,
            b if b < MAGIC.len() + 4 => Code::OtherVersion,
            _ => Code::Damaged,
        };
        for bit in 0..8 {
            let mut flipped = twin.clone();
            *flipped.get_mut(byte).unwrap() ^= 1 << bit;
            assert_eq!(parse_codes(&flipped), [want], "bit {bit} of byte {byte}");
        }
    }
}

/// `value` as postcard writes it.
fn encode<T: Serialize>(value: &T) -> Vec<u8> {
    postcard::to_extend(value, Vec::new()).unwrap()
}

/// The records of `file` as the twin writes them, each part named, so that one part can be changed alone.
fn parts(file: &File) -> Vec<(&'static str, Vec<u8>)> {
    vec![
        ("head", encode(&(&file.format, file.version, &file.tolerance, file.step))),
        ("steps", encode(&file.steps)),
        ("bodies", encode(&file.bodies)),
        ("shells", encode(&file.shells)),
        ("faces", encode(&file.faces)),
        ("loops", encode(&file.loops)),
        ("wires", encode(&file.wires)),
        ("uses", encode(&file.uses)),
        ("edges", encode(&file.edges)),
        ("vertices", encode(&file.vertices)),
        ("surfaces", encode(&file.surfaces)),
        ("curves", encode(&file.curves)),
        ("pcurves", encode(&file.pcurves)),
    ]
}

/// The twin of `file` with the part named `name` changed by `change`.
fn with_part(file: &File, name: &str, change: impl Fn(Vec<u8>) -> Vec<u8>) -> Vec<u8> {
    let records: Vec<u8> =
        parts(file).into_iter().flat_map(|(part, bytes)| if part == name { change(bytes) } else { bytes }).collect();
    twin_of(&records)
}

#[test]
fn the_parts_of_a_file_written_one_by_one_are_its_twin() {
    let (cube, twin) = cube();
    assert_eq!(with_part(&file_of(&cube).unwrap(), "", |b| b), twin);
}

#[test]
fn a_length_that_claims_more_than_the_file_holds_is_refused_not_believed() {
    // Every table, and the format's name, claiming 2^60 elements, or as many as a length can say, with only the
    // file's own bytes after it: reading runs out of bytes long before it could fill so many, and allocates no more
    // than the bytes pay for.
    let (cube, _) = cube();
    let file = file_of(&cube).unwrap();
    for (name, _) in parts(&file) {
        for claim in [1_u64 << 60, u64::MAX, 1 << 32, 1_000_000_000] {
            let lying = with_part(&file, name, |bytes| [varint(claim), bytes].concat());
            assert_eq!(parse_codes(&lying), [Code::NotReadable], "{name} claiming {claim}");
        }
    }
    // A surface's numbers claiming 2^60, inside a record: the cube's surfaces, then one more, a plane of generation 1.
    let lying = with_part(&file, "surfaces", |bytes| {
        let (count, rest) = postcard::take_from_bytes::<u64>(&bytes).unwrap();
        let plane = [varint(1), encode(&Some(SurfaceFamily::Plane)), varint(1 << 60), encode(&[1.0_f64; 12])];
        [varint(count + 1), rest.to_vec(), plane.concat()].concat()
    });
    assert_eq!(parse_codes(&lying), [Code::NotReadable], "a plane's numbers claiming 2^60");
}

#[test]
fn a_link_to_nothing_or_to_the_wrong_kind_or_of_generation_zero_is_refused() {
    // A link is written as its kind's index, its slot and its generation. The cube's first edge begins with its curve's.
    let (cube, twin) = cube();
    let file = file_of(&cube).unwrap();
    let edges = parts(&file).into_iter().find(|(name, _)| *name == "edges").unwrap().1;
    let (_, rest) = postcard::take_from_bytes::<u64>(&edges).unwrap();
    let (_, rest) = postcard::take_from_bytes::<u32>(rest).unwrap();
    let rest = rest.strip_prefix(&[1]).unwrap();
    let at = edges.len() - rest.len();
    let (kind, rest) = postcard::take_from_bytes::<EntityKind>(rest).unwrap();
    let (slot, rest) = postcard::take_from_bytes::<u32>(rest).unwrap();
    let (generation, rest) = postcard::take_from_bytes::<u32>(rest).unwrap();
    let after = edges.len() - rest.len();
    assert_eq!(kind, EntityKind::Curve);
    let link = |kind: &[u8], slot: u64, generation: u64| {
        with_part(&file, "edges", |bytes| {
            [bytes.get(..at).unwrap(), kind, &varint(slot), &varint(generation), bytes.get(after..).unwrap()].concat()
        })
    };
    let curve = encode(&EntityKind::Curve);
    assert_eq!(link(&curve, slot.into(), generation.into()), twin, "the curve link found, as written");
    let surface = encode(&EntityKind::Surface);
    assert_eq!(parse_codes(&link(&surface, 0, 1)), [Code::NotReadable], "a surface where a curve belongs");
    assert_eq!(parse_codes(&link(&varint(200), 0, 1)), [Code::NotReadable], "a kind there is not");
    assert_eq!(parse_codes(&link(&curve, 0, 0)), [Code::NotReadable], "generation zero");
    for (slot, generation) in
        [(999, 1), (u64::from(slot), u64::from(generation) + 6), (u32::MAX.into(), u32::MAX.into())]
    {
        let refused = codes(&link(&curve, slot, generation));
        assert!(!refused.is_empty() && !refused.contains(&Code::NotReadable), "curve:{slot}/{generation}: {refused:?}");
    }
    assert_eq!(parse_codes(&link(&curve, 1 << 32, 1)), [Code::NotReadable], "a slot no table has");
    // A slot of generation zero.
    let bodies = parts(&file).into_iter().find(|(name, _)| *name == "bodies").unwrap().1;
    assert_eq!(bodies.get(..2), Some(&[1_u8, 1][..]), "one body, of generation 1");
    let zero = with_part(&file, "bodies", |bytes| [&[1_u8, 0][..], bytes.get(2..).unwrap()].concat());
    assert_eq!(parse_codes(&zero), [Code::NotReadable]);
}

/// A change to a readable file.
type Change = Box<dyn FnOnce(&mut Value)>;

/// The cube's twin, made from its readable file with `change` made to it.
fn cube_changed(change: impl FnOnce(&mut Value)) -> Vec<u8> {
    let (cube, _) = cube();
    transcoded(&changed_file(&cube, change))
}

#[test]
fn numbers_no_record_or_family_takes_are_refused_naming_them() {
    let refused = |change: Change| codes(&cube_changed(change));
    for bad in ["nan", "inf", "-inf"] {
        let value = serde_json::json!(bad);
        let v = value.clone();
        let point = refused(Box::new(move |json| json["vertices"][0]["record"]["point"][0] = v));
        assert!(!point.is_empty(), "a vertex at {bad}");
        let v = value.clone();
        let range = refused(Box::new(move |json| json["edges"][0]["record"]["range"][1] = v));
        assert!(!range.is_empty(), "an edge's range to {bad}");
        let v = value.clone();
        let witness = refused(Box::new(move |json| json["loops"][0]["record"]["witness"][0] = v));
        assert!(!witness.is_empty(), "a witness at {bad}");
        let v = value.clone();
        let tolerance = refused(Box::new(move |json| json["tolerance"]["length"] = v));
        assert_eq!(tolerance, [Code::ToleranceOutOfRange], "a tolerance of {bad}");
        let v = value.clone();
        let plane = refused(Box::new(move |json| json["surfaces"][0]["record"]["numbers"][0] = v));
        assert!(!plane.is_empty(), "a plane through {bad}");
    }
}

#[test]
fn a_deviation_as_written_is_not_trusted_but_measured_again() {
    // An edge's deviation is measured on load, as `commit` measures it, so whatever the file says, NaN or infinity
    // included, the model has the one measured.
    let (cube, _) = cube();
    for bad in [json!("nan"), json!("inf"), json!("-inf"), json!(1e300)] {
        let twin = cube_changed(|json| json["edges"][0]["record"]["deviation"] = bad.clone());
        assert_eq!(load(&twin, &serial()).unwrap().digest(), cube.digest(), "a deviation of {bad}");
    }
}

#[test]
fn a_topology_that_runs_in_circles_or_does_not_close_is_refused_and_ends() {
    // The cube's records joined wrongly, beneath a good checksum: each is refused by the checks, which end.
    let wrong: Vec<(&str, Change)> = vec![
        ("a use whose next is itself", Box::new(|json| json["uses"][0]["record"]["next"] = json!("edge-use:0/1"))),
        ("a use whose radial is itself", Box::new(|json| json["uses"][0]["record"]["radial"] = json!("edge-use:0/1"))),
        (
            "a loop that begins in another's",
            Box::new(|json| json["loops"][0]["record"]["first"] = json!("edge-use:5/1")),
        ),
        ("a face of its own loop's neighbour", Box::new(|json| json["loops"][0]["record"]["face"] = json!("face:1/1"))),
        ("a face gone", Box::new(|json| json["faces"][0] = json!({"generation": 1}))),
        ("a use gone", Box::new(|json| json["uses"][3] = json!({"generation": 1}))),
        (
            "a shell that begins at a free slot",
            Box::new(|json| json["shells"][0]["record"]["face"] = json!("face:0/2")),
        ),
        ("a shell that is its own next", Box::new(|json| json["shells"][0]["record"]["next"] = json!("shell:0/1"))),
        (
            "an edge whose ends are one",
            Box::new(|json| json["edges"][0]["record"]["finish"] = json["edges"][0]["record"]["start"].clone()),
        ),
        (
            "a vertex starred on an edge it ends none of",
            Box::new(|json| {
                let ends = |e: &Value| [&e["record"]["start"], &e["record"]["finish"]].contains(&&json!("vertex:0/1"));
                let far = json["edges"].as_array().unwrap().iter().position(|e| !ends(e)).unwrap();
                json["vertices"][0]["record"]["star"]["edge"] = json!(format!("edge:{far}/1"));
            }),
        ),
    ];
    for (name, change) in wrong {
        let twin = cube_changed(change);
        assert!(!codes(&twin).is_empty(), "{name}");
    }
}

#[test]
fn a_model_at_the_last_step_loads_or_is_refused_and_its_next_edit_does_not_panic() {
    let twin = cube_changed(|json| json["step"] = json!(u64::MAX));
    if let Ok(model) = load(&twin, &serial()) {
        let mut edit = model.edit();
        edit.add(CurveRecord(Arc::new(Line::new(at(0.0, 0.0, 0.0), Direction::normalised(Vec3::new(1.0, 0.0, 0.0))))));
        let _ = edit.commit(&serial());
    }
}

/// The `k`th number of the stream seeded `seed`: the CRC-32 of the two, which is spread well enough to pick places and
/// bytes from.
fn chance(seed: u32, k: u32) -> u32 {
    crc::crc32(&[seed.to_le_bytes(), k.to_le_bytes()].concat())
}

/// What `trials` of one to four bytes changed at random in `twin`'s records, or the records cut, give, each sealed again:
/// how many were refused and how many loaded.
fn random_trials(twin: &[u8], seed: u32, trials: u32) -> (u32, u32) {
    let whole = records(twin);
    let length = u32::try_from(whole.len()).unwrap();
    let (mut refused, mut loaded) = (0, 0);
    for trial in 0..trials {
        let draw = |k: u32| chance(seed ^ trial.rotate_left(8), k);
        let mut changed = whole.to_vec();
        let draws = draw(0) % 5;
        for k in 1..=draws.max(1) {
            let place = usize::try_from(draw(k.wrapping_mul(2)).checked_rem(length).unwrap()).unwrap();
            *changed.get_mut(place).unwrap() = draw(k.wrapping_mul(2) | 1).to_le_bytes()[0];
        }
        if draws == 0 {
            changed.truncate(usize::try_from(draw(9).checked_rem(length).unwrap()).unwrap());
        }
        let tally = if codes(&twin_of(&changed)).is_empty() { &mut loaded } else { &mut refused };
        *tally = u32::saturating_add(*tally, 1);
    }
    (refused, loaded)
}

#[test]
fn thousands_of_bytes_changed_at_random_beneath_a_good_checksum_are_refused_or_load_checked() {
    // Most trials are refused as not the format or by the checks; a changed number may still make a model the checks
    // pass, which is then a model like any other. Nothing panics, and every trial ends: on the cube, and on the
    // cylinder, whose faces are curved.
    let (_, twin) = cube();
    let cylinder = save_binary(&models(&serial()).swap_remove(2).1).unwrap();
    for (name, twin, trials) in [("cube", &twin, 3000), ("cylinder", &cylinder, 1000)] {
        let (refused, loaded) = random_trials(twin, 0x5EED, trials);
        println!("{name}: {refused} refused, {loaded} loaded, of {trials}");
        assert_eq!(refused + loaded, trials);
        assert!(refused > trials / 2, "{name}: {refused} refused, {loaded} loaded");
    }
}

#[test]
fn records_of_random_bytes_beneath_a_good_checksum_are_refused() {
    // A thousand files whose records are noise, from none to 4 KiB of it, behind a good magic, version and checksum.
    for trial in 0..1000 {
        let length = chance(trial, 0) % 4096;
        let noise: Vec<u8> = (0..length).map(|k| chance(trial, k + 1).to_le_bytes()[0]).collect();
        assert!(!parse_codes(&twin_of(&noise)).is_empty(), "trial {trial}, {length} bytes");
    }
}
