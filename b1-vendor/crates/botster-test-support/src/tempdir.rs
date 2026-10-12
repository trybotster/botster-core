//! A unique temp root with a SHORT canonical path.
//!
//! macOS limits a Unix socket path to about 104 bytes, and `$TMPDIR` there is already `/var/folders/xx/yyyy.../T/`
//! (about 49 bytes, and 57 once `/var` is canonicalized to `/private/var`). The root is `<base>/bt<pid>-<n>`
//! with `<base>` canonicalized: `/tmp` when it exists (`/private/tmp` on macOS), else `$TMPDIR`.
//! Each call returns its own directory (BUILD.md Testing rule 6); the directory is removed on drop.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

/// The most bytes of a Unix socket path on macOS (`sun_path` is 104 bytes with the terminating NUL).
pub const SOCKET_PATH_LIMIT: usize = 103;

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A temp root. Dropping it removes the directory and everything in it.
#[derive(Debug)]
pub struct TempRoot {
    path: PathBuf,
}

impl TempRoot {
    /// Creates a new empty root.
    pub fn new() -> io::Result<TempRoot> {
        let tmp = Path::new("/tmp");
        let base = if tmp.is_dir() {
            tmp.to_path_buf()
        } else {
            std::env::temp_dir()
        }
        .canonicalize()?;
        loop {
            // The counter makes a name unique inside this process, the pid across processes. A leftover directory of a
            // dead process with the same pid makes `create_dir` fail with `AlreadyExists`; the next number is tried.
            let n = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = base.join(format!("bt{}-{n}", std::process::id()));
            match std::fs::create_dir(&path) {
                Ok(()) => return Ok(TempRoot { path }),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e),
            }
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The path of a socket `name` in the root, or an error when it would not fit a Unix socket address.
    pub fn socket_path(&self, name: &str) -> io::Result<PathBuf> {
        let path = self.path.join(name);
        if path.as_os_str().len() > SOCKET_PATH_LIMIT {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "socket path of {} bytes is over {SOCKET_PATH_LIMIT}",
                    path.as_os_str().len()
                ),
            ));
        }
        Ok(path)
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // BUILD.md Testing rule 6: each test has its own canonical root, short enough for a socket.
    #[test]
    fn a_root_is_canonical_short_unique_and_removed_on_drop() {
        let a = TempRoot::new().unwrap();
        let b = TempRoot::new().unwrap();
        assert_ne!(a.path(), b.path());
        assert_eq!(a.path(), a.path().canonicalize().unwrap());
        assert!(!a.path().starts_with("/var"), "{:?}", a.path());
        // Leaves at least 60 bytes for a socket name under the 103-byte limit.
        assert!(a.path().as_os_str().len() <= 40, "{:?}", a.path());
        let sock = a.socket_path("hub.sock").unwrap();
        assert!(sock.as_os_str().len() <= SOCKET_PATH_LIMIT);
        assert!(a.socket_path(&"x".repeat(120)).is_err());
        let gone = a.path().to_path_buf();
        drop(a);
        assert!(!gone.exists());
    }
}
