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

#[derive(Debug)]
struct Shared {
    /// `queues[s]` holds the bytes that side `s` wrote and side `1 - s` has not read.
    queues: [VecDeque<u8>; 2],
    descriptors: [VecDeque<Descriptor>; 2],
    capacity: usize,
    closed: [bool; 2],
}

/// One end of a duplex.
#[derive(Debug)]
pub struct End {
    shared: Arc<Mutex<Shared>>,
    side: usize,
    interest: Interest,
    write_error: Option<io::ErrorKind>,
}

fn lock(shared: &Mutex<Shared>) -> MutexGuard<'_, Shared> {
    shared.lock().unwrap_or_else(PoisonError::into_inner)
}

impl End {
    fn pair(capacity: usize) -> (End, End) {
        let shared = Arc::new(Mutex::new(Shared {
            queues: [VecDeque::new(), VecDeque::new()],
            descriptors: [VecDeque::new(), VecDeque::new()],
            capacity,
            closed: [false, false],
        }));
        let end = |side| End {
            shared: Arc::clone(&shared),
            side,
            interest: Interest::default(),
            write_error: None,
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
            && (!shared.queues[peer].is_empty()
                || !shared.descriptors[peer].is_empty()
                || shared.closed[peer]);
        let writable = !shared.closed[me]
            && (self.write_error.is_some()
                || shared.closed[peer]
                || shared.queues[me].len() < shared.capacity);
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
        let queue = &mut shared.queues[peer];
        if queue.is_empty() {
            return if shared.closed[peer] {
                Ok(0)
            } else {
                Err(io::ErrorKind::WouldBlock.into())
            };
        }
        let n = queue.len().min(buf.len()).min(max);
        for slot in &mut buf[..n] {
            *slot = queue.pop_front().unwrap_or_default();
        }
        Ok(n)
    }

    /// Writes as many bytes as the queue has room for.
    pub fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if let Some(kind) = self.write_error.take() {
            return Err(kind.into());
        }
        let mut shared = lock(&self.shared);
        let (me, peer) = (self.side, 1 - self.side);
        if shared.closed[me] || shared.closed[peer] {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        let room = shared.capacity - shared.queues[me].len();
        if room == 0 && !bytes.is_empty() {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        let n = room.min(bytes.len());
        shared.queues[me].extend(&bytes[..n]);
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
            shared.descriptors[1 - self.side].drain(..).collect()
        };
        // Dropped outside the lock: a descriptor can hold an endpoint of another duplex, whose drop takes its own lock.
        drop(released);
    }

    /// The next write fails with `kind` (A5-3: a failed write of the route transport).
    pub fn fail_next_write(&mut self, kind: io::ErrorKind) {
        self.write_error = Some(kind);
    }
}

impl Drop for End {
    /// A dropped end is a closed end (the connection boundary of a real stream: the peer sees the close).
    fn drop(&mut self) {
        self.close();
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

    /// Hands a descriptor to the peer. It never blocks (`SCM_RIGHTS` rides with bytes that the link already carries).
    ///
    /// Clause: Core DP-2.
    pub fn send_descriptor(&mut self, descriptor: Descriptor) -> io::Result<()> {
        let mut shared = lock(&self.0.shared);
        let (me, peer) = (self.0.side, 1 - self.0.side);
        if shared.closed[me] || shared.closed[peer] {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        shared.descriptors[me].push_back(descriptor);
        Ok(())
    }

    /// The next descriptor that the peer sent, in order.
    pub fn recv_descriptor(&mut self) -> Option<Descriptor> {
        let mut shared = lock(&self.0.shared);
        let peer = 1 - self.0.side;
        shared.descriptors[peer].pop_front()
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
        a.send_descriptor(Descriptor::new(route_worker)).unwrap();
        a.send_descriptor(Descriptor::new(7u32)).unwrap();
        assert!(
            b.end().readiness().readable,
            "a descriptor is readable work"
        );
        let mut worker_end = b
            .recv_descriptor()
            .unwrap()
            .downcast::<StreamEnd>()
            .unwrap();
        assert_eq!(b.recv_descriptor().unwrap().downcast::<u32>().unwrap(), 7);
        assert!(b.recv_descriptor().is_none());
        // The descriptor is the very stream end, still connected to the client.
        route_client.write(b"hi").unwrap();
        let mut buf = [0u8; 8];
        let n = worker_end.read(&mut buf).unwrap();
        assert_eq!(&buf[..n], &b"hi"[..n]);
        // A wrong type returns the descriptor.
        a.send_descriptor(Descriptor::new(1u8)).unwrap();
        assert!(b.recv_descriptor().unwrap().downcast::<u32>().is_err());
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
        a.send_descriptor(Descriptor::new(worker)).unwrap();
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
        a.send_descriptor(Descriptor::new(1u8)).unwrap();
        drop(b);
        assert_eq!(
            a.send_descriptor(Descriptor::new(2u8)).unwrap_err().kind(),
            io::ErrorKind::BrokenPipe
        );
        let (mut a, _b) = link_pair(2);
        Link::close(&mut a);
        assert_eq!(
            a.send_descriptor(Descriptor::new(3u8)).unwrap_err().kind(),
            io::ErrorKind::BrokenPipe
        );
    }

    /// Either end can send a descriptor, and the second end finds its peer too.
    #[test]
    fn the_second_end_sends_descriptors_too() {
        let (a, mut b) = link_pair(2);
        let mut a = a;
        b.send_descriptor(Descriptor::new(5u8)).unwrap();
        assert_eq!(a.recv_descriptor().unwrap().downcast::<u8>().unwrap(), 5);
        drop(a);
        assert_eq!(
            b.send_descriptor(Descriptor::new(6u8)).unwrap_err().kind(),
            io::ErrorKind::BrokenPipe
        );
    }
}
