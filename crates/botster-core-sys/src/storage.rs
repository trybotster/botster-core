//! The registry on disk, and the data directory that owns it (plan 2.3, `Storage`; Core LC-2, AD-6, AD-7, DP-8).
//!
//! One file per row, at a path that names its key ([`row_path`]): `<kind>/<components of the encoded id>.row`. A row's key
//! is never stored inside the file, so a damaged file still names its row (AD-1, AD-2: `RegistryCorrupt`), and a file whose
//! path does not decode was not written by Core: it is foreign, counted, and left alone.
//!
//! Every operation walks the path one component at a time, on directory descriptors (`openat`, `mkdirat`), so no joined
//! path is ever formed and neither `NAME_MAX` nor `PATH_MAX` limits an id. A confinement that checks every file operation
//! against its whole path (AppArmor, for example Docker's `docker-default` profile) still refuses a path above its own
//! limit, about 8 KiB: there, a write of a longer id fails with `StorageError::Failed` (`ENAMETOOLONG`) and has no effect
//! (lead ruling on #164, 2026-10-08; a ceiling of `CoreLimits.max_session_id_bytes` is an open steward question).
//!
//! A write is atomic: a temporary file in the row's directory, `fsync`, `renameat` over the row, then an `fsync` of the
//! directory; a directory that a write creates is synced in its parent (AD-7). An error before the rename has no effect
//! (`StorageError::Failed`): the temporary file and the directories that the write made are removed again, deepest first,
//! and only when empty. An error of the last directory sync leaves a row whose effect is unknown (`StorageError::Uncertain`,
//! which the host reports as `RegistryFailed{uncertain: true}`). A delete removes the row, then the directories that became
//! empty, best effort.
//!
//! The directory is the host's alone: mode `0700`, owned by the host's uid (AD-6), and held under an exclusive lock for as
//! long as the [`DataDir`] lives (LC-2).

mod row_path;

use crate::lock::{LockError, LockFile};
use botster_core_edges::edges::{Storage, StorageError};
use row_path::{is_dir_component, key_of, row_path, valid_kind, RowPath};
use rustix::fs::{
    fstat, mkdirat, openat, renameat, statat, unlinkat, AtFlags, Dir, FileType, Mode, OFlags,
};
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::os::unix::fs::DirBuilderExt;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

/// The key of the host epoch row (DP-8).
const EPOCH_KEY: &str = "meta/host-epoch";

/// The prefix of a temporary file of a write. A name with it never decodes as a row (`row_path::key_of`).
const TEMP_PREFIX: &str = ".tmp.";

/// Numbers the temporary files of this process; `O_EXCL` refuses a name that an earlier process left.
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

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

/// The errno of an I/O error. An error that the OS did not give one for is `EIO`: an input/output error, never a value that
/// looks like success.
fn errno(error: &io::Error) -> i32 {
    error
        .raw_os_error()
        .unwrap_or(rustix::io::Errno::IO.raw_os_error())
}

fn failed(error: impl Into<io::Error>) -> StorageError {
    StorageError::Failed {
        errno: errno(&error.into()),
    }
}

fn uncertain(error: impl Into<io::Error>) -> StorageError {
    StorageError::Uncertain {
        errno: errno(&error.into()),
    }
}

fn open_dir(parent: BorrowedFd<'_>, name: &str) -> io::Result<OwnedFd> {
    Ok(openat(
        parent,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )?)
}

/// Opens the directory `name` of `parent`, and creates it first when `create` is set. With `create`, the parent is synced
/// whether this call created the directory or an earlier, failed write did: the row below it survives a crash only when
/// every entry of its path is durable (AD-7). `None` when it does not exist and `create` is not set. `made` is set when this
/// call created the directory, before any later step of the call can fail.
fn child_dir(
    parent: BorrowedFd<'_>,
    name: &str,
    create: bool,
    made: &mut bool,
) -> io::Result<Option<OwnedFd>> {
    if create {
        match mkdirat(parent, name, Mode::from_raw_mode(0o700)) {
            Ok(()) => {
                *made = true;
                rustix::fs::fsync(parent)?;
            }
            Err(rustix::io::Errno::EXIST) => rustix::fs::fsync(parent)?,
            Err(error) => return Err(error.into()),
        }
    }
    match open_dir(parent, name) {
        Ok(fd) => Ok(Some(fd)),
        Err(error) if !create && error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// What a walk of the registry found: the keys of the rows, and the names that are not rows (foreign files).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Scan {
    pub keys: Vec<String>,
    pub foreign: usize,
}

/// The directories that one write created along a row's path. They are always consecutive and the deepest ones reached,
/// because a directory below a new one cannot exist before it: `count` directories that end at depth `end` (exclusive;
/// depth 0 is the kind). The deepest one may have no descriptor yet, when opening it failed.
#[derive(Debug, Default, Clone, Copy)]
struct Made {
    count: usize,
    end: usize,
}

/// The registry rows of one directory.
#[derive(Debug)]
pub struct FileStorage {
    dir: OwnedFd,
}

impl FileStorage {
    /// The directories of `path`, from the kind down to the row's own directory, into `chain`. `false` when one is missing
    /// and `create` is not set. `made` records the directories that this call created.
    fn chain(
        &self,
        path: &RowPath,
        create: bool,
        chain: &mut Vec<OwnedFd>,
        made: &mut Made,
    ) -> io::Result<bool> {
        for (depth, name) in std::iter::once(&path.kind).chain(&path.dirs).enumerate() {
            let parent = chain.last().map_or(self.dir.as_fd(), AsFd::as_fd);
            let mut new = false;
            let opened = child_dir(parent, name, create, &mut new);
            if new {
                made.count += 1;
                made.end = depth + 1;
            }
            match opened? {
                Some(fd) => chain.push(fd),
                None => return Ok(false),
            }
        }
        Ok(true)
    }

    /// The directories of `path` that exist, from the kind down, or `None` when one is missing.
    fn existing(&self, path: &RowPath) -> io::Result<Option<Vec<OwnedFd>>> {
        let mut chain = Vec::with_capacity(path.dirs.len() + 1);
        let complete = self.chain(path, false, &mut chain, &mut Made::default())?;
        Ok(complete.then_some(chain))
    }

    /// Removes the directories of `path` that a failed write created (`made`), deepest first, with `rmdir` only: a directory
    /// that is not empty stays, and so does every directory above it. Best effort: the write has failed already.
    fn unmake(&self, path: &RowPath, chain: &[OwnedFd], made: Made) {
        let names: Vec<&String> = std::iter::once(&path.kind).chain(&path.dirs).collect();
        for depth in (made.end - made.count..made.end).rev() {
            let parent = match depth {
                0 => self.dir.as_fd(),
                _ => chain[depth - 1].as_fd(),
            };
            if unlinkat(parent, names[depth].as_str(), AtFlags::REMOVEDIR).is_err() {
                break;
            }
            let _: rustix::io::Result<()> = rustix::fs::fsync(parent);
        }
    }

    /// Walks the registry: every path that decodes is a row, and every other name is foreign. The walk descends only into
    /// kind directories and full components, and never follows a link.
    pub fn scan(&self) -> Result<Scan, StorageError> {
        let mut scan = Scan::default();
        for (name, kind) in entries(self.dir.as_fd()).map_err(failed)? {
            match kind {
                FileType::Directory if valid_kind(&name) => {
                    let fd = open_dir(self.dir.as_fd(), &name).map_err(failed)?;
                    walk(fd.as_fd(), &name, &mut Vec::new(), &mut scan).map_err(failed)?;
                }
                _ => scan.foreign += 1,
            }
        }
        scan.keys.sort();
        Ok(scan)
    }
}

/// Puts the row `bytes` in place in the last directory of `chain`: a temporary file, `fsync`, then `renameat` over the row.
/// On an error the temporary file is removed and the row is as it was. Returns the row's directory, for its sync.
fn place<'a>(chain: &'a [OwnedFd], path: &RowPath, bytes: &[u8]) -> io::Result<BorrowedFd<'a>> {
    let dir = chain.last().expect("a kind directory at least").as_fd();
    let (temp, fd) = loop {
        let name = format!(
            "{TEMP_PREFIX}{}.{}",
            std::process::id(),
            TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        let flags =
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC;
        match openat(dir, name.as_str(), flags, Mode::from_raw_mode(0o600)) {
            Ok(fd) => break (name, fd),
            Err(rustix::io::Errno::EXIST) => continue,
            Err(error) => return Err(error.into()),
        }
    };
    let mut file = File::from(fd);
    let written = file.write_all(bytes).and_then(|()| file.sync_all());
    let renamed = written.and_then(|()| Ok(renameat(dir, temp.as_str(), dir, path.file.as_str())?));
    if let Err(error) = renamed {
        let _: rustix::io::Result<()> = unlinkat(dir, temp.as_str(), AtFlags::empty());
        return Err(error);
    }
    Ok(dir)
}

/// The names in a directory, with their types, not following links. `.` and `..` are left out.
fn entries(dir: BorrowedFd<'_>) -> io::Result<Vec<(String, FileType)>> {
    let mut out = Vec::new();
    for entry in Dir::read_from(dir)? {
        let entry = entry?;
        let Ok(name) = entry.file_name().to_str() else {
            out.push((String::new(), FileType::Unknown));
            continue;
        };
        if name == "." || name == ".." {
            continue;
        }
        let kind = FileType::from_raw_mode(statat(dir, name, AtFlags::SYMLINK_NOFOLLOW)?.st_mode);
        out.push((name.to_string(), kind));
    }
    Ok(out)
}

fn walk(
    dir: BorrowedFd<'_>,
    kind: &str,
    dirs: &mut Vec<String>,
    scan: &mut Scan,
) -> io::Result<()> {
    for (name, file_type) in entries(dir)? {
        match file_type {
            FileType::Directory if is_dir_component(&name) => {
                let fd = open_dir(dir, &name)?;
                dirs.push(name);
                walk(fd.as_fd(), kind, dirs, scan)?;
                dirs.pop();
            }
            FileType::RegularFile => {
                let path: Vec<&str> = dirs.iter().map(String::as_str).collect();
                match key_of(kind, &path, &name) {
                    Some(key) => scan.keys.push(key),
                    None => scan.foreign += 1,
                }
            }
            _ => scan.foreign += 1,
        }
    }
    Ok(())
}

impl Storage for FileStorage {
    fn write_row(&mut self, key: &str, bytes: &[u8]) -> Result<(), StorageError> {
        let path = row_path(key).ok_or_else(|| failed(rustix::io::Errno::INVAL))?;
        let mut chain = Vec::with_capacity(path.dirs.len() + 1);
        let mut made = Made::default();
        let placed = self
            .chain(&path, true, &mut chain, &mut made)
            .and_then(|_| place(&chain, &path, bytes));
        match placed {
            // The row is in place. If its directory entry is not durable, the effect is unknown (AD-7).
            Ok(dir) => rustix::fs::fsync(dir).map_err(uncertain),
            // Nothing took effect: the directories that this write made go too.
            Err(error) => {
                self.unmake(&path, &chain, made);
                Err(failed(error))
            }
        }
    }

    fn read_row(&self, key: &str) -> Result<Option<Vec<u8>>, StorageError> {
        let Some(path) = row_path(key) else {
            return Ok(None);
        };
        let Some(chain) = self.existing(&path).map_err(failed)? else {
            return Ok(None);
        };
        let dir = chain.last().expect("a kind directory at least").as_fd();
        let flags = OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
        let fd = match openat(dir, path.file.as_str(), flags, Mode::empty()) {
            Ok(fd) => fd,
            Err(rustix::io::Errno::NOENT) => return Ok(None),
            Err(error) => return Err(failed(error)),
        };
        let mut bytes = Vec::new();
        File::from(fd).read_to_end(&mut bytes).map_err(failed)?;
        Ok(Some(bytes))
    }

    fn delete_row(&mut self, key: &str) -> Result<(), StorageError> {
        let Some(path) = row_path(key) else {
            return Ok(());
        };
        let Some(chain) = self.existing(&path).map_err(failed)? else {
            return Ok(());
        };
        let dir = chain.last().expect("a kind directory at least").as_fd();
        match unlinkat(dir, path.file.as_str(), AtFlags::empty()) {
            Ok(()) | Err(rustix::io::Errno::NOENT) => {}
            Err(error) => return Err(failed(error)),
        }
        rustix::fs::fsync(dir).map_err(uncertain)?;
        // The directories that became empty go too, deepest first, best effort: a leftover directory is not a row.
        for (depth, name) in path.dirs.iter().enumerate().rev() {
            let parent = chain[depth].as_fd();
            if unlinkat(parent, name.as_str(), AtFlags::REMOVEDIR).is_err() {
                break;
            }
            let _: rustix::io::Result<()> = rustix::fs::fsync(parent);
        }
        Ok(())
    }

    fn list_rows(&self) -> Result<Vec<String>, StorageError> {
        self.scan().map(|scan| scan.keys)
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
    ///
    /// Two requirements on the host (AD-7, lead ruling on integration finding K4):
    /// - The parent of `path` exists. Core creates only `path` itself; a missing parent fails the open with an I/O error.
    /// - The parent of `path` is readable, because Core syncs it on every open. A parent that cannot be opened fails the open.
    ///
    /// Core syncs `path` and its parent and no other ancestor: the durability of the parent's own entry is the host's.
    pub fn open(path: &Path) -> Result<DataDir, OpenError> {
        create_data_dir(path, &mut sync_dir)?;
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
        // The entry of `rows` in the data directory is durable before any row is written below it (AD-7), on the first
        // open and on a retry after an open whose sync failed. This is the sync of `path` on every open.
        sync_dir(path)?;
        check_safe(&rows)?;
        let dir = File::open(&rows)?.into();
        let mut storage = FileStorage { dir };
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

/// Creates the data directory `path` with mode `0700` when it is missing, refuses it when it is not safe (AD-6; a file is
/// unsafe), then syncs its parent with `sync`, so that the entry of `path` is durable (AD-7). Only `path` itself is created: a missing parent fails with `NotFound`. The parent is
/// synced on every open, whether this open created `path` or an earlier, failed open did. The parent is opened through
/// `path/..`, so it is the directory that holds the entry, and it must be readable: a parent that cannot be opened fails the
/// open, which never claims a durability that it does not have. No other ancestor is synced: the host owns them. `sync` is
/// [`sync_dir`] in production, and an injected one in a test.
fn create_data_dir(
    path: &Path,
    sync: &mut dyn FnMut(&Path) -> io::Result<()>,
) -> Result<(), OpenError> {
    match fs::DirBuilder::new().mode(0o700).create(path) {
        // A path that exists as a file is refused by `check_safe`, as unsafe.
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error.into()),
    }
    check_safe(path)?;
    Ok(sync(&path.join(".."))?)
}

/// Syncs the directory `dir`.
fn sync_dir(dir: &Path) -> io::Result<()> {
    File::open(dir)?.sync_all()
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
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The errno of an OS error is kept, and an error with none is `EIO`, never 0 (integration finding K3).
    #[test]
    fn an_errno_is_kept_and_a_missing_one_is_eio() {
        assert_eq!(errno(&io::Error::from_raw_os_error(13)), 13);
        assert_eq!(
            errno(&io::Error::other("no code")),
            rustix::io::Errno::IO.raw_os_error()
        );
    }

    /// Every error of the open has its own words.
    #[test]
    fn open_errors_say_what_failed() {
        assert_eq!(
            OpenError::InUse.to_string(),
            "another host holds the data directory"
        );
        assert_eq!(
            OpenError::Unsafe("mode".into()).to_string(),
            "the data directory is not safe: mode"
        );
        assert!(OpenError::Io(io::Error::other("boom"))
            .to_string()
            .starts_with("the data directory failed: boom"));
        assert_eq!(
            OpenError::CorruptEpoch.to_string(),
            "the host epoch row is not a number"
        );
    }
}

/// The tests that write to a real disk (the fsync of a row, the lock, the epoch) run in the slow tier (BUILD.md testing rule 2):
/// their time is the time of the disk of the host, and a busy host breaks the budget of the default tier.
#[cfg(test)]
#[cfg(feature = "slow")]
mod slow_tests {
    use super::row_path::COMPONENT_CHARS;
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    fn dir() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    /// The joined path of a row, for a test that damages or inspects the file itself (the storage never joins paths).
    fn file_of(rows: &Path, key: &str) -> PathBuf {
        let path = row_path(key).unwrap();
        let mut out = rows.join(&path.kind);
        for dir in &path.dirs {
            out.push(dir);
        }
        out.join(&path.file)
    }

    /// AD-7: a row that was written reads back, and a missing row is none.
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
        storage
            .write_row("session/k", b"a long first value")
            .unwrap();
        storage.write_row("session/k", b"b").unwrap();
        assert_eq!(storage.read_row("session/k").unwrap(), Some(b"b".to_vec()));
    }

    /// Linux's `PATH_MAX` (`limits.h`), the longer of the two: macOS's is 1024.
    const PATH_MAX: usize = 4096;

    /// The longest full path that a process confined by AppArmor can name. AppArmor checks a file operation against its
    /// whole path, so a walk on directory descriptors does not get past it: in the Linux gate container (profile
    /// `docker-default`), `mkdirat` of 200-character names fails with `ENAMETOOLONG` at depth 41. Lead ruling on #164
    /// (2026-10-08); a ceiling of `CoreLimits.max_session_id_bytes` is an open steward question.
    const CONFINED_PATH_BYTES: usize = 8192;

    /// The length in bytes of an id whose code takes at least `chars` characters (5 bytes are 8 characters of base32).
    fn id_bytes_for_code(chars: usize) -> usize {
        chars.div_ceil(8) * 5
    }

    /// Lead ruling on A1: any id is a row, whatever its length: an id of 128 bytes (the default `max_session_id_bytes`)
    /// and ids far above it, whose paths take several directories, write, read back and list back sorted, and the listing
    /// finds nothing foreign. The longest one's path exceeds `PATH_MAX` and stays under `CONFINED_PATH_BYTES` (lead ruling
    /// on #164), so it holds under a confinement too.
    #[test]
    fn any_id_is_a_row_and_lists_back_sorted() {
        let tmp = dir();
        let rows = tmp.path().join("d").join("rows");
        let mut data = DataDir::open(&tmp.path().join("d")).unwrap();
        let storage = data.storage();
        let past_path_max = id_bytes_for_code(PATH_MAX + COMPONENT_CHARS);
        let keys: Vec<String> = [0usize, 128, 129, 1000, past_path_max]
            .iter()
            .map(|n| format!("session/{}", "x".repeat(*n)))
            .chain(["session/../etc".to_string(), "session/é".to_string()])
            .collect();
        let longest = file_of(&rows, &format!("session/{}", "x".repeat(past_path_max)))
            .as_os_str()
            .len();
        assert!(
            (PATH_MAX + 1..CONFINED_PATH_BYTES).contains(&longest),
            "{longest}"
        );
        for key in &keys {
            storage.write_row(key, key.as_bytes()).unwrap();
        }
        let scan = storage.scan().unwrap();
        assert_eq!(scan.foreign, 0);
        let mut expected = keys.clone();
        expected.push(EPOCH_KEY.to_string());
        expected.sort();
        assert_eq!(scan.keys, expected);
        for key in &keys {
            assert_eq!(
                storage.read_row(key).unwrap(),
                Some(key.as_bytes().to_vec())
            );
        }
    }

    /// LC-7 step 4: a deleted row is gone, the directories of its path that became empty go too, and deleting a missing
    /// row is not an error.
    #[test]
    fn delete_removes_the_row_and_its_empty_directories() {
        let tmp = dir();
        let rows = tmp.path().join("d").join("rows");
        let mut data = DataDir::open(&tmp.path().join("d")).unwrap();
        let storage = data.storage();
        let long = format!("session/{}", "y".repeat(1000));
        storage.write_row(&long, b"x").unwrap();
        storage.write_row("session/short", b"x").unwrap();
        storage.delete_row(&long).unwrap();
        storage.delete_row(&long).unwrap();
        assert_eq!(storage.read_row(&long).unwrap(), None);
        // Only the short row's file is left in the kind directory: no directory of the long path remains.
        let left: Vec<String> = fs::read_dir(rows.join("session"))
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        assert_eq!(left, vec![row_path("session/short").unwrap().file]);
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

    /// AD-6: a directory that others can read or enter is refused, and a new one is created with mode 0700.
    #[test]
    fn an_unsafe_directory_is_refused_and_a_new_one_is_private() {
        let tmp = dir();
        let open = tmp.path().join("open");
        fs::create_dir(&open).unwrap();
        fs::set_permissions(&open, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(matches!(DataDir::open(&open), Err(OpenError::Unsafe(_))));
        let fresh = tmp.path().join("fresh");
        DataDir::open(&fresh).unwrap();
        for path in [fresh.clone(), fresh.join("rows")] {
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o700
            );
        }
    }

    /// AD-1, AD-2, A10-2: a damaged row file is still the row of the key that its path names: it lists, and it reads back
    /// as the damaged bytes, for the host's decoder to refuse.
    #[test]
    fn a_damaged_row_is_still_named_by_its_path() {
        let tmp = dir();
        let rows = tmp.path().join("d").join("rows");
        let mut data = DataDir::open(&tmp.path().join("d")).unwrap();
        let storage = data.storage();
        storage.write_row("session/a", b"a row").unwrap();
        fs::write(file_of(&rows, "session/a"), [255, 0, 1]).unwrap();
        assert!(storage
            .list_rows()
            .unwrap()
            .contains(&"session/a".to_string()));
        assert_eq!(
            storage.read_row("session/a").unwrap(),
            Some(vec![255, 0, 1])
        );
    }

    /// Lead ruling on A1: a name that does not decode was not written by Core: it is counted as foreign, left on disk, and
    /// not listed; the rows around it list as before.
    #[test]
    fn a_foreign_file_is_counted_untouched_and_not_listed() {
        let tmp = dir();
        let rows = tmp.path().join("d").join("rows");
        let mut data = DataDir::open(&tmp.path().join("d")).unwrap();
        let storage = data.storage();
        storage.write_row("session/a", b"a row").unwrap();
        let foreign = [
            rows.join("notes.txt"),
            rows.join("session").join("NOT-BASE32.row"),
        ];
        for path in &foreign {
            fs::write(path, b"someone else's").unwrap();
        }
        let scan = storage.scan().unwrap();
        assert_eq!(scan.foreign, foreign.len());
        assert_eq!(
            scan.keys,
            vec![EPOCH_KEY.to_string(), "session/a".to_string()]
        );
        for path in &foreign {
            assert_eq!(fs::read(path).unwrap(), b"someone else's");
        }
    }

    /// Lead ruling on #164 (2026-10-08), AD-7: an id whose path is twice `CONFINED_PATH_BYTES` long. Where nothing limits the
    /// path (macOS, unconfined Linux), the row writes, reads back and lists. Where a confinement limits it (AppArmor), the
    /// write fails with `Failed`, never `Uncertain`, lists nothing and leaves no directory. The test prints which branch ran.
    #[test]
    fn an_id_past_a_confined_path_limit_writes_or_fails_cleanly() {
        let tmp = dir();
        let rows = tmp.path().join("d").join("rows");
        let mut data = DataDir::open(&tmp.path().join("d")).unwrap();
        let storage = data.storage();
        let key = format!(
            "long/{}",
            "x".repeat(id_bytes_for_code(2 * CONFINED_PATH_BYTES))
        );
        match storage.write_row(&key, b"x") {
            Ok(()) => {
                println!("branch: written; no confinement limits the path");
                assert_eq!(storage.read_row(&key).unwrap(), Some(b"x".to_vec()));
                assert!(storage.list_rows().unwrap().contains(&key));
            }
            Err(error) => {
                println!("branch: refused with {error:?}; a confinement limits the path");
                assert!(matches!(error, StorageError::Failed { .. }), "{error:?}");
                assert_eq!(
                    storage.scan().unwrap(),
                    Scan {
                        keys: vec![EPOCH_KEY.to_string()],
                        foreign: 0
                    }
                );
                assert!(!rows.join("long").exists(), "no directory is left");
            }
        }
    }

    /// AD-7, lead ruling on A1: a write whose path is blocked by a file that Core did not write (a regular file where a
    /// directory of the path must be) fails with `Failed`, never `Uncertain`; no row is listed, and the file is left as it
    /// was and counted as foreign.
    #[test]
    fn a_write_blocked_by_a_foreign_file_fails_and_leaves_the_file() {
        let tmp = dir();
        let rows = tmp.path().join("d").join("rows");
        let mut data = DataDir::open(&tmp.path().join("d")).unwrap();
        let storage = data.storage();
        let key = format!(
            "session/{}",
            "x".repeat(id_bytes_for_code(3 * COMPONENT_CHARS))
        );
        let path = row_path(&key).unwrap();
        assert!(path.dirs.len() >= 2, "{path:?}");
        // The first directory exists; a file stands where the second one must go.
        let first = rows.join(&path.kind).join(&path.dirs[0]);
        fs::create_dir_all(&first).unwrap();
        let blocker = first.join(&path.dirs[1]);
        fs::write(&blocker, b"someone else's").unwrap();
        match storage.write_row(&key, b"x") {
            Err(StorageError::Failed { .. }) => {}
            other => panic!("{other:?}"),
        }
        let scan = storage.scan().unwrap();
        assert_eq!(scan.keys, vec![EPOCH_KEY.to_string()]);
        assert_eq!(scan.foreign, 1);
        assert_eq!(fs::read(&blocker).unwrap(), b"someone else's");
    }

    /// AD-7 (lead ruling on #164, 2026-10-08): a write that fails after it made directories of its path removes them again,
    /// deepest first. The descriptor limit is lowered so that exactly two more descriptors can open: the kind and the first
    /// directory open, the second directory is made (`mkdirat` takes no descriptor), and opening it fails with `EMFILE`.
    /// nextest runs each test in its own process, so the lowered limit reaches no other test; it is restored before the
    /// asserts.
    #[test]
    fn a_failed_write_removes_the_directories_that_it_made() {
        use rustix::process::{getrlimit, setrlimit, Resource};
        use std::os::fd::AsRawFd;
        let tmp = dir();
        let rows = tmp.path().join("d").join("rows");
        let mut data = DataDir::open(&tmp.path().join("d")).unwrap();
        let storage = data.storage();
        let key = format!(
            "made/{}",
            "x".repeat(id_bytes_for_code(3 * COMPONENT_CHARS))
        );
        assert!(row_path(&key).unwrap().dirs.len() >= 2);
        let saved = getrlimit(Resource::Nofile);
        // The lowest free descriptor: every lower one is open, so the bound admits exactly two more.
        let next = File::open(tmp.path()).unwrap().as_raw_fd();
        let bound = u64::try_from(next).unwrap() + 2;
        setrlimit(
            Resource::Nofile,
            rustix::process::Rlimit {
                current: Some(bound),
                maximum: saved.maximum,
            },
        )
        .unwrap();
        let written = storage.write_row(&key, b"x");
        setrlimit(Resource::Nofile, saved).unwrap();
        match written {
            Err(StorageError::Failed { errno }) => {
                assert_eq!(errno, rustix::io::Errno::MFILE.raw_os_error())
            }
            other => panic!("{other:?}"),
        }
        assert!(!rows.join("made").exists(), "no directory is left");
        assert_eq!(storage.list_rows().unwrap(), vec![EPOCH_KEY.to_string()]);
    }

    /// Asserts that an open was refused with `EACCES`. Root ignores the permission bits that make the refusal, so for root
    /// the check is skipped, with the reason printed.
    fn assert_permission_denied(result: Result<DataDir, OpenError>) {
        if rustix::process::geteuid().is_root() {
            eprintln!("skipped: root ignores the permission bits of this case");
            return;
        }
        match result {
            Err(OpenError::Io(error)) => assert_eq!(error.kind(), io::ErrorKind::PermissionDenied),
            other => panic!("{other:?}"),
        }
    }

    /// Lead ruling on K4: Core creates only the data directory, so a missing parent fails the open with the I/O error of the
    /// system, and nothing is created.
    #[test]
    fn a_missing_parent_fails_the_open() {
        let tmp = dir();
        let parent = tmp.path().join("missing");
        match DataDir::open(&parent.join("d")) {
            Err(OpenError::Io(error)) => assert_eq!(error.kind(), io::ErrorKind::NotFound),
            other => panic!("{other:?}"),
        }
        assert!(!parent.exists());
    }

    /// Lead ruling on K4: the parent is synced on every open, so a parent that the host may write and enter but not read
    /// fails the open: Core never claims a durability that it does not have.
    #[test]
    fn a_parent_that_cannot_be_read_fails_the_open() {
        let tmp = dir();
        let parent = tmp.path().join("p");
        fs::create_dir(&parent).unwrap();
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o300)).unwrap();
        let result = DataDir::open(&parent.join("d"));
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o700)).unwrap();
        assert_permission_denied(result);
    }

    /// AD-7 (review finding P5-F9, lead ruling on K4): an open whose parent sync fails leaves the data directory that it
    /// created; the retry finds the directory and syncs the parent again, so the entry becomes durable.
    #[test]
    fn a_retried_open_syncs_the_parent_that_a_failed_open_left() {
        let tmp = dir();
        let path = tmp.path().join("d");
        let parent = fs::canonicalize(tmp.path()).unwrap();
        let synced_dir = |dir: &Path| fs::canonicalize(dir).unwrap();
        let mut failing = |dir: &Path| {
            assert_eq!(synced_dir(dir), parent);
            Err(io::Error::from_raw_os_error(
                rustix::io::Errno::IO.raw_os_error(),
            ))
        };
        assert!(create_data_dir(&path, &mut failing).is_err());
        assert!(path.is_dir(), "the failed open left its directory");
        let mut synced = Vec::new();
        let mut recording = |dir: &Path| {
            synced.push(synced_dir(dir));
            sync_dir(dir)
        };
        create_data_dir(&path, &mut recording).unwrap();
        assert_eq!(synced, vec![parent]);
    }

    /// Lead ruling on K4: no ancestor above the parent is synced, so a grandparent that the host may only enter (`0100`)
    /// does not fail the open.
    #[test]
    fn an_execute_only_grandparent_does_not_fail_the_open() {
        let tmp = dir();
        let grandparent = tmp.path().join("g");
        let parent = grandparent.join("p");
        fs::create_dir_all(&parent).unwrap();
        fs::set_permissions(&grandparent, fs::Permissions::from_mode(0o100)).unwrap();
        let result = DataDir::open(&parent.join("d")).map(drop);
        fs::set_permissions(&grandparent, fs::Permissions::from_mode(0o700)).unwrap();
        if let Err(error) = result {
            panic!("the open failed: {error}");
        }
    }

    /// A corrupt epoch row refuses the open: the registry is not guessed at.
    #[test]
    fn a_corrupt_epoch_refuses_the_open() {
        let tmp = dir();
        let path = tmp.path().join("d");
        drop(DataDir::open(&path).unwrap());
        fs::write(file_of(&path.join("rows"), EPOCH_KEY), b"not a number").unwrap();
        assert!(matches!(DataDir::open(&path), Err(OpenError::CorruptEpoch)));
    }

    /// AD-7: a write that cannot create its temporary file has no effect and says so (`Failed`).
    #[test]
    fn a_write_that_cannot_start_is_failed_with_no_effect() {
        let tmp = dir();
        let rows = tmp.path().join("d").join("rows");
        let mut data = DataDir::open(&tmp.path().join("d")).unwrap();
        let storage = data.storage();
        storage.write_row("session/a", b"old").unwrap();
        fs::set_permissions(rows.join("session"), fs::Permissions::from_mode(0o500)).unwrap();
        let result = storage.write_row("session/a", b"new");
        fs::set_permissions(rows.join("session"), fs::Permissions::from_mode(0o700)).unwrap();
        match result {
            Err(StorageError::Failed { errno }) => assert_ne!(errno, 0),
            // A user that ignores permissions (root) writes the row.
            Ok(()) => return,
            Err(other) => panic!("{other:?}"),
        }
        assert_eq!(
            storage.read_row("session/a").unwrap(),
            Some(b"old".to_vec())
        );
    }

    /// An error that is not "not found" is an error: a row path that is a directory cannot be read or deleted; a missing
    /// row is `None`, and its delete is quiet.
    #[test]
    fn only_a_missing_row_is_none_or_deleted_quietly() {
        let tmp = dir();
        let rows = tmp.path().join("d").join("rows");
        let mut data = DataDir::open(&tmp.path().join("d")).unwrap();
        let storage = data.storage();
        storage.write_row("session/other", b"x").unwrap();
        fs::create_dir(file_of(&rows, "session/x")).unwrap();
        assert!(storage.read_row("session/x").is_err());
        assert!(matches!(
            storage.delete_row("session/x"),
            Err(StorageError::Failed { .. })
        ));
        assert_eq!(storage.read_row("session/none").unwrap(), None);
        assert!(storage.delete_row("session/none").is_ok());
    }

    /// AD-6: the data directory that exists as a file is unsafe, not an I/O error; one that cannot be created is the I/O error
    /// of the system, with its own kind.
    #[test]
    fn open_tells_an_existing_file_from_a_failed_create() {
        let tmp = dir();
        let file = tmp.path().join("file");
        fs::write(&file, b"x").unwrap();
        assert!(
            matches!(DataDir::open(&file), Err(OpenError::Unsafe(_))),
            "a file"
        );
        let locked = tmp.path().join("locked");
        fs::create_dir(&locked).unwrap();
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o500)).unwrap();
        let result = DataDir::open(&locked.join("d"));
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o700)).unwrap();
        assert_permission_denied(result);
    }

    /// The rows directory that cannot be created is the I/O error of the system.
    #[test]
    fn open_reports_a_rows_directory_that_cannot_be_created() {
        let tmp = dir();
        let base = tmp.path().join("d");
        fs::DirBuilder::new().mode(0o700).create(&base).unwrap();
        fs::write(base.join("lock"), b"").unwrap();
        fs::set_permissions(&base, fs::Permissions::from_mode(0o500)).unwrap();
        let result = DataDir::open(&base);
        fs::set_permissions(&base, fs::Permissions::from_mode(0o700)).unwrap();
        assert_permission_denied(result);
        // A rows path that is a file is unsafe.
        let base = tmp.path().join("e");
        fs::DirBuilder::new().mode(0o700).create(&base).unwrap();
        fs::write(base.join("rows"), b"x").unwrap();
        assert!(matches!(DataDir::open(&base), Err(OpenError::Unsafe(_))));
    }
}
