//! The host's half of the route stall clock (steward ruling R-90; DESIGN.md "The route stall clock (R-90)").
//!
//! The host owns a route's two deadlines on the injected monotonic clock (TM-1, TM-3): the route's applied `stall_deadline`
//! (OU-1: default `reader_progress_deadline`) and `stall_close_after`. The worker owns the route machine (OU-2) and makes
//! every transition; the host only tells it that a deadline is due, as `stop_grace` ends in `HostMsg::Kill`.
//!
//! - **Arming.** The reader deadline runs from the pump that consumes the worker's `RouteProgress{waiting: true}` with a new
//!   mark, and ends with `waiting: false`. The pump that consumes a report is never earlier than the progress, so Core
//!   never stalls a reader early (OU-3). The close deadline runs from the pump that consumes `RouteStalled` (OU-2: no route
//!   closes by the stall alone before `stall_close_after`).
//! - **The due step** is one atomic step with a mandatory event (E3-1 item 2), as a due `Silent` is: it runs only in a
//!   pump with event budget (otherwise it is carried, and `more` is true), only with mandatory room (otherwise it is parked:
//!   it is absent from `ready()` and from `next_deadline()`, and a poll that frees room wakes the handle), and in due order
//!   (E3-1 item 5, TM-5). It sends the due message with the mark of the host's view, and holds one mandatory slot for the
//!   worker's answer (E3-1 item 4), so the event has room when the answer comes. No host-visible state changes before that
//!   event posts (R-90 follow-up, condition 1).
//! - **The answer.** `RouteStalled` or `RouteClosed` posts into the held slot. A worker that declines (its newer progress
//!   report, or its close, is already on the link) sends nothing more: the host gives the slot back when it consumes that
//!   newer report (condition 3).
//! - **No link, no clock** (R-90 Q3): a report that this host did not consume arms nothing, so a worker with no host runs
//!   no stall deadline, and a new host arms from the reports after `Adopted`.

use crate::engine::HostEngine;
use crate::io::LinkId;
use botster_core_contract::prelude::*;
use botster_core_link::msg::HostMsg;
use std::time::{Duration, Instant};

/// A due step that the host sent, and whose answer holds a mandatory slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Due {
    /// `RouteStallDue` at this mark.
    Stall(u64),
    /// `RouteCloseDue` for the stall at this mark.
    Close(u64),
}

/// The stall clock of one route, in the host's view.
#[derive(Debug)]
pub(crate) struct RouteClock {
    stall_deadline: Duration,
    stall_close_after: Duration,
    /// The mark of the last `RouteProgress{waiting: true}` that the host consumed, and the pump that consumed it.
    waiting: Option<(u64, Instant)>,
    /// The mark of the stall, and the instant of its close (`stall_close_after` after the pump that consumed `RouteStalled`).
    stalled: Option<(u64, Instant)>,
    /// The due step that was sent and is not answered.
    asked: Option<Due>,
    /// The asked step holds one mandatory slot for its answer. A mandatory frame that Core reads from the same link ahead of
    /// the answer, and that waits only for room, takes the slot over (`take_over_slot`): the answer is then an ordinary
    /// route report.
    slot: bool,
    /// The host consumed the worker's `RouteReceived` of this route. Before it, a route report with this id is from another
    /// route: a route that survived a host restart keeps its id in the worker, and this host numbers its routes from 1
    /// (ID-2; DP-8 maps them later). The worker sends `RouteReceived` before any report of the route that it binds, and the
    /// link is in order, so every report after it is this route's. The clock takes no report before it.
    received: bool,
    /// The due step ran in a pump with budget and found no mandatory room: it is parked until a poll frees room (E3-1 item
    /// 4). A due step with no budget is carried first; only a step that ran can be parked (E3-1 items 2 and 4).
    parked: bool,
}

impl RouteClock {
    pub(crate) fn new(limits: &AppliedRouteLimits) -> RouteClock {
        RouteClock {
            stall_deadline: limits.stall_deadline,
            stall_close_after: limits.stall_close_after,
            waiting: None,
            stalled: None,
            asked: None,
            slot: false,
            received: false,
            parked: false,
        }
    }

    /// The route's armed due step and its instant. None while a due step waits for its answer.
    pub(crate) fn deadline(&self) -> Option<(Instant, Due)> {
        if self.asked.is_some() {
            return None;
        }
        if let Some((mark, at)) = self.stalled {
            return Some((at, Due::Close(mark)));
        }
        let (mark, since) = self.waiting?;
        Some((since.checked_add(self.stall_deadline)?, Due::Stall(mark)))
    }

    /// True when the due step ran and found no mandatory room (E3-1 item 4).
    pub(crate) fn parked(&self) -> bool {
        self.parked
    }

    /// True while a due step of this route holds a mandatory slot for its answer.
    pub(crate) fn holds_slot(&self) -> bool {
        self.slot
    }
}

impl HostEngine {
    /// Runs the due step of `route` (R-90): it sends the due message with the host's mark, and holds one mandatory slot for
    /// the worker's answer. With no mandatory room the step is parked: it is not due again until a poll frees room (E3-1 item
    /// 4; `deadlines`).
    pub(crate) fn run_route_due(&mut self, route: RouteId) {
        let room = self.has_room();
        let Some(entry) = self.routes.get_mut(&route) else {
            return;
        };
        entry.clock.parked = !room;
        if !room {
            return;
        }
        let Some((_, due)) = entry.clock.deadline() else {
            return;
        };
        let session = entry.session.clone();
        let msg = match due {
            Due::Stall(mark) => HostMsg::RouteStallDue { route, mark },
            Due::Close(mark) => HostMsg::RouteCloseDue { route, mark },
        };
        let sent = self.send_msg(&session, msg);
        let entry = self.routes.get_mut(&route).expect("read above");
        if sent {
            entry.clock.asked = Some(due);
            entry.clock.slot = true;
            self.queue.reserve();
        } else {
            // No worker link: no stall deadline runs (R-90 Q3). The route closes with its session's loss.
            entry.clock.waiting = None;
            entry.clock.stalled = None;
        }
    }

    /// Ends the due step of `route` that waits for its answer, and gives back its slot when it still holds one.
    pub(crate) fn release_route_slot(&mut self, route: RouteId) {
        if let Some(entry) = self.routes.get_mut(&route) {
            entry.clock.asked = None;
            if std::mem::take(&mut entry.clock.slot) {
                self.queue.release();
            }
        }
    }

    /// The first route, in id order, of the worker of `link` whose due step holds a slot.
    pub(crate) fn slot_of_link(&self, link: LinkId) -> Option<RouteId> {
        let session = self.links.get(&link)?;
        self.routes
            .iter()
            .find(|(_, e)| e.session == *session && e.clock.slot)
            .map(|(id, _)| *id)
    }

    /// A mandatory frame (EV-5(b)) that Core reads from the worker's link ahead of a due step's answer, and that waits only
    /// for mandatory-queue room, takes over the slot of that step (R-90 follow-up, contracts `main` `1798a65d`): the
    /// reservation ends, and the frame's event posts in it. The step still waits for its answer, which is then an ordinary
    /// route report: with no room it is parked (EV-5(b)) until a poll frees room. Without this, a frame that the worker sent
    /// before its answer could wait for room that only the answer frees (0181's ST298-R1).
    pub(crate) fn take_over_slot(&mut self, link: LinkId) {
        let Some(route) = self.slot_of_link(link) else {
            return;
        };
        let entry = self.routes.get_mut(&route).expect("found above");
        entry.clock.slot = false;
        self.queue.release();
    }

    /// `RouteReceived` (DP-2) from the worker of `session`: the worker bound this route, so its later reports are this
    /// route's. A report from another session's worker names a route of that worker, not this one.
    pub(crate) fn route_received(&mut self, session: &SessionId, route: RouteId) {
        if let Some(entry) = self.routes.get_mut(&route) {
            if entry.session == *session {
                entry.clock.received = true;
            }
        }
    }

    /// The clock of `route` takes this report from the worker of `session`: the route is that session's, and the host
    /// consumed its `RouteReceived`.
    fn clock_takes_reports(&self, session: &SessionId, route: RouteId) -> bool {
        self.routes
            .get(&route)
            .is_some_and(|e| e.session == *session && e.clock.received)
    }

    /// `RouteProgress` (OU-3a): the reader deadline runs from this pump while the route waits, and ends when it does not.
    /// A report that differs from the view of a due stall step means that the worker declined it: the slot is given back.
    pub(crate) fn route_progress(
        &mut self,
        session: &SessionId,
        route: RouteId,
        mark: u64,
        waiting: bool,
    ) {
        if !self.clock_takes_reports(session, route) {
            return;
        }
        let now = self.now;
        let Some(entry) = self.routes.get_mut(&route) else {
            return;
        };
        if matches!(entry.clock.asked, Some(Due::Stall(asked)) if !waiting || asked != mark) {
            self.release_route_slot(route);
        }
        let Some(entry) = self.routes.get_mut(&route) else {
            return;
        };
        let before = entry.clock.waiting;
        entry.clock.waiting = match (waiting, before, now) {
            (false, ..) | (true, _, None) => None,
            // The same mark that already waits keeps its start.
            (true, Some((old, since)), _) if old == mark => Some((old, since)),
            (true, _, Some(now)) => Some((mark, now)),
        };
        // A new step, or none, replaces the step that was parked: only a step that ran can be parked (E3-1 items 2 and 4).
        if entry.clock.waiting != before {
            entry.clock.parked = false;
        }
    }

    /// `RouteStalled` (OU-2): the close deadline runs from this pump, and the reader deadline ends. The event goes into the
    /// slot that the due step held. False when the report is not this route's (`clock_takes_reports`): it posts no event.
    pub(crate) fn route_stalled(&mut self, session: &SessionId, route: RouteId, mark: u64) -> bool {
        if !self.clock_takes_reports(session, route) {
            return false;
        }
        self.release_route_slot(route);
        let now = self.now;
        if let Some(entry) = self.routes.get_mut(&route) {
            entry.clock.waiting = None;
            entry.clock.parked = false;
            entry.clock.stalled = now.and_then(|now| {
                now.checked_add(entry.clock.stall_close_after)
                    .map(|at| (mark, at))
            });
        }
        true
    }

    /// `RouteResumed` (OU-2): the close deadline ends. The worker's next progress report arms the reader deadline again.
    /// False when the report is not this route's: it posts no event.
    pub(crate) fn route_resumed(&mut self, session: &SessionId, route: RouteId) -> bool {
        if !self.clock_takes_reports(session, route) {
            return false;
        }
        if matches!(
            self.routes.get(&route).and_then(|e| e.clock.asked),
            Some(Due::Close(_))
        ) {
            self.release_route_slot(route);
        }
        if let Some(entry) = self.routes.get_mut(&route) {
            entry.clock.stalled = None;
            entry.clock.parked = false;
        }
        true
    }

    /// `RouteClosed` of the worker: a due step's slot is given back, and the close posts into it (or the session's end
    /// posts it later, OU-7).
    pub(crate) fn route_closed_report(&mut self, route: RouteId) {
        self.release_route_slot(route);
    }

    /// True when the frame of `route`'s answer has the slot that a due step held.
    pub(crate) fn route_holds_slot(&self, route: RouteId) -> bool {
        self.routes
            .get(&route)
            .is_some_and(|e| e.clock.holds_slot())
    }
}
