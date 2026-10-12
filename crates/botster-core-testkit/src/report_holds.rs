//! The report holds of the process edge (`docs/core-testkit-controls.md`): `hold_worker_report`, `release_worker_report`,
//! `held_reports` (Core A12-1a) and `scheduler_delay` (Core A5-2; steward ruling R-88, group E).
//!
//! The holder is the host's `link_recv` edge on both tiers: the testkit's in-memory edge (`SimEdges`), and on the real tier
//! the edge tap, after its own take-ahead. The worker's own code produces and writes the report; the edge holds it before
//! Core reads it.
//!
//! - **Frames.** The edge reads each link's inbound bytes with Core's own frame decoder. The first frame, a hello, names the
//!   link's instance. A frame that is not a worker message ends the record of the link: the driver closes the link for it.
//! - **Arms.** An arm belongs to one session, by its instance. `hold_worker_report` arms the worker's next report (any worker
//!   message). `scheduler_delay` arms the next report of its point: `Launched` for `running` (Start to Running), `Exited`
//!   for `exited` (Stop to Exited).
//! - **The hold.** While the link's instance has an arm (or the instance is not known yet and some arm exists), the edge takes
//!   the link's bytes ahead and gives the driver only the complete frames before the armed frame. At the armed frame the
//!   hold starts: that frame and every later byte wait, in order. Nothing is dropped, changed or reordered. With no arm,
//!   every byte passes through at once.
//! - **The count.** `held_reports` counts the holds that hold a frame now: the holds of both controls.
//! - **`scheduler_delay`'s pumps.** The hold starts in a `link_recv` of the driver, so inside a pump: that pump is pump 1.
//!   The edge learns each pump start from the scheduler: the driver asks `ChoicePoint::PumpBound` once, first, in every pump
//!   (`HostDriver::pump`; the host test `the_pump_bound_is_the_first_choice_of_each_pump`). At the start of pump `pumps + 1`
//!   the frames are given out, and the driver reads every link in every pump, so Core reads the report in that pump.
//! - **The worker's exit waits too.** While the instance has an arm or a hold and its link is open, the process edge keeps
//!   that worker's reaped exit and gives it after the hold ends. Without that, Core would see the worker's end after its
//!   last report, drain the link to `WouldBlock`, and close it (R-78 item 4): the held report would be lost.
//! - **`break_control`.** On a session with a hold, the break drops every byte that the edge took ahead, ends the hold, and
//!   drops every later byte up to the link's end: the report stays unread, as on a broken socket.
//! - One session never has holds of both controls: the second is refused, `ControlError::Refused({reason: "mixed_holds"})`.

use crate::controls::{parse, session_row, ControlRegistry};
use crate::harness::TestkitHarness;
use botster_core_conformance::ControlError;
use botster_core_contract::prelude::{InstanceId, SessionId};
use botster_core_edges::edges::{ExitStatus, ProcessIdentity};
use botster_core_host::LinkId;
use botster_core_link::frame::{FrameDecoder, FrameType};
use botster_core_link::hello::Hello;
use botster_core_link::msg::WorkerMsg;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, VecDeque};
use std::io;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

/// The bytes that one take-ahead read asks for.
const READ_CHUNK: usize = 16_384;

/// The payload bound of the edge's decoder: the largest length that a frame header can hold. Core's own bound of a link is
/// lower, so the edge refuses no frame that Core reads.
const BOUND: u32 = u32::MAX;

/// The report holds of one host, shared by its edges, its scheduler and its controls.
pub type SharedReportHolds = Arc<Mutex<ReportHolds>>;

pub(crate) fn lock(holds: &Mutex<ReportHolds>) -> MutexGuard<'_, ReportHolds> {
    holds.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The report of a `scheduler_delay`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Point {
    /// The ready report: `Launched` (Start to Running).
    Running,
    /// The exit report: `Exited` (Stop to Exited).
    Exited,
}

/// The control that set a hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// `hold_worker_report`: the worker's next report.
    Worker,
    /// `scheduler_delay`: the next report of `point`, given out at the start of pump `pumps + 1`.
    Delay { point: Point, pumps: u32 },
}

impl Kind {
    fn takes(self, report: Report) -> bool {
        match self {
            Kind::Worker => true,
            Kind::Delay {
                point: Point::Running,
                ..
            } => report == Report::Launched,
            Kind::Delay {
                point: Point::Exited,
                ..
            } => report == Report::Exited,
        }
    }

    fn same_control(self, other: Kind) -> bool {
        matches!(
            (self, other),
            (Kind::Worker, Kind::Worker) | (Kind::Delay { .. }, Kind::Delay { .. })
        )
    }
}

/// A hold that holds a frame of a link.
#[derive(Debug, Clone, Copy)]
struct Holding {
    kind: Kind,
    /// The pumps begun since the hold took its frame, counting the pump that took it.
    pumps: u32,
}

/// What a complete frame of a link is to the holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    /// A frame that no arm takes (the hello, or a frame after the record ended).
    Plain,
    /// A worker message.
    Report(Report),
}

/// The kind of a worker message, as the arms tell them apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Report {
    /// `Launched`: the ready report (Start to Running).
    Launched,
    /// `Exited`: the exit report (Stop to Exited).
    Exited,
    /// Any other worker message.
    Other,
}

/// How a link ended, kept until the driver reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum End {
    /// `Ok(0)`: the peer closed the link.
    Closed,
    /// A read error of that kind.
    Failed(io::ErrorKind),
}

/// The edge's record of one link. Offsets count the link's inbound bytes from its first byte.
#[derive(Debug)]
struct Link {
    /// `None` once the record of the link ended.
    decoder: Option<FrameDecoder>,
    /// The instance that the link's hello names.
    instance: Option<InstanceId>,
    /// The bytes read from the inner edge and not yet given to the driver: the bytes from `given` to `read`.
    ahead: VecDeque<u8>,
    read: u64,
    given: u64,
    /// The offset where the frame that the decoder reads now began.
    frame_start: u64,
    /// The complete frames from `clear` on, not yet given out or held: start, end and class.
    frames: VecDeque<(u64, u64, Class)>,
    /// The driver may take the bytes before this offset.
    clear: u64,
    /// The hold that holds the frame at `clear`, if any: the bytes from `clear` on wait.
    holding: Option<Holding>,
    /// The end that a read met after `ahead`, kept until the driver reads it.
    end: Option<End>,
    /// `break_control` broke the link while a hold was set: every later byte is dropped.
    broken: bool,
}

impl Link {
    fn new() -> Link {
        Link {
            decoder: Some(FrameDecoder::new(BOUND)),
            instance: None,
            ahead: VecDeque::new(),
            read: 0,
            given: 0,
            frame_start: 0,
            frames: VecDeque::new(),
            clear: 0,
            holding: None,
            end: None,
            broken: false,
        }
    }

    /// Records `bytes` that a read took from the inner edge: they go to `ahead`, and the complete frames among them to
    /// `frames`.
    fn took(&mut self, bytes: &[u8]) {
        self.ahead.extend(bytes);
        self.read += bytes.len() as u64;
        let mut rest = bytes;
        let mut at = self.read - bytes.len() as u64;
        while let Some(decoder) = self.decoder.as_mut() {
            // The decoder takes bytes up to the end of the current frame.
            let used = decoder.push(rest);
            rest = &rest[used..];
            at += used as u64;
            let frame = match decoder.next_frame() {
                Ok(Some(frame)) => frame,
                Ok(None) => return,
                // The bound is the largest length that a header holds, so this is never a length error.
                Err(_) => {
                    self.decoder = None;
                    return;
                }
            };
            let start = std::mem::replace(&mut self.frame_start, at);
            let class = match &self.instance {
                None => {
                    self.instance = (frame.kind == FrameType::HELLO)
                        .then(|| Hello::decode(&frame.payload).ok().map(|h| h.instance))
                        .flatten();
                    if self.instance.is_none() {
                        self.decoder = None;
                    }
                    Class::Plain
                }
                Some(_) => match (frame.kind == FrameType::WORKER_MSG)
                    .then(|| WorkerMsg::decode(&frame.payload).ok())
                    .flatten()
                {
                    Some(WorkerMsg::Launched { .. }) => Class::Report(Report::Launched),
                    Some(WorkerMsg::Exited { .. }) => Class::Report(Report::Exited),
                    Some(_) => Class::Report(Report::Other),
                    // The driver closes the link at this frame: the record ends.
                    None => {
                        self.decoder = None;
                        Class::Plain
                    }
                },
            };
            self.frames.push_back((start, at, class));
        }
    }

    /// The bytes that the driver may take now.
    fn free(&self) -> usize {
        usize::try_from(self.clear - self.given).unwrap_or(usize::MAX)
    }

    /// Gives the driver what it may take now, up to `buf`'s length. `None` when it may take nothing.
    fn give(&mut self, buf: &mut [u8]) -> Option<usize> {
        let n = self.free().min(buf.len());
        if n == 0 {
            return None;
        }
        for (slot, byte) in buf.iter_mut().zip(self.ahead.drain(..n)) {
            *slot = byte;
        }
        self.given += n as u64;
        Some(n)
    }
}

/// The report holds of one host.
#[derive(Debug, Default)]
pub struct ReportHolds {
    /// The armed holds of each session, by its instance, in the order of the controls.
    arms: BTreeMap<InstanceId, VecDeque<Kind>>,
    links: BTreeMap<LinkId, Link>,
    /// The worker process of each instance that the host spawned.
    workers: BTreeMap<ProcessIdentity, InstanceId>,
    /// The reaped exits of workers whose instance has an arm or a hold, in the order of their reap.
    exits: VecDeque<(ProcessIdentity, ExitStatus)>,
}

impl ReportHolds {
    /// Whether the bytes of a link of `instance` (`None`: not known yet) are taken ahead, frame by frame.
    fn takes_ahead(
        arms: &BTreeMap<InstanceId, VecDeque<Kind>>,
        instance: Option<&InstanceId>,
    ) -> bool {
        match instance {
            Some(instance) => arms.get(instance).is_some_and(|arms| !arms.is_empty()),
            None => arms.values().any(|arms| !arms.is_empty()),
        }
    }

    /// The kinds of the holds of `instance`: armed, and holding a frame.
    fn kinds_of(&self, instance: &InstanceId) -> Vec<Kind> {
        let armed = self.arms.get(instance).into_iter().flatten().copied();
        let holding = self
            .links
            .values()
            .filter(|link| link.instance.as_ref() == Some(instance))
            .filter_map(|link| link.holding.map(|h| h.kind));
        armed.chain(holding).collect()
    }

    /// Gives out, or holds, the complete frames of `link` in order, and moves `clear`.
    fn settle(&mut self, link: LinkId) {
        let Some(state) = self.links.get_mut(&link) else {
            return;
        };
        let arms = state
            .instance
            .as_ref()
            .and_then(|instance| self.arms.get_mut(instance));
        if state.holding.is_none() {
            let mut arms = arms;
            while let Some((start, end, class)) = state.frames.pop_front() {
                let taken = match (class, arms.as_deref_mut()) {
                    // A frame whose first byte the driver has taken is never held.
                    (Class::Report(report), Some(arms)) if start >= state.given => arms
                        .iter()
                        .position(|kind| kind.takes(report))
                        .map(|at| arms.remove(at).expect("the position is inside the arms")),
                    _ => None,
                };
                if let Some(kind) = taken {
                    state.clear = start;
                    state.frames.push_front((start, end, class));
                    state.holding = Some(Holding { kind, pumps: 1 });
                    break;
                }
                state.clear = end;
            }
        }
        if state.holding.is_none() && state.decoder.is_none() {
            // The record ended: nothing later is held.
            state.clear = state.read;
        }
        if state.holding.is_none() && !Self::takes_ahead(&self.arms, state.instance.as_ref()) {
            // No arm can take a later frame: every byte passes through, a partial frame too.
            state.clear = state.read;
        }
    }

    /// The driver's `link_recv` on `link`, over `read`, the inner edge's own `link_recv` of that link.
    ///
    /// # Errors
    /// The inner edge's error, in its place in the link's bytes.
    pub fn recv(
        &mut self,
        link: LinkId,
        buf: &mut [u8],
        mut read: impl FnMut(&mut [u8]) -> io::Result<usize>,
    ) -> io::Result<usize> {
        self.links.entry(link).or_insert_with(Link::new);
        loop {
            let state = self.links.get_mut(&link).expect("the link is recorded");
            if let Some(n) = state.give(buf) {
                return Ok(n);
            }
            if let Some(end) = state.end {
                // Every byte before the end is read. A held frame waits, and the end waits behind it.
                if state.holding.is_some() {
                    return Err(io::ErrorKind::WouldBlock.into());
                }
                // With no hold, a partial frame goes out before the end.
                state.clear = state.read;
                if let Some(n) = state.give(buf) {
                    return Ok(n);
                }
                state.end = None;
                return match end {
                    End::Closed => Ok(0),
                    End::Failed(kind) => Err(kind.into()),
                };
            }
            // With no arm for the link, a read asks for no more than the driver does, so the edge keeps no byte that the
            // driver did not ask for. Taken ahead, the link is read in chunks.
            let ahead =
                state.holding.is_some() || Self::takes_ahead(&self.arms, state.instance.as_ref());
            let mut chunk = vec![0u8; if ahead { READ_CHUNK } else { buf.len() }];
            match read(&mut chunk) {
                Ok(0) => state.end = Some(End::Closed),
                // A broken link's later bytes are dropped (`break_control`).
                Ok(_) if state.broken => {}
                Ok(n) => {
                    state.took(&chunk[..n]);
                    self.settle(link);
                }
                // Nothing more now: the driver takes nothing (a held frame, or a partial frame, waits).
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => return Err(e),
                // No progress: the driver reads again (#287 RC-R2-1: no retry here without progress).
                Err(e) if e.kind() == io::ErrorKind::Interrupted => return Err(e),
                Err(e) => state.end = Some(End::Failed(e.kind())),
            }
        }
    }

    /// The driver closed `link`.
    pub fn closed(&mut self, link: LinkId) {
        self.links.remove(&link);
    }

    /// The host spawned the worker `identity` of `instance`.
    pub fn spawned(&mut self, identity: ProcessIdentity, instance: InstanceId) {
        self.workers.insert(identity, instance);
    }

    /// Whether the exit of the worker `identity` waits for a hold of its instance.
    fn exit_waits(&self, identity: ProcessIdentity) -> bool {
        let Some(instance) = self.workers.get(&identity) else {
            return false;
        };
        let open = self
            .links
            .values()
            .any(|link| link.instance.as_ref() == Some(instance) && !link.broken);
        open && !self.kinds_of(instance).is_empty()
    }

    /// The process edge's `poll_process_exit`, over `poll`, the inner edge's own. An exit of a worker whose instance has a hold
    /// waits; it is given, in the order of the reaps, once the hold ends.
    pub fn poll_exit(
        &mut self,
        mut poll: impl FnMut() -> Option<(ProcessIdentity, ExitStatus)>,
    ) -> Option<(ProcessIdentity, ExitStatus)> {
        if let Some(at) = self.exits.iter().position(|(id, _)| !self.exit_waits(*id)) {
            return self.exits.remove(at);
        }
        while let Some(exit) = poll() {
            if !self.exit_waits(exit.0) {
                return Some(exit);
            }
            self.exits.push_back(exit);
        }
        None
    }

    /// Whether this host's edge holds anything that it would give the driver now: free bytes, or a free exit.
    pub fn gives(&self) -> bool {
        self.links.values().any(|link| link.free() > 0)
            || self.exits.iter().any(|(id, _)| !self.exit_waits(*id))
    }

    /// A pump begins (the driver asks `ChoicePoint::PumpBound`). Each `scheduler_delay` hold counts it, and the hold whose
    /// pumps have passed gives its frames out, so the driver reads them in this pump.
    pub fn pump_started(&mut self) {
        let ids: Vec<LinkId> = self.links.keys().copied().collect();
        for link in ids {
            let Some(state) = self.links.get_mut(&link) else {
                continue;
            };
            if let Some(Holding {
                kind: Kind::Delay { pumps, .. },
                pumps: begun,
            }) = state.holding.as_mut()
            {
                *begun += 1;
                if *begun > *pumps {
                    self.end_hold(link);
                }
            }
        }
    }

    /// Ends the hold of `link`: its frame and every later frame go to the settle, in order.
    fn end_hold(&mut self, link: LinkId) {
        if let Some(state) = self.links.get_mut(&link) {
            state.holding = None;
            if let Some((_, end, _)) = state.frames.pop_front() {
                state.clear = end;
            }
        }
        self.settle(link);
    }

    /// `hold_worker_report`: arms the hold of the worker's next report.
    ///
    /// # Errors
    /// `Bad` when the session has a `hold_worker_report` already; `Refused({reason: "mixed_holds"})` when it has a
    /// `scheduler_delay`.
    pub fn hold_worker_report(&mut self, instance: InstanceId) -> Result<(), ControlError> {
        self.arm(instance, Kind::Worker, "hold_worker_report")
    }

    /// `scheduler_delay`: arms the hold of the next report of `point`, for `pumps` pumps.
    ///
    /// # Errors
    /// `Refused({reason: "mixed_holds"})` when the session has a `hold_worker_report`.
    pub fn delay(
        &mut self,
        instance: InstanceId,
        point: Point,
        pumps: u32,
    ) -> Result<(), ControlError> {
        self.arm(instance, Kind::Delay { point, pumps }, "scheduler_delay")
    }

    fn arm(&mut self, instance: InstanceId, kind: Kind, control: &str) -> Result<(), ControlError> {
        let kinds = self.kinds_of(&instance);
        if kinds.iter().any(|k| !k.same_control(kind)) {
            return Err(ControlError::Refused(json!({"reason": "mixed_holds"})));
        }
        if kind == Kind::Worker && !kinds.is_empty() {
            return Err(ControlError::Bad(format!(
                "{control}: the session has a hold of its next report already"
            )));
        }
        self.arms.entry(instance).or_default().push_back(kind);
        Ok(())
    }

    /// `release_worker_report`: ends the session's `hold_worker_report`. A hold that holds a frame gives it out; an armed one
    /// is disarmed. True when the edge gives bytes or an exit now (the caller signals the wake).
    ///
    /// # Errors
    /// `Bad` when the session has no `hold_worker_report`.
    pub fn release_worker_report(&mut self, instance: &InstanceId) -> Result<bool, ControlError> {
        if let Some(arms) = self.arms.get_mut(instance) {
            if let Some(at) = arms.iter().position(|k| *k == Kind::Worker) {
                arms.remove(at);
                return Ok(self.gives());
            }
        }
        let held = self.links.iter().find_map(|(link, state)| {
            (state.instance.as_ref() == Some(instance)
                && state.holding.is_some_and(|h| h.kind == Kind::Worker))
            .then_some(*link)
        });
        let Some(link) = held else {
            return Err(ControlError::Bad(
                "release_worker_report: the session has no hold_worker_report".to_string(),
            ));
        };
        self.end_hold(link);
        Ok(self.gives())
    }

    /// `held_reports`: the holds of the session that hold a frame now.
    pub fn held_reports(&self, instance: &InstanceId) -> usize {
        self.links
            .values()
            .filter(|link| link.instance.as_ref() == Some(instance) && link.holding.is_some())
            .count()
    }

    /// `break_control` on a session: when it has a hold, every byte that the edge took ahead is dropped, its holds end, and
    /// every later byte of its links is dropped up to the end. True when the session had a hold.
    pub fn broken(&mut self, instance: &InstanceId) -> bool {
        if self.kinds_of(instance).is_empty() {
            return false;
        }
        self.arms.remove(instance);
        for state in self.links.values_mut() {
            if state.instance.as_ref() == Some(instance) {
                state.given = state.read;
                state.clear = state.read;
                state.ahead.clear();
                state.frames.clear();
                state.holding = None;
                state.decoder = None;
                state.broken = true;
            }
        }
        true
    }
}

/// The controls of this module. The testkit registers them; the real tier serves the same names from its tap.
pub(crate) const CONTROLS: [&str; 4] = [
    "hold_worker_report",
    "release_worker_report",
    "held_reports",
    "scheduler_delay",
];

pub(crate) fn register_controls(registry: &mut ControlRegistry) {
    registry.register("hold_worker_report", |h, handle, args| {
        testkit_control(h, handle, "hold_worker_report", args)
    });
    registry.register("release_worker_report", |h, handle, args| {
        testkit_control(h, handle, "release_worker_report", args)
    });
    registry.register("held_reports", |h, handle, args| {
        testkit_control(h, handle, "held_reports", args)
    });
    registry.register("scheduler_delay", |h, handle, args| {
        testkit_control(h, handle, "scheduler_delay", args)
    });
}

/// The testkit tier: the session's instance is the one that its stored row names.
fn testkit_control(
    harness: &mut TestkitHarness,
    handle: &str,
    op: &str,
    args: &Value,
) -> Result<Value, ControlError> {
    let (holds, wake) = harness
        .report_holds_of(handle)
        .ok_or_else(|| ControlError::Bad(format!("the handle '{handle}' is not open")))?;
    control(
        &holds,
        op,
        args,
        |session| Ok(session_row(harness, handle, session)?.instance),
        || wake.signal(),
    )
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OfSession {
    session: SessionId,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Delay {
    session: SessionId,
    point: Point,
    pumps: u32,
}

/// One control of this module on the holds of a host, on both tiers. `instance_of` gives the instance of a session (its
/// stored row); `signal` signals the host's wake, which a release that gives bytes or an exit out calls. Neither runs while
/// the holds are locked.
///
/// # Errors
/// `Bad` arguments or a refused hold (see each control); `Unsupported` for a name that is not one of [`CONTROLS`].
pub(crate) fn control(
    holds: &Mutex<ReportHolds>,
    op: &str,
    args: &Value,
    instance_of: impl FnOnce(&SessionId) -> Result<InstanceId, ControlError>,
    signal: impl FnOnce(),
) -> Result<Value, ControlError> {
    match op {
        "hold_worker_report" => {
            let OfSession { session } = parse(args)?;
            let instance = instance_of(&session)?;
            lock(holds).hold_worker_report(instance)?;
            Ok(Value::Null)
        }
        "release_worker_report" => {
            let OfSession { session } = parse(args)?;
            let instance = instance_of(&session)?;
            let gives = lock(holds).release_worker_report(&instance)?;
            if gives {
                signal();
            }
            Ok(Value::Null)
        }
        "held_reports" => {
            let OfSession { session } = parse(args)?;
            let instance = instance_of(&session)?;
            let count = lock(holds).held_reports(&instance);
            Ok(json!({ "count": count }))
        }
        "scheduler_delay" => {
            let Delay {
                session,
                point,
                pumps,
            } = parse(args)?;
            if pumps == 0 {
                return Err(ControlError::Bad(
                    "scheduler_delay: `pumps` is at least 1 (the pump that holds the report is pump 1)".to_string(),
                ));
            }
            let instance = instance_of(&session)?;
            lock(holds).delay(instance, point, pumps)?;
            Ok(Value::Null)
        }
        _ => Err(ControlError::Unsupported),
    }
}

#[cfg(test)]
mod harness_tests;

#[cfg(test)]
mod tests;
