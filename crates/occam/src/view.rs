//! Source: original
//! `occam view`'s files and answers in their one JSON form (docs/architecture/live-view.md): the state file beside a
//! model, which names its viewer's process, port and token; what a viewer says of the model it watches; and what the
//! command answers, in JSON and in words. The command line runs the viewer; this is only what it writes and says.

use occam_report::{DisplayUnits, Report, SCHEMA};
use serde_json::{Value, json};

use crate::run::report_value;

/// A model's viewer, as its state file names it: the server's process, the port it listens on, on 127.0.0.1, and the
/// token every address it serves begins with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Viewer {
    /// The server's process.
    pub pid: u32,
    /// Its port on 127.0.0.1.
    pub port: u16,
    /// What every address it serves begins with, so that no other page on the machine can reach it.
    pub token: String,
}

impl Viewer {
    /// The state file's one JSON form.
    #[must_use]
    pub fn json(&self) -> String {
        json!({"schema": SCHEMA, "pid": self.pid, "port": self.port, "token": self.token}).to_string()
    }

    /// The viewer a state file names, or `None` for text that is not one, which is then stale.
    #[must_use]
    pub fn from_json(text: &str) -> Option<Self> {
        let value: Value = serde_json::from_str(text).ok()?;
        let number = |name: &str| value.get(name).and_then(Value::as_u64);
        let token = value.get("token").and_then(Value::as_str).filter(|token| !token.is_empty())?;
        number("schema").filter(|&schema| schema == u64::from(SCHEMA))?;
        Some(Self {
            pid: u32::try_from(number("pid")?).ok()?,
            port: u16::try_from(number("port")?).ok()?,
            token: token.to_string(),
        })
    }

    /// The page's address.
    #[must_use]
    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}/{}/", self.port, self.token)
    }
}

/// A model's digest as the viewer and its page write it: sixteen hexadecimal digits.
#[must_use]
pub fn digest_text(digest: u64) -> String {
    format!("{digest:016x}")
}

/// What a viewer says of the model it watches.
#[derive(Clone, Debug, PartialEq)]
pub struct Watching {
    /// The model's file, as an absolute path.
    pub model: String,
    /// The server's process.
    pub pid: u32,
    /// How many pages have it open.
    pub pages: u32,
    /// The digest of the last model that read whole, which the page draws, if one has.
    pub digest: Option<u64>,
    /// Why the file does not read now, if it does not: the page keeps drawing the last model that did.
    pub reports: Vec<Report>,
}

impl Watching {
    /// Its one JSON form.
    #[must_use]
    pub fn json(&self) -> String {
        let reports: Vec<Value> = self.reports.iter().map(|r| report_value(r, DisplayUnits::default())).collect();
        let digest = self.digest.map(digest_text);
        json!({"schema": SCHEMA, "model": self.model, "pid": self.pid, "pages": self.pages, "digest": digest,
            "reports": reports})
        .to_string()
    }

    /// What `text`, its one JSON form, says, or `None` for text that is not it.
    #[must_use]
    pub fn from_json(text: &str) -> Option<Self> {
        let value: Value = serde_json::from_str(text).ok()?;
        let digest = match value.get("digest")? {
            Value::Null => None,
            digest => Some(u64::from_str_radix(digest.as_str()?, 16).ok()?),
        };
        let reports = value.get("reports")?.as_array()?.iter().map(|r| Report::from_json(&r.to_string()).ok());
        Some(Self {
            model: value.get("model")?.as_str()?.to_string(),
            pid: u32::try_from(value.get("pid")?.as_u64()?).ok()?,
            pages: u32::try_from(value.get("pages")?.as_u64()?).ok()?,
            digest,
            reports: reports.collect::<Option<Vec<Report>>>()?,
        })
    }
}

/// What `occam view` answered: the viewer of the model, if one runs, and what became of it.
#[derive(Clone, Debug, PartialEq)]
pub struct Viewed {
    /// The model, as given.
    pub model: String,
    /// The viewer that runs, and what it says of the model, if one runs.
    pub running: Option<(Viewer, Watching)>,
    /// Whether this command started it.
    pub started: bool,
    /// Whether this command stopped it.
    pub stopped: bool,
    /// Whether this command opened the page in a browser.
    pub opened: bool,
    /// What refused the command, and why the model does not read, if it does not.
    pub reports: Vec<Report>,
}

impl Viewed {
    /// Nothing done yet for `model`.
    #[must_use]
    pub fn of(model: &str) -> Self {
        Self { model: model.to_string(), running: None, started: false, stopped: false, opened: false, reports: vec![] }
    }

    /// Whether everything asked was answered and the model reads.
    #[must_use]
    pub fn ok(&self) -> bool {
        self.reports.is_empty() && self.running.as_ref().is_none_or(|(_, watching)| watching.reports.is_empty())
    }

    /// Every report: what refused the command, then why the model does not read.
    fn all_reports(&self) -> impl Iterator<Item = &Report> {
        self.reports.iter().chain(self.running.iter().flat_map(|(_, watching)| &watching.reports))
    }

    /// Its one JSON form: the page's address and the token, the server's process and port, and the model's digest,
    /// where one runs.
    #[must_use]
    pub fn json(&self) -> String {
        let reports: Vec<Value> = self.all_reports().map(|r| report_value(r, DisplayUnits::default())).collect();
        let mut answer = json!({"schema": SCHEMA, "ok": self.ok(), "model": self.model,
            "running": self.running.is_some(), "started": self.started, "stopped": self.stopped});
        if let (Some((viewer, watching)), Some(fields)) = (&self.running, answer.as_object_mut()) {
            fields.insert("url".into(), viewer.url().into());
            fields.insert("token".into(), viewer.token.clone().into());
            fields.insert("port".into(), viewer.port.into());
            fields.insert("pid".into(), viewer.pid.into());
            fields.insert("pages".into(), watching.pages.into());
            fields.insert("digest".into(), watching.digest.map(digest_text).into());
            fields.insert("opened".into(), self.opened.into());
        }
        if let Some(fields) = answer.as_object_mut() {
            fields.insert("reports".into(), reports.into());
        }
        answer.to_string()
    }

    /// It in words, a line each, with whether each goes to standard output: where the viewer is, or that none runs,
    /// then each report, for standard error.
    #[must_use]
    pub fn words(&self) -> Vec<(bool, String)> {
        let model = &self.model;
        let said = match &self.running {
            Some((viewer, _)) if self.started && self.opened => {
                Some(format!("The viewer for {model} is at {}, open in a browser.", viewer.url()))
            }
            Some((viewer, _)) if self.started => Some(format!("The viewer for {model} is at {}.", viewer.url())),
            Some((viewer, watching)) => Some(format!(
                "The viewer for {model} runs at {}, process {}, with {} {} open.",
                viewer.url(),
                viewer.pid,
                watching.pages,
                if watching.pages == 1 { "page" } else { "pages" }
            )),
            None if self.stopped => Some(format!("The viewer for {model} has stopped.")),
            None if self.reports.is_empty() => Some(format!("No viewer runs for {model}.")),
            None => None,
        };
        let refused = self.all_reports().map(|report| (false, report.message(DisplayUnits::default())));
        said.map(|line| (true, line)).into_iter().chain(refused).collect()
    }
}

#[cfg(test)]
mod tests;
