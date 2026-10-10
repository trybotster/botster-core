//! The in-memory duplex edges: `Link` and `RouteTransport` (plan 2.3), with the readiness flags of plan 2.5 rule 8.
//!
//! One duplex is two ends over two bounded byte queues. A read or a write that cannot proceed returns
//! `io::ErrorKind::WouldBlock`. A closed end gives the peer `Ok(0)` on `read` and `BrokenPipe` on `write`.
//!
//! **Readiness (plan 2.5, rule 8).** Each end has two flags and an interest. The flags follow the queue state: *readable*
//! when bytes (or a descriptor, or the peer's close) wait, *writable* when a write would be taken (or would fail at once). The
//! owner sets the interest as the real loop would register it. An end is ready work only when a set flag matches an interest
//! that the rules allow ([`End::is_ready`]).

use crate::scheduler::SchedulerHandle;
use botster_core_edges::{Link, RouteTransport};
use std::any::Any;
use std::collections::VecDeque;
use std::io;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

/// What an owner has registered for an end (plan 2.5: read interest follows the receive buffer, write interest follows the
/// outbound queue).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Interest {
    pub read: bool,
    pub write: bool,
}

/// The two flags of an end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Readiness {
    pub readable: bool,
    pub writable: bool,
}

/// A descriptor handed over a link. It moves by value; the receiver downcasts it to the in-memory object that it expects (a
/// route stream end, for DP-2).
///
/// Clause: Core DP-2.
pub struct Descriptor(Box<dyn Any + Send>);

impl Descriptor {
    pub fn new<T: Any + Send>(object: T) -> Descriptor {
        Descriptor(Box::new(object))
    }

    /// The object, or the descriptor itself when it holds another type.
    pub fn downcast<T: Any + Send>(self) -> Result<T, Descriptor> {
        self.0.downcast::<T>().map(|b| *b).map_err(Descriptor)
    }
}

impl std::fmt::Debug for Descriptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Descriptor")
    }
}

/// What a test scripts about the writes and reads of one side (the route controls of `docs/core-testkit-controls.md`). A
/// control is state of the edge, never a branch in a machine.
#[derive(Debug, Default, Clone)]
struct SideControl {
    /// While set, this side's stream takes no byte from its owner (`route_gate`).
    gate: bool,
    /// At most this many more bytes are taken, then none (`route_accept`). `None`: no limit.
    accept: Option<usize>,
    /// The next write fails with this kind, once (`fail_writes`).
    fail_write: Option<io::ErrorKind>,
    /// The next read fails with this kind, once (a unit-test fault of the worker binding's read loop).
    fail_read: Option<io::ErrorKind>,
    /// The end reports itself writable although it takes no byte (`route_spurious_ready`).
    spurious_writable: bool,
    /// Each read returns at most this many bytes (`route_read_size`).
    read_cap: Option<usize>,
    /// The next descriptor that this side hands over fails (`fail_handoff`).
    fail_descriptor: bool,
}

#[derive(Debug)]
struct Shared {
    control: [SideControl; 2],
    /// The stream was reset at one end with no close handshake (`drop_transport`): every call fails.
    reset: bool,
    /// `queues[s]` holds the bytes that side `s` wrote and side `1 - s` has not read.
    queues: [VecDeque<u8>; 2],
    /// `descriptors[s]`: the descriptors that side `s` sent and side `1 - s` has not received, each with the stream offset
    /// of the byte that it rides with (`SCM_RIGHTS`: a descriptor travels with a byte).
    descriptors: [VecDeque<(u64, Descriptor)>; 2],
    /// `sent[s]`: the bytes that side `s` wrote so far. `taken[s]`: the bytes of them that side `1 - s` read so far.
    sent: [u64; 2],
    taken: [u64; 2],
    capacity: usize,
    closed: [bool; 2],
    /// `live[s]`: the handle of side `s` exists (it is dropped at most once; an end is not cloned).
    live: [bool; 2],
    /// `owned[s]`: the owner of side `s` holds it (`EndControl::owned`, the worker's bind of a route stream). Until then a
    /// byte through it moves through a handle that is not the owner's.
    owned: [bool; 2],
    /// `foreign[s]`: the bytes read from or written to side `s` before its owner held it (`route_stream_holders`).
    foreign: [u64; 2],
}

/// One end of a duplex.
#[derive(Debug)]
pub struct End {
    shared: Arc<Mutex<Shared>>,
    side: usize,
    interest: Interest,
}

/// A handle on the controls of one end. It stays valid after the end moved (into a descriptor, into a worker), which is how a
/// test scripts a stream whose end it no longer holds.
#[derive(Debug, Clone)]
pub struct EndControl {
    shared: Arc<Mutex<Shared>>,
    side: usize,
}

fn lock(shared: &Mutex<Shared>) -> MutexGuard<'_, Shared> {
    shared.lock().unwrap_or_else(PoisonError::into_inner)
}

impl End {
    fn pair(capacity: usize) -> (End, End) {
        let shared = Arc::new(Mutex::new(Shared {
            control: [SideControl::default(), SideControl::default()],
            reset: false,
            queues: [VecDeque::new(), VecDeque::new()],
            descriptors: [VecDeque::new(), VecDeque::new()],
            sent: [0, 0],
            taken: [0, 0],
            capacity,
            closed: [false, false],
            live: [true, true],
            owned: [false, false],
            foreign: [0, 0],
        }));
        let end = |side| End {
            shared: Arc::clone(&shared),
            side,
            interest: Interest::default(),
        };
        (end(0), end(1))
    }

    pub fn set_interest(&mut self, interest: Interest) {
        self.interest = interest;
    }

    pub fn interest(&self) -> Interest {
        self.interest
    }

    pub fn readiness(&self) -> Readiness {
        let shared = lock(&self.shared);
        let (me, peer) = (self.side, 1 - self.side);
        let readable = !shared.closed[me]
            && (shared.reset
                || !shared.queues[peer].is_empty()
                || !shared.descriptors[peer].is_empty()
                || shared.closed[peer]);
        let control = &shared.control[me];
        let takes =
            !control.gate && control.accept != Some(0) && shared.queues[me].len() < shared.capacity;
        let writable = !shared.closed[me]
            && (shared.reset
                || control.fail_write.is_some()
                || control.spurious_writable
                || shared.closed[peer]
                || takes);
        Readiness { readable, writable }
    }

    /// Plan 2.5 rule 8: ready work only when a set flag matches an allowed interest.
    pub fn is_ready(&self) -> bool {
        let flags = self.readiness();
        (flags.readable && self.interest.read) || (flags.writable && self.interest.write)
    }

    /// The bytes that wait to be read.
    fn available(&self) -> usize {
        lock(&self.shared).queues[1 - self.side].len()
    }

    /// Reads at most `max` bytes into `buf`.
    fn read_up_to(&mut self, buf: &mut [u8], max: usize) -> io::Result<usize> {
        let mut shared = lock(&self.shared);
        let (me, peer) = (self.side, 1 - self.side);
        if shared.closed[me] {
            return Ok(0);
        }
        if shared.reset {
            return Err(io::ErrorKind::ConnectionReset.into());
        }
        if let Some(kind) = shared.control[me].fail_read.take() {
            return Err(kind.into());
        }
        let max = shared.control[me].read_cap.map_or(max, |cap| max.min(cap));
        // A read never crosses the byte of a descriptor that was not received: the descriptor comes first.
        let before_descriptor = shared.descriptors[peer]
            .front()
            .map(|(at, _)| usize::try_from(at - shared.taken[peer]).unwrap_or(usize::MAX));
        let max = before_descriptor.map_or(max, |left| max.min(left));
        let queue = &mut shared.queues[peer];
        if queue.is_empty() || max == 0 {
            return if queue.is_empty() && shared.closed[peer] {
                Ok(0)
            } else {
                Err(io::ErrorKind::WouldBlock.into())
            };
        }
        let n = queue.len().min(buf.len()).min(max);
        for slot in &mut buf[..n] {
            *slot = queue.pop_front().unwrap_or_default();
        }
        shared.taken[peer] += n as u64;
        if !shared.owned[me] {
            shared.foreign[me] += n as u64;
        }
        Ok(n)
    }

    /// Writes as many bytes as the queue has room for.
    pub fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let mut shared = lock(&self.shared);
        Self::write_locked(&mut shared, self.side, bytes)
    }

    fn write_locked(shared: &mut Shared, me: usize, bytes: &[u8]) -> io::Result<usize> {
        let peer = 1 - me;
        if shared.reset {
            return Err(io::ErrorKind::ConnectionReset.into());
        }
        if let Some(kind) = shared.control[me].fail_write.take() {
            return Err(kind.into());
        }
        if shared.closed[me] || shared.closed[peer] {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        let control = &shared.control[me];
        let room = shared.capacity - shared.queues[me].len();
        let room = control.accept.map_or(room, |accept| room.min(accept));
        if (control.gate || room == 0) && !bytes.is_empty() {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        let n = room.min(bytes.len());
        shared.queues[me].extend(&bytes[..n]);
        shared.sent[me] += n as u64;
        if !shared.owned[me] {
            shared.foreign[me] += n as u64;
        }
        if let Some(accept) = &mut shared.control[me].accept {
            *accept -= n;
        }
        Ok(n)
    }

    /// Closes this end. The peer reads what is queued, then `Ok(0)`.
    ///
    /// The bytes that this end wrote stay queued until the peer reads them. The descriptors that the peer sent and this end
    /// has not received are released, as the kernel releases the descriptors of a closed socket.
    pub fn close(&mut self) {
        let released: Vec<Descriptor> = {
            let mut shared = lock(&self.shared);
            shared.closed[self.side] = true;
            shared.descriptors[1 - self.side]
                .drain(..)
                .map(|(_, descriptor)| descriptor)
                .collect()
        };
        // Dropped outside the lock: a descriptor can hold an endpoint of another duplex, whose drop takes its own lock.
        drop(released);
    }

    /// The next write fails with `kind` (A5-3: a failed write of the route transport).
    pub fn fail_next_write(&mut self, kind: io::ErrorKind) {
        self.control().fail_next_write(kind);
    }

    /// A handle on this end's controls that outlives the end.
    pub fn control(&self) -> EndControl {
        EndControl {
            shared: Arc::clone(&self.shared),
            side: self.side,
        }
    }

    /// True when the peer's writes toward this end are blocked: the queue toward this end is full, so the owner of the other
    /// end sees a stream that takes no more bytes (`input_blocked`, Core DP-5).
    pub fn peer_write_blocked(&self) -> bool {
        let shared = lock(&self.shared);
        shared.queues[1 - self.side].len() >= shared.capacity
    }
}

impl EndControl {
    fn with<R>(&self, f: impl FnOnce(&mut SideControl) -> R) -> R {
        f(&mut lock(&self.shared).control[self.side])
    }

    /// While `on`, the stream takes no byte from this end's owner; `route_accept` limits end with it (`route_gate`, Core OU-2,
    /// OU-3b). `on: false` releases the gate and the limit.
    pub fn gate(&self, on: bool) {
        self.with(|c| {
            c.gate = on;
            if !on {
                c.accept = None;
            }
        });
    }

    /// The stream takes at most `bytes` more bytes from this end's owner, then none until [`EndControl::gate`] with `false`
    /// (`route_accept`, Core OU-3a, OU-4).
    pub fn accept_at_most(&self, bytes: usize) {
        self.with(|c| c.accept = Some(bytes));
    }

    /// The next write of this end's owner fails with `kind` (`fail_writes`, Core OU-2b, A2-3).
    pub fn fail_next_write(&self, kind: io::ErrorKind) {
        self.with(|c| c.fail_write = Some(kind));
    }

    /// The next read of this end's owner fails with `kind`, once: `Interrupted`, `WouldBlock`, or a lost stream.
    pub fn fail_next_read(&self, kind: io::ErrorKind) {
        self.with(|c| c.fail_read = Some(kind));
    }

    /// Lifts a pending write failure.
    pub fn clear_write_failure(&self) {
        self.with(|c| c.fail_write = None);
    }

    /// The end reports write readiness although it takes no byte (`route_spurious_ready`, Core OU-6).
    pub fn spurious_writable(&self, on: bool) {
        self.with(|c| c.spurious_writable = on);
    }

    /// Each read of this end returns at most `bytes` bytes (`route_read_size`, Core A5-2); `None` removes the limit.
    pub fn read_at_most(&self, bytes: Option<usize>) {
        self.with(|c| c.read_cap = bytes);
    }

    /// The next descriptor that this end hands to its peer fails; the descriptor is dropped, so the object in it closes
    /// (`fail_handoff`, Core DP-2: the route closes `HandoffFailed` and Core closes the descriptor).
    pub fn fail_next_handoff(&self) {
        self.with(|c| c.fail_descriptor = true);
    }

    /// The controls of the other end of the duplex: a client's control of the worker's end of its route stream.
    pub fn peer(&self) -> EndControl {
        EndControl {
            shared: Arc::clone(&self.shared),
            side: 1 - self.side,
        }
    }

    /// The bytes that this end's owner has written so far, which the stream took (`route_stream_written`, R-14.3).
    pub fn written(&self) -> u64 {
        lock(&self.shared).sent[self.side]
    }

    /// The owner of this side holds it from now on (the worker's bind of a route stream, DP-2): the bytes through it are the
    /// owner's, and `peer_holders` does not count them.
    pub fn owned(&self) {
        lock(&self.shared).owned[self.side] = true;
    }

    /// The peer side's holders, as `route_stream_holders` reads them from the client's end (Core DP-2, OU-1): the number of
    /// live handles of the peer's end (0 or 1: an end is not cloned), and the bytes read from or written to it before its
    /// owner held it.
    pub fn peer_holders(&self) -> (usize, u64) {
        let shared = lock(&self.shared);
        let peer = 1 - self.side;
        (usize::from(shared.live[peer]), shared.foreign[peer])
    }

    /// The stream is reset with no close handshake: every later read and write of both ends fails with `ConnectionReset`
    /// (`drop_transport`, Core OU-5).
    pub fn reset(&self) {
        lock(&self.shared).reset = true;
    }

    /// True while this side holds a report that its peer has not consumed: bytes or a descriptor that it wrote, or its own
    /// close (the peer's end of file), and the peer's end is still open. It reads the state and changes nothing
    /// (`edges_quiet`).
    pub fn holds_for_peer(&self) -> bool {
        let shared = lock(&self.shared);
        let (me, peer) = (self.side, 1 - self.side);
        !shared.closed[peer]
            && (!shared.queues[me].is_empty()
                || !shared.descriptors[me].is_empty()
                || shared.closed[me])
    }
}

impl Drop for End {
    /// A dropped end is a closed end (the connection boundary of a real stream: the peer sees the close).
    fn drop(&mut self) {
        self.close();
        lock(&self.shared).live[self.side] = false;
    }
}

/// The in-memory control link (plan 2.3, `Link`): a duplex that also passes descriptor objects by value.
#[derive(Debug)]
pub struct LinkEnd(End);

/// A connected link: the two ends, each with `capacity` bytes of queue per direction.
pub fn link_pair(capacity: usize) -> (LinkEnd, LinkEnd) {
    let (a, b) = End::pair(capacity);
    (LinkEnd(a), LinkEnd(b))
}

impl LinkEnd {
    pub fn end(&mut self) -> &mut End {
        &mut self.0
    }

    pub fn is_ready(&self) -> bool {
        self.0.is_ready()
    }

    /// Hands a descriptor to the peer with the first byte of `bytes`, as `sendmsg` with `SCM_RIGHTS` does: `Ok(n)`, `n >= 1`,
    /// when the link took the descriptor and the first `n` bytes. On an error nothing was taken, and the descriptor comes
    /// back: `WouldBlock` when the link takes no byte now, another kind when the handoff failed (DESIGN.md "The handoff").
    ///
    /// Clause: Core DP-2.
    pub fn send_with_descriptor(
        &mut self,
        bytes: &[u8],
        descriptor: Descriptor,
    ) -> Result<usize, (Descriptor, io::Error)> {
        let mut shared = lock(&self.0.shared);
        let me = self.0.side;
        if bytes.is_empty() {
            return Err((descriptor, io::ErrorKind::InvalidInput.into()));
        }
        if std::mem::take(&mut shared.control[me].fail_descriptor) {
            return Err((descriptor, io::ErrorKind::ConnectionReset.into()));
        }
        let at = shared.sent[me];
        match End::write_locked(&mut shared, me, bytes) {
            Ok(n) => {
                shared.descriptors[me].push_back((at, descriptor));
                Ok(n)
            }
            Err(error) => Err((descriptor, error)),
        }
    }

    /// The next descriptor that the peer sent, once every byte before its own byte was read (`recvmsg` returns it with that
    /// byte). A read stops before that byte until the descriptor is taken.
    pub fn recv_descriptor(&mut self) -> Option<Descriptor> {
        let mut shared = lock(&self.0.shared);
        let peer = 1 - self.0.side;
        let taken = shared.taken[peer];
        match shared.descriptors[peer].front() {
            Some((at, _)) if *at == taken => shared.descriptors[peer].pop_front().map(|(_, d)| d),
            _ => None,
        }
    }
}

impl Link for LinkEnd {
    fn send(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.write(bytes)
    }

    fn recv(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let max = buf.len();
        self.0.read_up_to(buf, max)
    }

    fn close(&mut self) {
        self.0.close();
    }
}

/// The in-memory connected stream of a route (plan 2.3, `RouteTransport`). The size of each read is a choice point of the
/// scheduler.
///
/// Clause: Core A5-2 ("the route-transport edge varies its read sizes").
#[derive(Debug)]
pub struct StreamEnd {
    end: End,
    scheduler: SchedulerHandle,
}

/// A connected stream: the two ends, each with `capacity` bytes of queue per direction.
pub fn stream_pair(scheduler: &SchedulerHandle, capacity: usize) -> (StreamEnd, StreamEnd) {
    let (a, b) = End::pair(capacity);
    let stream = |end| StreamEnd {
        end,
        scheduler: scheduler.clone(),
    };
    (stream(a), stream(b))
}

impl StreamEnd {
    pub fn end(&mut self) -> &mut End {
        &mut self.end
    }

    pub fn is_ready(&self) -> bool {
        self.end.is_ready()
    }
}

impl RouteTransport for StreamEnd {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let available = self.end.available().min(buf.len());
        if available == 0 {
            // Nothing to read (WouldBlock, or the peer closed): no draw, so the stream of choices does not depend on how
            // often a caller polls.
            return self.end.read_up_to(buf, buf.len());
        }
        let max = self.scheduler.with(|s| s.route_read_size(available));
        self.end.read_up_to(buf, max)
    }

    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.end.write(bytes)
    }

    fn close(&mut self) {
        self.end.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Hands `descriptor` over with one byte, as the host's writer does with the first byte of a frame.
    fn hand(link: &mut LinkEnd, descriptor: Descriptor) -> Result<(), io::ErrorKind> {
        link.send_with_descriptor(b"d", descriptor)
            .map(|n| assert_eq!(n, 1))
            .map_err(|(_, error)| error.kind())
    }

    /// Takes the next descriptor and the byte that it rides with.
    fn take(link: &mut LinkEnd) -> Option<Descriptor> {
        let descriptor = link.recv_descriptor()?;
        assert_eq!(link.recv(&mut [0u8; 1]).unwrap(), 1);
        Some(descriptor)
    }

    fn drain(link: &mut LinkEnd) -> Vec<u8> {
        let mut out = Vec::new();
        let mut buf = [0u8; 64];
        while let Ok(n) = link.recv(&mut buf) {
            if n == 0 {
                break;
            }
            out.extend_from_slice(&buf[..n]);
        }
        out
    }

    #[test]
    fn bytes_cross_in_order_and_a_full_queue_blocks() {
        let (mut a, mut b) = link_pair(4);
        assert_eq!(a.send(b"abcdef").unwrap(), 4);
        assert_eq!(a.send(b"ef").unwrap_err().kind(), io::ErrorKind::WouldBlock);
        assert_eq!(drain(&mut b), b"abcd");
        assert_eq!(
            b.recv(&mut [0u8; 4]).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        assert_eq!(a.send(b"ef").unwrap(), 2);
        assert_eq!(drain(&mut b), b"ef");
    }

    /// A closed end gives the peer the queued bytes, then `Ok(0)`; a write to it fails (the `Link` and `RouteTransport` docs).
    #[test]
    fn a_close_reaches_the_peer_after_the_queued_bytes() {
        let (mut a, mut b) = link_pair(8);
        a.send(b"xy").unwrap();
        Link::close(&mut a);
        assert_eq!(drain(&mut b), b"xy");
        assert_eq!(b.recv(&mut [0u8; 4]).unwrap(), 0);
        assert_eq!(b.send(b"z").unwrap_err().kind(), io::ErrorKind::BrokenPipe);
    }

    /// Plan 2.5 rule 8: an end is ready work only when a set flag matches an allowed interest.
    #[test]
    fn readiness_follows_the_interest_rules() {
        let (mut a, mut b) = link_pair(4);
        // Nothing queued: writable only.
        assert_eq!(
            b.end().readiness(),
            Readiness {
                readable: false,
                writable: true
            }
        );
        assert!(!b.is_ready(), "no interest registered");
        b.end().set_interest(Interest {
            read: true,
            write: false,
        });
        assert!(!b.is_ready(), "no bytes, and no write interest");
        a.send(b"ab").unwrap();
        assert!(b.is_ready(), "bytes and read interest");
        // The owner removes read interest (a parked link, rule 7): the flag stays set and the link is not work.
        b.end().set_interest(Interest {
            read: false,
            write: false,
        });
        assert!(b.end().readiness().readable);
        assert!(!b.is_ready());
        // Write interest only counts while the queue has room.
        b.end().set_interest(Interest {
            read: false,
            write: true,
        });
        assert!(b.is_ready());
        b.send(b"wxyz").unwrap();
        assert!(!b.end().readiness().writable);
        assert!(!b.is_ready());
        // The peer's close is readable, so a loop with read interest learns of it.
        Link::close(&mut a);
        b.end().set_interest(Interest {
            read: true,
            write: false,
        });
        assert!(b.is_ready());
    }

    /// Plan 2.3: descriptor objects pass by value, in order, and a closed peer refuses them.
    #[test]
    fn descriptors_pass_by_value() {
        let sched = SchedulerHandle::with_seed(0);
        let (mut a, mut b) = link_pair(4);
        let (route_worker, mut route_client) = stream_pair(&sched, 8);
        hand(&mut a, Descriptor::new(route_worker)).unwrap();
        hand(&mut a, Descriptor::new(7u32)).unwrap();
        assert!(
            b.end().readiness().readable,
            "a descriptor is readable work"
        );
        let mut worker_end = take(&mut b).unwrap().downcast::<StreamEnd>().unwrap();
        assert_eq!(take(&mut b).unwrap().downcast::<u32>().unwrap(), 7);
        assert!(take(&mut b).is_none());
        // The descriptor is the very stream end, still connected to the client.
        route_client.write(b"hi").unwrap();
        let mut buf = [0u8; 8];
        let n = worker_end.read(&mut buf).unwrap();
        assert_eq!(&buf[..n], &b"hi"[..n]);
        // A wrong type returns the descriptor.
        hand(&mut a, Descriptor::new(1u8)).unwrap();
        assert!(take(&mut b).unwrap().downcast::<u32>().is_err());
    }

    /// DP-2 (DESIGN.md "The handoff"): a descriptor rides with one byte. A read stops before that byte until the descriptor is
    /// taken, and the descriptor is offered only once every earlier byte was read, so the receiver always has the descriptor
    /// before the frame that it starts. A link that takes no byte gives the descriptor back.
    #[test]
    fn a_descriptor_arrives_with_its_byte_and_never_before_earlier_bytes() {
        let (mut a, mut b) = link_pair(8);
        a.send(b"ab").unwrap();
        assert_eq!(
            a.send_with_descriptor(b"cd", Descriptor::new(1u8)).unwrap(),
            2
        );
        a.send(b"e").unwrap();
        assert!(
            b.recv_descriptor().is_none(),
            "two earlier bytes are unread"
        );
        let mut buf = [0u8; 8];
        assert_eq!(
            b.recv(&mut buf).unwrap(),
            2,
            "the read stops before the descriptor's byte"
        );
        assert_eq!(&buf[..2], b"ab");
        assert_eq!(
            b.recv(&mut buf).unwrap_err().kind(),
            io::ErrorKind::WouldBlock,
            "no byte after the descriptor is read before it"
        );
        assert!(
            b.end().readiness().readable,
            "the descriptor is readable work"
        );
        assert_eq!(b.recv_descriptor().unwrap().downcast::<u8>().unwrap(), 1);
        assert_eq!(drain(&mut b), b"cde");

        let (mut a, _b) = link_pair(1);
        a.send(b"x").unwrap();
        let (back, error) = a
            .send_with_descriptor(b"y", Descriptor::new(2u8))
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
        assert_eq!(
            back.downcast::<u8>().unwrap(),
            2,
            "a blocked send gives the descriptor back"
        );
        assert_eq!(
            a.send_with_descriptor(b"", Descriptor::new(3u8))
                .unwrap_err()
                .1
                .kind(),
            io::ErrorKind::InvalidInput,
            "a descriptor needs a byte to ride with"
        );
    }

    /// A5-2: the read size of a route stream is a seed-chosen number of bytes. Every byte arrives in order whatever the sizes,
    /// the sizes vary with the seed, and the same seed repeats them.
    #[test]
    fn a_seed_chooses_the_read_sizes() {
        let sizes = |seed| {
            let sched = SchedulerHandle::with_seed(seed);
            let (mut worker, mut client) = stream_pair(&sched, 64);
            let data: Vec<u8> = (0..40).collect();
            client.write(&data).unwrap();
            let (mut got, mut sizes) = (Vec::new(), Vec::new());
            let mut buf = [0u8; 16];
            while got.len() < data.len() {
                let n = worker.read(&mut buf).unwrap();
                assert!(n >= 1);
                sizes.push(n);
                got.extend_from_slice(&buf[..n]);
            }
            assert_eq!(got, data);
            sizes
        };
        assert_eq!(sizes(1), sizes(1));
        assert!((2..12).any(|seed| sizes(seed) != sizes(1)));
    }

    #[test]
    fn a_stream_reports_would_block_peer_close_and_a_write_failure() {
        let sched = SchedulerHandle::with_seed(0);
        let (mut worker, mut client) = stream_pair(&sched, 8);
        assert_eq!(
            worker.read(&mut [0u8; 4]).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        client.end().fail_next_write(io::ErrorKind::ConnectionReset);
        assert_eq!(
            client.write(b"a").unwrap_err().kind(),
            io::ErrorKind::ConnectionReset
        );
        assert_eq!(client.write(b"a").unwrap(), 1, "the failure is one write");
        RouteTransport::close(&mut client);
        assert_eq!(worker.read(&mut [0u8; 4]).unwrap(), 1);
        assert_eq!(worker.read(&mut [0u8; 4]).unwrap(), 0);
    }

    /// A dropped end closes its side: the survivor reads the queued bytes, then `Ok(0)`, its writes fail, and it sees the close.
    #[test]
    fn a_dropped_link_end_closes_its_side() {
        let (mut a, mut b) = link_pair(8);
        a.send(b"xy").unwrap();
        drop(a);
        b.end().set_interest(Interest {
            read: true,
            write: true,
        });
        assert_eq!(drain(&mut b), b"xy", "queued bytes are kept");
        assert_eq!(b.recv(&mut [0u8; 4]).unwrap(), 0);
        assert_eq!(b.send(b"z").unwrap_err().kind(), io::ErrorKind::BrokenPipe);
        assert!(
            b.is_ready(),
            "the close is readable and the failing write is writable"
        );
    }

    /// The same for a route stream, and for a stream end that moved into a descriptor and is dropped with it.
    #[test]
    fn a_dropped_route_end_closes_its_side() {
        let sched = SchedulerHandle::with_seed(0);
        let (worker, mut client) = stream_pair(&sched, 8);
        drop(worker);
        assert_eq!(client.read(&mut [0u8; 4]).unwrap(), 0);
        assert_eq!(
            client.write(b"z").unwrap_err().kind(),
            io::ErrorKind::BrokenPipe
        );

        let (mut a, b) = link_pair(4);
        let (worker, mut client) = stream_pair(&sched, 8);
        hand(&mut a, Descriptor::new(worker)).unwrap();
        // The receiver closes without taking the descriptor: the descriptor, and the route end in it, are released.
        drop(b);
        assert_eq!(client.read(&mut [0u8; 4]).unwrap(), 0);
        assert_eq!(
            client.write(b"z").unwrap_err().kind(),
            io::ErrorKind::BrokenPipe
        );
    }

    /// A5-3: an injected write failure is write-ready work for an open end, even while the queue is full.
    #[test]
    fn an_injected_write_failure_is_write_ready_on_a_full_queue() {
        let sched = SchedulerHandle::with_seed(0);
        let (mut worker, _client) = stream_pair(&sched, 2);
        worker.end().set_interest(Interest {
            read: false,
            write: true,
        });
        worker.write(b"ab").unwrap();
        assert!(!worker.is_ready(), "full queue, no failure pending");
        worker.end().fail_next_write(io::ErrorKind::ConnectionReset);
        assert!(worker.is_ready());
        assert_eq!(
            worker.write(b"c").unwrap_err().kind(),
            io::ErrorKind::ConnectionReset
        );
        assert!(!worker.is_ready(), "the failure was delivered");
    }

    /// A descriptor is debug-printed without its content.
    #[test]
    fn a_descriptor_prints_its_name() {
        assert_eq!(format!("{:?}", Descriptor::new(1u8)), "Descriptor");
    }

    /// The interest that the owner set is the interest that it reads back.
    #[test]
    fn the_interest_reads_back() {
        let (mut a, _b) = link_pair(2);
        assert_eq!(a.end().interest(), Interest::default());
        a.end().set_interest(Interest {
            read: true,
            write: false,
        });
        assert_eq!(
            a.end().interest(),
            Interest {
                read: true,
                write: false
            }
        );
    }

    /// A descriptor cannot be sent when either end is closed, and an open pair accepts it.
    #[test]
    fn a_descriptor_needs_two_open_ends() {
        let (mut a, b) = link_pair(2);
        hand(&mut a, Descriptor::new(1u8)).unwrap();
        drop(b);
        assert_eq!(
            hand(&mut a, Descriptor::new(2u8)).unwrap_err(),
            io::ErrorKind::BrokenPipe
        );
        let (mut a, _b) = link_pair(2);
        Link::close(&mut a);
        assert_eq!(
            hand(&mut a, Descriptor::new(3u8)).unwrap_err(),
            io::ErrorKind::BrokenPipe
        );
    }

    /// Either end can send a descriptor, and the second end finds its peer too.
    #[test]
    fn the_second_end_sends_descriptors_too() {
        let (a, mut b) = link_pair(2);
        let mut a = a;
        hand(&mut b, Descriptor::new(5u8)).unwrap();
        assert_eq!(take(&mut a).unwrap().downcast::<u8>().unwrap(), 5);
        drop(a);
        assert_eq!(
            hand(&mut b, Descriptor::new(6u8)).unwrap_err(),
            io::ErrorKind::BrokenPipe
        );
    }

    fn pair_of_streams() -> (StreamEnd, StreamEnd) {
        stream_pair(&SchedulerHandle::with_seed(0), 8)
    }

    /// `route_stream_holders`: the client's end reads the worker end's live handles and the bytes that moved through it before
    /// its owner held it; `route_stream_written`: the bytes that the worker end wrote. `peer` reaches the worker end's
    /// controls from the client's end.
    #[test]
    fn the_client_end_reads_the_worker_ends_holders_and_written_bytes() {
        let (mut worker, mut client) = pair_of_streams();
        let peer = client.end().control().peer();
        assert_eq!(client.end().control().peer_holders(), (1, 0));
        assert_eq!(worker.write(b"ab").unwrap(), 2);
        assert_eq!(client.write(b"c").unwrap(), 1);
        let mut buf = [0u8; 8];
        assert_eq!(worker.read(&mut buf).unwrap(), 1);
        assert_eq!(
            client.end().control().peer_holders(),
            (1, 3),
            "bytes before the owner holds the end are not the owner's"
        );
        worker.end().control().owned();
        assert_eq!(worker.write(b"de").unwrap(), 2);
        assert_eq!(client.write(b"f").unwrap(), 1);
        assert_eq!(worker.read(&mut buf).unwrap(), 1);
        assert_eq!(
            client.end().control().peer_holders(),
            (1, 3),
            "the owner's bytes"
        );
        assert_eq!(peer.written(), 4);
        assert_eq!(
            client.end().control().written(),
            2,
            "the client's own writes"
        );
        peer.gate(true);
        assert!(matches!(worker.write(b"g"), Err(e) if e.kind() == io::ErrorKind::WouldBlock));
        drop(worker);
        assert_eq!(
            client.end().control().peer_holders(),
            (0, 3),
            "the dropped end has no holder"
        );
        assert_eq!(peer.written(), 4);
    }

    /// `route_gate`: the stream takes no byte from the worker while on, reports itself not writable, and takes bytes again when
    /// released. The other direction is not affected.
    #[test]
    fn a_gated_end_takes_no_byte_and_is_not_writable() {
        let (mut worker, mut client) = pair_of_streams();
        worker.end().set_interest(Interest {
            read: false,
            write: true,
        });
        let gate = worker.end().control();
        gate.gate(true);
        assert_eq!(
            worker.write(b"x").unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        assert!(!worker.is_ready());
        assert_eq!(
            client.write(b"y").unwrap(),
            1,
            "the other direction is open"
        );
        gate.gate(false);
        assert!(worker.is_ready());
        assert_eq!(worker.write(b"x").unwrap(), 1);
    }

    /// `route_accept`: at most N more bytes, a straddling write takes its prefix, then none until the gate is released.
    #[test]
    fn an_accept_limit_takes_a_prefix_and_then_blocks() {
        let (mut worker, _client) = pair_of_streams();
        let control = worker.end().control();
        control.accept_at_most(3);
        assert_eq!(worker.write(b"abcdef").unwrap(), 3);
        assert_eq!(
            worker.write(b"d").unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        worker.end().set_interest(Interest {
            read: false,
            write: true,
        });
        assert!(!worker.is_ready());
        control.gate(false);
        assert_eq!(worker.write(b"def").unwrap(), 3);
        control.accept_at_most(0);
        assert_eq!(
            worker.write(b"g").unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
    }

    /// `route_spurious_ready`: the end reports write readiness while it takes no byte (the report is not progress, OU-3a).
    #[test]
    fn a_spurious_ready_end_reports_writable_and_takes_nothing() {
        let (mut worker, _client) = pair_of_streams();
        worker.end().set_interest(Interest {
            read: false,
            write: true,
        });
        let control = worker.end().control();
        control.gate(true);
        assert!(!worker.is_ready());
        control.spurious_writable(true);
        assert!(worker.is_ready());
        assert_eq!(
            worker.write(b"x").unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        control.spurious_writable(false);
        assert!(!worker.is_ready());
    }

    /// `fail_writes`: the next write fails once; the handle works after the end moved.
    #[test]
    fn a_write_failure_can_be_scripted_through_a_handle() {
        let (mut worker, _client) = pair_of_streams();
        let control = worker.end().control();
        let mut moved = Descriptor::new(worker)
            .downcast::<StreamEnd>()
            .ok()
            .unwrap();
        control.fail_next_write(io::ErrorKind::BrokenPipe);
        assert_eq!(
            moved.write(b"a").unwrap_err().kind(),
            io::ErrorKind::BrokenPipe
        );
        assert_eq!(moved.write(b"a").unwrap(), 1);
        control.fail_next_write(io::ErrorKind::BrokenPipe);
        control.clear_write_failure();
        assert_eq!(
            moved.write(b"b").unwrap(),
            1,
            "a lifted failure does not fire"
        );
    }

    /// `drop_transport`: a reset fails every call of both ends with `ConnectionReset`, and a reset end is readable and writable
    /// so a loop learns of it.
    #[test]
    fn a_reset_fails_both_ends() {
        let (mut worker, mut client) = pair_of_streams();
        worker.write(b"a").unwrap();
        client.end().control().reset();
        for end in [&mut worker, &mut client] {
            end.end().set_interest(Interest {
                read: true,
                write: true,
            });
            assert!(end.is_ready());
            assert_eq!(
                end.read(&mut [0u8; 4]).unwrap_err().kind(),
                io::ErrorKind::ConnectionReset
            );
            assert_eq!(
                end.write(b"x").unwrap_err().kind(),
                io::ErrorKind::ConnectionReset
            );
        }
    }

    /// `route_read_size`: each read returns at most this many bytes, whatever the seed chooses.
    #[test]
    fn a_read_cap_bounds_every_read() {
        let (mut worker, mut client) = pair_of_streams();
        client.write(&[7u8; 8]).unwrap();
        worker.end().control().read_at_most(Some(2));
        let mut buf = [0u8; 8];
        for _ in 0..4 {
            assert!(worker.read(&mut buf).unwrap() <= 2);
        }
        worker.end().control().read_at_most(None);
        client.write(&[7u8; 8]).unwrap();
        let mut total = 0;
        while total < 8 {
            total += worker.read(&mut buf).unwrap();
        }
    }

    /// A scripted read failure fails the next read once, and takes no byte.
    #[test]
    fn a_read_failure_fails_one_read_and_keeps_the_bytes() {
        let (mut worker, mut client) = pair_of_streams();
        client.write(b"ab").unwrap();
        worker
            .end()
            .control()
            .fail_next_read(io::ErrorKind::Interrupted);
        let mut buf = [0u8; 8];
        assert_eq!(
            worker.read(&mut buf).unwrap_err().kind(),
            io::ErrorKind::Interrupted
        );
        let mut got = Vec::new();
        while got.len() < 2 {
            let n = worker.read(&mut buf).unwrap();
            got.extend_from_slice(&buf[..n]);
        }
        assert_eq!(got, b"ab");
    }

    /// `input_blocked`: the end sees whether its peer's writes toward it are blocked, which is the worker's view of a full input
    /// queue; `fail_handoff`: the next descriptor fails and is dropped.
    #[test]
    fn a_full_queue_blocks_the_peer_and_a_handoff_can_fail() {
        let (mut worker, mut client) = pair_of_streams();
        assert!(!worker.end().peer_write_blocked());
        client.write(&[1u8; 8]).unwrap();
        assert!(worker.end().peer_write_blocked());
        let mut buf = [0u8; 8];
        worker.read(&mut buf).unwrap();

        let (mut a, mut b) = link_pair(2);
        let (end, mut other) = pair_of_streams();
        a.end().control().fail_next_handoff();
        assert_eq!(
            hand(&mut a, Descriptor::new(end)).unwrap_err(),
            io::ErrorKind::ConnectionReset
        );
        assert!(take(&mut b).is_none());
        assert_eq!(
            other.read(&mut buf).unwrap(),
            0,
            "the dropped descriptor closed its stream end"
        );
        let (end, _other) = pair_of_streams();
        hand(&mut a, Descriptor::new(end)).unwrap();
        assert!(take(&mut b).is_some(), "only one handoff failed");
    }

    /// `edges_quiet`: a side holds a report for its peer while it has written bytes or a descriptor that the peer has not
    /// taken, or has closed without the peer having closed; a closed peer takes nothing more, so nothing is held for it.
    #[test]
    fn a_side_holds_what_its_peer_has_not_consumed() {
        let (mut a, mut b) = link_pair(64);
        let held = a.end().control();
        assert!(!held.holds_for_peer(), "a fresh link holds nothing");
        a.send(b"x").unwrap();
        assert!(held.holds_for_peer(), "bytes the peer has not read");
        assert_eq!(drain(&mut b), b"x");
        assert!(!held.holds_for_peer());
        let (end, _other) = pair_of_streams();
        hand(&mut a, Descriptor::new(end)).unwrap();
        assert!(
            held.holds_for_peer(),
            "a descriptor the peer has not received"
        );
        assert!(take(&mut b).is_some());
        assert!(!held.holds_for_peer());
        assert!(
            !b.end().control().holds_for_peer(),
            "the other side wrote nothing"
        );
        a.end().close();
        assert!(
            held.holds_for_peer(),
            "an end of file the peer has not read"
        );
        b.end().close();
        assert!(!held.holds_for_peer(), "a closed peer takes nothing more");
        let (mut c, mut d) = link_pair(64);
        c.send(b"y").unwrap();
        d.end().close();
        assert!(
            !c.end().control().holds_for_peer(),
            "bytes toward a closed peer are not a report"
        );
    }
}
