//! The descriptors of the control link (DP-2; plan section 6.3 item 1, revision 23ae).
//!
//! **The framing rule.** The host sends each `AttachRoute` frame from its first byte with the route's stream, and sends no
//! other descriptor. The kernels place a descriptor segment differently in a read (macOS at its start, Linux at its end),
//! but one read holds at most one segment, and the read that holds the segment's first byte carries its descriptors. So the
//! descriptors of a read belong to the one `AttachRoute` frame whose first byte is in that read, and not to a byte position.
//! The frame can end in a later read: a frame keeps the read where its first byte was until it is complete.
//!
//! **Faults.** The link is broken (a host fault) when a read with descriptors holds the first byte of no `AttachRoute` frame
//! or of more than one, when an `AttachRoute` frame starts in a read with no descriptor, when its read has another count of
//! descriptors than the one stream of the frame, or when descriptors come before the hello passes. The worker closes the
//! link and every descriptor of that read once: [`Action::CloseDescriptor`] for each unbound one, and the route that took
//! one of them ends `HandoffFailed` (its transport closes, and the host is told once). The bind happens in frame order, when
//! each `AttachRoute` frame is complete, so a later frame of the same read already sees the route; a fault found after the
//! bind ends that route.

use super::route::DescriptorId;
use super::{Action, LinkState, Worker};
use botster_core_contract::prelude::{RouteCloseReason, RouteId};
use std::collections::BTreeMap;

/// The link reads that came with descriptors, until every frame whose first byte is in them is complete.
#[derive(Debug, Default)]
pub(super) struct Handoffs {
    /// The number of the current link read, counted from 1.
    read: u64,
    /// The read that holds the first byte of the frame in the decoder, or of the last frame (0 before the first frame).
    frame_start: u64,
    /// The reads with descriptors whose frames are not all checked, by their numbers.
    reads: BTreeMap<u64, DescriptorRead>,
}

/// One read with descriptors.
#[derive(Debug)]
struct DescriptorRead {
    descriptors: Vec<DescriptorId>,
    /// The route of the `AttachRoute` frame whose first byte is in this read, once it took the read's descriptor.
    route: Option<RouteId>,
}

impl Worker {
    /// A link read begins with its descriptors. Returns false when its bytes are not read: the link is not open (the
    /// descriptors close), or descriptors came before the hello passed (a fault).
    pub(super) fn link_read_begins(&mut self, descriptors: Vec<DescriptorId>) -> bool {
        self.handoffs.read += 1;
        if descriptors.is_empty() {
            return true;
        }
        let read = self.handoffs.read;
        self.handoffs.reads.insert(
            read,
            DescriptorRead {
                descriptors,
                route: None,
            },
        );
        match self.link {
            LinkState::Ready => true,
            LinkState::AwaitHello => {
                self.descriptor_fault(read);
                false
            }
            LinkState::Closing | LinkState::Closed => {
                self.close_unbound();
                false
            }
        }
    }

    /// The byte at the current offset of the current read is the first byte of a frame. Every earlier read with descriptors
    /// has no frame left that is not complete, so each is checked.
    pub(super) fn frame_starts(&mut self) {
        let read = self.handoffs.read;
        self.handoffs.frame_start = read;
        self.check_reads(Some(read));
    }

    /// The current read ended. A read whose last frame is not complete stays until that frame is complete; every other read
    /// with descriptors is checked.
    pub(super) fn link_read_ends(&mut self, frame_open: bool) {
        let keep = frame_open.then_some(self.handoffs.frame_start);
        self.check_reads(keep);
    }

    /// The descriptor of the complete `AttachRoute` frame of `route`: the one descriptor of the read that holds the frame's
    /// first byte. `None` is a fault, already handled.
    pub(super) fn attach_descriptor(&mut self, route: RouteId) -> Option<DescriptorId> {
        let start = self.handoffs.frame_start;
        let read = self.handoffs.reads.get_mut(&start);
        match read {
            Some(read) if read.route.is_none() && read.descriptors.len() == 1 => {
                read.route = Some(route);
                Some(read.descriptors[0])
            }
            Some(_) => {
                self.descriptor_fault(start);
                None
            }
            None => {
                // The frame starts in a read with no descriptor.
                self.close_unbound();
                self.close_link();
                None
            }
        }
    }

    /// Checks every read with descriptors except `keep`: each one must have given its descriptor to an `AttachRoute` frame.
    fn check_reads(&mut self, keep: Option<u64>) {
        let done: Vec<u64> = self
            .handoffs
            .reads
            .keys()
            .copied()
            .filter(|&read| Some(read) != keep)
            .collect();
        for read in done {
            if self.handoffs.reads[&read].route.is_some() {
                self.handoffs.reads.remove(&read);
            } else {
                self.descriptor_fault(read);
            }
        }
    }

    /// A host fault in the descriptors of `read`: the link closes, and so does each descriptor of the read, once.
    fn descriptor_fault(&mut self, read: u64) {
        if let Some(fault) = self.handoffs.reads.remove(&read) {
            self.close_read(fault);
        }
        self.close_unbound();
        self.close_link();
    }

    /// Closes each descriptor of a read once: the route that took one ends `HandoffFailed`, which closes its transport.
    fn close_read(&mut self, read: DescriptorRead) {
        match read.route {
            Some(route) => self.end_route(route, RouteCloseReason::HandoffFailed),
            None => {
                for id in read.descriptors {
                    self.actions.push_back(Action::CloseDescriptor(id));
                }
            }
        }
    }

    /// The descriptors of a link that ended, was replaced or broke (DP-8): none of them will be bound. A read whose
    /// descriptor a route took keeps its route.
    pub(super) fn close_unbound(&mut self) {
        let reads = std::mem::take(&mut self.handoffs.reads);
        for read in reads.into_values().filter(|read| read.route.is_none()) {
            self.close_read(read);
        }
        self.handoffs.frame_start = 0;
    }
}
