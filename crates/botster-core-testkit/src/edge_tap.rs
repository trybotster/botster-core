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
//!   each link names its instance. The instances whose worker the taps of a data directory met ([`Heard`]): a hello that
//!   a tap read, or Core's connection to the worker's endpoint. Before that, the recorded pid may not run the worker
//!   binary yet (the guard's wrap stage execs it).
//! - The harness's own calls of the inner edges, which the tap records as no call of Core: a stored row and its write
//!   ([`Tap::stored_row`], [`Tap::store_row`]; `corrupt_registry_row`), the identity check ([`Tap::identity_state`];
//!   `payload_alive`) and the identity-checked kill of a recorded process group ([`Tap::kill_group`]; `lose_worker`).
//! - The record of Core's signals: every `signal_group` call of Core, in order, with its target identity. It is Core's only
//!   signalling edge call (steward ruling R-49 Q1). The harness's own `kill_group` is not in it.
//! - The stand-ins at worker endpoint paths ([`Tap::add_stand_in`]; `impostor_worker`, `withhold_control_link`,
//!   `announce_protocol`, R-49 Q4): the tap steps them after each `connect_worker` and `link_send` of Core, so a stand-in
//!   accepts Core's connection and answers Core's hello within that call. A stand-in ends with the tap, after the inner
//!   edges and the data directory's lock, and then gives the path back to the real endpoint. [`Tap::signals_since_answer`]
//!   gives the signals that Core sent from a stand-in's first answer on (`signals_received`).
//! - The held starts ([`Tap::hold_launch`], [`Tap::release_launch`]; `hold_start_at`, `release_start_at`, R-49 Q2): the
//!   tap reads Core's outbound frames on a link until it knows that no held start can meet its `Launch` there. A `link_send`
//!   that reaches a held start's `Launch` gives the inner edge only the bytes before that frame, and every later send on the
//!   link is `WouldBlock`, so Core keeps the `Launch` and every byte after it in its own outbound buffer: no byte is
//!   reordered, dropped or changed. Core's write interest on the held link does not reach the inner edge, so no readiness
//!   wakes the host for it. The release gives the inner edge that interest, and Core's next flush sends the bytes.

use crate::route_reports::{RouteReports, Seen};
use crate::stand_in::StandIn;
use crate::stream_holders::{socket_id, SocketId};
use botster_core_contract::prelude::*;
use botster_core_edges::edges::{
    ExitStatus, GroupSignal, IdentityState, ProcessIdentity, SpawnError, StorageError,
};
use botster_core_edges::scheduler::{ChoicePoint, Scheduler};
use botster_core_host::driver::{DescriptorSendError, HostEdges, HostWake, WorkerSpawn};
use botster_core_host::session::{Row, ROW_PREFIX};
use botster_core_host::LinkId;
use botster_core_link::frame::{Frame, FrameDecoder, FrameError, FrameType, DEFAULT_MAX_PAYLOAD};
use botster_core_link::hello::Hello;
use botster_core_link::msg::{HostMsg, WorkerMsg};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::io;
use std::os::fd::AsFd;
use std::os::unix::net::UnixStream;
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
    /// The decoder of the bytes that the inner edge took from Core on the link, until the tap knows that no held start can
    /// meet its `Launch` here. Its bound is [`OUTBOUND_BOUND`].
    sent: Option<FrameDecoder>,
}

impl LinkTap {
    fn accepted() -> LinkTap {
        LinkTap {
            held: VecDeque::new(),
            end: None,
            frames: Some(FrameDecoder::new(DEFAULT_MAX_PAYLOAD)),
            instance: None,
            sent: Some(FrameDecoder::new(OUTBOUND_BOUND)),
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
    /// instance (and puts it in `heard`), and the worker's `Launched` report names the payload in `launches`. The tap reads
    /// no frame after the report, after a first frame that is not a hello, or after bytes that are not a frame.
    fn observe(
        &mut self,
        bytes: &[u8],
        launches: &mut BTreeMap<InstanceId, ProcessIdentity>,
        heard: &Heard,
    ) {
        let mut rest = bytes;
        while let Some(decoder) = self.frames.as_mut() {
            // The decoder takes bytes up to the end of the current frame.
            let used = decoder.push(rest);
            rest = &rest[used..];
            match decoder.next_frame() {
                Ok(None) => return,
                Ok(Some(frame)) => {
                    if !self.read(frame.kind, &frame.payload, launches) {
                        self.frames = None;
                    }
                    if let Some(instance) = &self.instance {
                        lock(heard).insert(instance.clone());
                    }
                }
                Err(_) => self.frames = None,
            }
        }
    }

    /// Reads one inbound frame. False when the tap reads no later frame of the link.
    fn read(
        &mut self,
        kind: FrameType,
        payload: &[u8],
        launches: &mut BTreeMap<InstanceId, ProcessIdentity>,
    ) -> bool {
        let Some(instance) = &self.instance else {
            if kind != FrameType::HELLO {
                return false;
            }
            self.instance = Hello::decode(payload).ok().map(|h| h.instance);
            return self.instance.is_some();
        };
        if kind != FrameType::WORKER_MSG {
            return false;
        }
        match WorkerMsg::decode(payload) {
            Ok(WorkerMsg::Launched { payload, .. }) => {
                launches.insert(
                    instance.clone(),
                    ProcessIdentity {
                        pid: payload.pid,
                        start_time: payload.start_time,
                    },
                );
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

/// Pushes `bytes` through `decoder`, frame by frame, up to the first complete frame that `stop` takes. Returns the offset in
/// `bytes` where that frame begins (0 when it began before `bytes`), or `None` when no frame in `bytes` is one.
///
/// # Errors
/// The bytes are not frames.
fn find_frame(
    decoder: &mut FrameDecoder,
    bytes: &[u8],
    mut stop: impl FnMut(&Frame) -> bool,
) -> Result<Option<usize>, FrameError> {
    let mut start = 0;
    let mut at = 0;
    loop {
        at += decoder.push(&bytes[at..]);
        let Some(frame) = decoder.next_frame()? else {
            return Ok(None);
        };
        if stop(&frame) {
            return Ok(Some(start));
        }
        start = at;
    }
}

/// The payload bound of the tap's decoder of Core's outbound frames: the largest length that a frame header can hold. Core's
/// own bound of a worker link (`HostEngine::link_frame_bound`) follows its configured limits and is at most `u32::MAX / 2`,
/// so the decoder refuses no frame that Core can send, and the tap finds every `Launch` whole. It copies only the frames
/// that Core already holds in its outbound buffer.
const OUTBOUND_BOUND: u32 = u32::MAX;

/// Whether `frame` is the host's order to launch the payload (AD-7 step 4).
fn is_launch(frame: &Frame) -> bool {
    frame.kind == FrameType::HOST_MSG
        && matches!(HostMsg::decode(&frame.payload), Ok(HostMsg::Launch(_)))
}

/// A start that the tap holds at its `Launch` (`hold_start_at`, R-49 Q2).
#[derive(Debug, Default)]
struct Hold {
    /// The link where Core's `Launch` met the hold: from there on, the tap gives the inner edge no byte of that link.
    link: Option<LinkId>,
    /// The last write interest that Core asked for on that link during the hold.
    write_interest: bool,
}

/// A hand-over that the tap holds (`hold_handoff`, Core OU-9, A5-1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HandoffHold {
    Off,
    /// The next `link_send_descriptor` meets the hold.
    Armed,
    /// Core's hand-over met the hold on `link`: every descriptor send there is `Blocked` until the release.
    Held {
        link: LinkId,
        /// The last write interest that Core asked for on that link during the hold.
        write_interest: bool,
    },
}

/// The registry rows that passed through the taps of one data directory, by session: Core's own decoding of each row.
/// They outlive the handle and the row's removal, so a later handle of the directory can name a session's processes.
pub type Rows = Arc<Mutex<BTreeMap<SessionId, Row>>>;

/// The instances whose worker the taps of one data directory met: a hello that a tap read, or Core's connection to the
/// worker's endpoint (an adoption). Only the worker binary sends the hello or listens on that endpoint, so each one proves
/// that the recorded pid has exec'd the worker binary, which starts no child but its payload. They outlive the handle, as
/// [`Rows`] do.
pub type Heard = Arc<Mutex<BTreeSet<InstanceId>>>;

/// The state of one tap, shared by the driver's edges and the harness's controls.
pub struct Tap<E> {
    inner: E,
    links: BTreeMap<LinkId, LinkTap>,
    accepts: VecDeque<LinkId>,
    exits: VecDeque<(ProcessIdentity, ExitStatus)>,
    rows: Rows,
    heard: Heard,
    /// The payload of each worker's `Launched` report that the tap read (Core AD-7, LC-5). It outlives the link. The host
    /// keeps a started payload in memory and writes it to the row only at a later row write. A report that the tap has
    /// not read gives no entry, and no entry never means that no payload runs.
    launches: BTreeMap<InstanceId, ProcessIdentity>,
    /// Every `signal_group` call of Core, in order, with its target identity (R-49 Q1).
    signals: Vec<(ProcessIdentity, GroupSignal)>,
    /// The stand-ins at worker endpoint paths, each with the length of `signals` at its first answer. Declared after
    /// `inner`, so the inner edges and the data directory's lock end first, and only then do the real endpoints get their
    /// paths back (R-49 Q4 conditions 1 and 2).
    stand_ins: Vec<(StandIn, Option<usize>)>,
    /// The held starts, by instance (`hold_start_at`, R-49 Q2).
    holds: BTreeMap<InstanceId, Hold>,
    /// The held control links of starts, by instance (`hold_start`, plan 23bg): `None` while the hold is armed, and the
    /// worker's link once it arrived and is held from the driver.
    link_holds: BTreeMap<InstanceId, Option<LinkId>>,
    /// The new links whose hello the tap has not read yet, while a `hold_start` is armed, in the order of their accept.
    unnamed: VecDeque<LinkId>,
    /// The next hand-over of a route's stream endpoint fails, once (`fail_handoff`, Core DP-2).
    fail_handoff: bool,
    /// The held hand-over (`hold_handoff`, Core OU-9).
    handoff: HandoffHold,
    /// The bytes that Core gave `close_route_stream` to write to a route's stream, by the stream's socket
    /// (`route_stream_holders`, plan 23bf). It is the only `HostEdges` call that moves bytes through a route's stream.
    stream_bytes: BTreeMap<SocketId, u64>,
    /// The worker's route reports that the driver consumed (`route_reports`, R-66): only the driver's own `link_recv` calls
    /// and the bytes that they gave count, never the tap's take-ahead.
    reports: RouteReports,
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
        self.sort_unnamed();
        let mut quiet = !self.holds_anything();
        if quiet {
            if self.take_link() {
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
                            tap.observe(&buf[..n], &mut self.launches, &self.heard);
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

    /// A link that a `hold_start` holds is not a report: its bytes wait for the release, and the edges can be quiet.
    fn holds_anything(&self) -> bool {
        !self.accepts.is_empty()
            || !self.exits.is_empty()
            || !self.unnamed.is_empty()
            || self
                .links
                .iter()
                .any(|(link, tap)| tap.holds_anything() && !self.link_held_for_start(*link))
    }

    /// Whether `link` is held by a `hold_start`.
    fn link_held_for_start(&self, link: LinkId) -> bool {
        self.link_holds.values().any(|held| *held == Some(link))
    }

    /// Takes the next new link of the inner edge: to `accepts`, or, while a `hold_start` is armed, to `unnamed` until its
    /// hello names its instance ([`Tap::sort_unnamed`]). False when the inner edge has no new link.
    fn take_link(&mut self) -> bool {
        let Some(link) = self.inner.accept_link() else {
            return false;
        };
        self.links.insert(link, LinkTap::accepted());
        if self.link_holds.values().any(Option::is_none) {
            self.unnamed.push_back(link);
            self.sort_unnamed();
        } else {
            self.accepts.push_back(link);
        }
        true
    }

    /// Reads ahead on each unnamed link until its hello names its instance (plan 23bg). A link of an armed `hold_start`
    /// is held from the driver; any other link goes to `accepts`, in order: a link whose first frame is no hello, one that
    /// ended first, or one of an instance with no armed hold. With no armed hold left, every unnamed link goes to
    /// `accepts`. The bytes read ahead stay in the link's record, in order, for the driver.
    fn sort_unnamed(&mut self) {
        let Tap {
            inner,
            links,
            accepts,
            launches,
            heard,
            link_holds,
            unnamed,
            ..
        } = self;
        let mut buf = vec![0u8; READ_CHUNK];
        let mut waiting = VecDeque::new();
        while let Some(link) = unnamed.pop_front() {
            let Some(state) = links.get_mut(&link) else {
                continue;
            };
            let armed = link_holds.values().any(Option::is_none);
            while armed && state.instance.is_none() && state.frames.is_some() && state.end.is_none()
            {
                match inner.link_recv(link, &mut buf) {
                    Ok(0) => state.end = Some(End::Closed),
                    Ok(n) => {
                        state.observe(&buf[..n], launches, heard);
                        state.held.extend(&buf[..n]);
                    }
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                    Err(e) => state.end = Some(End::Failed(e.kind())),
                }
            }
            if armed && state.instance.is_none() && state.frames.is_some() && state.end.is_none() {
                waiting.push_back(link);
                continue;
            }
            match state.instance.as_ref().and_then(|i| link_holds.get_mut(i)) {
                Some(slot) if slot.is_none() => *slot = Some(link),
                _ => accepts.push_back(link),
            }
        }
        *unnamed = waiting;
    }

    /// Arms `hold_start` for `instance` (Core LC-4, A2-1; steward ruling, plan 23bg): the worker's control link is held from
    /// the driver when it arrives, so Core sees no link of the worker and the session stays `Starting`. False when a hold of
    /// `instance` is already set.
    pub fn hold_link(&mut self, instance: InstanceId) -> bool {
        if self.link_holds.contains_key(&instance) {
            return false;
        }
        self.link_holds.insert(instance, None);
        true
    }

    /// Whether the hold of `instance` holds the worker's link (`start_held`): false while it is only armed.
    pub fn link_held(&self, instance: &InstanceId) -> bool {
        matches!(self.link_holds.get(instance), Some(Some(_)))
    }

    /// Ends the hold of `instance` (`release_start`). A held link goes to the driver's next `accept_link`, with every byte
    /// that the tap read ahead, in order, and the wake edge signals. An armed hold is disarmed, and the later link goes to the
    /// driver as usual. False when no hold of `instance` is set.
    pub fn release_link(&mut self, instance: &InstanceId) -> bool {
        let Some(held) = self.link_holds.remove(instance) else {
            return false;
        };
        if let Some(link) = held {
            self.accepts.push_back(link);
        }
        self.sort_unnamed();
        self.inner.wake().signal();
        true
    }

    /// What the driver consumed of the reports of `route` from the worker of `instance` (`route_reports`, R-66). While a report
    /// that the driver received waits for its next call, the wake edge signals, so that the host pumps and consumes it.
    pub fn route_reports(&self, instance: &InstanceId, route: RouteId) -> Seen {
        let wake = self.inner.wake();
        self.reports.answer(instance, route, || wake.signal())
    }

    /// Whether a tap of this data directory met the worker of `instance` ([`Heard`]).
    pub fn heard(&self, instance: &InstanceId) -> bool {
        lock(&self.heard).contains(instance)
    }

    /// The link whose hello (or adoption) names `instance`, while the driver has not closed it.
    pub fn link_of(&self, instance: &InstanceId) -> Option<LinkId> {
        self.links
            .iter()
            .find(|(_, tap)| tap.instance.as_ref() == Some(instance))
            .map(|(link, _)| *link)
    }

    /// The payload that the `Launched` report of the worker of `instance` named. `None` when the tap has not read that report:
    /// the report may still be unread, or another tap read it, so `None` never means that no payload runs.
    pub fn launch_of(&self, instance: &InstanceId) -> Option<ProcessIdentity> {
        self.launches.get(instance).copied()
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

    /// Puts `stand_in` at its endpoint path for the life of the tap. The harness installs it before Core opened the data
    /// directory (R-49 Q4 condition 1).
    pub fn add_stand_in(&mut self, stand_in: StandIn) {
        self.stand_ins.push((stand_in, None));
    }

    /// Steps every stand-in, and keeps the length of the signal record at each one's first answer.
    fn step_stand_ins(&mut self) {
        for (stand_in, answered_at) in &mut self.stand_ins {
            if stand_in.step() && answered_at.is_none() {
                *answered_at = Some(self.signals.len());
            }
        }
    }

    /// The signals that Core sent, in order and with their targets, from the first answer of the stand-in of `session` on:
    /// every `signal_group` call of Core, to any target (R-49 Q1). Empty before an answer. `None` when no stand-in of
    /// `session` is in this tap. The harness puts at most one stand-in at a session's endpoint.
    pub fn signals_since_answer(
        &self,
        session: &SessionId,
    ) -> Option<Vec<(ProcessIdentity, GroupSignal)>> {
        let (_, answered_at) = self
            .stand_ins
            .iter()
            .find(|(s, _)| s.session() == session)?;
        Some(answered_at.map_or_else(Vec::new, |from| self.signals[from..].to_vec()))
    }

    /// Every signalling edge call of Core through this tap, in order, with its target: `signal_group` is the only one of
    /// `HostEdges` (R-49 Q1, widened by R-55 for `reuse_pid`).
    pub fn signals(&self) -> &[(ProcessIdentity, GroupSignal)] {
        &self.signals
    }

    /// Holds the start of `instance` at its `Launch` (`hold_start_at`, R-49 Q2): the worker gets no byte of the host's
    /// `Launch` until [`Tap::release_launch`], so it launches no payload. False when a hold of `instance` is already set.
    pub fn hold_launch(&mut self, instance: InstanceId) -> bool {
        if self.holds.contains_key(&instance) {
            return false;
        }
        self.holds.insert(instance, Hold::default());
        true
    }

    /// Whether Core's `Launch` of `instance` met its hold: the tap gives the inner edge no byte of the held link.
    pub fn launch_held(&self, instance: &InstanceId) -> bool {
        self.holds
            .get(instance)
            .is_some_and(|hold| hold.link.is_some())
    }

    /// Ends the hold of `instance` (`release_start_at`). The write interest that Core asked for on the held link reaches the
    /// inner edge, so readiness wakes the host, and Core's next flush sends the `Launch` and every byte after it, in order.
    /// False when no hold of `instance` is set.
    pub fn release_launch(&mut self, instance: &InstanceId) -> bool {
        let Some(hold) = self.holds.remove(instance) else {
            return false;
        };
        if let Some(link) = hold.link {
            let handoff_held =
                matches!(self.handoff, HandoffHold::Held { link: held, .. } if held == link);
            self.inner
                .set_write_interest(link, hold.write_interest && !handoff_held);
        }
        true
    }

    /// Arms `fail_handoff` (Core DP-2): the next `link_send_descriptor` gives the endpoint back with `Failed` and the inner
    /// edge takes nothing, as a `sendmsg` that fails takes nothing. Core then closes the endpoint, so the client reads the
    /// end of its stream, and the route closes `HandoffFailed`.
    pub fn fail_next_handoff(&mut self) {
        self.fail_handoff = true;
    }

    /// Arms `hold_handoff` (Core OU-9, A5-1): the next `link_send_descriptor` is `Blocked`, and so is every later one on
    /// its link until [`Tap::release_handoff`]. The driver keeps the frame and the endpoint in order, so `attach` returns
    /// and the worker has no transport for the route. A held hand-over is not a report, so it does not make the edges
    /// busy. False when a hand-over is already armed or held.
    pub fn hold_handoff(&mut self) -> bool {
        if self.handoff != HandoffHold::Off {
            return false;
        }
        self.handoff = HandoffHold::Armed;
        true
    }

    /// Ends the hold of `hold_handoff` (Core OU-9, A5-2). The write interest that Core asked for on the held link reaches
    /// the inner edge, and the wake edge signals, so the next pump sends the held frame with its endpoint first.
    pub fn release_handoff(&mut self) {
        if let HandoffHold::Held {
            link,
            write_interest,
        } = std::mem::replace(&mut self.handoff, HandoffHold::Off)
        {
            let launch_held = self.holds.values().any(|hold| hold.link == Some(link));
            self.inner
                .set_write_interest(link, write_interest && !launch_held);
        }
        self.inner.wake().signal();
    }

    /// The bytes that Core gave `close_route_stream` for the stream of `socket` (`route_stream_holders`, plan 23bf). Core is
    /// sans-IO: `HostEdges` is its only I/O path, and `close_route_stream` is its only call that writes to a route's stream;
    /// no call reads one. The count is the bytes that Core asked the edge to write, so an upper bound of what the stream
    /// took; 0 is exact.
    pub fn stream_bytes(&self, socket: SocketId) -> u64 {
        self.stream_bytes.get(&socket).copied().unwrap_or(0)
    }

    /// Whether a descriptor send on `link` meets the hold of `hold_handoff`. An armed hold takes `link`.
    fn handoff_held(&mut self, link: LinkId) -> bool {
        match self.handoff {
            HandoffHold::Armed => {
                self.handoff = HandoffHold::Held {
                    link,
                    write_interest: false,
                };
                true
            }
            HandoffHold::Held { link: held, .. } => held == link,
            HandoffHold::Off => false,
        }
    }

    /// The hold whose `Launch` met it on `link`.
    fn hold_on(&mut self, link: LinkId) -> Option<&mut Hold> {
        self.holds.values_mut().find(|hold| hold.link == Some(link))
    }

    /// How many of `bytes`, which Core sends on `link`, the inner edge may take: none on a held link; the bytes before the
    /// `Launch` of a held start; otherwise all. A send that begins with that `Launch` holds the link.
    fn sendable(&mut self, link: LinkId, bytes: &[u8]) -> usize {
        if self.hold_on(link).is_some() {
            return 0;
        }
        let Some(state) = self.links.get_mut(&link) else {
            return bytes.len();
        };
        let held = match &state.instance {
            Some(instance) if self.holds.contains_key(instance) => instance.clone(),
            // A start that is not held, or a link whose hello the tap has not read: no `Launch` there is held.
            Some(_) => {
                state.sent = None;
                return bytes.len();
            }
            None => return bytes.len(),
        };
        let Some(mut decoder) = state.sent.clone() else {
            return bytes.len();
        };
        match find_frame(&mut decoder, bytes, is_launch) {
            // A refusal of the decoder holds too: the tap gives the inner edge no byte that it could not read.
            Ok(Some(0)) | Err(_) => {
                if let Some(hold) = self.holds.get_mut(&held) {
                    hold.link = Some(link);
                }
                0
            }
            Ok(Some(start)) => start,
            Ok(None) => bytes.len(),
        }
    }

    /// Reads the bytes that the inner edge took on `link` (`sendable`).
    fn sent(&mut self, link: LinkId, bytes: &[u8]) {
        if let Some(state) = self.links.get_mut(&link) {
            if let Some(decoder) = state.sent.as_mut() {
                // A refusal is final in the decoder, so the next `sendable` of a held start on this link holds.
                let _ = find_frame(decoder, bytes, |_| false);
            }
        }
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
    pub fn new(inner: E, rows: Rows, heard: Heard) -> (EdgeTap<E>, Weak<Mutex<Tap<E>>>) {
        let tap = Arc::new(Mutex::new(Tap {
            inner,
            links: BTreeMap::new(),
            accepts: VecDeque::new(),
            exits: VecDeque::new(),
            rows,
            heard,
            launches: BTreeMap::new(),
            signals: Vec::new(),
            stand_ins: Vec::new(),
            holds: BTreeMap::new(),
            link_holds: BTreeMap::new(),
            unnamed: VecDeque::new(),
            fail_handoff: false,
            handoff: HandoffHold::Off,
            stream_bytes: BTreeMap::new(),
            reports: RouteReports::default(),
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
        let mut tap = self.tap();
        tap.signals.push((identity, signal));
        tap.inner.signal_group(identity, signal);
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
        tap.sort_unnamed();
        loop {
            if let Some(link) = tap.accepts.pop_front() {
                return Some(link);
            }
            if !tap.take_link() {
                return None;
            }
        }
    }

    fn connect_worker(&mut self, instance: &InstanceId) -> Option<LinkId> {
        let mut tap = self.tap();
        let link = tap.inner.connect_worker(instance);
        tap.step_stand_ins();
        let link = link?;
        tap.links.insert(link, LinkTap::connected(instance));
        lock(&tap.heard).insert(instance.clone());
        Some(link)
    }

    fn remove_endpoint(&mut self, instance: &InstanceId) {
        self.tap().inner.remove_endpoint(instance);
    }

    fn link_recv(&mut self, link: LinkId, buf: &mut [u8]) -> io::Result<usize> {
        let mut tap = self.tap();
        let Tap {
            inner,
            links,
            launches,
            heard,
            reports,
            ..
        } = &mut *tap;
        // R-66: the driver's call consumes what it received before; it receives the bytes that the call gives.
        reports.call(link);
        let n = match links.get_mut(&link) {
            None => inner.link_recv(link, buf)?,
            Some(state) if !state.held.is_empty() => {
                let n = buf.len().min(state.held.len());
                for (slot, byte) in buf.iter_mut().zip(state.held.drain(..n)) {
                    *slot = byte;
                }
                n
            }
            Some(state) => match state.end.take() {
                Some(End::Closed) => 0,
                Some(End::Failed(kind)) => return Err(kind.into()),
                None => {
                    let n = inner.link_recv(link, buf)?;
                    state.observe(&buf[..n], launches, heard);
                    n
                }
            },
        };
        reports.received(link, &buf[..n]);
        Ok(n)
    }

    fn link_send(&mut self, link: LinkId, bytes: &[u8]) -> io::Result<usize> {
        let mut tap = self.tap();
        let until = tap.sendable(link, bytes);
        if until == 0 && !bytes.is_empty() {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        let sent = tap.inner.link_send(link, &bytes[..until]);
        if let Ok(n) = sent {
            tap.sent(link, &bytes[..n]);
        }
        tap.step_stand_ins();
        sent
    }

    fn link_close(&mut self, link: LinkId) {
        let mut tap = self.tap();
        tap.links.remove(&link);
        tap.reports.closed(link);
        tap.inner.link_close(link);
    }

    fn set_write_interest(&mut self, link: LinkId, on: bool) {
        let mut tap = self.tap();
        // A held link keeps Core's interest for its release; the inner edge gets none.
        let mut held = false;
        if let Some(hold) = tap.hold_on(link) {
            hold.write_interest = on;
            held = true;
        }
        if let HandoffHold::Held {
            link: handoff_link,
            write_interest,
        } = &mut tap.handoff
        {
            if *handoff_link == link {
                *write_interest = on;
                held = true;
            }
        }
        tap.inner.set_write_interest(link, on && !held);
    }

    fn set_read_interest(&mut self, link: LinkId, on: bool) {
        self.tap().inner.set_read_interest(link, on);
    }

    fn link_send_descriptor(
        &mut self,
        link: LinkId,
        bytes: &[u8],
        endpoint: StreamEndpoint,
    ) -> Result<(usize, Option<StreamEndpoint>), (StreamEndpoint, DescriptorSendError)> {
        let mut tap = self.tap();
        if tap.hold_on(link).is_some() || tap.handoff_held(link) {
            return Err((endpoint, DescriptorSendError::Blocked));
        }
        if std::mem::take(&mut tap.fail_handoff) {
            return Err((endpoint, DescriptorSendError::Failed));
        }
        let sent = tap.inner.link_send_descriptor(link, bytes, endpoint);
        if let Ok((n, _)) = &sent {
            tap.sent(link, &bytes[..*n]);
        }
        sent
    }

    fn close_route_stream(&mut self, endpoint: StreamEndpoint, bytes: &[u8]) {
        let mut tap = self.tap();
        // The real harness's endpoint is a `UnixStream`; the inner edge gets the same object back.
        let endpoint = match endpoint.downcast::<UnixStream>() {
            Ok(stream) => {
                if let Ok(socket) = socket_id(stream.as_fd()) {
                    *tap.stream_bytes.entry(socket).or_default() += bytes.len() as u64;
                }
                StreamEndpoint::new(stream)
            }
            Err(endpoint) => endpoint,
        };
        tap.inner.close_route_stream(endpoint, bytes);
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
