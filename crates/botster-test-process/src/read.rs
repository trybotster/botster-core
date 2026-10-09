//! Bounded reads of a child's pipe, a FIFO or a socket: each read blocks in `poll` on the descriptor for at most the time
//! that is left before its deadline, so a silent writer fails the test with a clear message, never a hang. No thread is
//! started and none is left behind. (Not for a terminal device: macOS `poll` does not support one.)

use crate::Deadline;
use std::io::{self, Read};
use std::os::fd::AsFd;

/// How a bounded read failed.
#[derive(Debug)]
pub enum ReadError {
    /// The deadline passed first; the bytes read so far are kept.
    Deadline {
        limit: std::time::Duration,
        partial: Vec<u8>,
    },
    /// The read itself failed.
    Io(io::Error),
}

impl std::fmt::Display for ReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReadError::Deadline { limit, partial } => write!(
                f,
                "nothing ended the read within {limit:?} (read so far: {:?})",
                String::from_utf8_lossy(partial)
            ),
            ReadError::Io(error) => write!(f, "the read failed: {error}"),
        }
    }
}

impl std::error::Error for ReadError {}

/// A reader whose reads are bounded by deadlines. It buffers what it read past a line's end for the next line.
#[derive(Debug)]
pub struct Bounded<R> {
    reader: R,
    buffer: Vec<u8>,
    eof: bool,
}

impl<R: Read + AsFd> Bounded<R> {
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            buffer: Vec::new(),
            eof: false,
        }
    }

    /// Waits until the descriptor is readable (data, an end of file or an error), then reads once. Returns false at the end
    /// of file.
    fn fill(&mut self, deadline: Deadline) -> Result<bool, ReadError> {
        if self.eof {
            return Ok(false);
        }
        loop {
            let mut fds = [rustix::event::PollFd::new(
                &self.reader,
                rustix::event::PollFlags::IN,
            )];
            // timer: deadline — bounds the wait for the writer.
            match rustix::event::poll(&mut fds, Some(&deadline.timespec())) {
                Ok(0) => {
                    return Err(ReadError::Deadline {
                        limit: deadline.limit(),
                        partial: std::mem::take(&mut self.buffer),
                    })
                }
                Ok(_) => break,
                Err(rustix::io::Errno::INTR) => continue,
                Err(error) => return Err(ReadError::Io(error.into())),
            }
        }
        // `poll` reported the descriptor readable, so the read does not block, and a signal cannot interrupt it (EINTR).
        let mut chunk = [0; 4096];
        match self.reader.read(&mut chunk) {
            Ok(0) => {
                self.eof = true;
                Ok(false)
            }
            Ok(n) => {
                self.buffer.extend_from_slice(&chunk[..n]);
                Ok(true)
            }
            Err(error) => Err(ReadError::Io(error)),
        }
    }

    /// The next line, with its newline, read by `deadline`. `None` at the end of file with nothing left; a last line with no
    /// newline is returned as it is.
    ///
    /// # Errors
    /// The deadline passed first, or the read failed.
    pub fn line(&mut self, deadline: Deadline) -> Result<Option<String>, ReadError> {
        loop {
            if let Some(end) = self.buffer.iter().position(|&b| b == b'\n') {
                let line: Vec<u8> = self.buffer.drain(..=end).collect();
                return Ok(Some(String::from_utf8_lossy(&line).into_owned()));
            }
            if !self.fill(deadline)? {
                if self.buffer.is_empty() {
                    return Ok(None);
                }
                let line = std::mem::take(&mut self.buffer);
                return Ok(Some(String::from_utf8_lossy(&line).into_owned()));
            }
        }
    }

    /// Everything up to the end of file, read by `deadline`. The end of file of a pipe proves that every holder of its writing
    /// end has closed it or ended.
    ///
    /// # Errors
    /// The deadline passed first, or the read failed.
    pub fn to_eof(&mut self, deadline: Deadline) -> Result<Vec<u8>, ReadError> {
        while self.fill(deadline)? {}
        Ok(std::mem::take(&mut self.buffer))
    }

    /// The reader, with any buffered bytes dropped.
    pub fn into_inner(self) -> R {
        self.reader
    }
}

/// The first line of `reader`, read within the cleanup bound, and the reader for the rest.
///
/// # Panics
/// No full or partial line came within the bound, the reader ended first, or the read failed.
pub fn first_line<R: Read + AsFd>(reader: R) -> (Bounded<R>, String) {
    let mut bounded = Bounded::new(reader);
    match bounded.line(Deadline::cleanup()) {
        Ok(Some(line)) => (bounded, line),
        Ok(None) => panic!("the reader ended before its first line"),
        Err(error) => panic!("no first line: {error}"),
    }
}

/// Reads `reader` to its end of file within the cleanup bound: every holder of the pipe's writing end has closed it or ended.
///
/// # Panics
/// The end of file did not come within the bound, or the read failed.
pub fn eof<R: Read + AsFd>(reader: R) {
    if let Err(error) = Bounded::new(reader).to_eof(Deadline::cleanup()) {
        panic!("no end of file: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::time::Duration;

    #[test]
    fn lines_are_split_and_a_last_line_without_a_newline_is_kept() {
        let (reader, mut writer) = std::io::pipe().unwrap();
        writer.write_all(b"one\ntwo\nthree").unwrap();
        drop(writer);
        let mut reader = Bounded::new(reader);
        let deadline = Deadline::cleanup();
        assert_eq!(reader.line(deadline).unwrap().as_deref(), Some("one\n"));
        assert_eq!(reader.line(deadline).unwrap().as_deref(), Some("two\n"));
        assert_eq!(reader.line(deadline).unwrap().as_deref(), Some("three"));
        assert_eq!(reader.line(deadline).unwrap(), None);
        assert_eq!(reader.line(deadline).unwrap(), None);
    }

    #[test]
    fn a_silent_writer_fails_the_read_at_the_deadline_with_what_was_read() {
        let (reader, mut writer) = std::io::pipe().unwrap();
        writer.write_all(b"half").unwrap();
        let mut reader = Bounded::new(reader);
        let error = reader
            .line(Deadline::after(Duration::ZERO))
            .expect_err("a silent writer");
        assert_eq!(
            error.to_string(),
            "nothing ended the read within 0ns (read so far: \"half\")"
        );
        let error = reader
            .to_eof(Deadline::after(Duration::ZERO))
            .expect_err("still open");
        assert!(matches!(error, ReadError::Deadline { partial, .. } if partial.is_empty()));
        drop(writer);
    }

    #[test]
    fn the_end_of_file_returns_everything_after_the_last_line() {
        let (reader, mut writer) = std::io::pipe().unwrap();
        writer.write_all(b"a\nb\nc").unwrap();
        drop(writer);
        let (mut rest, line) = first_line(reader);
        assert_eq!(line, "a\n");
        assert_eq!(rest.to_eof(Deadline::cleanup()).unwrap(), b"b\nc");
        assert!(rest.to_eof(Deadline::cleanup()).unwrap().is_empty());
        drop(rest.into_inner());
    }

    #[test]
    fn a_reader_that_ends_before_its_first_line_fails_and_a_silent_one_fails_at_the_bound() {
        let (reader, writer) = std::io::pipe().unwrap();
        drop(writer);
        let failed = std::panic::catch_unwind(move || first_line(reader)).expect_err("ended");
        assert_eq!(
            failed.downcast_ref::<&str>().copied(),
            Some("the reader ended before its first line")
        );
        let (reader, writer) = std::io::pipe().unwrap();
        drop(writer);
        eof(reader);
    }

    #[test]
    fn eof_reads_everything_the_writer_wrote() {
        let (mut reader, mut writer) = std::io::pipe().unwrap();
        writer.write_all(b"left\nover").unwrap();
        drop(writer);
        eof(&mut reader);
        let mut rest = [0; 16];
        assert_eq!(reader.read(&mut rest).unwrap(), 0, "eof left bytes unread");
    }

    #[test]
    fn a_failed_read_is_an_io_error() {
        // A directory polls readable, and its read fails (EISDIR).
        let dir = tempfile::tempdir().unwrap();
        let error = Bounded::new(std::fs::File::open(dir.path()).unwrap())
            .line(Deadline::cleanup())
            .expect_err("a directory cannot be read");
        assert!(matches!(error, ReadError::Io(_)), "{error}");
        assert!(error.to_string().starts_with("the read failed: "));
    }
}
