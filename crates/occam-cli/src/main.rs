//! Source: original
//! The occam command-line tool: a thin facade over the `occam` crate, for agents and people. `--json` gives the one JSON form
//! of the result or the report on standard output (L8). The exit status is 0 on success and 1 when the operation is refused.
#![allow(clippy::print_stdout, clippy::print_stderr, reason = "a command-line tool prints")]

use std::process::ExitCode;

use occam::{Code, DisplayUnits, DistanceQuery, NamedValue, Report, distance_json};

/// The first problem with the command line. `position` counts arguments from 1.
fn bad(position: u32) -> Report {
    Report::new(Code::BadArgument).with_value(NamedValue::count("position", position))
}

fn numbers<const N: usize>(text: &str, position: u32) -> Result<[f64; N], Report> {
    let parsed: Result<Vec<f64>, _> = text.split(',').map(str::parse).collect();
    parsed.ok().and_then(|list| list.try_into().ok()).ok_or_else(|| bad(position))
}

#[derive(Default)]
struct Flags {
    from: Option<[f64; 3]>,
    to: Option<[f64; 3]>,
    model: Option<[f64; 2]>,
    tolerance: Option<[f64; 1]>,
}

impl Flags {
    const NAMES: [&'static str; 4] = ["--from", "--to", "--model-tolerance", "--tolerance"];

    fn set(&mut self, flag: &str, value: &str, position: u32) -> Result<(), Report> {
        match flag {
            "--from" => self.from = Some(numbers(value, position)?),
            "--to" => self.to = Some(numbers(value, position)?),
            "--model-tolerance" => self.model = Some(numbers(value, position)?),
            _ => self.tolerance = Some(numbers(value, position)?),
        }
        Ok(())
    }

    /// The query, or `missing` as the position of whatever was left out.
    fn query(self, missing: u32) -> Result<DistanceQuery, Report> {
        match (self.from, self.to, self.model, self.tolerance) {
            (Some(from), Some(to), Some([model_length_tolerance, model_angle_tolerance]), Some([tolerance])) => {
                Ok(DistanceQuery { from, to, model_length_tolerance, model_angle_tolerance, tolerance })
            }
            _ => Err(bad(missing)),
        }
    }
}

fn wants_json(arguments: &[String]) -> bool {
    arguments.iter().any(|argument| argument == "--json")
}

fn parse(arguments: &[String]) -> Result<DistanceQuery, Report> {
    if arguments.first().map(String::as_str) != Some("distance") {
        return Err(bad(1));
    }
    let mut rest = arguments.iter().zip(1_u32..).skip(1);
    let mut flags = Flags::default();
    while let Some((flag, position)) = rest.next() {
        if flag == "--json" {
            continue;
        }
        if !Flags::NAMES.contains(&flag.as_str()) {
            return Err(bad(position));
        }
        let Some((value, at)) = rest.next() else {
            return Err(bad(position.saturating_add(1)));
        };
        flags.set(flag, value, at)?;
    }
    flags.query(u32::try_from(arguments.len()).unwrap_or(u32::MAX).saturating_add(1))
}

/// What to say and the exit status: 0 for success, 1 for a refused operation and 2 for a bad command line.
fn run(arguments: &[String]) -> (String, u8) {
    let json = wants_json(arguments);
    match parse(arguments) {
        Ok(query) => {
            let answer = distance_json(&query);
            (if json { answer.json } else { answer.words }, u8::from(answer.refused))
        }
        Err(report) => {
            let units = DisplayUnits::default();
            let text = if json {
                report.to_json(units).unwrap_or_else(|failed| failed.code.text().to_string())
            } else {
                report.message(units)
            };
            (text, 2)
        }
    }
}

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let (text, status) = run(&arguments);
    if status == 0 || wants_json(&arguments) {
        println!("{text}");
    } else {
        eprintln!("{text}");
    }
    ExitCode::from(status)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(arguments: &[&str]) -> Vec<String> {
        arguments.iter().map(ToString::to_string).collect()
    }

    const GOOD: &[&str] = &[
        "distance",
        "--from",
        "0,0,1",
        "--to",
        "3,4,1",
        "--model-tolerance",
        "0.001,0.00001",
        "--tolerance",
        "0.01",
        "--json",
    ];

    fn query() -> DistanceQuery {
        DistanceQuery {
            from: [0.0, 0.0, 1.0],
            to: [3.0, 4.0, 1.0],
            model_length_tolerance: 0.001,
            model_angle_tolerance: 0.00001,
            tolerance: 0.01,
        }
    }

    #[test]
    fn every_flag_reaches_the_query() {
        assert_eq!(parse(&strings(GOOD)), Ok(query()));
    }

    #[test]
    fn flags_can_come_in_any_order() {
        let arguments = strings(&[
            "distance",
            "--json",
            "--tolerance",
            "0.01",
            "--model-tolerance",
            "0.001,0.00001",
            "--to",
            "3,4,1",
            "--from",
            "0,0,1",
        ]);
        assert_eq!(parse(&arguments), Ok(query()));
    }

    #[test]
    fn without_the_flag_the_answer_is_in_words() {
        let arguments = strings(GOOD.strip_suffix(&["--json"]).unwrap_or(GOOD));
        assert_eq!(run(&arguments), ("5 mm".to_string(), 0));
    }

    #[test]
    fn with_the_flag_a_success_is_the_json_form() {
        let (text, status) = run(&strings(GOOD));
        assert!(text.starts_with(r#"{"schema":1,"value":5.0,"#), "{text}");
        assert_eq!(status, 0);
    }

    fn position(arguments: &[&str]) -> f64 {
        let report = parse(&strings(arguments)).unwrap_err();
        assert_eq!(report.code, Code::BadArgument);
        report.value("position").map_or(f64::NAN, |value| value.value)
    }

    #[test]
    fn a_bad_argument_is_reported_by_its_position() {
        assert_eq!(position(&[]), 1.0, "no command");
        assert_eq!(position(&["cut"]), 1.0, "unknown command");
        assert_eq!(position(&["distance", "--sideways"]), 2.0, "unknown flag");
        assert_eq!(position(&["distance", "--from"]), 3.0, "flag with no value");
        assert_eq!(position(&["distance", "--from", "1,2"]), 3.0, "too few coordinates");
        assert_eq!(position(&["distance", "--from", "1,2,3,4"]), 3.0, "too many coordinates");
        assert_eq!(position(&["distance", "--from", "1,2,x"]), 3.0, "not a number");
        assert_eq!(position(&["distance", "--tolerance", "0.1,0.2"]), 3.0, "a list where one number belongs");
        assert_eq!(position(&["distance", "--tolerance", "x"]), 3.0, "a tolerance that is not a number");
        assert_eq!(position(&["distance", "--model-tolerance", "0.1"]), 3.0, "too few model tolerances");
    }

    #[test]
    fn a_missing_flag_is_reported_one_past_the_end() {
        assert_eq!(position(&["distance", "--from", "0,0,0"]), 4.0);
        assert_eq!(position(&["distance", "--from", "0,0,0", "--to", "1,1,1"]), 6.0);
        assert_eq!(position(&["distance", "--from", "0,0,0", "--to", "1,1,1", "--tolerance", "1"]), 8.0);
    }

    #[test]
    fn a_bad_argument_is_said_the_way_the_caller_asked() {
        let (json, status) = run(&strings(&["distance", "--sideways", "--json"]));
        assert_eq!(status, 2);
        assert_eq!(Report::from_json(&json).map(|report| report.code), Ok(Code::BadArgument));
        let (words, status) = run(&strings(&["distance", "--sideways"]));
        assert_eq!(status, 2);
        assert!(words.starts_with("Argument 2 is not understood."), "{words}");
    }
}
