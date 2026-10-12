//! The stall clock of a route (steward ruling R-90; DESIGN.md "The stall clock (R-90)").
//!
//! **The split.** Core's host owns the two deadlines (`stall_deadline` and `stall_close_after`) on its injected clock (TM-1,
//! TM-3). The worker owns the route machine (OU-2): it measures progress (OU-3a), makes every transition, and reports it. The
//! worker reads no clock for a stall: the host tells it that a deadline is due ([`HostMsg::RouteStallDue`],
//! [`HostMsg::RouteCloseDue`]), as `stop_grace` ends in `HostMsg::Kill`.
//!
//! **The progress mark.** Each route counts the writes that its transport accepted bytes of (`RouteWritten Ok(n)`, `n > 0`:
//! the progress point of OU-3a). A readiness that takes no byte does not count (OU-6). The worker reports
//! `RouteProgress{mark, waiting}` when `waiting` changes, and when the mark changes while `waiting` is true. `waiting` is
//! true while the route is `Open` and holds bytes that the transport has not accepted.
//!
//! **Coalescing and order.** As the `Output` report does (`worker.rs`), a route's change waits while its last progress report
//! is not written, and goes out when it is written. Before any other report, every route with a waiting change sends one
//! report of its latest state (`flush_route_progress`, called by `report`). So the report of a change is on the link before
//! every report that the worker sends after that change, and progress reports alone queue at most one unwritten report per
//! route (one more for each other report).
//!
//! **The mark check.** A due message names the mark of the host's view. The worker makes the transition only while its
//! own state still matches: a report of newer progress is then already on the link, after the host's view, so a reader that
//! makes progress never loses output (OU-3).
//!
//! **Stall and resync.** At the stall (OU-3b) the worker drops the droppable frames that are not started, and queues
//! `resync{stalled}` at the resync point: right after a frame that is started (OU-4), before the kept frames. The `resync`
//! is the first write after the started frame, so the route resumes on the first byte that its transport accepts. At the
//! resume, the new baseline sequence and the output that the model holds go right behind the `resync` (OU-9).
//!
//! [`HostMsg::RouteStallDue`]: botster_core_link::msg::HostMsg::RouteStallDue
//! [`HostMsg::RouteCloseDue`]: botster_core_link::msg::HostMsg::RouteCloseDue

use super::{Kind, Queued, Worker};
use crate::worker::LinkState;
use botster_core_contract::prelude::*;
use botster_core_link::msg::WorkerMsg;
use botster_route_codec::prelude::{Resync, ResyncReason, ToClient};

/// The stall state of one route.
#[derive(Debug, Default)]
pub(super) struct Clock {
    /// The writes of the route that the transport accepted bytes of (OU-3a).
    mark: u64,
    /// `Some(mark)` while the route is `Stalled`: the mark of the stall (OU-2).
    stalled: Option<u64>,
    /// The last state that a progress report sent on this link: the mark and `waiting`. `None` before the first one.
    sent: Option<(u64, bool)>,
    /// The end of the last progress report in the sent bytes (`queued_total` after it).
    sent_to: u64,
    /// The state changed while the last progress report was not written: one report of the latest state follows.
    unsent: bool,
}

impl Clock {
    pub(super) fn stalled(&self) -> bool {
        self.stalled.is_some()
    }

    /// The state to report: the mark, and whether the route is `Open` with bytes that the transport has not accepted.
    fn state(&self, queued: usize) -> (u64, bool) {
        (self.mark, self.stalled.is_none() && queued > 0)
    }

    /// True when `state` differs from the last report in a way that the host measures: `waiting` changed, or the mark
    /// changed while `waiting` is true. With no report yet, only a waiting route needs one (the host's default is "not
    /// waiting").
    fn changed(&self, state: (u64, bool)) -> bool {
        match self.sent {
            None => state.1,
            Some(sent) => sent.1 != state.1 || (state.1 && sent.0 != state.0),
        }
    }
}

impl Worker {
    /// The transport of route `id` accepted bytes (OU-3a): the mark advances, and a `Stalled` route resumes (OU-2).
    pub(super) fn route_progressed(&mut self, id: RouteId) {
        let Some(route) = self.routes.routes.get_mut(&id) else {
            return;
        };
        route.stall.mark += 1;
        if route.stall.stalled.is_some() {
            self.resume_route(id);
        }
    }

    /// `HostMsg::RouteStallDue` (R-90, OU-3b): the route becomes `Stalled` only while it is `Open`, it holds bytes that the
    /// transport has not accepted, and its mark is still `mark`. Otherwise the worker does nothing: its newer report is on
    /// the link already, and the host releases its reservation when it reads it.
    pub(in crate::worker) fn on_route_stall_due(&mut self, id: RouteId, mark: u64) {
        let Some(route) = self.routes.routes.get_mut(&id) else {
            return;
        };
        if route.stall.state(route.queued) != (mark, true) {
            return;
        }
        // The droppable frames that are not started are dropped (OU-3b). A started frame is completed (OU-4): a frame with
        // written bytes, or with a write out (the transport can accept its bytes). With no byte of the route accepted yet
        // (mark 0), the front frame is `attached`, which opens the stream: the resync point is after it too.
        let started = usize::from(route.written > 0 || route.writing || route.stall.mark == 0);
        let mut kept = route.queue.split_off(started.min(route.queue.len()));
        kept.retain(|frame| {
            let keep = frame.kind != Kind::Droppable;
            if !keep {
                route.queued -= frame.bytes.len();
                if frame.exempt {
                    route.baseline -= frame.bytes.len();
                }
            }
            keep
        });
        // The resync point is the frame boundary after the started frame (OU-9): the `resync` goes there, before the kept
        // frames. It is the first write after the started frame, so its acceptance resumes the route.
        let resync = route.entry(
            &ToClient::Resync(Resync {
                reason: ResyncReason::Stalled,
            })
            .encode(),
            Kind::Resync,
            true,
        );
        route.queue.push_back(resync);
        route.queue.append(&mut kept);
        route.stall.stalled = Some(mark);
        self.report(&WorkerMsg::RouteStalled { route: id, mark });
        self.pump_route(id);
        // A `Stalled` route leaves the PTY read budget (OU-3b, OU-7).
        self.settle_routes();
    }

    /// `HostMsg::RouteCloseDue` (R-90, OU-2): a route that is still `Stalled` at `mark` closes. The close keeps the first
    /// reason: `StallTimeout` with no reason before it (a failed close: no more frames, OU-2b), otherwise the healthy close's.
    pub(in crate::worker) fn on_route_close_due(&mut self, id: RouteId, mark: u64) {
        let Some(route) = self.routes.routes.get(&id) else {
            return;
        };
        if route.stall.stalled != Some(mark) {
            return;
        }
        let reason = route.closing.unwrap_or(RouteCloseReason::StallTimeout);
        self.end_route(id, reason);
    }

    /// `Stalled → Open` (OU-2): the new baseline sequence at a new cut `R`, and the output that the model holds after it, go
    /// right behind the `resync` (OU-9), and the host is told. A baseline that cannot be formed closes the route
    /// `SnapshotTooLarge` (OU-2).
    fn resume_route(&mut self, id: RouteId) {
        let Some(route) = self.routes.routes.get(&id) else {
            return;
        };
        let sequence = self.baseline_sequence(route);
        let output = self.unfed_output(route);
        let sequence = match sequence {
            Ok(frames) if frames.iter().all(|frame| route.fits(frame)) => frames,
            _ => {
                let route = self.routes.routes.get_mut(&id).expect("read above");
                route.stall.stalled = None;
                self.close_healthy(id, RouteCloseReason::SnapshotTooLarge);
                return;
            }
        };
        let route = self.routes.routes.get_mut(&id).expect("read above");
        route.stall.stalled = None;
        // The `resync` is unwritten, or partly written at the front; a `resync` written whole is gone, and the new sequence
        // goes first.
        let at = route
            .queue
            .iter()
            .position(|frame| frame.kind == Kind::Resync)
            .map_or(0, |i| i + 1);
        let mut entries: Vec<Queued> = Vec::with_capacity(sequence.len() + output.len());
        for frame in &sequence {
            entries.push(route.entry(frame, Kind::Droppable, true));
        }
        for frame in &output {
            entries.push(route.entry(frame, Kind::Droppable, false));
        }
        for (i, entry) in entries.into_iter().enumerate() {
            route.queue.insert(at + i, entry);
        }
        if let Some(resync) = route.queue.iter_mut().find(|f| f.kind == Kind::Resync) {
            resync.kind = Kind::Kept;
        }
        let mark = route.stall.mark;
        self.report(&WorkerMsg::RouteResumed { route: id, mark });
    }

    /// Sends each route's progress report whose state changed, or marks it to follow when the last one is written.
    pub(in crate::worker) fn report_route_progress(&mut self) {
        if self.link != LinkState::Ready {
            return;
        }
        let written = self.written_total;
        let mut due = Vec::new();
        for (&id, route) in &mut self.routes.routes {
            let state = route.stall.state(route.queued);
            if !route.stall.changed(state) {
                continue;
            }
            if written < route.stall.sent_to {
                route.stall.unsent = true;
            } else {
                due.push(id);
            }
        }
        for id in due {
            self.send_route_progress(id);
        }
    }

    /// The flush rule: before any other report, each route with a waiting change sends one report of its latest state, in
    /// route-id order. Called by `report`.
    pub(in crate::worker) fn flush_route_progress(&mut self) {
        let due: Vec<RouteId> = self
            .routes
            .routes
            .iter()
            .filter(|(_, route)| route.stall.unsent)
            .map(|(&id, _)| id)
            .collect();
        for id in due {
            self.send_route_progress(id);
        }
    }

    /// The driver wrote more of the link: a route whose last progress report is written sends its waiting change.
    pub(in crate::worker) fn route_progress_written(&mut self) {
        if self.link != LinkState::Ready {
            return;
        }
        let written = self.written_total;
        let due: Vec<RouteId> = self
            .routes
            .routes
            .iter()
            .filter(|(_, route)| route.stall.unsent && written >= route.stall.sent_to)
            .map(|(&id, _)| id)
            .collect();
        for id in due {
            self.send_route_progress(id);
        }
    }

    /// The new link of an adoption counts its bytes from zero, and its host has none of the old link's reports: each route
    /// reports its state again on it, after `Adopted` (Q3 of R-90: the new host arms from the reports after `Adopted`).
    pub(in crate::worker) fn routes_relinked(&mut self) {
        for route in self.routes.routes.values_mut() {
            route.stall.sent = None;
            route.stall.sent_to = 0;
            route.stall.unsent = false;
        }
    }

    fn send_route_progress(&mut self, id: RouteId) {
        let Some(route) = self.routes.routes.get_mut(&id) else {
            return;
        };
        route.stall.unsent = false;
        let (mark, waiting) = route.stall.state(route.queued);
        if !route.stall.changed((mark, waiting)) {
            return;
        }
        route.stall.sent = Some((mark, waiting));
        self.send_report(&WorkerMsg::RouteProgress {
            route: id,
            mark,
            waiting,
        });
        let sent_to = self.queued_total;
        if let Some(route) = self.routes.routes.get_mut(&id) {
            route.stall.sent_to = sent_to;
        }
    }
}
