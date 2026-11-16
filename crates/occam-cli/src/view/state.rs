//! Source: original
//! The state file beside a model, `<model>.view`, which names its viewer's process, port and token, so that a second
//! `occam view` finds the running one (docs/architecture/live-view.md#the-server). It is written whole, through a file
//! beside it renamed over it, readable only by its owner where the system has owners; text that names no viewer is
//! stale. And the token: 128 bits from the standard library's randomly keyed hasher.

use std::hash::{BuildHasher, RandomState};
use std::path::{Path, PathBuf};

use occam::Viewer;

use crate::commands::beside;

/// The state file of `model`.
pub fn path(model: &Path) -> PathBuf {
    beside(model, ".view")
}

/// The viewer the state file `file` names, if it is there and names one.
pub fn read(file: &Path) -> Option<Viewer> {
    Viewer::from_json(&std::fs::read_to_string(file).ok()?)
}

/// Writes `viewer` to the state file `file`, whole or not at all.
///
/// # Errors
///
/// What the system said when the file or the one beside it could not be written or renamed.
pub fn write(file: &Path, viewer: &Viewer) -> std::io::Result<()> {
    let writing = beside(file, ".writing");
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let written = options.open(&writing).and_then(|mut opened| {
        std::io::Write::write_all(&mut opened, viewer.json().as_bytes())?;
        opened.sync_all()
    });
    let renamed = written.and_then(|()| std::fs::rename(&writing, file));
    if renamed.is_err() {
        let _ = std::fs::remove_file(&writing);
    }
    renamed
}

/// Removes the state file `file` if it still names the viewer whose token is `token`.
pub fn remove_if_ours(file: &Path, token: &str) {
    if read(file).is_some_and(|viewer| viewer.token == token) {
        let _ = std::fs::remove_file(file);
    }
}

/// Whether the state file `file` names another viewer than the one whose token is `token`: then that one is the
/// model's, and this one should stop.
pub fn taken(file: &Path, token: &str) -> bool {
    read(file).is_some_and(|viewer| viewer.token != token)
}

/// A new token: 32 hexadecimal digits that no other page on the machine can guess.
pub fn token() -> String {
    let seed = (std::process::id(), std::time::SystemTime::now());
    let (first, second) = (RandomState::new().hash_one(seed), RandomState::new().hash_one(seed));
    format!("{first:016x}{second:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_state_file_is_written_whole_read_back_and_removed_only_by_its_own_viewer() {
        let dir = std::env::temp_dir().join(format!("occam-state-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let file = path(&dir.join("part.occam"));
        assert!(file.ends_with("part.occam.view"));
        let viewer = Viewer { pid: 7, port: 40000, token: token() };
        assert_eq!(viewer.token.len(), 32);
        assert_ne!(viewer.token, token(), "a new token each time");
        write(&file, &viewer).unwrap();
        assert_eq!(read(&file), Some(viewer.clone()));
        assert!(!dir.join("part.occam.view.writing").exists(), "the file beside it is gone");
        assert!(!taken(&file, &viewer.token), "its own token");
        assert!(taken(&file, "another"));
        remove_if_ours(&file, "another");
        assert!(file.exists(), "another viewer's token removes nothing");
        remove_if_ours(&file, &viewer.token);
        assert_eq!(read(&file), None);
        assert!(!taken(&file, "another"), "no file names no other viewer");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
