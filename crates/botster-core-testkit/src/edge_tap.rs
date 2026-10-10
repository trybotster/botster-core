//! The pass-through wrapper of a host's production edges (plan 23l): the RealCoreHarness composes
//! `HostDriver<EdgeTap<RealEdges>>` over `botster_core::open_parts`, so every id runs on Core's own composition with the
//! edges observed, not replaced. Core has no test branch.
//!
//! The boundary is `HostEdges` itself (lead ruling, plan 23l). Every inbound edge is a "take the next item" call
//! (`link_recv`, `accept_link`, `poll_process_exit`), so the tap can take ahead, hold what it took, and hand it out unchanged
//! and in order. The driver's read of a link can stop before `WouldBlock`: at its read budget, or at input that it must
//! hold (plan 23l). So an item can wait more than one pump. While the tap holds an item, the edges are not quiet
//! ([`Tap::quiet`]); while the driver's own read stopped early, it reports more work (`report.more`). The runner pumps
//! until both are clear. The tap signals the wake edge so that a pump comes.
//!
//! - `edges_quiet` ([`Tap::quiet`]): the tap holds nothing that it has not handed out, and a bounded zero-wait take finds
//!   nothing new on any edge. Bytes still inside a worker process are invisible here: quiet means "nothing arrived and nothing is
//!   unread", never "the worker finished".
//! - `break_control` ([`Tap::break_link`]): the inner edge's own `link_close`. Core's end of the stream is dropped, so the
//!   worker reads EOF and its writes fail; every later call on that `LinkId` reaches the inner edge's closed-link state
//!   (`Ok(0)` on `link_recv`, `BrokenPipe` on `link_send`), never a raw descriptor (steward condition, plan 23l).
//! - The record of the processes: every registry row that passes through `write_row` or `read_rows` is decoded with Core's
//!   own decoder (`Row::decode`), so the harness knows each session's instance, worker and payload identity; the hello of
//!   each link names its instance.
//! - The harness's own calls of the inner edges, which the tap records as no call of Core: a stored row and its write
//!   ([`Tap::stored_row`], [`Tap::store_row`]; `corrupt_registry_row`), the identity check ([`Tap::identity_state`];
//!   `payload_alive`) and the identity-checked kill of a recorded process group ([`Tap::kill_group`]; `lose_worker`).

use botster_core_contract::prelude::*;
use botster_core_edges::edges::{
    ExitStatus, GroupSignal, IdentityState, ProcessIdentity, SpawnError, StorageError,
};
use botster_core_edges::scheduler::{ChoicePoint, Scheduler};
use botster_core_host::driver::{DescriptorSendError, HostEdges, HostWake, WorkerSpawn};
use botster_core_host::session::{Row, ROW_PREFIX};
use botster_core_host::LinkId;
use botster_core_link::frame::{FrameDecoder, FrameType, DEFAULT_MAX_PAYLOAD};
use botster_core_link::hello::Hello;
use botster_core_link::msg::WorkerMsg;
use std::collections::{BTreeMap, VecDeque};
use std::io;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};

/// The bytes that one take-ahead read asks for.
const READ_CHUNK: usize = 16_384;

/// Locks a mutex of the tap. A panic while it was held leaves plain data behind, so a poisoned lock is still usable.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// How a link ended, held until the driver reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum End {
    /// `Ok(0)`: the peer closed the link.
    Closed,
    /// A read error of that kind.
    Failed(io::ErrorKind),
}

/// What the tap knows about one link.
#[derive(Debug)]
struct LinkTap {
    /// Bytes taken ahead, not yet handed to the driver.
    held: VecDeque<u8>,
    /// The end that a take-ahead met after `held`, not yet handed to the driver.
    end: Option<End>,
    /// The decoder of the link's inbound frames: from the first frame until the worker's `Launched` report is read, or until
    /// a frame is not one that the tap reads.
    frames: Option<FrameDecoder>,
    /// The instance that the link's hello names, or that `connect_worker` asked for.
    instance: Option<InstanceId>,
    /// The payload that the worker's `Launched` report names (Core LC-5). The host keeps it in memory and writes it to the
    /// session's row only at a later row write, so while a started session runs, only its link names the payload.
    payload: Option<ProcessIdentity>,
}

impl LinkTap {
    fn accepted() -> LinkTap {
        LinkTap {
            held: VecDeque::new(),
            end: None,
            frames: Some(FrameDecoder::new(DEFAULT_MAX_PAYLOAD)),
            instance: None,
            payload: None,
        }
    }

    fn connected(instance: &InstanceId) -> LinkTap {
        LinkTap {
            frames: None,
            instance: Some(instance.clone()),
            ..LinkTap::accepted()
        }
    }

    /// Reads the link's frames from bytes that passed through, without changing them: the first frame, a hello, names the
    /// instance, and the worker's `Launched` report names the payload. The tap reads no frame after the report, after a
    /// first frame that is not a hello, or after bytes that are not a frame.
    fn observe(&mut self, bytes: &[u8]) {
        let Some(decoder) = self.frames.as_mut() else {
            return;
        };
        decoder.push(bytes);
        loop {
            let Some(decoder) = self.frames.as_mut() else {
                return;
            };
            match decoder.next_frame() {
                Ok(None) => return,
                Ok(Some(frame)) => {
                    if !self.read(frame.kind, &frame.payload) {
                        self.frames = None;
                    }
                }
                Err(_) => self.frames = None,
            }
        }
    }

    /// Reads one inbound frame. False when the tap reads no later frame of the link.
    fn read(&mut self, kind: FrameType, payload: &[u8]) -> bool {
        if self.instance.is_none() {
            if kind != FrameType::HELLO {
                return false;
            }
            self.instance = Hello::decode(payload).ok().map(|h| h.instance);
            return self.instance.is_some();
        }
        if kind != FrameType::WORKER_MSG {
            return false;
        }
        match WorkerMsg::decode(payload) {
            Ok(WorkerMsg::Launched { payload, .. }) => {
                self.payload = Some(ProcessIdentity {
                    pid: payload.pid,
                    start_time: payload.start_time,
                });
                false
            }
            Ok(_) => true,
            Err(_) => false,
        }
    }

    fn holds_anything(&self) -> bool {
        !self.held.is_empty() || self.end.is_some()
    }
}

/// The registry rows that passed through the taps of one data directory, by session: Core's own decoding of each row.
/// They outlive the handle and the row's removal, so a later handle of the directory can name a session's processes.
pub type Rows = Arc<Mutex<BTreeMap<SessionId, Row>>>;

/// The state of one tap, shared by the driver's edges and the harness's controls.
pub struct Tap<E> {
    inner: E,
    links: BTreeMap<LinkId, LinkTap>,
    accepts: VecDeque<LinkId>,
    exits: VecDeque<(ProcessIdentity, ExitStatus)>,
    rows: Rows,
}

impl<E: HostEdges> Tap<E> {
    fn row(&mut self, key: &str, bytes: &[u8]) {
        if let Some(id) = key.strip_prefix(ROW_PREFIX) {
            let id = SessionId(id.to_string());
            if let Some(row) = Row::decode(&id, bytes) {
                lock(&self.rows).insert(id, row);
            }
        }
    }

    /// Answers whether the edges are quiet: the tap holds nothing that it has not handed to the driver, and a zero-wait
    /// take on every inbound edge finds nothing new (plan 23l, R-43 item 8). The take is bounded: only when the tap holds
    /// nothing, and at most one new link, one exit and one read chunk for each link. When the tap holds something, it
    /// signals the wake edge, so that the host pumps and reads it (TM-6).
    pub fn quiet(&mut self) -> bool {
        let mut quiet = !self.holds_anything();
        if quiet {
            if let Some(link) = self.inner.accept_link() {
                self.links.insert(link, LinkTap::accepted());
                self.accepts.push_back(link);
                quiet = false;
            }
            if let Some(exit) = self.inner.poll_process_exit() {
                self.exits.push_back(exit);
                quiet = false;
            }
            let mut buf = vec![0u8; READ_CHUNK];
            for (link, tap) in &mut self.links {
                loop {
                    match self.inner.link_recv(*link, &mut buf) {
                        Ok(0) => tap.end = Some(End::Closed),
                        Ok(n) => {
                            tap.observe(&buf[..n]);
                            tap.held.extend(&buf[..n]);
                        }
                        Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                        Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                        Err(e) => tap.end = Some(End::Failed(e.kind())),
                    }
                    quiet = false;
                    break;
                }
            }
        }
        if !quiet {
            self.inner.wake().signal();
        }
        quiet
    }

    fn holds_anything(&self) -> bool {
        !self.accepts.is_empty()
            || !self.exits.is_empty()
            || self.links.values().any(LinkTap::holds_anything)
    }

    /// The link whose hello (or adoption) names `instance`, while the driver has not closed it.
    pub fn link_of(&self, instance: &InstanceId) -> Option<LinkId> {
        self.links
            .iter()
            .find(|(_, tap)| tap.instance.as_ref() == Some(instance))
            .map(|(link, _)| *link)
    }

    /// The payload that the `Launched` report on the link of `instance` named, while the driver has not closed that link.
    pub fn payload_of(&self, instance: &InstanceId) -> Option<ProcessIdentity> {
        self.links
            .values()
            .find(|tap| tap.instance.as_ref() == Some(instance))
            .and_then(|tap| tap.payload)
    }

    /// Breaks `link` at the edge (Core LC-5, A2-1): the inner edge closes it, and what the tap held for it is dropped, so
    /// every later call on it reaches the inner edge's closed-link state. The driver's own `link_close` comes later.
    pub fn break_link(&mut self, link: LinkId) {
        if let Some(tap) = self.links.get_mut(&link) {
            tap.held.clear();
            tap.end = None;
        }
        self.inner.link_close(link);
    }

    /// The bytes of the row `key` as the inner storage edge holds them: the bytes that Core's own encoder wrote. `None` when
    /// no row has exactly that key, or the storage cannot be read.
    pub fn stored_row(&mut self, key: &str) -> Option<Vec<u8>> {
        self.inner
            .read_rows(key)
            .ok()?
            .into_iter()
            .find_map(|(k, bytes)| (k == key).then_some(bytes))
    }

    /// Writes the row `key` through the inner storage edge (Core A10-2: the storage edge damages the stored bytes). The write
    /// is the harness's, not Core's, so it passes by the record of the session's processes: the record keeps what Core
    /// wrote.
    ///
    /// # Errors
    /// The inner storage edge's error.
    pub fn store_row(&mut self, key: &str, bytes: &[u8]) -> Result<(), StorageError> {
        self.inner.write_row(key, bytes)
    }

    /// The inner edge's identity check of `identity` (Core AD-6): its pid and its start time.
    pub fn identity_state(&self, identity: ProcessIdentity) -> IdentityState {
        self.inner.identity_state(identity)
    }

    /// Ends the process group of `identity` from outside Core, as a kill does (Core AD-2, IN-7): the inner edge's own
    /// identity-checked `signal_group` sends `KILL`, and only when the pid and the start time still match, so a process that
    /// reuses the pid is never signalled (Core AD-6). Then the wake edge signals, so that the host pumps and meets the end.
    /// The call is the harness's, not Core's: it goes to the inner edge, and the tap records it as no call of Core.
    pub fn kill_group(&mut self, identity: ProcessIdentity) -> IdentityState {
        let state = self.inner.identity_state(identity);
        if state == IdentityState::Matches {
            self.inner.signal_group(identity, GroupSignal::Kill);
            self.inner.wake().signal();
        }
        state
    }
}

/// The pass-through edges that the driver owns: every call goes to the inner edges through the shared [`Tap`].
pub struct EdgeTap<E> {
    tap: Arc<Mutex<Tap<E>>>,
    scheduler: SchedulerTap<E>,
}

/// The inner edges' scheduler, one locked call at a time: `HostEdges::scheduler` lends a `&mut dyn Scheduler`, which cannot
/// borrow through the tap's lock, so this proxy takes the lock for each choice and asks the inner scheduler.
struct SchedulerTap<E> {
    tap: Arc<Mutex<Tap<E>>>,
}

impl<E: HostEdges> Scheduler for SchedulerTap<E> {
    fn pick(&mut self, point: ChoicePoint, candidates: usize) -> usize {
        lock(&self.tap).inner.scheduler().pick(point, candidates)
    }

    fn bound(&mut self, point: ChoicePoint, max: usize) -> usize {
        lock(&self.tap).inner.scheduler().bound(point, max)
    }
}

impl<E: HostEdges> EdgeTap<E> {
    /// Wraps `inner`. The harness keeps the returned `Weak` only: the tap ends with the driver, and with it the inner
    /// edges and the data directory's lock (LC-2), so a reopen of the directory is never refused by a tap.
    pub fn new(inner: E, rows: Rows) -> (EdgeTap<E>, Weak<Mutex<Tap<E>>>) {
        let tap = Arc::new(Mutex::new(Tap {
            inner,
            links: BTreeMap::new(),
            accepts: VecDeque::new(),
            exits: VecDeque::new(),
            rows,
        }));
        let weak = Arc::downgrade(&tap);
        let scheduler = SchedulerTap {
            tap: Arc::clone(&tap),
        };
        (EdgeTap { tap, scheduler }, weak)
    }

    fn tap(&self) -> MutexGuard<'_, Tap<E>> {
        lock(&self.tap)
    }
}

impl<E: HostEdges> HostEdges for EdgeTap<E> {
    fn fill_random(&mut self, buf: &mut [u8]) {
        self.tap().inner.fill_random(buf);
    }

    fn write_row(&mut self, key: &str, bytes: &[u8]) -> Result<(), StorageError> {
        let mut tap = self.tap();
        tap.inner.write_row(key, bytes)?;
        tap.row(key, bytes);
        Ok(())
    }

    fn delete_row(&mut self, key: &str) -> Result<(), StorageError> {
        self.tap().inner.delete_row(key)
    }

    fn read_rows(&mut self, prefix: &str) -> Result<Vec<(String, Vec<u8>)>, StorageError> {
        let mut tap = self.tap();
        let rows = tap.inner.read_rows(prefix)?;
        for (key, bytes) in &rows {
            tap.row(key, bytes);
        }
        Ok(rows)
    }

    fn spawn_worker(&mut self, spec: &WorkerSpawn) -> Result<ProcessIdentity, SpawnError> {
        self.tap().inner.spawn_worker(spec)
    }

    fn signal_group(&mut self, identity: ProcessIdentity, signal: GroupSignal) {
        self.tap().inner.signal_group(identity, signal);
    }

    fn identity_state(&self, identity: ProcessIdentity) -> IdentityState {
        self.tap().inner.identity_state(identity)
    }

    fn poll_process_exit(&mut self) -> Option<(ProcessIdentity, ExitStatus)> {
        let mut tap = self.tap();
        match tap.exits.pop_front() {
            Some(exit) => Some(exit),
            None => tap.inner.poll_process_exit(),
        }
    }

    fn accept_link(&mut self) -> Option<LinkId> {
        let mut tap = self.tap();
        if let Some(link) = tap.accepts.pop_front() {
            return Some(link);
        }
        let link = tap.inner.accept_link()?;
        tap.links.insert(link, LinkTap::accepted());
        Some(link)
    }

    fn connect_worker(&mut self, instance: &InstanceId) -> Option<LinkId> {
        let mut tap = self.tap();
        let link = tap.inner.connect_worker(instance)?;
        tap.links.insert(link, LinkTap::connected(instance));
        Some(link)
    }

    fn remove_endpoint(&mut self, instance: &InstanceId) {
        self.tap().inner.remove_endpoint(instance);
    }

    fn link_recv(&mut self, link: LinkId, buf: &mut [u8]) -> io::Result<usize> {
        let mut tap = self.tap();
        let Tap { inner, links, .. } = &mut *tap;
        let Some(state) = links.get_mut(&link) else {
            return inner.link_recv(link, buf);
        };
        if !state.held.is_empty() {
            let n = buf.len().min(state.held.len());
            for (slot, byte) in buf.iter_mut().zip(state.held.drain(..n)) {
                *slot = byte;
            }
            return Ok(n);
        }
        match state.end.take() {
            Some(End::Closed) => Ok(0),
            Some(End::Failed(kind)) => Err(kind.into()),
            None => {
                let n = inner.link_recv(link, buf)?;
                state.observe(&buf[..n]);
                Ok(n)
            }
        }
    }

    fn link_send(&mut self, link: LinkId, bytes: &[u8]) -> io::Result<usize> {
        self.tap().inner.link_send(link, bytes)
    }

    fn link_close(&mut self, link: LinkId) {
        let mut tap = self.tap();
        tap.links.remove(&link);
        tap.inner.link_close(link);
    }

    fn set_write_interest(&mut self, link: LinkId, on: bool) {
        self.tap().inner.set_write_interest(link, on);
    }

    fn set_read_interest(&mut self, link: LinkId, on: bool) {
        self.tap().inner.set_read_interest(link, on);
    }

    fn link_send_descriptor(
        &mut self,
        link: LinkId,
        bytes: &[u8],
        endpoint: StreamEndpoint,
    ) -> Result<usize, (StreamEndpoint, DescriptorSendError)> {
        self.tap().inner.link_send_descriptor(link, bytes, endpoint)
    }

    fn wake(&self) -> Arc<dyn HostWake> {
        self.tap().inner.wake()
    }

    fn settle_wake(&mut self) {
        self.tap().inner.settle_wake();
    }

    fn scheduler(&mut self) -> &mut dyn Scheduler {
        &mut self.scheduler
    }

    fn diagnostics(&self) -> serde_json::Value {
        self.tap().inner.diagnostics()
    }
}

#[cfg(test)]
mod tests;
