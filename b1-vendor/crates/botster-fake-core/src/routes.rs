//! Routes: `attach`, `Detach` and what the worker reports about them (Core OU-1, OU-2, DP-2, DP-7, A2-3).
//!
//! The route endpoint is `botster-fake-route`. This file only registers routes, maps ids and turns the worker's route closes into
//! host events. The route bytes never pass through the host (Core DP-1, TH-3): the harness reaches them with `route_write` and `route_read`.

use crate::pump::{fail, ok, Step};
use crate::state::*;
use crate::{FakeCore, Inner};
use botster_core_contract::prelude::*;
use botster_fake_route::{AttachSpec, HistoryRequest, Read, RouteEvent};
use botster_route_codec::prelude::{CloseReason, RouteEndReason};
use serde_json::Value;
use std::time::Duration;

impl Inner {
    /// Core OU-1: the sync registration of a route.
    pub(crate) fn attach(
        &mut self,
        _client: ClientId,
        session: SessionId,
        _transport: &RouteTransport,
        options: AttachOptions,
    ) -> Result<AttachResult, CoreError> {
        let state = self
            .adm
            .get(&session)
            .ok_or_else(|| CoreError::new(ErrorCode::UnknownSession, "no such session"))?
            .state;
        // Core A2-1: `attach` is admitted for `Starting`, `Running` and `Exited`.
        if !matches!(state, Adm::Starting | Adm::Running | Adm::Exited)
            || !self.sessions.contains_key(&session)
        {
            return Self::err(
                ErrorCode::WrongState,
                "attach is not admitted in this state",
            );
        }
        let limits = self.limits.clone();
        let tag_max = limits.max_route_tag_bytes as usize;
        // Core EV-8: a route that answers queries needs a positive `query_deadline`, at most `max_query_deadline`.
        let query = options.query_deadline;
        if options.answers_queries
            && !query.is_some_and(|d| d > Duration::ZERO && d <= limits.max_query_deadline)
        {
            return Self::err(
                ErrorCode::InvalidInput { field: None },
                "query_deadline is required and positive",
            );
        }
        // Core OU-1: `route_tag` and `owner` are at most `max_route_tag_bytes`.
        if options
            .route_tag
            .as_ref()
            .is_some_and(|t| t.len() > tag_max)
            || options.owner.as_ref().is_some_and(|t| t.len() > tag_max)
        {
            return Self::err(
                ErrorCode::InvalidInput { field: None },
                "a route tag is too long",
            );
        }
        let s = self.sessions.get_mut(&session).expect("session");
        // Core OU-1: `RouteLimit` at `routes_per_session`.
        if s.routes.len() >= limits.routes_per_session as usize {
            return Self::err(ErrorCode::RouteLimit, "routes_per_session reached");
        }
        let Some(worker) = s.worker.as_mut() else {
            return Self::err(ErrorCode::WrongState, "the session has no worker");
        };
        let spec = AttachSpec {
            history: match options.history {
                Some(HistoryMode::Screen) => HistoryRequest::Screen,
                Some(HistoryMode::Full) => HistoryRequest::Full,
                _ => HistoryRequest::None,
            },
            route_features: options.route_features.clone(),
            writable: options.input,
            answers_queries: options.answers_queries,
        };
        let wroute = worker.attach(&spec);
        let route = RouteId(self.next_route);
        self.next_route += 1;
        s.routes.insert(route, wroute);
        let instance = s.instance.clone();
        self.routes.insert(
            route,
            RouteRef {
                session,
                instance,
                worker_route: wroute,
                ready: false,
                closed: false,
                tag: options.route_tag.clone(),
            },
        );
        self.signal();
        let choice = options.route_limits.unwrap_or_default();
        Ok(AttachResult {
            route,
            limits: AppliedRouteLimits {
                max_frame_bytes: choice
                    .max_frame_bytes
                    .unwrap_or(limits.effective_max_route_frame_bytes()),
                max_screen_frame_bytes: choice
                    .max_screen_frame_bytes
                    .unwrap_or(limits.max_snapshot_bytes + 1),
                max_history_page_bytes: choice
                    .max_history_page_bytes
                    .unwrap_or(limits.max_history_page_bytes),
                max_paste_bytes: limits.max_paste_bytes,
                max_query_bytes: limits.max_query_bytes,
                max_query_reply_bytes: limits.max_query_reply_bytes,
                max_file_bytes: limits.max_file_bytes,
                query_deadline: query.unwrap_or_default(),
                stall_deadline: options
                    .stall_deadline
                    .unwrap_or(limits.reader_progress_deadline),
                stall_close_after: limits.stall_close_after,
                route_input_queue_bytes: limits.route_input_queue_bytes,
            },
        })
    }

    /// Core DP-7: `Detach` completes after `RouteClosed` is posted.
    pub(crate) fn detach(&mut self, route: RouteId, reason: DetachReason) -> Step {
        let Some(r) = self.routes.get(&route).filter(|r| !r.closed) else {
            return fail(ErrorCode::UnknownRoute, "no such route");
        };
        let (sid, wroute) = (r.session.clone(), r.worker_route);
        let reason = match reason {
            DetachReason::Replaced => "replaced",
            DetachReason::Revoked => "revoked",
            _ => "detached",
        };
        if let Some(w) = self.sessions.get_mut(&sid).and_then(|s| s.worker.as_mut()) {
            let _ = w.control(
                "close_route",
                &serde_json::json!({ "reason": reason }),
                Some(wroute),
            );
        }
        self.drain_route_events(&sid);
        ok(OpOutput::Unit)
    }

    fn drain_route_events(&mut self, sid: &SessionId) {
        let events = self
            .sessions
            .get_mut(sid)
            .and_then(|s| s.worker.as_mut())
            .map(|w| w.take_route_events())
            .unwrap_or_default();
        for ev in events {
            self.route_event(sid, ev);
        }
    }

    /// A route closed in the worker: the host learns once, with the first reason (Core OU-2, A2-3).
    pub(crate) fn route_event(&mut self, sid: &SessionId, ev: RouteEvent) {
        let (wroute, reason) = match ev {
            RouteEvent::Closed {
                route,
                reason,
                exit,
            } => {
                let reason = match reason {
                    CloseReason::Replaced => RouteCloseReason::Replaced,
                    CloseReason::Revoked => RouteCloseReason::Revoked,
                    CloseReason::SessionEnded => {
                        let exit = self.sessions.get(sid).and_then(|s| s.exit).unwrap_or(Exit {
                            code: exit.and_then(|e| e.code),
                            signal: exit.and_then(|e| e.signal),
                            cause: ExitCause::Other,
                        });
                        RouteCloseReason::SessionEnded { exit }
                    }
                    CloseReason::ProtocolError { code } => RouteCloseReason::BadFrame { code },
                    CloseReason::AttachFailed { .. } => RouteCloseReason::SnapshotTooLarge,
                    _ => RouteCloseReason::Detached,
                };
                (route, reason)
            }
            RouteEvent::Ended { route, reason } => (route, end_reason(reason)),
        };
        let Some((route, r)) = self
            .routes
            .iter_mut()
            .find(|(_, r)| &r.session == sid && r.worker_route == wroute && !r.closed)
        else {
            return;
        };
        r.closed = true;
        let (route, tag) = (*route, r.tag.clone());
        if let Some(s) = self.sessions.get_mut(sid) {
            s.routes.remove(&route);
        }
        self.post(Event::RouteClosed {
            route,
            reason,
            route_tag: tag,
        });
    }
}

/// The client end of a route (Core DP-1: the host is off the data path). It keeps no hold on the handle that attached it: the worker keeps
/// the route when the host drops (Core DP-8), so the end reaches the worker through the data directory once the handle is gone.
pub struct FakeRouteEnd {
    core: std::sync::Weak<std::sync::Mutex<Inner>>,
    store: std::sync::Arc<std::sync::Mutex<StoreData>>,
    route: RouteId,
    session: SessionId,
    /// The instance that the route belongs to: a later instance of the same session id is never reached (Core ID-1).
    instance: InstanceId,
    wroute: u32,
}

impl FakeRouteEnd {
    /// Runs `f` on the worker of the route: through the handle that attached it, through the handle that adopted the session since, or
    /// through the data directory while no handle holds it.
    fn with<R>(
        &self,
        f: impl FnOnce(&mut botster_fake_route::FakeWorker, Option<bool>) -> R,
    ) -> Option<R> {
        let holder = || {
            let store = self.store.lock().ok()?;
            store.holders.get(&self.session)?.upgrade()
        };
        for arc in [self.core.upgrade(), holder()].into_iter().flatten() {
            let mut inner = arc.lock().ok()?;
            let ready = inner
                .routes
                .get(&self.route)
                .filter(|r| r.instance == self.instance)
                .map(|r| r.ready);
            if let Some(worker) = inner
                .sessions
                .get_mut(&self.session)
                .filter(|s| s.instance == self.instance)
                .and_then(|s| s.worker.as_mut())
            {
                return Some(f(worker, ready));
            }
        }
        let mut store = self.store.lock().ok()?;
        store
            .rows
            .get_mut(&self.session)
            .filter(|s| s.instance == self.instance)?
            .worker
            .as_mut()
            .map(|w| f(w, None))
    }

    pub fn write(&self, bytes: &[u8]) {
        self.with(|w, _| w.write(self.wroute, bytes));
        // Core TH-3: the program reads its input as it arrives. The data plane needs no host pump (Core TH-3).
        if let Some(arc) = self.core.upgrade() {
            if let Ok(mut inner) = arc.lock() {
                inner.run_program(&self.session);
            }
        }
    }

    /// The baseline is released by the first pump after the attach (Core OU-9, OR-1).
    pub fn read(&self, max: usize) -> Read {
        self.with(|w, ready| {
            if ready == Some(false) {
                Read::Empty
            } else {
                w.read(self.wroute, max)
            }
        })
        .unwrap_or(Read::Eof { ended: None })
    }

    /// A control of the worker that serves the route (design 6.3): the test hooks of `botster-fake-route`.
    pub fn control(&self, op: &str, args: &Value) -> Result<Value, String> {
        self.with(|w, _| w.control(op, args, Some(self.wroute)).map_err(|e| e.0))
            .unwrap_or_else(|| Err("no worker".into()))
    }
}

impl FakeCore {
    /// The client end of a route that this handle minted (Core ID-1: a route id belongs to one handle).
    pub fn route_end(&self, route: RouteId) -> Option<FakeRouteEnd> {
        let inner = self.lock();
        let r = inner.routes.get(&route)?;
        Some(FakeRouteEnd {
            core: std::sync::Arc::downgrade(&self.inner),
            store: inner.store.clone(),
            route,
            session: r.session.clone(),
            instance: r.instance.clone(),
            wroute: r.worker_route,
        })
    }
}

/// The host close reason of a route that the worker ended (Core OU-2b). The fake worker never sends `handoff_failed` or
/// `session_lost`, so only this unit test reaches those arms.
fn end_reason(reason: RouteEndReason) -> RouteCloseReason {
    match reason {
        RouteEndReason::HandoffFailed => RouteCloseReason::HandoffFailed,
        RouteEndReason::WriteFailed => RouteCloseReason::WriteFailed,
        RouteEndReason::Stalled => RouteCloseReason::StallTimeout,
        RouteEndReason::SessionLost => RouteCloseReason::SessionLost,
        _ => RouteCloseReason::PeerClosed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_worker_end_reason_maps_to_one_close_reason() {
        let cases = [
            (RouteEndReason::NotConnected, RouteCloseReason::PeerClosed),
            (
                RouteEndReason::HandoffFailed,
                RouteCloseReason::HandoffFailed,
            ),
            (RouteEndReason::TransportLost, RouteCloseReason::PeerClosed),
            (RouteEndReason::WriteFailed, RouteCloseReason::WriteFailed),
            (RouteEndReason::Stalled, RouteCloseReason::StallTimeout),
            (RouteEndReason::SessionLost, RouteCloseReason::SessionLost),
            (RouteEndReason::Other, RouteCloseReason::PeerClosed),
        ];
        for (end, close) in cases {
            assert_eq!(end_reason(end), close, "{end:?}");
        }
    }
}
