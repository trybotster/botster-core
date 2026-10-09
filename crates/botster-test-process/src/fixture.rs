//! The blocked fixture child: `/bin/cat` reading a FIFO whose only writer is the test. It waits without CPU (no sleep loop,
//! no spin), and it ends by itself when the test is gone: the test's death closes the writer, and `cat` reads the end of
//! file. A guard ends it earlier by a signal.
//!
//! The test opens a read end (non-blocking, so the open does not wait for a writer) and then the writer, and holds both. So a
//! `cat` that opens the FIFO while the test lives never blocks in its open, and bytes that the test sends before a `cat` opens
//! it wait in the FIFO (the test never reads its own end). A `cat` that opens the FIFO after the test died blocks in the open;
//! the group guard of the test (its anchor) ends it then. The FIFO operations of a fixture script
//! are external commands, never shell builtins (a signal interrupts a builtin's FIFO open).

use crate::OwnedChild;
use rustix::fs::{FileType, Mode, OFlags};
use std::io;
use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The FIFO of blocked fixture children and its two ends, which the test holds. Dropping it (or the test's death) ends every
/// `cat` that reads it.
#[derive(Debug)]
pub struct Blocker {
    path: PathBuf,
    /// The read end, held so that a write before any `cat` opens the FIFO waits in it, never fails.
    reader: Option<OwnedFd>,
    writer: Option<OwnedFd>,
}

impl Blocker {
    /// Makes the FIFO `name` in `dir` and opens its two ends. Both are close-on-exec: no child inherits them.
    ///
    /// # Errors
    /// The FIFO could not be made or opened.
    pub fn new(dir: &Path, name: &str) -> io::Result<Self> {
        let path = dir.join(name);
        // rustix has no `mkfifoat` on macOS: the external command makes the FIFO, a child owned with a bounded wait.
        let made = OwnedChild::spawn(
            Command::new("/usr/bin/mkfifo")
                .arg("-m")
                .arg("600")
                .arg(&path),
        )?
        .status();
        if !made.success() {
            return Err(io::Error::other(format!(
                "mkfifo {} failed: {made}",
                path.display()
            )));
        }
        if !rustix::fs::stat(&path)
            .is_ok_and(|stat| FileType::from_raw_mode(stat.st_mode) == FileType::Fifo)
        {
            return Err(io::Error::other(format!(
                "{} is not a FIFO",
                path.display()
            )));
        }
        let reader = rustix::fs::open(
            &path,
            OFlags::RDONLY | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        )?;
        let writer = rustix::fs::open(
            &path,
            OFlags::WRONLY | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        )?;
        Ok(Self {
            path,
            reader: Some(reader),
            writer: Some(writer),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The shell words of a blocked child, `/bin/cat '<fifo>'`, for a fixture script.
    pub fn shell(&self) -> String {
        format!("/bin/cat {}", quoted(&self.path))
    }

    /// A command of a blocked child: `/bin/cat <fifo>` with no input and no output, so it holds no pipe of the test.
    pub fn command(&self) -> Command {
        let mut command = Command::new("/bin/cat");
        command
            .arg(&self.path)
            .stdin(Stdio::null())
            .stdout(Stdio::null());
        command
    }

    /// Writes `bytes` to the FIFO. A blocked child that copies the FIFO to a pipe of the test passes them on, which proves that
    /// it has opened the FIFO: from then on, the test's death ends it.
    ///
    /// # Errors
    /// The writer was released, or the write failed or was short (a FIFO takes up to `PIPE_BUF` bytes at once).
    pub fn send(&mut self, bytes: &[u8]) -> io::Result<()> {
        let writer = self
            .writer
            .as_ref()
            .ok_or_else(|| io::Error::other("the blocker was released"))?;
        let written = rustix::io::write(writer, bytes)?;
        if written != bytes.len() {
            return Err(io::Error::other("a short write to the FIFO"));
        }
        Ok(())
    }

    /// Closes both ends: every blocked child reads the end of file and exits with code 0.
    pub fn release(&mut self) {
        self.writer = None;
        self.reader = None;
    }
}

/// `path` quoted for a POSIX shell.
pub fn quoted(path: &Path) -> String {
    format!("'{}'", path.display().to_string().replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_is_quoted_for_the_shell() {
        assert_eq!(quoted(Path::new("/a b/c")), "'/a b/c'");
        assert_eq!(quoted(Path::new("/it's")), "'/it'\\''s'");
    }
}
