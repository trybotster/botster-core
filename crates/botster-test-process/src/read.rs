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

    /// The error of a read whose deadline passed, with the bytes read so far.
    fn passed(&mut self, deadline: Deadline) -> ReadError {
        ReadError::Deadline {
            limit: deadline.limit(),
            partial: std::mem::take(&mut self.buffer),
        }
    }

    /// Waits until the descriptor is readable (data, an end of file or an error), then reads once. Returns false at the end
    /// of file.
    fn fill(&mut self, deadline: Deadline) -> Result<bool, ReadError> {
        if self.eof {
            return Ok(false);
        }
        let mut fds = [rustix::event::PollFd::new(
            &self.reader,
            rustix::event::PollFlags::IN,
        )];
        if poll_by(&mut fds, deadline)? == 0 {
            return Err(self.passed(deadline));
        }
        self.read_once()
    }

    /// One read, after `poll` reported the descriptor readable: it does not block, and a signal cannot interrupt it (EINTR).
    /// Returns false at the end of file.
    fn read_once(&mut self) -> Result<bool, ReadError> {
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

    /// The bytes read and not yet returned.
    pub(crate) fn buffered(&self) -> &[u8] {
        &self.buffer
    }

    /// The next line, with its newline, read by `deadline`. `None` at the end of file with nothing left; a last line with no
    /// newline is returned as it is. A line already buffered is returned at any time; no read starts after the deadline.
    ///
    /// # Errors
    /// The deadline passed first, or the read failed.
    pub fn line(&mut self, deadline: Deadline) -> Result<Option<String>, ReadError> {
        loop {
            if let Some(end) = self.buffer.iter().position(|&b| b == b'\n') {
                let line: Vec<u8> = self.buffer.drain(..=end).collect();
                return Ok(Some(String::from_utf8_lossy(&line).into_owned()));
            }
            // No read starts after the deadline, so the loop ends at it even when a read makes no progress.
            if deadline.expired() {
                return Err(self.passed(deadline));
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

    /// Everything up to the end of file, read by `deadline`; no read starts after the deadline. The end of file of a pipe
    /// proves that every holder of its writing
    /// end has closed it or ended.
    ///
    /// # Errors
    /// The deadline passed first, or the read failed.
    pub fn to_eof(&mut self, deadline: Deadline) -> Result<Vec<u8>, ReadError> {
        loop {
            // No read starts after the deadline, so the loop ends at it even when a read makes no progress.
            if deadline.expired() {
                return Err(self.passed(deadline));
            }
            if !self.fill(deadline)? {
                return Ok(std::mem::take(&mut self.buffer));
            }
        }
    }

    /// The reader, with any buffered bytes dropped.
    pub fn into_inner(self) -> R {
        self.reader
    }
}

/// Waits in `poll` until a descriptor of `fds` is ready, at most until `deadline`: the number of ready descriptors, 0 when
/// the deadline came first.
fn poll_by(fds: &mut [rustix::event::PollFd<'_>], deadline: Deadline) -> Result<usize, ReadError> {
    match crate::deadline::retry_interrupted(
        // timer: deadline — bounds the wait for the writer.
        || rustix::event::poll(fds, Some(&deadline.timespec())),
        || deadline.expired(),
    ) {
        Ok(ready) => Ok(ready.unwrap_or(0)),
        Err(error) => Err(ReadError::Io(error.into())),
    }
}

/// Both readers to their end of file by `deadline`, read together: one `poll` waits on each reader that has not ended, so a
/// writer that fills one pipe while the test waits on the other does not block. No read starts after the deadline. Both
/// descriptors are set non-blocking first: a read of a descriptor that `poll` did not report ready fails at once
/// (`WouldBlock`), never blocks.
///
/// # Errors
/// A descriptor cannot be set non-blocking, the deadline passed first (the error keeps what `first` read; `second` keeps its
/// own bytes), or a read failed.
pub(crate) fn both_to_eof<A: Read + AsFd, B: Read + AsFd>(
    first: &mut Bounded<A>,
    second: &mut Bounded<B>,
    deadline: Deadline,
) -> Result<(Vec<u8>, Vec<u8>), ReadError> {
    for fd in [first.reader.as_fd(), second.reader.as_fd()] {
        rustix::io::ioctl_fionbio(fd, true).map_err(|error| ReadError::Io(error.into()))?;
    }
    loop {
        if first.eof && second.eof {
            return Ok((
                std::mem::take(&mut first.buffer),
                std::mem::take(&mut second.buffer),
            ));
        }
        if deadline.expired() {
            return Err(first.passed(deadline));
        }
        let ready = {
            let mut fds: Vec<_> = [
                (first.eof, first.reader.as_fd()),
                (second.eof, second.reader.as_fd()),
            ]
            .into_iter()
            .filter(|(ended, _)| !ended)
            .map(|(_, fd)| {
                rustix::event::PollFd::from_borrowed_fd(fd, rustix::event::PollFlags::IN)
            })
            .collect();
            if poll_by(&mut fds, deadline)? == 0 {
                return Err(first.passed(deadline));
            }
            fds.iter()
                .map(|fd| !fd.revents().is_empty())
                .collect::<Vec<_>>()
        };
        // `ready` lists the readers that had not ended, in order.
        let mut ready = ready.into_iter();
        if !first.eof && ready.next() == Some(true) {
            first.read_once()?;
        }
        if !second.eof && ready.next() == Some(true) {
            second.read_once()?;
        }
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
        writer.write_all(b"half\nrest").unwrap();
        let mut reader = Bounded::new(reader);
        assert_eq!(
            reader.line(Deadline::cleanup()).unwrap().as_deref(),
            Some("half\n")
        );
        // The end of the line is ready, but no read starts after the deadline.
        writer.write_all(b"\n").unwrap();
        let error = reader
            .line(Deadline::after(Duration::ZERO))
            .expect_err("the deadline passed");
        assert_eq!(
            error.to_string(),
            "nothing ended the read within 0ns (read so far: \"rest\")"
        );
        let error = reader
            .to_eof(Deadline::after(Duration::ZERO))
            .expect_err("the deadline passed");
        assert!(matches!(error, ReadError::Deadline { partial, .. } if partial.is_empty()));
        drop(writer);
        assert_eq!(reader.to_eof(Deadline::cleanup()).unwrap(), b"\n");
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

    /// The writer fills the second pipe beyond its capacity before it writes the first: a reader that waited on the first
    /// pipe alone would wait until the deadline.
    #[test]
    fn both_pipes_are_read_together_to_their_ends() {
        let (first, mut first_writer) = std::io::pipe().unwrap();
        let (second, mut second_writer) = std::io::pipe().unwrap();
        let writer = std::thread::spawn(move || {
            second_writer.write_all(&[7; 1 << 20]).unwrap();
            drop(second_writer);
            first_writer.write_all(b"done").unwrap();
        });
        let (mut first, mut second) = (Bounded::new(first), Bounded::new(second));
        let read = both_to_eof(&mut first, &mut second, Deadline::cleanup());
        let again = both_to_eof(&mut first, &mut second, Deadline::cleanup());
        // The readers go first: a writer still blocked on a full pipe then fails (EPIPE), never hangs the join.
        drop((first, second));
        writer.join().unwrap();
        let (one, two) = read.unwrap();
        assert_eq!(one, b"done");
        assert_eq!(two.len(), 1 << 20);
        assert!(two.iter().all(|&b| b == 7));
        let (one, two) = again.unwrap();
        assert!(one.is_empty() && two.is_empty(), "both ended");
    }

    /// The first pipe ends while the second still has a writer: the ended pipe leaves the `poll`, and the read of the other
    /// goes on to its end.
    #[test]
    fn a_pipe_that_ended_first_does_not_stop_the_read_of_the_other() {
        let (first, mut first_writer) = std::io::pipe().unwrap();
        let (second, mut second_writer) = std::io::pipe().unwrap();
        first_writer.write_all(b"early").unwrap();
        drop(first_writer);
        let writer = std::thread::spawn(move || second_writer.write_all(&[7; 1 << 20]).unwrap());
        let (mut first, mut second) = (Bounded::new(first), Bounded::new(second));
        let read = both_to_eof(&mut first, &mut second, Deadline::cleanup());
        // The readers go first: a writer still blocked on a full pipe then fails (EPIPE), never hangs the join.
        drop((first, second));
        writer.join().unwrap();
        let (one, two) = read.unwrap();
        assert_eq!(one, b"early");
        assert_eq!(two.len(), 1 << 20);
    }

    /// The first pipe holds more than its capacity and the second stays silent until the first ends: each read waits for
    /// its own pipe, so the second is not read before it is ready.
    #[test]
    fn a_silent_second_pipe_is_not_read_before_it_is_ready() {
        let (first, mut first_writer) = std::io::pipe().unwrap();
        let (second, second_writer) = std::io::pipe().unwrap();
        let writer = std::thread::spawn(move || {
            first_writer.write_all(&[7; 1 << 20]).unwrap();
            drop(first_writer);
            drop(second_writer);
        });
        let (mut first, mut second) = (Bounded::new(first), Bounded::new(second));
        let read = both_to_eof(&mut first, &mut second, Deadline::cleanup());
        // The readers go first: a writer still blocked on a full pipe then fails (EPIPE), never hangs the join.
        drop((first, second));
        writer.join().unwrap();
        let (one, two) = read.unwrap();
        assert_eq!(one.len(), 1 << 20);
        assert!(two.is_empty());
    }

    /// A silent writer fails the read at the deadline, with what the first reader read; the second keeps its own bytes.
    #[test]
    fn a_silent_writer_fails_both_reads_at_the_deadline() {
        let (first, mut first_writer) = std::io::pipe().unwrap();
        let (second, mut second_writer) = std::io::pipe().unwrap();
        first_writer.write_all(b"out").unwrap();
        second_writer.write_all(b"err").unwrap();
        let (mut first, mut second) = (Bounded::new(first), Bounded::new(second));
        assert!(first.fill(Deadline::cleanup()).unwrap());
        assert!(second.fill(Deadline::cleanup()).unwrap());
        let error =
            both_to_eof(&mut first, &mut second, Deadline::after(Duration::ZERO)).unwrap_err();
        assert_eq!(
            error.to_string(),
            "nothing ended the read within 0ns (read so far: \"out\")"
        );
        assert_eq!(second.buffered(), b"err");
        drop((first_writer, second_writer));
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
