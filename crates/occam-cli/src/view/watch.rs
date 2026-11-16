//! Source: original
//! Looking at the model file, as the viewer does several times a second (docs/architecture/live-view.md#the-server):
//! its size and modification time, and, when they change, the file read and opened. A file that opens reads whole,
//! and its model takes the last one's place. One that does not may be half-written, so it is read again at the next
//! look, and reported only once a look finds it as the last one did; a file that is not there is reported the same
//! way. The model is opened here, on the caller's thread, never on the server's async threads.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use occam::{Code, Encoding, Fault, Input, Remedy, RemedyKind, Report, Selection, Session};

use crate::commands::{beside, exec, not_read};

/// A model that read whole: its digest, and its bytes as the binary twin, which the page loads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Model {
    pub digest: u64,
    pub twin: Vec<u8>,
}

/// What a look found that the last did not.
#[derive(Clone, Debug, PartialEq)]
pub enum Looked {
    /// The file reads whole, as this model.
    Read(Model),
    /// The file does not read, for these reasons, and has not changed since the last look.
    Refused(Vec<Report>),
}

/// What a look sees of the file without reading it: its length and when it last changed, or that it is not there.
type Seen = Option<(u64, Option<SystemTime>)>;

/// The model file, and what the looks at it have seen.
pub struct Watcher {
    model: PathBuf,
    /// What the last look saw.
    seen: Option<Seen>,
    /// What the file was when it was last read whole or reported.
    judged: Option<Seen>,
}

impl Watcher {
    /// The file `model`, not yet looked at.
    pub const fn new(model: PathBuf) -> Self {
        Self { model, seen: None, judged: None }
    }

    /// The file judged as it is now, whatever the last look saw: the viewer's first look, before it says it listens.
    pub fn first(&mut self) -> Looked {
        let now = seen(&self.model);
        self.seen = Some(now);
        self.judged = Some(now);
        match read(&self.model) {
            Ok(model) => Looked::Read(model),
            Err(reports) => Looked::Refused(reports),
        }
    }

    /// What this look found that the last did not: nothing if the file is as it was when last judged, or changed since
    /// the last look and does not read yet.
    pub fn look(&mut self) -> Option<Looked> {
        let now = seen(&self.model);
        if self.judged == Some(now) {
            return None;
        }
        let settled = self.seen.replace(now) == Some(now);
        match read(&self.model) {
            Ok(model) => {
                self.judged = Some(now);
                Some(Looked::Read(model))
            }
            Err(reports) if settled => {
                self.judged = Some(now);
                Some(Looked::Refused(reports))
            }
            Err(_) => None,
        }
    }
}

/// The selection and the highlight beside `model`, each in its one JSON form, or that of none where its file is not
/// there or holds no selection: what the page is sent.
pub fn marks(model: &Path) -> [String; 2] {
    [".selection", ".highlight"].map(|suffix| {
        let read = std::fs::read_to_string(beside(model, suffix)).ok();
        read.and_then(|text| Selection::from_json(&text)).map_or_else(Selection::none_json, |marked| marked.json())
    })
}

/// What a look sees of `model`.
fn seen(model: &Path) -> Seen {
    let metadata = std::fs::metadata(model).ok()?;
    Some((metadata.len(), metadata.modified().ok()))
}

/// The model `model` holds, or why it holds none.
fn read(model: &Path) -> Result<Model, Vec<Report>> {
    let bytes = std::fs::read(model).map_err(|error| vec![unread(model, &error)])?;
    let session = Session::open(&bytes, &exec())?;
    let twin = session.encoded(Encoding::Binary).unwrap_or_else(|| Ok(Vec::new())).map_err(|report| vec![report])?;
    Ok(Model { digest: session.digest().unwrap_or_default(), twin })
}

/// The report for `model`, which could not be read, as `error` says: `view.model-missing` if it is not there.
fn unread(model: &Path, error: &std::io::Error) -> Report {
    if error.kind() != std::io::ErrorKind::NotFound {
        return not_read("--model", model);
    }
    Report::new(Code::ModelMissing, Fault::InvalidInput)
        .with_input(Input::at("--model").given(&model.display().to_string()))
        .with_remedy(Remedy::new(RemedyKind::MakeModel))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A box, as `occam run` would write it in `encoding`.
    fn boxed(size: u32, encoding: Encoding) -> (u64, Vec<u8>) {
        let mut session = Session::new(encoding);
        let request = format!(
            r#"{{"schema": 1, "units": {{"length": "mm", "angle": "deg"}}, "steps": [{{"op": "box", "size": [{size}, 2, 3]}}]}}"#
        );
        let (_, saved) = session.run(&request);
        (session.digest().unwrap(), saved.unwrap())
    }

    fn codes(looked: Option<Looked>) -> Vec<Code> {
        match looked {
            Some(Looked::Refused(reports)) => reports.iter().map(|report| report.code).collect(),
            other => panic!("not refused: {other:?}"),
        }
    }

    #[test]
    fn a_file_that_reads_whole_is_drawn_and_one_that_does_not_is_reported_once_it_holds_still() {
        let dir = std::env::temp_dir().join(format!("occam-watch-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("part.occam");
        let _ = std::fs::remove_file(&file);
        let mut watcher = Watcher::new(file.clone());
        let Looked::Refused(missing) = watcher.first() else { panic!("no model yet") };
        assert_eq!((missing[0].code, missing[0].remedies[0].kind), (Code::ModelMissing, RemedyKind::MakeModel));
        assert_eq!(watcher.look(), None, "reported once");
        let (digest, json) = boxed(1, Encoding::Json);
        std::fs::write(&file, &json).unwrap();
        let Some(Looked::Read(model)) = watcher.look() else { panic!("a model that reads whole") };
        assert_eq!(model.digest, digest);
        assert!(model.twin.starts_with(b"\x89occam"), "the binary twin, though the file is JSON");
        assert_eq!(watcher.look(), None, "nothing new");
        let (next, twin) = boxed(2, Encoding::Binary);
        std::fs::write(&file, twin.get(..twin.len() / 2).unwrap()).unwrap();
        assert_eq!(watcher.look(), None, "half-written: read again at the next look");
        assert_eq!(codes(watcher.look()), [Code::Damaged], "still half, so a damaged twin");
        std::fs::write(&file, &twin).unwrap();
        assert_eq!(watcher.look(), Some(Looked::Read(Model { digest: next, twin })));
        std::fs::write(&file, b"not a model").unwrap();
        assert_eq!(watcher.look(), None);
        assert_eq!(codes(watcher.look()), [Code::NotReadable]);
        std::fs::remove_file(&file).unwrap();
        assert_eq!(watcher.look(), None);
        assert_eq!(codes(watcher.look()), [Code::ModelMissing], "gone");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
