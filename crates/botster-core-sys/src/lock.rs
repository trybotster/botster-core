//! The exclusive lock of a data directory (plan 2.3, `Storage`): a POSIX record lock (`fcntl`, `F_SETLK`) on a lock file,
//! and a table of the lock files that this process holds.
//!
//! Clause: Core LC-2 (one host owns a data directory; a second `open` is refused while the lock is held; the lock is
//! released when the handle is dropped).
//!
//! Why a record lock (id_2): a fork copies every descriptor of the process into the child until the child's exec closes
//! it. A `flock` belongs to the open file description, so a child that another thread was starting held the lock of a
//! dropped handle until its exec (263 to 698 ms in the real harness), and the next open was refused. A record lock belongs
//! to the process: a child never holds it, and the drop releases it at once. An open-file-description lock (`F_OFD_SETLK`)
//! would keep the `flock` defect, because a child shares the description.
//!
//! Two rules of a record lock, and how this module keeps them:
//! - A second lock of the same file in the same process does not conflict: the process holds the lock, not a descriptor.
//!   The table (by device and inode) refuses a second holder in this process.
//! - The process loses the lock when it closes ANY descriptor of the file. So this module never opens a file that the
//!   table holds: it reads the metadata of the path first, and a descriptor that it opened anyway (the path was replaced
//!   between the two) is kept with the holder until the holder's drop. A host must never open `<data dir>/lock`
//!   itself: a read, a copy or a snapshot that opens and closes it releases Core's lock. A stat of it is safe.
//!
//! The lock file is opened with `O_CLOEXEC` (as every `File` of std), so the image that a child execs has no copy.

use rustix::fs::{fcntl_lock, FlockOperation};
use rustix::io::Errno;
use std::collections::BTreeMap;
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io;
use std::os::unix::fs::MetadataExt as _;
use std::path::Path;
use std::sync::{Mutex, MutexGuard, PoisonError};

/// Why the lock was not taken.
#[derive(Debug)]
pub enum LockError {
    /// Another process, or another holder in this process, holds the lock.
    Held,
    /// The file could not be opened or locked, or the table of held lock files is poisoned.
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

/// A file by its device and inode.
type Key = (u64, u64);

/// The lock files that this process holds, by key, each with the descriptors of the file that must stay open until its
/// holder's drop (a close would release the holder's lock).
type Table = BTreeMap<Key, Vec<File>>;

/// The table of this process.
static HELD: Mutex<Table> = Mutex::new(BTreeMap::new());

/// An exclusive lock on a file. The lock ends when the value is dropped, or when the process ends.
#[derive(Debug)]
pub struct LockFile {
    /// The holder's descriptor; `None` only in the drop.
    file: Option<File>,
    key: Key,
    table: &'static Mutex<Table>,
}

impl LockFile {
    /// Opens `path` (created when missing) and takes an exclusive lock without waiting.
    ///
    /// A second call on the same file conflicts, in this process (by the table, also through another path of the same
    /// file) and in any other (by the record lock).
    pub fn try_exclusive(path: &Path) -> Result<LockFile, LockError> {
        try_exclusive_in(&HELD, path)
    }

    /// The locked file, for the owner to write to (LC-2 keeps the host epoch under this lock). Do not close a copy of
    /// it: the close of any descriptor of the file releases the lock.
    pub fn file(&self) -> &File {
        self.file
            .as_ref()
            .expect("the holder's descriptor closes only in the drop")
    }
}

impl Drop for LockFile {
    /// Closes the holder's descriptor, which releases the lock, and then the descriptors kept with it, under the table's
    /// mutex: no other open in this process sees the key free before the lock is released. A poisoned table is still
    /// cleared of this key: the lock is released either way, and every later open refuses the poisoned table.
    fn drop(&mut self) {
        let mut table = self.table.lock().unwrap_or_else(PoisonError::into_inner);
        drop(self.file.take());
        table.remove(&self.key);
    }
}

/// The table, or the `Io` error of a poisoned one: a panic interrupted a change to it, so it may not name every lock that
/// this process holds, and an open fails closed instead of treating a file as free.
fn table(table: &Mutex<Table>) -> Result<MutexGuard<'_, Table>, LockError> {
    table.lock().map_err(|_| {
        LockError::Io(io::Error::other(
            "the table of the lock files that this process holds is poisoned (a panic interrupted a change to it), so no \
             lock is taken",
        ))
    })
}

fn key_of(meta: &std::fs::Metadata) -> Key {
    (meta.dev(), meta.ino())
}

/// `LockFile::try_exclusive` with the table `held`. The whole open runs under its mutex.
fn try_exclusive_in(held: &'static Mutex<Table>, path: &Path) -> Result<LockFile, LockError> {
    let mut table = table(held)?;
    // A file that this process holds is never opened: the close of the new descriptor would release the lock. A path whose
    // metadata cannot be read (a missing file, which the open creates, or an error, which the open reports) goes on to
    // the open: a held file that it opens anyway is kept with its holder below.
    if let Ok(meta) = std::fs::metadata(path) {
        if table.contains_key(&key_of(&meta)) {
            return Err(LockError::Held);
        }
    }
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .map_err(LockError::Io)?;
    let key = key_of(&file.metadata().map_err(LockError::Io)?);
    if let Some(kept) = table.get_mut(&key) {
        // The path was replaced by a file that this process holds after the metadata was read: the new descriptor stays
        // open with the holder.
        kept.push(file);
        return Err(LockError::Held);
    }
    match fcntl_lock(&file, FlockOperation::NonBlockingLockExclusive) {
        Ok(()) => {}
        // POSIX gives either errno for a lock that another process holds. No holder of this file is in this process, so
        // the close of `file` releases nothing.
        Err(Errno::AGAIN | Errno::ACCESS) => return Err(LockError::Held),
        Err(errno) => return Err(LockError::Io(errno.into())),
    }
    table.insert(key, Vec::new());
    Ok(LockFile {
        file: Some(file),
        key,
        table: held,
    })
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

    /// A second open of a file that this process holds is refused before it opens the file (its close would release the
    /// lock), through the same path and through a hard link: no descriptor is kept with the holder.
    #[test]
    fn a_held_file_is_refused_without_an_open() {
        static TABLE: Mutex<Table> = Mutex::new(BTreeMap::new());
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("lock");
        let link = dir.path().join("link");
        let holder = try_exclusive_in(&TABLE, &path).unwrap();
        std::fs::hard_link(&path, &link).unwrap();
        for second in [&path, &link] {
            assert!(matches!(
                try_exclusive_in(&TABLE, second),
                Err(LockError::Held)
            ));
        }
        assert!(TABLE.lock().unwrap()[&holder.key].is_empty());
    }

    /// The path replaced, between the metadata and the open, by a file that this process holds: the new descriptor stays
    /// with the holder, the open is refused, and the drop of the holder closes both and frees the key.
    #[test]
    fn a_descriptor_of_a_held_file_stays_open_until_the_holders_drop() {
        static TABLE: Mutex<Table> = Mutex::new(BTreeMap::new());
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("lock");
        let holder = try_exclusive_in(&TABLE, &path).unwrap();
        let key = holder.key;
        let file = OpenOptions::new().read(true).open(&path).unwrap();
        TABLE.lock().unwrap().get_mut(&key).unwrap().push(file);
        assert_eq!(TABLE.lock().unwrap()[&key].len(), 1);
        drop(holder);
        assert!(TABLE.lock().unwrap().is_empty());
        assert!(try_exclusive_in(&TABLE, &path).is_ok());
    }

    /// A poisoned table fails closed: the open is an I/O error before it touches the file, never a free lock. The drop of a
    /// holder still releases its lock and its key.
    #[test]
    fn a_poisoned_table_refuses_every_open() {
        static TABLE: Mutex<Table> = Mutex::new(BTreeMap::new());
        let dir = tempfile::tempdir().unwrap();
        let held = dir.path().join("held");
        let holder = try_exclusive_in(&TABLE, &held).unwrap();
        let poison = std::thread::spawn(|| {
            let _table = TABLE.lock().unwrap();
            panic!("poison the table");
        });
        assert!(poison.join().is_err() && TABLE.is_poisoned());
        let path = dir.path().join("lock");
        let error = try_exclusive_in(&TABLE, &path).unwrap_err();
        assert!(
            matches!(&error, LockError::Io(io) if io.to_string().contains("poisoned")),
            "{error}"
        );
        assert!(!path.exists(), "a poisoned table opens no file");
        drop(holder);
        assert!(TABLE
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_empty());
    }
}
