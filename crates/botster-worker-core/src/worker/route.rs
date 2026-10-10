//! The stream routes of the session (P4a; DESIGN.md "P4a: the stream route").
//!
//! **Handoff (DP-2).** A route's stream rides on the first byte of its `AttachRoute` frame, so the driver delivers
//! [`Input::Descriptor`](super::Input::Descriptor) before the bytes of that frame. The machine keeps the unbound
//! descriptors of the current link in order, and each `AttachRoute` binds the oldest one.
//!
//! **Baseline (OU-9).** At the bind, in one step, the worker takes the baseline at the model's consumed cut `R`: `attached`,
//! `baseline_begin`, `screen`, `baseline_end`, `live`. Output that the model holds and has not applied (`unfed`) is after
//! `R`, so it is the first `output` frame; later PTY output follows. Each byte after `R` is on the route once.
//!
//! **Backpressure (OU-3d).** Each route has a queue with the threshold `route_queue_bytes`. The worker gives the driver a
//! PTY read budget: the smallest free payload space of the routes, so that no read can overflow a queue and no byte is
//! dropped. The baseline goes whole into the route's empty queue at the bind. Steward ruling R-45: the one baseline
//! sequence in delivery (the attach frames through `live`, with their stream prefixes) is exempt from the threshold, and
//! only the frames behind it count (the output after `R`, the held suffix included). A route over the threshold only
//! because of its baseline is not "not progressing" (9B) and gets no stall clock of its own (this PR has no stall clock).
//! While the frames behind the sequence are at the threshold, the budget is 0 and no PTY byte is read. A resync (a later
//! PR) starts a new sequence and must set its exempt bytes the same way.
//!
//! **Frame bounds (DP-3).** Every frame is checked against its codec bound (`bound_of`) before it is queued, and the
//! snapshot's size is checked before it is allocated. A route whose limits cannot carry the attach frames is closed
//! `HandoffFailed` with no frame (steward ruling R-44; the host's floor check, A19-1, keeps this from happening). So is a
//! route with no common terminal format (the host refuses it first, OU-1).

use super::{model, Action, Worker};
use botster_core_contract::prelude::*;
use botster_core_link::msg::WorkerMsg;
use botster_route_codec::prelude::{
    bound_of, stream_wrap, AttachFailedReason, Attached, BaselineBegin, BaselineEnd, CloseReason,
    FrameBounds, HistoryState, HistoryUnavailable, RouteClosed as RouteClosedFrame, RouteLimits,
    ToClient,
};
use std::collections::{BTreeMap, VecDeque};

/// A descriptor that the control link delivered. The driver keeps the object; the machine has the id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DescriptorId(pub u64);

/// The optional route features that this worker can send (OU-1: `attached.features` is the intersection with the client's).
/// None yet: `output_deflate` is not negotiated (DP-3).
const ROUTE_FEATURES: &[&str] = &[];

/// The bytes that the stream binding adds to a frame (`stream_wrap`, TB-L2), and the overhead of one `output` frame (that
/// prefix and the type byte).
const STREAM_PREFIX: usize = 4;
const OUTPUT_OVERHEAD: usize = STREAM_PREFIX + 1;

/// The routes of the session, and the descriptors that wait for their `AttachRoute`.
#[derive(Debug, Default)]
pub(super) struct Routes {
    unbound: VecDeque<DescriptorId>,
    routes: BTreeMap<RouteId, Route>,
    /// The last budget that the driver got was a limit: when the last route ends, the limit is lifted once.
    limited: bool,
}

#[derive(Debug)]
struct Route {
    limits: AppliedRouteLimits,
    route_tag: Option<String>,
    /// Stream-delimited frames in order; `written` bytes of the first one are on the transport.
    queue: VecDeque<Vec<u8>>,
    written: usize,
    /// The bytes of `queue` that are not written yet.
    queued: usize,
    /// The unwritten bytes of the baseline sequence (the attach frames through `live`, with their stream prefixes) at the
    /// front of `queue`. Steward ruling R-45 exempts them from the `route_queue_bytes` threshold: only the frames behind
    /// the sequence count.
    baseline: usize,
    /// A `RouteWrite` is out (one at a time).
    writing: bool,
    /// A healthy close: `route_closed` is the last queued frame, and the route closes when it is written (OU-2b).
    closing: Option<RouteCloseReason>,
}

impl Route {
    fn push(&mut self, frame: &ToClient) {
        self.push_encoded(&frame.encode());
    }

    fn push_encoded(&mut self, frame: &[u8]) {
        let bytes = stream_wrap(frame);
        self.queued += bytes.len();
        self.queue.push_back(bytes);
    }

    fn bounds(&self) -> FrameBounds {
        FrameBounds {
            max_frame: self.limits.max_frame_bytes,
            max_screen: self.limits.max_screen_frame_bytes,
            max_history: self.limits.max_history_page_bytes,
        }
    }

    /// True when the encoded frame is within its bound on this route (DP-3, `bound_of`).
    fn fits(&self, frame: &[u8]) -> bool {
        frame
            .first()
            .is_some_and(|&kind| frame.len() as u64 <= bound_of(kind, &self.bounds()))
    }

    /// The PTY bytes that the queue can still take as `output` frames: `free` bytes of queue hold `n` payload bytes in
    /// `ceil(n / per)` frames of `OUTPUT_OVERHEAD` each. With `f = free / (per + OUTPUT_OVERHEAD) + 1`, the payload
    /// `free - f * OUTPUT_OVERHEAD` needs at most `f` frames, so it always fits.
    fn free_payload(&self, queue_bytes: usize) -> usize {
        let free = queue_bytes.saturating_sub(self.queued - self.baseline);
        let per = self.output_payload();
        let frames = free / (per + OUTPUT_OVERHEAD) + 1;
        free.saturating_sub(frames * OUTPUT_OVERHEAD)
    }

    /// The payload of one `output` frame: the frame bound less the type byte (TS-3). A route that takes output carried its
    /// attach frames, so its bound holds the type byte and at least one byte.
    fn output_payload(&self) -> usize {
        usize::try_from(self.limits.max_frame_bytes)
            .unwrap_or(usize::MAX)
            .saturating_sub(1)
    }
}

impl Routes {
    pub(super) fn descriptor(&mut self, id: DescriptorId) {
        self.unbound.push_back(id);
    }

    /// The descriptors of a link that ended or was replaced (DP-8): none of them will be bound.
    pub(super) fn take_unbound(&mut self) -> Vec<DescriptorId> {
        self.unbound.drain(..).collect()
    }
}

/// The wire form of the applied limits (OU-1, "wire projection").
fn wire_limits(limits: &AppliedRouteLimits) -> RouteLimits {
    let ms = |d: std::time::Duration| u64::try_from(d.as_millis()).unwrap_or(u64::MAX);
    RouteLimits {
        max_frame_bytes: limits.max_frame_bytes,
        max_screen_frame_bytes: limits.max_screen_frame_bytes,
        max_history_page_bytes: limits.max_history_page_bytes,
        max_paste_bytes: limits.max_paste_bytes,
        max_file_bytes: limits.max_file_bytes,
        max_query_reply_bytes: limits.max_query_reply_bytes,
        query_client_deadline_ms: ms(limits.query_deadline),
        stall_deadline_ms: ms(limits.stall_deadline),
        stall_close_after_ms: ms(limits.stall_close_after),
    }
}

/// The client's first format that this worker can emit (OU-1); with no list, this worker's first.
fn negotiate(client: &[String]) -> Option<String> {
    let mine: Vec<String> = model::snapshot_formats()
        .iter()
        .map(botster_core_link::route::terminal_format)
        .collect();
    if client.is_empty() {
        return mine.into_iter().next();
    }
    client.iter().find(|name| mine.contains(name)).cloned()
}

impl Worker {
    /// `AttachRoute` (OU-1, OU-9, DP-2): the oldest unbound descriptor is this route's stream, and the baseline is queued in
    /// this step. An `AttachRoute` with no descriptor is a host fault: the link closes, as for a bad frame.
    pub(super) fn on_attach_route(
        &mut self,
        route: RouteId,
        options: AttachOptions,
        limits: AppliedRouteLimits,
    ) {
        let Some(descriptor) = self.routes.unbound.pop_front() else {
            self.close_link();
            return;
        };
        self.actions
            .push_back(Action::BindRoute { descriptor, route });
        let mut entry = Route {
            limits,
            route_tag: options.route_tag.clone(),
            queue: VecDeque::new(),
            written: 0,
            queued: 0,
            baseline: 0,
            writing: false,
            closing: None,
        };
        match self.baseline(&entry, &options) {
            Ok((frames, sequence)) => {
                for (i, frame) in frames.iter().enumerate() {
                    entry.push_encoded(frame);
                    if i + 1 == sequence {
                        // R-45: the baseline sequence ends at `live`; the output after `R` is behind it and counts.
                        entry.baseline = entry.queued;
                    }
                }
            }
            Err(RouteCloseReason::HandoffFailed) => {
                // R-44: a failed handoff sends no frame (OU-2b); the transport closes and the host is told once.
                self.routes.routes.insert(route, entry);
                self.end_route(route, RouteCloseReason::HandoffFailed);
                return;
            }
            Err(reason) => {
                // No partial baseline (OU-9): `route_closed` is the only frame, when the route's bound carries it.
                let closed = ToClient::RouteClosed(RouteClosedFrame {
                    reason: CloseReason::AttachFailed {
                        reason: AttachFailedReason::SnapshotTooLarge,
                    },
                    exit: None,
                })
                .encode();
                entry.closing = Some(reason);
                if !entry.fits(&closed) {
                    // Not even `route_closed` fits: the transport closes with no frame.
                    self.routes.routes.insert(route, entry);
                    self.end_route(route, reason);
                    return;
                }
                entry.push_encoded(&closed);
            }
        }
        self.routes.routes.insert(route, entry);
        self.pump_route(route);
        self.send_budget();
    }

    /// The encoded baseline frames at the consumed cut `R`, then the output after `R` that the model holds (OU-9), and the
    /// number of frames in the baseline sequence (through `live`, R-45). Each frame is within its bound on the route (DP-3).
    fn baseline(
        &self,
        route: &Route,
        options: &AttachOptions,
    ) -> Result<(Vec<Vec<u8>>, usize), RouteCloseReason> {
        // The host refuses an attach with no common format (OU-1); a worker that gets one cannot make a working route
        // from the options, so the handoff failed (R-44).
        let Some(terminal_format) = negotiate(&options.terminal_formats) else {
            return Err(RouteCloseReason::HandoffFailed);
        };
        // OU-2: a baseline that cannot be formed closes the route `SnapshotTooLarge`.
        let model = self
            .model
            .as_ref()
            .ok_or(RouteCloseReason::SnapshotTooLarge)?;
        // Two limits (DESIGN.md "Frames"): the native snapshot, and the route's screen frame (the snapshot and the type byte).
        // Both are checked on the library's size, before the snapshot is allocated (DP-3).
        let limit = self
            .limits
            .max_snapshot_bytes
            .min(route.limits.max_screen_frame_bytes.saturating_sub(1));
        let snapshot = model
            .term
            .snapshot_at_most(usize::try_from(limit).unwrap_or(usize::MAX))
            .ok()
            .flatten()
            .ok_or(RouteCloseReason::SnapshotTooLarge)?;
        let features = options
            .route_features
            .iter()
            .filter(|name| ROUTE_FEATURES.contains(&name.as_str()))
            .cloned()
            .collect();
        let history = match options.history {
            None | Some(HistoryMode::None) => HistoryState::NotRequested,
            // History pages are a later P4a PR; until then a route that asks for them is told that they are unavailable.
            Some(_) => HistoryState::Unavailable(HistoryUnavailable::Other),
        };
        let frames = [
            ToClient::Attached(Attached {
                features,
                terminal_format,
                limits: wire_limits(&route.limits),
            }),
            ToClient::BaselineBegin(BaselineBegin {
                rows: u32::from(model.term.rows()),
                cols: u32::from(model.term.cols()),
                modes: model.modes(),
            }),
            ToClient::Screen {
                payload: snapshot.into(),
            },
            ToClient::BaselineEnd(BaselineEnd { history }),
            ToClient::Live,
        ];
        let mut encoded: Vec<Vec<u8>> = frames.iter().map(ToClient::encode).collect();
        // A host keeps `max_frame_bytes` at least the attach frames (A19-1); a route below it cannot be served, so the
        // handoff failed (R-44).
        if !encoded.iter().all(|frame| route.fits(frame)) {
            return Err(RouteCloseReason::HandoffFailed);
        }
        let sequence = encoded.len();
        let per = route.output_payload();
        encoded.extend(model.unfed().chunks(per).map(|chunk| {
            ToClient::Output {
                payload: chunk.into(),
            }
            .encode()
        }));
        Ok((encoded, sequence))
    }

    /// PTY output goes to every route that is not closing, unchanged and in order, in `output` frames within the route's
    /// frame bound (OU-12, DP-3).
    pub(super) fn route_output(&mut self, bytes: &[u8]) {
        let ids: Vec<RouteId> = self.routes.routes.keys().copied().collect();
        for id in ids {
            let route = self.routes.routes.get_mut(&id).expect("listed");
            if route.closing.is_some() {
                continue;
            }
            let per = route.output_payload();
            for chunk in bytes.chunks(per) {
                route.push(&ToClient::Output {
                    payload: chunk.into(),
                });
            }
            self.pump_route(id);
        }
        self.send_budget();
    }

    /// Starts the next write of a route, when none is out.
    fn pump_route(&mut self, id: RouteId) {
        let Some(route) = self.routes.routes.get_mut(&id) else {
            return;
        };
        if route.writing {
            return;
        }
        let Some(front) = route.queue.front() else {
            return;
        };
        route.writing = true;
        let bytes = front[route.written..].to_vec();
        self.actions
            .push_back(Action::RouteWrite { route: id, bytes });
    }

    /// The answer to a `RouteWrite` (OU-3a: progress is the bytes that the kernel accepted).
    pub(super) fn on_route_written(&mut self, id: RouteId, result: Result<usize, i32>) {
        let Some(route) = self.routes.routes.get_mut(&id) else {
            return;
        };
        route.writing = false;
        let n = match result {
            // A terminal write error (the driver reports `WouldBlock` as `Ok(0)`): a failed close (OU-2b `write_failed`). A
            // healthy close that was delivering keeps its first reason (OU-2).
            Err(_) => {
                let reason = route.closing.unwrap_or(RouteCloseReason::WriteFailed);
                self.end_route(id, reason);
                return;
            }
            // Nothing taken: the next `RouteWritable` retries.
            Ok(0) => return,
            Ok(n) => n,
        };
        // A write is the rest of the front frame (`pump_route`), and the driver reports at most the bytes of the write.
        let Some(front) = route.queue.front() else {
            return;
        };
        route.written += n;
        route.queued -= n;
        // The queue is written front first, so the baseline sequence's bytes go first (R-45).
        route.baseline = route.baseline.saturating_sub(n);
        if route.written == front.len() {
            route.queue.pop_front();
            route.written = 0;
        }
        if route.queue.is_empty() {
            if let Some(reason) = route.closing {
                self.end_route(id, reason);
                return;
            }
        }
        self.pump_route(id);
        self.send_budget();
    }

    pub(super) fn on_route_writable(&mut self, id: RouteId) {
        self.pump_route(id);
    }

    /// Closes the transport once and reports the close once (OU-2).
    fn end_route(&mut self, id: RouteId, reason: RouteCloseReason) {
        let Some(route) = self.routes.routes.remove(&id) else {
            return;
        };
        self.actions.push_back(Action::RouteClose { route: id });
        self.report(&WorkerMsg::RouteClosed {
            route: id,
            reason,
            route_tag: route.route_tag,
        });
        self.send_budget();
    }

    /// The PTY read budget (OU-3d): the smallest free payload space of the routes that are not closing. With no such route
    /// the limit is lifted, once.
    fn send_budget(&mut self) {
        let queue_bytes = usize::try_from(self.limits.route_queue_bytes).unwrap_or(usize::MAX);
        let budget = self
            .routes
            .routes
            .values()
            .filter(|r| r.closing.is_none())
            .map(|r| r.free_payload(queue_bytes))
            .min();
        match budget {
            Some(n) => {
                self.routes.limited = true;
                self.actions.push_back(Action::PtyReadBudget(Some(n)));
            }
            None if self.routes.limited => {
                self.routes.limited = false;
                self.actions.push_back(Action::PtyReadBudget(None));
            }
            None => {}
        }
    }

    /// The unbound descriptors of a link that ended or was replaced are closed (DP-8).
    pub(super) fn close_unbound(&mut self) {
        for id in self.routes.take_unbound() {
            self.actions.push_back(Action::CloseDescriptor(id));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn route(max_frame_bytes: u64, queued: usize) -> Route {
        Route {
            limits: AppliedRouteLimits {
                max_frame_bytes,
                max_screen_frame_bytes: 0,
                max_history_page_bytes: 0,
                max_paste_bytes: 0,
                max_query_bytes: 0,
                max_query_reply_bytes: 0,
                max_file_bytes: 0,
                query_deadline: std::time::Duration::ZERO,
                stall_deadline: std::time::Duration::ZERO,
                stall_close_after: std::time::Duration::ZERO,
                route_input_queue_bytes: 0,
            },
            route_tag: None,
            queue: VecDeque::new(),
            written: 0,
            queued,
            baseline: 0,
            writing: false,
            closing: None,
        }
    }

    /// OU-3d: the budget is the largest payload whose `output` frames always fit the free queue space, for every frame size.
    #[test]
    fn the_free_payload_always_fits_the_queue_as_output_frames() {
        for max_frame in [2u64, 3, 7, 64, 1025] {
            for queued in [0usize, 1, 50, 99, 100] {
                let r = route(max_frame, queued);
                let payload = r.free_payload(100);
                let per = r.output_payload();
                let cost = payload + payload.div_ceil(per) * OUTPUT_OVERHEAD;
                assert!(
                    cost <= 100 - queued,
                    "frame {max_frame}, queued {queued}: {payload} costs {cost}"
                );
            }
        }
        assert_eq!(
            route(1025, 0).free_payload(100),
            95,
            "one frame of 95 bytes and its 5"
        );
        assert_eq!(
            route(1025, 100).free_payload(100),
            0,
            "a full queue takes nothing"
        );
        assert_eq!(
            route(1025, 101).free_payload(100),
            0,
            "an overfull queue takes nothing"
        );
    }

    #[test]
    fn an_output_frame_carries_the_frame_bound_less_the_type_byte() {
        assert_eq!(route(1024, 0).output_payload(), 1023);
        assert_eq!(route(2, 0).output_payload(), 1);
    }

    /// OU-1: the client's first format that the worker emits; with no list, the worker's own first.
    #[test]
    fn the_format_is_the_clients_first_that_the_worker_emits() {
        let mine = botster_core_link::route::terminal_format(&model::snapshot_formats()[0]);
        assert_eq!(negotiate(&[]), Some(mine.clone()));
        assert_eq!(
            negotiate(&["no_such_format".into(), mine.clone()]),
            Some(mine.clone())
        );
        assert_eq!(negotiate(&["no_such_format".into()]), None);
    }
}
