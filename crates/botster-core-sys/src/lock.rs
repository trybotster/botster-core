//! `flock` on a lock file (plan 2.3, `Storage`).
//!
//! Clause: Core LC-2 (one host owns a data directory; a second `open` is refused while the lock is held).

use rustix::fs::{flock, FlockOperation};
use rustix::io::Errno;
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io;
use std::path::Path;

/// Why the lock was not taken.
#[derive(Debug)]
pub enum LockError {
    /// Another open file description holds the lock.
    Held,
    /// The file could not be opened or locked.
    Io(io::Error),
}

impl fmt::Display for LockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LockError::Held => f.write_str("the lock is held by another owner"),
            LockError::Io(error) => write!(f, "the lock file failed: {error}"),
        }
    }
}

impl std::error::Error for LockError {}

/// An exclusive lock on a file. The lock ends when the value is dropped, or when the process ends.
#[derive(Debug)]
pub struct LockFile {
    file: File,
}

impl LockFile {
    /// Opens `path` (created when missing) and takes an exclusive lock without waiting.
    ///
    /// The lock is `flock`: a second call on the same path conflicts, in this process and in any other.
    pub fn try_exclusive(path: &Path) -> Result<LockFile, LockError> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)
            .map_err(LockError::Io)?;
        match flock(&file, FlockOperation::NonBlockingLockExclusive) {
            Ok(()) => Ok(LockFile { file }),
            Err(Errno::WOULDBLOCK) => Err(LockError::Held),
            Err(errno) => Err(LockError::Io(errno.into())),
        }
    }

    /// The locked file, for the owner to write to (LC-2 keeps the host epoch under this lock).
    pub fn file(&self) -> &File {
        &self.file
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_lock_on_the_same_path_is_refused_while_the_first_is_held() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("lock");
        let first = LockFile::try_exclusive(&path).unwrap();
        assert!(matches!(
            LockFile::try_exclusive(&path),
            Err(LockError::Held)
        ));
        drop(first);
        assert!(LockFile::try_exclusive(&path).is_ok());
    }

    #[test]
    fn a_held_lock_and_an_io_failure_read_differently() {
        let held = LockError::Held.to_string();
        let io = LockError::Io(io::Error::other("boom")).to_string();
        assert!(!held.is_empty() && io.contains("boom"));
        assert_ne!(held, io);
    }

    #[test]
    fn different_paths_do_not_conflict() {
        let dir = tempfile::tempdir().unwrap();
        let _a = LockFile::try_exclusive(&dir.path().join("a")).unwrap();
        assert!(LockFile::try_exclusive(&dir.path().join("b")).is_ok());
    }

    #[test]
    fn a_missing_directory_is_an_io_error_not_a_held_lock() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("missing").join("lock");
        assert!(matches!(
            LockFile::try_exclusive(&path),
            Err(LockError::Io(_))
        ));
    }
}
