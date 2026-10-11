//! The worker's route reports that Core has consumed, for the control `route_reports {session, route}` (steward ruling R-66).
//! Both tiers keep this record at the same point: the host's `link_recv` edge (the real tier's tap, the testkit's in-memory
//! edge). It changes no byte and no call: it decodes only the bytes that the driver received, with Core's own decoders.
//!
//! - **Consumed.** The driver's `read_link` calls `link_recv` again only when it has decoded every byte that it read before:
//!   each complete frame went to the engine (`Delivery::Done`), or it holds one frame in `held` and reads again only after
//!   `retry_held` gave it to the engine. The engine posts the events of a frame inside that call (`feed`). So when the
//!   driver calls `link_recv` on a link again, every frame that was complete in the bytes it received before is consumed, and
//!   its events are posted. A frame becomes consumed at that later call, never earlier.
//! - **The answer.** `closed`: Core consumed the worker's `RouteClosed` for the route. `activity_before_close`: Core consumed a
//!   `ClientInput` observation of the route (the report that posts `Activity{source: Client(route)}`) before that close, or
//!   with no close yet. Only the frames of one link are ordered: a route is the route of one worker link.
//! - **No more than a pump.** A frame that the driver received and has not yet confirmed by a later call needs one more
//!   pump. The control then signals the host's wake, as the tap's `edges_quiet` does when it holds bytes, so that the
//!   waiting step pumps at once. That is the only effect of the control.
//! - The record stops on a link at a frame that the driver closes the link for: a first frame that is not a hello, or a later
//!   one that is not a worker message. A frame above Core's own bound never completes here: the driver closes the link at its
//!   header, and `link_close` drops the link's record.

use botster_core_contract::prelude::{InstanceId, RouteId};
use botster_core_host::LinkId;
use botster_core_link::frame::{FrameDecoder, FrameType};
use botster_core_link::hello::Hello;
use botster_core_link::msg::{Observation, WorkerMsg};
use std::collections::BTreeMap;

/// The payload bound of the record's decoder: the largest length that a frame header can hold. Core's own bound of a link is
/// lower, so the record refuses no frame that Core reads.
const RECEIVED_BOUND: u32 = u32::MAX;

/// A worker report of a route, in a frame that the driver received.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Report {
    /// `Observed{ClientInput{route}}`.
    Activity(RouteId),
    /// `RouteClosed{route}`.
    Closed(RouteId),
}

/// The answer of `route_reports` for one route.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Seen {
    pub activity_before_close: bool,
    pub closed: bool,
}

/// The frames of one link that the driver received.
#[derive(Debug)]
struct Received {
    /// `None` once the record stops on the link.
    decoder: Option<FrameDecoder>,
    /// The instance that the link's hello names.
    instance: Option<InstanceId>,
    /// The reports in complete frames that the driver received and has not yet confirmed by a later call.
    given: Vec<Report>,
}

impl Received {
    fn new() -> Received {
        Received {
            decoder: Some(FrameDecoder::new(RECEIVED_BOUND)),
            instance: None,
            given: Vec::new(),
        }
    }

    /// Reads one frame. False when the driver closes the link for it.
    fn read(&mut self, kind: FrameType, payload: &[u8]) -> bool {
        if self.instance.is_none() {
            self.instance = (kind == FrameType::HELLO)
                .then(|| Hello::decode(payload).ok().map(|h| h.instance))
                .flatten();
            return self.instance.is_some();
        }
        if kind != FrameType::WORKER_MSG {
            return false;
        }
        match WorkerMsg::decode(payload) {
            Ok(WorkerMsg::Observed {
                observation: Observation::ClientInput { route, .. },
            }) => self.given.push(Report::Activity(route)),
            Ok(WorkerMsg::RouteClosed { route, .. }) => self.given.push(Report::Closed(route)),
            Ok(_) => {}
            Err(_) => return false,
        }
        true
    }
}

/// The record of one host: the frames of each link that the driver received, and the answer of each route.
#[derive(Debug, Default)]
pub struct RouteReports {
    links: BTreeMap<LinkId, Received>,
    seen: BTreeMap<(InstanceId, RouteId), Seen>,
}

impl RouteReports {
    /// The driver calls `link_recv` on `link`: every report that it received on the link before is consumed, in order.
    pub fn call(&mut self, link: LinkId) {
        let Some(received) = self.links.get_mut(&link) else {
            return;
        };
        let Some(instance) = &received.instance else {
            return;
        };
        for report in received.given.drain(..) {
            let (route, close) = match report {
                Report::Activity(route) => (route, false),
                Report::Closed(route) => (route, true),
            };
            let seen = self.seen.entry((instance.clone(), route)).or_default();
            if close {
                seen.closed = true;
            } else if !seen.closed {
                seen.activity_before_close = true;
            }
        }
    }

    /// The driver received `bytes` on `link` from that call.
    pub fn received(&mut self, link: LinkId, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        let received = self.links.entry(link).or_insert_with(Received::new);
        let mut rest = bytes;
        while let Some(decoder) = received.decoder.as_mut() {
            // The decoder takes bytes up to the end of the current frame.
            let used = decoder.push(rest);
            rest = &rest[used..];
            // The bound is the largest length that a header holds, so the decoder refuses no frame.
            let Ok(next) = decoder.next_frame() else {
                unreachable!("a frame decoder of bound u32::MAX refuses no frame");
            };
            let Some(frame) = next else {
                return;
            };
            if !received.read(frame.kind, &frame.payload) {
                received.decoder = None;
            }
        }
        // The driver closes the link at the frame that stopped the record, so no later call consumes what is left.
        received.given.clear();
    }

    /// The driver closed `link`: no later call consumes the reports that it received on the link.
    pub fn closed(&mut self, link: LinkId) {
        self.links.remove(&link);
    }

    /// The answer for `route` of the worker of `instance`.
    pub fn seen(&self, instance: &InstanceId, route: RouteId) -> Seen {
        self.seen
            .get(&(instance.clone(), route))
            .copied()
            .unwrap_or_default()
    }

    /// Whether a report waits for a later call of the driver: the next pump consumes it.
    pub fn waits(&self) -> bool {
        self.links.values().any(|r| !r.given.is_empty())
    }
}

#[cfg(test)]
mod tests;
