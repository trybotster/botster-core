//! Pure decisions of the real I/O adapter. The Driver uses these decisions for every real edge.

use std::io;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IoFailure {
    Retry,
    Blocked,
    Closed,
}

pub fn failure(error: &io::Error) -> IoFailure {
    match error.kind() {
        io::ErrorKind::Interrupted => IoFailure::Retry,
        io::ErrorKind::WouldBlock => IoFailure::Blocked,
        _ => IoFailure::Closed,
    }
}

pub fn poll_interrupted(result: io::Result<()>) -> io::Result<bool> {
    match result {
        Ok(()) => Ok(false),
        Err(error) if error.kind() == io::ErrorKind::Interrupted => Ok(true),
        Err(error) => Err(error),
    }
}

pub fn due(deadline: Option<Instant>, now: Instant) -> bool {
    deadline.is_some_and(|at| at <= now)
}

/// The drain of the PTY that `Action::DrainPty` asks for at the payload's exit: the output written before the exit is read
/// before the exit is reported (EV-4, ST-5), and output that a remaining process of the group writes later cannot hold the
/// exit back.
///
/// The count of the PTY (`FIONREAD`) can leave out output that the terminal has not moved to its read buffer yet. A
/// nonblocking read moves it before it reports that no byte is left. So the drain reads the count, then reads until a read
/// finds nothing or the end of the output. If that flushing read finds bytes, the drain reads at most the count measured
/// once right after it, so a process that keeps writing cannot extend the drain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Drain {
    /// The bytes still to read of the count taken when the drain was asked.
    Counted(usize),
    /// The count is read. The next read flushes, and it ends the drain unless it finds bytes.
    Flush,
    /// The bytes still to read of the count measured after the flushing read.
    Flushed(usize),
    /// The drain is complete: `PtyDrained` is due.
    Done,
}

impl Drain {
    /// A drain of the PTY that holds `pending` bytes now.
    pub fn asked(pending: usize) -> Drain {
        match pending {
            0 => Drain::Flush,
            left => Drain::Counted(left),
        }
    }

    /// The most bytes that the next read takes, within the driver's read bound `chunk`.
    pub fn want(self, chunk: usize) -> usize {
        match self {
            Drain::Counted(left) | Drain::Flushed(left) => left.min(chunk),
            Drain::Flush | Drain::Done => chunk,
        }
    }

    /// The drain after a read that found `n` bytes. `measure` gives the count of the PTY; it is called only after the
    /// flushing read. A read that finds nothing or the end of the output completes the drain (`Drain::Done`).
    pub fn after_read(
        self,
        n: usize,
        measure: impl FnOnce() -> io::Result<usize>,
    ) -> io::Result<Drain> {
        Ok(match self {
            Drain::Counted(left) if n < left => Drain::Counted(left - n),
            Drain::Counted(_) => Drain::Flush,
            Drain::Flush => match measure()? {
                0 => Drain::Done,
                left => Drain::Flushed(left),
            },
            Drain::Flushed(left) if n < left => Drain::Flushed(left - n),
            Drain::Flushed(_) | Drain::Done => Drain::Done,
        })
    }
}

pub struct ReadyState {
    pub link_open: bool,
    pub control_readable: bool,
    pub pty_registered: bool,
    pub pty_readable: bool,
    pub draining: bool,
    pub queued_inputs: usize,
}

impl ReadyState {
    pub fn timeout(&self, deadline: Option<Instant>, now: Instant) -> Option<Duration> {
        let busy = (self.link_open && self.control_readable)
            || (self.pty_registered && self.pty_readable)
            || self.draining
            || self.queued_inputs != 0;
        if busy {
            Some(Duration::ZERO)
        } else {
            deadline.map(|at| at.saturating_duration_since(now))
        }
    }
}

pub fn control_ready(previous: bool, readable: bool, closed: bool, error: bool) -> bool {
    previous || readable || closed || error
}

pub fn writable_ready(previous: bool, writable: bool, error: bool) -> bool {
    previous || writable || error
}

pub fn read_control(link_open: bool, readable: bool) -> bool {
    link_open && readable
}

pub fn read_pty(registered: bool, readable: bool, draining: bool) -> bool {
    (registered && readable) || draining
}

pub fn flush(link_open: bool, bytes: usize) -> bool {
    link_open && bytes != 0
}

pub fn keep_writing(writable: bool, bytes: usize) -> bool {
    writable && bytes != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_have_distinct_retry_blocked_and_closed_results() {
        for (kind, expected) in [
            (io::ErrorKind::Interrupted, IoFailure::Retry),
            (io::ErrorKind::WouldBlock, IoFailure::Blocked),
            (io::ErrorKind::BrokenPipe, IoFailure::Closed),
            (io::ErrorKind::InvalidData, IoFailure::Closed),
        ] {
            assert_eq!(failure(&io::Error::from(kind)), expected);
        }
        assert!(!poll_interrupted(Ok(())).unwrap());
        assert!(poll_interrupted(Err(io::ErrorKind::Interrupted.into())).unwrap());
        for kind in [io::ErrorKind::WouldBlock, io::ErrorKind::BrokenPipe] {
            assert_eq!(poll_interrupted(Err(kind.into())).unwrap_err().kind(), kind);
        }
    }

    #[test]
    fn a_deadline_is_due_at_its_instant_and_after_it() {
        let now = Instant::now();
        assert!(!due(None, now));
        assert!(due(Some(now), now));
        assert!(due(Some(now - Duration::from_secs(1)), now));
        assert!(!due(Some(now + Duration::from_secs(1)), now));
    }

    #[test]
    fn every_ready_source_prevents_a_blocking_wait() {
        let now = Instant::now();
        let later = now + Duration::from_secs(3);
        for bits in 0u8..64 {
            let state = ReadyState {
                link_open: bits & 1 != 0,
                control_readable: bits & 2 != 0,
                pty_registered: bits & 4 != 0,
                pty_readable: bits & 8 != 0,
                draining: bits & 16 != 0,
                queued_inputs: usize::from(bits & 32 != 0),
            };
            let ready = match bits {
                b if b & 3 == 3 => true,
                b if b & 12 == 12 => true,
                b if b & 48 != 0 => true,
                _ => false,
            };
            assert_eq!(state.timeout(None, now), ready.then_some(Duration::ZERO));
            assert_eq!(state.timeout(Some(now), now), Some(Duration::ZERO));
            assert_eq!(
                state.timeout(Some(later), now),
                Some(if ready {
                    Duration::ZERO
                } else {
                    Duration::from_secs(3)
                })
            );
        }
    }

    #[test]
    fn readiness_retains_each_event_until_the_edge_clears_it() {
        for bits in 0u8..16 {
            assert_eq!(
                control_ready(bits & 1 != 0, bits & 2 != 0, bits & 4 != 0, bits & 8 != 0),
                bits != 0
            );
        }
        for bits in 0u8..8 {
            assert_eq!(
                writable_ready(bits & 1 != 0, bits & 2 != 0, bits & 4 != 0),
                bits != 0
            );
        }
    }

    #[test]
    fn each_io_operation_requires_its_own_ready_edge() {
        for open in [false, true] {
            for ready in [false, true] {
                assert_eq!(read_control(open, ready), (open, ready) == (true, true));
                for drain in [false, true] {
                    assert_eq!(
                        read_pty(open, ready, drain),
                        drain || (open, ready) == (true, true)
                    );
                }
                for bytes in [0, 1, 4096] {
                    assert_eq!(flush(open, bytes), open && bytes > 0);
                    assert_eq!(keep_writing(ready, bytes), ready && bytes > 0);
                }
            }
        }
    }

    /// Reads like the driver: `reads` gives the bytes that each read finds (0: nothing or the end), `measures` the count
    /// after the flushing read. Returns the bytes that the drain read and the reads it made.
    fn drain_with(
        pending: usize,
        reads: &[usize],
        measures: &[usize],
        chunk: usize,
    ) -> (usize, usize) {
        let (mut reads, mut measures) = (reads.iter(), measures.iter());
        let mut drain = Drain::asked(pending);
        let (mut total, mut made) = (0, 0);
        while drain != Drain::Done {
            let found = (*reads.next().expect("the drain read again")).min(drain.want(chunk));
            made += 1;
            total += found;
            drain = match found {
                0 => Drain::Done,
                n => drain
                    .after_read(n, || {
                        Ok(*measures.next().expect("the drain measured again"))
                    })
                    .unwrap(),
            };
        }
        assert!(measures.next().is_none(), "every measure was used");
        (total, made)
    }

    /// A31: output that the count left out (still in the terminal's flip buffer) is read: after the counted bytes, the
    /// flushing read finds it, and the count measured once after that read bounds the rest.
    #[test]
    fn a_drain_reads_the_output_that_its_count_left_out() {
        let chunk = 8;
        // The count saw 3 bytes; 4 more were not counted. The flushing read finds 2, and the count after it the other 2.
        let reads = [3, 2, 2];
        let (total, made) = drain_with(3, &reads, &[2], chunk);
        assert_eq!(total, reads.iter().sum::<usize>());
        assert_eq!(
            made,
            reads.len(),
            "the drain ends when the measured count is read"
        );
        // A count of 0 still makes the flushing read; a count of 0 after it ends the drain.
        let reads = [2];
        let (total, made) = drain_with(0, &reads, &[0], chunk);
        assert_eq!((total, made), (2, reads.len()));
        // A flushing read that finds nothing ends the drain at once.
        assert_eq!(drain_with(3, &[3, 0], &[], chunk), (3, 2));
    }

    /// A31: a process that keeps writing cannot hold the exit back: past the count, the drain reads at most one flushing
    /// read and the count measured once after it.
    #[test]
    fn a_writer_that_keeps_writing_cannot_extend_the_drain() {
        let chunk = 8;
        let (pending, measured) = (5, 6);
        let endless = [chunk; 64];
        let (total, made) = drain_with(pending, &endless, &[measured], chunk);
        assert!(total <= pending + chunk + measured, "{total}");
        assert!(
            made < endless.len(),
            "the drain ended while the writer still wrote"
        );
    }
}
