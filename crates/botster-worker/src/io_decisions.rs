//! Pure decisions of the real I/O adapter. The Driver uses these decisions for every real edge.

use botster_core_link::launch::WorkerLaunch;
use botster_worker_core::{CandidateId, DescriptorId, Drain, Input, WorkerConfig};
use std::collections::{BTreeMap, VecDeque};
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
    /// The drain that a `DrainPty` asked for; `None` when no drain is asked.
    pub draining: Option<Drain>,
    pub queued_inputs: usize,
    pub pending_write: bool,
    pub write_blocked: bool,
    /// The worker endpoint may hold a connection to accept.
    pub endpoint_readable: bool,
    /// A candidate may hold bytes to read.
    pub candidate_readable: bool,
    /// The PTY read budget (`PtyReadBudget`, OU-3d): `Some(0)` stops every PTY read, a drain's too.
    pub pty_budget: Option<usize>,
    /// A route can write or read in the next turn (`route_io::Routes::busy`).
    pub routes_busy: bool,
}

impl ReadyState {
    pub fn timeout(&self, deadline: Option<Instant>, now: Instant) -> Option<Duration> {
        // A budget of zero stops the PTY's reads, so a readable PTY or a drain that still reads waits for the next budget.
        // A complete drain (`Drain::Done`) reports `PtyDrained` without a read.
        let pty_reads = self.pty_budget != Some(0);
        let busy = (self.link_open && self.control_readable)
            || (self.pty_registered && self.pty_readable && pty_reads)
            || self
                .draining
                .is_some_and(|drain| drain == Drain::Done || pty_reads)
            || self.queued_inputs != 0
            || (self.pending_write && !self.write_blocked)
            || self.endpoint_readable
            || self.candidate_readable
            || self.routes_busy;
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

/// The bytes of the next PTY read: one chunk, bounded by what a drain still reads (`Drain::want`) and by the read budget
/// (OU-3d). `0`: no read now (the budget is spent).
pub fn pty_read_bytes(chunk: usize, draining: Option<Drain>, budget: Option<usize>) -> usize {
    let want = draining.map_or(chunk, |drain| drain.want(chunk));
    budget.map_or(want, |budget| budget.min(want))
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

/// The candidate of a poll token, or `None` for a token below the candidates or a route stream's token.
pub fn candidate_of(token: usize) -> Option<CandidateId> {
    if crate::route_io::is_route_token(token) {
        return None;
    }
    token
        .checked_sub(FIRST_CANDIDATE)
        .map(|n| CandidateId(n as u64))
}

/// The id of the next accepted candidate. Ids count up from 0 and never repeat in one worker, so an input of a closed
/// candidate never names a later one.
pub fn take_candidate(next: &mut u64) -> CandidateId {
    let id = CandidateId(*next);
    *next += 1;
    id
}

/// The route streams of the control link (DP-2): each received stream under the id that the machine knows, until the
/// machine binds it to a route (the route transport takes it, `route_io::Routes`) or closes it. Dropping a stream closes
/// it. `T` is the stream: `OwnedFd` in the driver.
#[derive(Debug)]
pub struct DescriptorTable<T> {
    next: u64,
    unbound: BTreeMap<DescriptorId, T>,
}

impl<T> Default for DescriptorTable<T> {
    fn default() -> Self {
        DescriptorTable {
            next: 0,
            unbound: BTreeMap::new(),
        }
    }
}

impl<T> DescriptorTable<T> {
    /// Keeps a received stream under a new id. Ids count up from 1 and never repeat in one worker.
    pub fn keep(&mut self, stream: T) -> DescriptorId {
        self.next += 1;
        let id = DescriptorId(self.next);
        self.unbound.insert(id, stream);
        id
    }

    /// `BindRoute`: the stream of `descriptor` leaves the table for the route's transport. `None` for an id that the table
    /// does not hold.
    pub fn take(&mut self, descriptor: DescriptorId) -> Option<T> {
        self.unbound.remove(&descriptor)
    }

    /// `CloseDescriptor`, or a read that the adoption fence dropped: the unbound stream closes.
    pub fn close(&mut self, descriptor: DescriptorId) {
        drop(self.unbound.remove(&descriptor));
    }

    /// The streams that the machine will never hear of (the adoption fence dropped their reads) close.
    pub fn close_all(&mut self, descriptors: Vec<DescriptorId>) {
        for descriptor in descriptors {
            self.close(descriptor);
        }
    }
}

/// The endpoint waits for its next event, and the driver accepts nothing from it, while it is not readable. The readiness
/// is edge-triggered: a `WouldBlock` clears it, and the next endpoint event sets it again.
pub fn endpoint_waits(readable: bool) -> bool {
    !readable
}

/// A candidate waits for its next event, and the driver reads nothing of it, while it is not readable (the same
/// edge-triggered rule as the endpoint).
pub fn candidate_waits(readable: bool) -> bool {
    !readable
}

/// The machine's configuration from the launch arguments. `startup` is `--startup-ms`: the bound of a candidate's hello,
/// and of a worker with no payload and no host (AD-7).
pub fn worker_config(launch: &WorkerLaunch) -> WorkerConfig {
    WorkerConfig {
        startup: Duration::from_millis(launch.startup_ms),
        ..WorkerConfig::new(launch.instance.clone(), launch.token, launch.host_epoch)
    }
}

/// The fence of `Action::AdoptLink(adopted)` on the inputs that the driver queued and the machine did not handle yet. An
/// input of the old link is dropped: the machine hears nothing of the old link after the action. An input of the adopted
/// candidate becomes the same input of the control link. When the driver no longer holds the candidate (`held` is false:
/// its connection ended), the fence fails closed: the machine hears that its new link ended (`LinkClosed`), and nothing
/// that it sends can reach the old host. Returns the descriptors of the old link's reads that it dropped: the machine never
/// hears of them, so the driver closes them (DP-8).
pub fn fence(inputs: &mut VecDeque<Input>, adopted: CandidateId, held: bool) -> Vec<DescriptorId> {
    let mut dropped = Vec::new();
    let queued = std::mem::take(inputs);
    inputs.extend(queued.into_iter().filter_map(|input| match input {
        Input::LinkBytes { descriptors, .. } => {
            dropped.extend(descriptors);
            None
        }
        Input::LinkClosed | Input::LinkWritten { .. } => None,
        Input::CandidateBytes(id, bytes) if id == adopted => Some(Input::LinkBytes {
            bytes,
            descriptors: Vec::new(),
        }),
        Input::CandidateClosed(id) if id == adopted => Some(Input::LinkClosed),
        other => Some(other),
    }));
    if !held && !inputs.contains(&Input::LinkClosed) {
        inputs.push_back(Input::LinkClosed);
    }
    dropped
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

    /// OU-3d: a PTY read takes one chunk, no more than a drain still reads, and no more than the budget; a spent budget
    /// reads nothing. A31: the flushing read of a drain takes a whole chunk.
    #[test]
    fn a_pty_read_is_bounded_by_the_chunk_the_drain_and_the_budget() {
        assert_eq!(pty_read_bytes(64, None, None), 64);
        assert_eq!(pty_read_bytes(64, Some(Drain::Counted(10)), None), 10);
        assert_eq!(pty_read_bytes(64, Some(Drain::Counted(10)), Some(7)), 7);
        assert_eq!(pty_read_bytes(64, Some(Drain::Counted(10)), Some(0)), 0);
        assert_eq!(pty_read_bytes(64, Some(Drain::Counted(100)), None), 64);
        assert_eq!(pty_read_bytes(64, Some(Drain::Counted(7)), Some(10)), 7);
        assert_eq!(pty_read_bytes(64, Some(Drain::Flush), None), 64);
        assert_eq!(pty_read_bytes(64, Some(Drain::Flush), Some(7)), 7);
        assert_eq!(pty_read_bytes(64, None, Some(7)), 7);
        assert_eq!(pty_read_bytes(64, None, Some(100)), 64);
        assert_eq!(pty_read_bytes(64, None, Some(0)), 0);
    }

    /// A route stream's token is never a candidate's.
    #[test]
    fn a_route_token_names_no_candidate() {
        assert_eq!(candidate_of(FIRST_CANDIDATE), Some(CandidateId(0)));
        assert_eq!(candidate_of(crate::route_io::ROUTE_TOKEN | 1), None);
        assert_eq!(candidate_of(usize::MAX), None);
    }

    #[test]
    fn every_ready_source_prevents_a_blocking_wait() {
        let now = Instant::now();
        let later = now + Duration::from_secs(3);
        for bits in 0u16..8192 {
            let budget_spent = bits & 2048 != 0;
            let state = ReadyState {
                link_open: bits & 1 != 0,
                control_readable: bits & 2 != 0,
                pty_registered: bits & 4 != 0,
                pty_readable: bits & 8 != 0,
                // A drain that is complete, or one that still reads.
                draining: (bits & 16 != 0).then_some(if bits & 4096 != 0 {
                    Drain::Counted(5)
                } else {
                    Drain::Done
                }),
                queued_inputs: usize::from(bits & 32 != 0),
                pending_write: bits & 64 != 0,
                write_blocked: bits & 128 != 0,
                endpoint_readable: bits & 256 != 0,
                candidate_readable: bits & 512 != 0,
                routes_busy: bits & 1024 != 0,
                pty_budget: budget_spent.then_some(0),
            };
            let ready = match bits {
                b if b & 3 == 3 => true,
                // OU-3d: a spent budget stops the PTY's reads, so a readable PTY does not keep the loop busy.
                b if b & 12 == 12 && !budget_spent => true,
                // A complete drain reports without a read; a drain that still reads waits for a budget.
                b if b & 16 != 0 && (b & 4096 == 0 || !budget_spent) => true,
                b if b & 32 != 0 => true,
                b if b & 192 == 64 => true,
                b if b & 768 != 0 => true,
                b if b & 1024 != 0 => true,
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
            Input::LinkBytes {
                bytes: vec![1],
                descriptors: vec![DescriptorId(7)],
            },
            Input::Timer,
            Input::CandidateBytes(adopted, vec![2]),
            Input::LinkWritten { total: 9 },
            Input::CandidateBytes(other, vec![3]),
            Input::LinkClosed,
            Input::CandidateClosed(other),
            Input::CandidateClosed(adopted),
            Input::Candidate(CandidateId(4)),
        ]);
        let mut gone = inputs.clone();
        // The old link's read had a descriptor that the machine never heard of: the driver closes it.
        assert_eq!(fence(&mut inputs, adopted, true), vec![DescriptorId(7)]);
        assert_eq!(
            inputs,
            VecDeque::from([
                Input::Timer,
                Input::LinkBytes {
                    bytes: vec![2],
                    descriptors: Vec::new(),
                },
                Input::CandidateBytes(other, vec![3]),
                Input::CandidateClosed(other),
                Input::LinkClosed,
                Input::Candidate(CandidateId(4)),
            ])
        );

        // P3 review of #176 (L1): a candidate that the driver no longer holds gives one `LinkClosed` of the new link.
        fence(&mut gone, adopted, false);
        assert_eq!(
            gone.iter().filter(|i| **i == Input::LinkClosed).count(),
            1,
            "{gone:?}"
        );
        let mut empty = VecDeque::new();
        fence(&mut empty, adopted, true);
        assert!(empty.is_empty(), "a held candidate adds no input");
        fence(&mut empty, adopted, false);
        assert_eq!(empty, VecDeque::from([Input::LinkClosed]));
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

    /// Candidate ids count up from 0, one per accepted connection, and never repeat.
    #[test]
    fn candidate_ids_count_up_and_never_repeat() {
        let mut next = 0;
        let ids: Vec<CandidateId> = (0..3).map(|_| take_candidate(&mut next)).collect();
        assert_eq!(ids, [CandidateId(0), CandidateId(1), CandidateId(2)]);
        assert_eq!(next, 3);
    }

    /// A stream that records its close, for the descriptor table.
    struct Stream(u8, std::rc::Rc<std::cell::RefCell<Vec<u8>>>);

    impl Drop for Stream {
        fn drop(&mut self) {
            self.1.borrow_mut().push(self.0);
        }
    }

    /// DP-2: received streams get distinct ids that count up from 1. A taken stream is the route transport's; an unbound
    /// one lives until `CloseDescriptor` or the fence closes it. A stream closes once, and an unknown id closes nothing.
    #[test]
    fn the_descriptor_table_keeps_each_stream_until_its_close() {
        let closed = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let mut table = DescriptorTable::default();
        let ids: Vec<DescriptorId> = (1..=3)
            .map(|n| table.keep(Stream(n, closed.clone())))
            .collect();
        assert_eq!(ids, [DescriptorId(1), DescriptorId(2), DescriptorId(3)]);
        let taken = table.take(ids[0]).expect("a kept stream");
        assert!(table.take(ids[0]).is_none(), "a stream is taken once");
        table.close(ids[0]);
        assert!(closed.borrow().is_empty(), "a taken stream is the route's");
        table.close(ids[1]);
        assert_eq!(*closed.borrow(), [2]);
        table.close(ids[1]);
        table.close(DescriptorId(99));
        assert!(table.take(DescriptorId(99)).is_none());
        assert_eq!(*closed.borrow(), [2], "unknown ids close nothing");
        drop(taken);
        assert_eq!(*closed.borrow(), [2, 1]);
        drop(table.take(ids[2]));
        assert_eq!(*closed.borrow(), [2, 1, 3]);
        let fenced = [
            table.keep(Stream(4, closed.clone())),
            table.keep(Stream(5, closed.clone())),
        ];
        assert_eq!(fenced, [DescriptorId(4), DescriptorId(5)]);
        table.close_all(fenced.to_vec());
        assert_eq!(
            *closed.borrow(),
            [2, 1, 3, 4, 5],
            "the fence closes every dropped stream"
        );
    }

    /// The endpoint and a candidate wait while they are not readable (edge-triggered readiness).
    #[test]
    fn the_endpoint_and_a_candidate_wait_while_not_readable() {
        assert!(endpoint_waits(false));
        assert!(!endpoint_waits(true));
        assert!(candidate_waits(false));
        assert!(!candidate_waits(true));
    }

    /// AD-7: the machine's startup bound is `--startup-ms`; the identity is the launch's.
    #[test]
    fn the_worker_config_takes_the_startup_bound_and_the_identity_of_the_launch() {
        let launch = WorkerLaunch {
            control: "/d/c".into(),
            instance: botster_core_contract::prelude::InstanceId("7-1".into()),
            host_epoch: 7,
            token: [0xAB; 32],
            endpoint: "/d/w/7-1".into(),
            startup_ms: 1_234,
        };
        let config = worker_config(&launch);
        assert_eq!(config.startup, Duration::from_millis(1_234));
        assert_ne!(
            WorkerConfig::new(launch.instance.clone(), launch.token, 7).startup,
            config.startup,
            "the default bound differs, so the test sees the field"
        );
        assert_eq!(config.instance, launch.instance);
        assert_eq!(config.token, launch.token);
        assert_eq!(config.host_epoch, 7);
    }
}
