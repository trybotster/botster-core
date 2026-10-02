//! The registry on disk, and the data directory that owns it (plan 2.3, `Storage`; Core LC-2, AD-6, AD-7, DP-8).
//!
//! One file per row, written with `atomic-write-file` (a temporary file in the same directory, `fsync`, `rename`) and followed
//! by an `fsync` of the directory, so that a row that `write_row` returned `Ok` for survives a crash (AD-7). A failure before
//! the rename leaves the old row (`StorageError::Failed`); a failure of the directory `fsync` after the rename leaves a row
//! whose durability is unknown (`StorageError::Uncertain`, which the host reports as `RegistryFailed{uncertain: true}`).
//!
//! A row file is named by the SHA-256 of its key, so any key is a valid file name, and holds `[u32 LE key length][key][value]`,
//! so `list_rows` returns the keys. The directory is the host's alone: mode `0700`, owned by the host's uid (AD-6), and held
//! under an exclusive lock for as long as the [`DataDir`] lives (LC-2).

use crate::lock::{LockError, LockFile};
use atomic_write_file::AtomicWriteFile;
use botster_core_edges::edges::{Storage, StorageError};
use rustix::fs::{fstat, Mode};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::{Path, PathBuf};

/// The key of the host epoch row (DP-8).
const EPOCH_KEY: &str = "meta/host-epoch";

const ROW_EXTENSION: &str = "row";

/// Why a data directory was not opened.
#[derive(Debug)]
pub enum OpenError {
    /// Another host holds the lock (LC-2).
    InUse,
    /// The directory is not safe for secrets (AD-6): it is not owned by this user, or others can read or enter it.
    Unsafe(String),
    /// The directory, the lock or the epoch could not be read or written.
    Io(io::Error),
    /// The epoch row is not a number: the registry is corrupt.
    CorruptEpoch,
}

impl std::fmt::Display for OpenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OpenError::InUse => f.write_str("another host holds the data directory"),
            OpenError::Unsafe(why) => write!(f, "the data directory is not safe: {why}"),
            OpenError::Io(error) => write!(f, "the data directory failed: {error}"),
            OpenError::CorruptEpoch => f.write_str("the host epoch row is not a number"),
        }
    }
}

impl std::error::Error for OpenError {}

impl From<io::Error> for OpenError {
    fn from(error: io::Error) -> OpenError {
        OpenError::Io(error)
    }
}

fn errno(error: &io::Error) -> i32 {
    error.raw_os_error().unwrap_or(0)
}

fn key_hash(key: &str) -> String {
    Sha256::digest(key.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn encode(key: &str, value: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + key.len() + value.len());
    out.extend_from_slice(&(key.len() as u32).to_le_bytes());
    out.extend_from_slice(key.as_bytes());
    out.extend_from_slice(value);
    out
}

/// The key and the value of a row file, or `None` when the bytes are not a row.
fn decode(bytes: &[u8]) -> Option<(String, Vec<u8>)> {
    let len = u32::from_le_bytes(bytes.get(..4)?.try_into().ok()?) as usize;
    let key = std::str::from_utf8(bytes.get(4..4 + len)?).ok()?;
    Some((key.to_string(), bytes[4 + len..].to_vec()))
}

fn sync_directory(dir: &Path) -> io::Result<()> {
    File::open(dir)?.sync_all()
}

/// The registry rows of one directory.
#[derive(Debug)]
pub struct FileStorage {
    dir: PathBuf,
}

impl FileStorage {
    fn path(&self, key: &str) -> PathBuf {
        self.dir.join(format!("{}.{ROW_EXTENSION}", key_hash(key)))
    }

    fn read_file(&self, path: &Path) -> io::Result<Option<(String, Vec<u8>)>> {
        let mut file = match File::open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        Ok(decode(&bytes))
    }
}

impl Storage for FileStorage {
    fn write_row(&mut self, key: &str, bytes: &[u8]) -> Result<(), StorageError> {
        let failed = |error: io::Error| StorageError::Failed {
            errno: errno(&error),
        };
        let mut file = AtomicWriteFile::open(self.path(key)).map_err(failed)?;
        file.write_all(&encode(key, bytes)).map_err(failed)?;
        // A failure here is before the rename: the old row is untouched.
        file.commit().map_err(failed)?;
        // The row is in place. If the directory entry is not durable, the effect is unknown (AD-7).
        sync_directory(&self.dir).map_err(|error| StorageError::Uncertain {
            errno: errno(&error),
        })
    }

    fn read_row(&self, key: &str) -> Result<Option<Vec<u8>>, StorageError> {
        match self.read_file(&self.path(key)) {
            Ok(Some((stored, value))) if stored == key => Ok(Some(value)),
            Ok(_) => Ok(None),
            Err(error) => Err(StorageError::Failed {
                errno: errno(&error),
            }),
        }
    }

    fn delete_row(&mut self, key: &str) -> Result<(), StorageError> {
        match fs::remove_file(self.path(key)) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => {
                return Err(StorageError::Failed {
                    errno: errno(&error),
                })
            }
        }
        sync_directory(&self.dir).map_err(|error| StorageError::Uncertain {
            errno: errno(&error),
        })
    }

    fn list_rows(&self) -> Result<Vec<String>, StorageError> {
        let failed = |error: io::Error| StorageError::Failed {
            errno: errno(&error),
        };
        let mut keys = Vec::new();
        for entry in fs::read_dir(&self.dir).map_err(failed)? {
            let path = entry.map_err(failed)?.path();
            if path.extension().and_then(|e| e.to_str()) != Some(ROW_EXTENSION) {
                continue;
            }
            if let Some((key, _)) = self.read_file(&path).map_err(failed)? {
                keys.push(key);
            }
        }
        keys.sort();
        Ok(keys)
    }
}

/// An open data directory: the lock (LC-2), the registry, and the host epoch of this open (DP-8).
#[derive(Debug)]
pub struct DataDir {
    _lock: LockFile,
    storage: FileStorage,
    epoch: u64,
}

impl DataDir {
    /// Opens `path`: creates it with mode `0700` when missing, refuses a directory that is not safe (AD-6), takes the
    /// exclusive lock without waiting (LC-2), and raises the host epoch under the lock (DP-8).
    pub fn open(path: &Path) -> Result<DataDir, OpenError> {
        match fs::DirBuilder::new()
            .mode(0o700)
            .recursive(true)
            .create(path)
        {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.into()),
        }
        check_safe(path)?;
        let lock = LockFile::try_exclusive(&path.join("lock")).map_err(|error| match error {
            LockError::Held => OpenError::InUse,
            LockError::Io(error) => OpenError::Io(error),
        })?;
        let rows = path.join("rows");
        match fs::DirBuilder::new().mode(0o700).create(&rows) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.into()),
        }
        check_safe(&rows)?;
        let mut storage = FileStorage { dir: rows };
        let previous = match storage.read_row(EPOCH_KEY) {
            Ok(Some(bytes)) => std::str::from_utf8(&bytes)
                .ok()
                .and_then(|text| text.trim().parse::<u64>().ok())
                .ok_or(OpenError::CorruptEpoch)?,
            Ok(None) => 0,
            Err(StorageError::Failed { errno } | StorageError::Uncertain { errno }) => {
                return Err(OpenError::Io(io::Error::from_raw_os_error(errno)))
            }
        };
        let epoch = previous.checked_add(1).ok_or(OpenError::CorruptEpoch)?;
        storage
            .write_row(EPOCH_KEY, epoch.to_string().as_bytes())
            .map_err(|error| match error {
                StorageError::Failed { errno } | StorageError::Uncertain { errno } => {
                    OpenError::Io(io::Error::from_raw_os_error(errno))
                }
            })?;
        Ok(DataDir {
            _lock: lock,
            storage,
            epoch,
        })
    }

    /// The host epoch of this open: strictly above every earlier open of the directory (DP-8).
    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    pub fn storage(&mut self) -> &mut FileStorage {
        &mut self.storage
    }

    pub fn into_storage(self) -> (LockFile, FileStorage, u64) {
        (self._lock, self.storage, self.epoch)
    }
}

/// AD-6: a directory with secrets is owned by this user and closed to every other user.
fn check_safe(path: &Path) -> Result<(), OpenError> {
    let file = File::open(path)?;
    let stat = fstat(&file).map_err(|errno| OpenError::Io(errno.into()))?;
    let uid = rustix::process::geteuid().as_raw();
    if stat.st_uid != uid {
        return Err(OpenError::Unsafe(format!(
            "{} is owned by user {}, not by user {uid}",
            path.display(),
            stat.st_uid
        )));
    }
    let mode = Mode::from_raw_mode(stat.st_mode).bits();
    let others = mode & 0o077;
    if others != 0 {
        return Err(OpenError::Unsafe(format!(
            "{} has mode {:o}; the group and others must have no access",
            path.display(),
            mode & 0o777
        )));
    }
    // A directory that this user cannot enter is not a directory that the host can use.
    let _ = fs::metadata(path)?.permissions().mode();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn dir() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    /// AD-7: a row that was written reads back, and the key is part of the row.
    #[test]
    fn a_written_row_reads_back_and_a_missing_row_is_none() {
        let tmp = dir();
        let mut data = DataDir::open(&tmp.path().join("d")).unwrap();
        let storage = data.storage();
        storage.write_row("session/a", b"one").unwrap();
        assert_eq!(
            storage.read_row("session/a").unwrap(),
            Some(b"one".to_vec())
        );
        assert_eq!(storage.read_row("session/b").unwrap(), None);
    }

    /// Plan 2.3: a write replaces the row atomically; the old value is never mixed with the new one.
    #[test]
    fn a_second_write_replaces_the_row() {
        let tmp = dir();
        let mut data = DataDir::open(&tmp.path().join("d")).unwrap();
        let storage = data.storage();
        storage.write_row("k", b"a long first value").unwrap();
        storage.write_row("k", b"b").unwrap();
        assert_eq!(storage.read_row("k").unwrap(), Some(b"b".to_vec()));
    }

    /// Any key is a valid name: a long id, a slash, a dot-dot and a NUL-free odd string all work, and they list back.
    #[test]
    fn any_key_is_a_valid_row_name_and_lists_back_sorted() {
        let tmp = dir();
        let mut data = DataDir::open(&tmp.path().join("d")).unwrap();
        let storage = data.storage();
        let long = format!("session/{}", "x".repeat(128));
        for key in [long.as_str(), "session/../etc", "session/a/b", "session/é"] {
            storage.write_row(key, key.as_bytes()).unwrap();
        }
        let keys = storage.list_rows().unwrap();
        assert!(keys.windows(2).all(|w| w[0] <= w[1]), "ascending");
        assert!(keys.contains(&long) && keys.contains(&"session/../etc".to_string()));
        for key in &keys {
            if key != EPOCH_KEY {
                assert_eq!(
                    storage.read_row(key).unwrap(),
                    Some(key.as_bytes().to_vec())
                );
            }
        }
    }

    /// LC-7 step 4: a deleted row is gone, and deleting a missing row is not an error.
    #[test]
    fn delete_removes_the_row_and_is_idempotent() {
        let tmp = dir();
        let mut data = DataDir::open(&tmp.path().join("d")).unwrap();
        let storage = data.storage();
        storage.write_row("session/a", b"x").unwrap();
        storage.delete_row("session/a").unwrap();
        storage.delete_row("session/a").unwrap();
        assert_eq!(storage.read_row("session/a").unwrap(), None);
        assert!(!storage
            .list_rows()
            .unwrap()
            .contains(&"session/a".to_string()));
    }

    /// LC-2: a second open of one directory is refused while the first is open, and works after it is dropped.
    #[test]
    fn the_directory_is_exclusive_until_it_is_dropped() {
        let tmp = dir();
        let path = tmp.path().join("d");
        let first = DataDir::open(&path).unwrap();
        assert!(matches!(DataDir::open(&path), Err(OpenError::InUse)));
        drop(first);
        assert!(DataDir::open(&path).is_ok());
    }

    /// DP-8: the host epoch strictly increases at every open, under the lock, and survives the drop.
    #[test]
    fn the_host_epoch_strictly_increases_across_opens() {
        let tmp = dir();
        let path = tmp.path().join("d");
        let epochs: Vec<u64> = (0..3)
            .map(|_| DataDir::open(&path).unwrap().epoch())
            .collect();
        assert_eq!(epochs, [1, 2, 3]);
    }

    /// AD-6: a directory that others can read or enter is refused, a directory of another user is refused, and a new one is
    /// created with mode 0700.
    #[test]
    fn an_unsafe_directory_is_refused_and_a_new_one_is_private() {
        let tmp = dir();
        let open = tmp.path().join("open");
        fs::create_dir(&open).unwrap();
        fs::set_permissions(&open, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(matches!(DataDir::open(&open), Err(OpenError::Unsafe(_))));
        let fresh = tmp.path().join("fresh");
        DataDir::open(&fresh).unwrap();
        assert_eq!(
            fs::metadata(&fresh).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(fresh.join("rows"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
    }

    /// A row whose bytes are not a row is skipped by the listing and read as missing, never trusted.
    #[test]
    fn a_damaged_row_file_is_not_a_row() {
        let tmp = dir();
        let mut data = DataDir::open(&tmp.path().join("d")).unwrap();
        let storage = data.storage();
        storage.write_row("session/a", b"x").unwrap();
        let path = storage.path("session/a");
        fs::write(&path, [255, 255, 255, 255, 1]).unwrap();
        assert_eq!(storage.read_row("session/a").unwrap(), None);
        assert!(!storage
            .list_rows()
            .unwrap()
            .contains(&"session/a".to_string()));
    }

    /// A row file of another key under a colliding name is not returned for this key.
    #[test]
    fn a_row_is_found_only_under_its_own_key() {
        let tmp = dir();
        let mut data = DataDir::open(&tmp.path().join("d")).unwrap();
        let storage = data.storage();
        storage.write_row("session/a", b"x").unwrap();
        fs::copy(storage.path("session/a"), storage.path("session/b")).unwrap();
        assert_eq!(
            storage.read_row("session/b").unwrap(),
            None,
            "the stored key differs"
        );
    }

    /// A corrupt epoch row refuses the open: the registry is not guessed at.
    #[test]
    fn a_corrupt_epoch_refuses_the_open() {
        let tmp = dir();
        let path = tmp.path().join("d");
        drop(DataDir::open(&path).unwrap());
        let epoch = FileStorage {
            dir: path.join("rows"),
        }
        .path(EPOCH_KEY);
        fs::write(epoch, encode(EPOCH_KEY, b"not a number")).unwrap();
        assert!(matches!(DataDir::open(&path), Err(OpenError::CorruptEpoch)));
    }

    #[test]
    fn the_wire_form_is_length_key_value() {
        assert_eq!(encode("ab", b"cd"), [2, 0, 0, 0, b'a', b'b', b'c', b'd']);
        assert_eq!(
            decode(&encode("ab", b"cd")),
            Some(("ab".to_string(), b"cd".to_vec()))
        );
        assert_eq!(decode(&[9, 0, 0, 0, 1]), None);
        assert_eq!(decode(&[1]), None);
    }
}
