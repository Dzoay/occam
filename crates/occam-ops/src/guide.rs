//! Source: original
//! How to write a request, as `occam run --help` gives it to an agent that has only the command (the bracket test). It is
//! made from the reader's own lists, the operations and the fields each takes, the units, the measurements and the
//! default tolerance, so it names nothing the reader refuses and leaves out nothing it takes.

use occam_report::{AngleUnit, LengthUnit, figure};

use crate::measure::Measurement;
use crate::operation::Operation;
use crate::request::{SCHEMA, default_tolerance};

/// The names of `all`, joined as a list in words: `a, b or c`.
fn either(all: &[&str]) -> String {
    match all.split_last() {
        Some((last, [])) => (*last).to_string(),
        Some((last, rest)) => format!("{} or {last}", rest.join(", ")),
        None => String::new(),
    }
}

/// The operations, each on its own lines under `operations`, with what it takes.
fn operations() -> String {
    let done = Operation::ALL.into_iter().filter(|operation| operation.supported());
    let lines: Vec<String> =
        done.map(|operation| format!("  {:<10} {}", operation.name(), operation.takes())).collect();
    lines.join("\n")
}

/// The guide to writing a request.
#[must_use]
pub fn guide() -> String {
    let lengths: Vec<&str> = LengthUnit::ALL.iter().map(|unit| unit.text()).collect();
    let angles: Vec<&str> = AngleUnit::ALL.iter().map(|unit| unit.text()).collect();
    let measurements: Vec<&str> = Measurement::ALL.iter().map(|m| m.name()).collect();
    let refused: Vec<&str> = Operation::ALL.into_iter().filter(|op| !op.supported()).map(Operation::name).collect();
    let tolerance = default_tolerance().map_or_else(
        |_| String::new(),
        |t| format!(" ({} mm and {} rad if left out)", figure(t.length().millimetres()), figure(t.angle().radians())),
    );
    format!(
        r#"A request is a JSON file of steps, run in order on the model:

  {{"schema": {SCHEMA}, "units": {{"length": "mm", "angle": "deg"}}, "steps": [
    {{"op": "box", "label": "block", "size": [40, 20, 10]}},
    {{"op": "measure", "target": "block", "what": ["volume"]}}
  ]}}

  schema     {SCHEMA}
  units      length: {lengths}; angle: {angles}. Every number the request gives is in these.
  tolerance  {{"length": number, "angle": number}}, optional: what a new model is built to{tolerance}.
  steps      each with "op", an optional "label", and its operation's fields, below. A field that names an entity takes
             an earlier step's label, for what that step made, or an id such as body:0/1.

A point or a vector is [x, y, z]; a direction is [x, y, z] of any length. A frame is {{"origin": point, "z": direction,
"x": direction}}, each optional, and the world's if left out.

Operations:
{operations}

Measurements: {measurements}.
Refused by this build, by name: {refused}.

Bad input is refused before any step runs, each with its code and its place, such as steps[2].size[0]. A step that
fails stops the request, and the model is written back only if every step went through. The result says what each
step made, created, changed and deleted, by id, and describes what it created."#,
        lengths = either(&lengths),
        angles = either(&angles),
        operations = operations(),
        measurements = measurements.join(", "),
        refused = refused.join(", "),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::request::Request;

    #[test]
    fn the_guide_names_every_operation_this_build_does_and_every_field_each_takes() {
        let guide = guide();
        for operation in Operation::ALL.into_iter().filter(|operation| operation.supported()) {
            let line = guide.lines().find(|line| line.starts_with(&format!("  {:<10} ", operation.name())));
            let Some(line) = line else { panic!("{operation:?} has no line in the guide") };
            for field in operation.fields() {
                assert!(line.contains(field), "{operation:?}'s line names {field}: {line}");
            }
        }
        assert!(guide.contains("Refused by this build, by name: boolean, fillet, chamfer."), "{guide}");
        assert!(guide.contains("Measurements: volume, area, centre-of-mass, bounds."), "{guide}");
        assert!(guide.contains("length: mm, cm, m or in; angle: rad or deg."), "{guide}");
        assert!(guide.contains("(1e-7 mm and 1e-9 rad if left out)"), "{guide}");
    }

    #[test]
    fn the_guide_s_own_example_is_a_request_the_reader_takes() {
        let guide = guide();
        let start = guide.find("  {\"schema\"").unwrap_or_default();
        let end = guide.find("  ]}").map_or(0, |end| end + 4);
        let example = guide.get(start..end).unwrap_or_default();
        let request = Request::parse(example).map_err(|reports| format!("{reports:?}: {example}"));
        assert_eq!(request.map(|r| r.steps.len()), Ok(2));
    }

    #[test]
    fn a_list_in_words_ends_with_or() {
        assert_eq!(either(&["a", "b", "c"]), "a, b or c");
        assert_eq!(either(&["a", "b"]), "a or b");
        assert_eq!(either(&["a"]), "a");
        assert_eq!(either(&[]), "");
    }
}
