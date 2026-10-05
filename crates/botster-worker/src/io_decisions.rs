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

pub struct ReadyState {
    pub link_open: bool,
    pub control_readable: bool,
    pub pty_registered: bool,
    pub pty_readable: bool,
    pub draining: Option<usize>,
    pub queued_inputs: usize,
    pub pending_write: bool,
    pub write_blocked: bool,
}

impl ReadyState {
    pub fn timeout(&self, deadline: Option<Instant>, now: Instant) -> Option<Duration> {
        let busy = (self.link_open && self.control_readable)
            || (self.pty_registered && self.pty_readable)
            || self.draining.is_some()
            || self.queued_inputs != 0
            || (self.pending_write && !self.write_blocked);
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

pub fn read_pty(registered: bool, readable: bool, draining: Option<usize>) -> bool {
    (registered && readable) || draining.is_some()
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
        for bits in 0u16..256 {
            let state = ReadyState {
                link_open: bits & 1 != 0,
                control_readable: bits & 2 != 0,
                pty_registered: bits & 4 != 0,
                pty_readable: bits & 8 != 0,
                draining: (bits & 16 != 0).then_some(0),
                queued_inputs: usize::from(bits & 32 != 0),
                pending_write: bits & 64 != 0,
                write_blocked: bits & 128 != 0,
            };
            let ready = match bits {
                b if b & 3 == 3 => true,
                b if b & 12 == 12 => true,
                b if b & 48 != 0 => true,
                b if b & 192 == 64 => true,
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
                for drain in [None, Some(0), Some(3)] {
                    assert_eq!(
                        read_pty(open, ready, drain),
                        drain.is_some() || (open, ready) == (true, true)
                    );
                }
                for bytes in [0, 1, 4096] {
                    assert_eq!(flush(open, bytes), open && bytes > 0);
                    assert_eq!(keep_writing(ready, bytes), ready && bytes > 0);
                }
            }
        }
    }
}
