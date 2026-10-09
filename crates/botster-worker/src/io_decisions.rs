//! Pure decisions of the real I/O adapter. The Driver uses these decisions for every real edge.

use botster_worker_core::{CandidateId, Input};
use std::collections::VecDeque;
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
    /// The worker endpoint may hold a connection to accept.
    pub endpoint_readable: bool,
    /// A candidate may hold bytes to read.
    pub candidate_readable: bool,
}

impl ReadyState {
    pub fn timeout(&self, deadline: Option<Instant>, now: Instant) -> Option<Duration> {
        let busy = (self.link_open && self.control_readable)
            || (self.pty_registered && self.pty_readable)
            || self.draining.is_some()
            || self.queued_inputs != 0
            || (self.pending_write && !self.write_blocked)
            || self.endpoint_readable
            || self.candidate_readable;
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

/// The poll token of the first candidate. A candidate's token is this value plus its id (DESIGN.md part 7).
pub const FIRST_CANDIDATE: usize = 5;

/// The poll token of candidate `id`.
pub fn candidate_token(id: CandidateId) -> usize {
    FIRST_CANDIDATE.saturating_add(usize::try_from(id.0).unwrap_or(usize::MAX))
}

/// The candidate of a poll token, or `None` for a token below the candidates.
pub fn candidate_of(token: usize) -> Option<CandidateId> {
    token
        .checked_sub(FIRST_CANDIDATE)
        .map(|n| CandidateId(n as u64))
}

/// The fence of `Action::AdoptLink(adopted)` on the inputs that the driver queued and the machine did not handle yet. An
/// input of the old link is dropped: the machine hears nothing of the old link after the action. An input of the adopted
/// candidate becomes the same input of the control link.
pub fn fence(inputs: &mut VecDeque<Input>, adopted: CandidateId) {
    let queued = std::mem::take(inputs);
    inputs.extend(queued.into_iter().filter_map(|input| match input {
        Input::LinkBytes(_) | Input::LinkClosed | Input::LinkWritten { .. } => None,
        Input::CandidateBytes(id, bytes) if id == adopted => Some(Input::LinkBytes(bytes)),
        Input::CandidateClosed(id) if id == adopted => Some(Input::LinkClosed),
        other => Some(other),
    }));
}

/// The result of the removal of the worker endpoint at the worker's end. An endpoint that is not there is no failure: the
/// host removes it at `Remove` (DESIGN.md part 1).
pub fn unlinked(result: io::Result<()>) -> io::Result<()> {
    match result {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

/// The errno of a PTY that is gone, and of an OS failure that carries no errno.
pub const EIO: i32 = rustix::io::Errno::IO.raw_os_error();

/// What the driver does with the result of one PTY write of `len` bytes (`None`: no PTY any more).
#[derive(Debug, PartialEq, Eq)]
pub enum PtyWrite {
    /// The write was interrupted: keep the bytes for the next turn.
    Retry,
    /// Give `Input::PtyWritten(result)`; with `wait_writable`, after write interest is on (the PTY took no byte).
    Report {
        result: Result<usize, i32>,
        wait_writable: bool,
    },
}

pub fn pty_write(written: Option<io::Result<usize>>, len: usize) -> PtyWrite {
    let result = match written {
        // A write to a PTY that is gone fails as a write to a closed PTY does.
        None => Err(EIO),
        Some(Ok(n)) => Ok(n),
        Some(Err(error)) => match failure(&error) {
            IoFailure::Retry => return PtyWrite::Retry,
            IoFailure::Blocked => Ok(0),
            IoFailure::Closed => Err(error.raw_os_error().unwrap_or(EIO)),
        },
    };
    PtyWrite::Report {
        wait_writable: result == Ok(0) && len != 0,
        result,
    }
}

/// The PTY's write interest after asking for `on` (plan 2.5): `reregister` when the poll must change, and the new state.
/// A PTY that left the loop has no write interest.
#[derive(Debug, PartialEq, Eq)]
pub struct WriteInterest {
    pub reregister: bool,
    pub wants_write: bool,
}

/// A PTY event ends the wait of a write that the PTY did not take only when the PTY is writable and that write waits.
pub fn pty_writable(wants_write: bool, writable: bool) -> bool {
    wants_write && writable
}

pub fn pty_write_interest(registered: bool, wants_write: bool, on: bool) -> WriteInterest {
    WriteInterest {
        reregister: registered && on != wants_write,
        wants_write: registered && on,
    }
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
        for bits in 0u16..1024 {
            let state = ReadyState {
                link_open: bits & 1 != 0,
                control_readable: bits & 2 != 0,
                pty_registered: bits & 4 != 0,
                pty_readable: bits & 8 != 0,
                draining: (bits & 16 != 0).then_some(0),
                queued_inputs: usize::from(bits & 32 != 0),
                pending_write: bits & 64 != 0,
                write_blocked: bits & 128 != 0,
                endpoint_readable: bits & 256 != 0,
                candidate_readable: bits & 512 != 0,
            };
            let ready = match bits {
                b if b & 3 == 3 => true,
                b if b & 12 == 12 => true,
                b if b & 48 != 0 => true,
                b if b & 192 == 64 => true,
                b if b & 768 != 0 => true,
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

    /// Plan 2.4: a PTY write reports what the PTY took, or its errno; it waits for write readiness only when the PTY took
    /// no byte of a non-empty write; an interrupted write keeps its bytes.
    #[test]
    fn a_pty_write_reports_its_result_and_waits_only_when_no_byte_was_taken() {
        let report = |result, wait_writable| PtyWrite::Report {
            result,
            wait_writable,
        };
        assert_eq!(pty_write(Some(Ok(3)), 5), report(Ok(3), false));
        assert_eq!(pty_write(Some(Ok(0)), 5), report(Ok(0), true));
        assert_eq!(pty_write(Some(Ok(0)), 0), report(Ok(0), false));
        let blocked = || Some(Err(io::ErrorKind::WouldBlock.into()));
        assert_eq!(pty_write(blocked(), 5), report(Ok(0), true));
        assert_eq!(pty_write(blocked(), 0), report(Ok(0), false));
        assert_eq!(
            pty_write(Some(Err(io::ErrorKind::Interrupted.into())), 5),
            PtyWrite::Retry
        );
        let pipe = rustix::io::Errno::PIPE.raw_os_error();
        assert_eq!(
            pty_write(Some(Err(io::Error::from_raw_os_error(pipe))), 5),
            report(Err(pipe), false)
        );
        assert_eq!(
            pty_write(Some(Err(io::ErrorKind::BrokenPipe.into())), 5),
            report(Err(EIO), false)
        );
        assert_eq!(pty_write(None, 5), report(Err(EIO), false));
    }

    /// Plan 2.5: only a writable PTY with a waiting write gives `PtyWritable`; a readable-only event or a write that does
    /// not wait gives none.
    #[test]
    fn only_a_writable_event_ends_the_wait_of_a_write() {
        assert!(pty_writable(true, true));
        assert!(!pty_writable(true, false));
        assert!(!pty_writable(false, true));
        assert!(!pty_writable(false, false));
    }

    /// Plan 2.5: write interest changes the poll only when the PTY is in it and the interest differs.
    #[test]
    fn write_interest_changes_the_poll_only_for_a_registered_pty_and_a_new_value() {
        let change = |reregister, wants_write| WriteInterest {
            reregister,
            wants_write,
        };
        for wants_write in [false, true] {
            for on in [false, true] {
                assert_eq!(
                    pty_write_interest(false, wants_write, on),
                    change(false, false)
                );
            }
        }
        assert_eq!(pty_write_interest(true, false, true), change(true, true));
        assert_eq!(pty_write_interest(true, true, false), change(true, false));
        assert_eq!(pty_write_interest(true, true, true), change(false, true));
        assert_eq!(pty_write_interest(true, false, false), change(false, false));
    }

    /// DESIGN.md part 7: each candidate has its own poll token above the fixed ones, and only those tokens name one.
    #[test]
    fn a_candidate_token_names_that_candidate_only() {
        for id in [0, 1, 7, 4096] {
            let token = candidate_token(CandidateId(id));
            assert!(token >= FIRST_CANDIDATE);
            assert_eq!(candidate_of(token), Some(CandidateId(id)));
        }
        for token in 0..FIRST_CANDIDATE {
            assert_eq!(candidate_of(token), None);
        }
    }

    /// The fence (DP-8, `Action::AdoptLink`): no queued input of the old link reaches the machine, the adopted candidate's
    /// inputs become control-link inputs in their order, and every other input stays in its place.
    #[test]
    fn the_fence_drops_the_old_link_and_turns_the_adopted_candidate_into_the_link() {
        let (adopted, other) = (CandidateId(2), CandidateId(3));
        let mut inputs = VecDeque::from([
            Input::LinkBytes(vec![1]),
            Input::Timer,
            Input::CandidateBytes(adopted, vec![2]),
            Input::LinkWritten { total: 9 },
            Input::CandidateBytes(other, vec![3]),
            Input::LinkClosed,
            Input::CandidateClosed(other),
            Input::CandidateClosed(adopted),
            Input::Candidate(CandidateId(4)),
        ]);
        fence(&mut inputs, adopted);
        assert_eq!(
            inputs,
            VecDeque::from([
                Input::Timer,
                Input::LinkBytes(vec![2]),
                Input::CandidateBytes(other, vec![3]),
                Input::CandidateClosed(other),
                Input::LinkClosed,
                Input::Candidate(CandidateId(4)),
            ])
        );
    }

    /// DESIGN.md part 1: the worker removes its endpoint at its end; an endpoint that the host removed first is no failure.
    #[test]
    fn a_missing_endpoint_at_the_end_is_no_failure() {
        assert!(unlinked(Ok(())).is_ok());
        assert!(unlinked(Err(io::ErrorKind::NotFound.into())).is_ok());
        assert_eq!(
            unlinked(Err(io::ErrorKind::PermissionDenied.into()))
                .unwrap_err()
                .kind(),
            io::ErrorKind::PermissionDenied
        );
    }
}
