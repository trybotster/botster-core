//! Parent-side ingress (plan section 5.1): the child's host calls and log
//! lines, held for the Hub, and the credit accounts that bound them.
//!
//! Every child frame here is credit-bounded, so the reader never blocks on
//! the Hub and this queue never grows past the credits it granted:
//! - `Call` frames are bounded by the ingress bytes, and each holds one unit
//!   of the attached delivery pool;
//! - `Reply` frames are bounded by the reply credits; a reply holds its
//!   credit until the Hub releases it and the returning credit leaves;
//! - `Log` frames are bounded by the log count and bytes.
//!
//! A frame's cost is its whole encoded length (type byte plus payload), which
//! both sides know exactly. A frame that exceeds its credit is a protocol
//! violation: the child's credit-checked sender never sends one.
//!
//! The account mirrors the child's, not the Hub's: spent credit stays spent
//! until the writer takes the `Credit` frame that returns it. A drain, a
//! released reply, or a returned pool unit only queues that frame. So a child
//! that ignores its credits cannot spend a return that is still queued, and
//! the queued returns can never exceed the grants. Credit is restored when
//! the writer takes the frame, not after it is written: a compliant child may
//! read and spend it before the parent's write call returns.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use crate::engine::{CallId, DeliveryPool, PoolOverdraw};
use crate::session::RequestId;

use super::protocol::{
    CreditFrame, CreditGrants, HostCallFrame, HostCallKindFrame, LogFrame, PluginMessageBody,
};

/// Callback run outside every lock after the child's host call or log line
/// enters the ingress queue. It must return promptly; the Hub drains with
/// [`super::PluginProcess::drain_ingress`].
pub type PluginIngressNotifier = Arc<dyn Fn() + Send + Sync + 'static>;

/// The kind of one host call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginHostCallKind {
    /// A call that the Hub answers with one result invocation, admitted
    /// through the delivery pool (`DeliveryPool::admit_result`) or released
    /// (`DeliveryPool::release_call`). It holds one delivery unit until then.
    Call {
        /// Largest result request the call accepts; its unit holds this many
        /// request bytes.
        max_result_bytes: usize,
    },
    /// A chain's final result. It holds one reply credit until the Hub calls
    /// [`super::PluginProcess::release_reply`].
    Reply,
}

/// One host call from the child.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginHostCall {
    /// Call or reply.
    pub kind: PluginHostCallKind,
    /// Unique among this process's live calls and replies.
    pub call_id: CallId,
    /// The invocation that was running in the child when it made the call.
    pub invocation_request_id: RequestId,
    /// Hub-defined call body.
    pub body: PluginMessageBody,
    /// Encoded frame length, as charged to the child's credits.
    pub frame_bytes: usize,
}

/// One log line from the child.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginLog {
    /// Lines the child dropped for lack of log credit since its previous
    /// sent line.
    pub dropped_since_last: u64,
    /// Hub-defined log body.
    pub body: PluginMessageBody,
    /// Encoded frame length, as charged to the child's credits.
    pub frame_bytes: usize,
}

/// One ingress item, in the order the child sent it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginIngress {
    /// A host call or reply.
    HostCall(PluginHostCall),
    /// A log line.
    Log(PluginLog),
}

impl PluginIngress {
    /// Encoded frame length of the item.
    #[must_use]
    pub fn frame_bytes(&self) -> usize {
        match self {
            Self::HostCall(call) => call.frame_bytes,
            Self::Log(log) => log.frame_bytes,
        }
    }
}

/// Credit that a drain returns to the child.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct Returned {
    pub ingress_bytes: usize,
    pub log_count: usize,
    pub log_bytes: usize,
}

/// What the reader does with an accepted `Call`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CallAdmission {
    /// Queued for the Hub.
    Queued,
    /// The pool's generation retired; the call is dropped. The process is
    /// being stopped, so no credit returns.
    Dropped,
}

pub(super) struct Ingress {
    state: Mutex<IngressState>,
    notifier: Mutex<Option<PluginIngressNotifier>>,
}

struct IngressState {
    queue: VecDeque<PluginIngress>,
    grants: CreditGrants,
    /// Frame bytes of `Call` frames whose ingress credit has not left in a
    /// taken `Credit` frame (queued, drained, or returning).
    call_bytes: usize,
    /// `Log` frames, and their bytes, whose credit has not left.
    log_count: usize,
    log_bytes: usize,
    /// Ids the child spent whose credit has not left: one namespace for
    /// calls and replies. At most the granted units plus the reply credits.
    open: HashMap<u64, Open>,
    /// Open replies in `open`.
    open_replies: usize,
    delivery: Option<Delivery>,
}

enum Open {
    /// Holds one delivery unit and its declared result bytes.
    Call { declared: usize },
    /// Holds one reply credit. `released` once the Hub released it; its
    /// credit then waits for the writer.
    Reply { released: bool },
}

/// The delivery grant and what the child holds of it.
struct Delivery {
    pool: DeliveryPool,
    slots: usize,
    request_bytes: usize,
    used_slots: usize,
    used_bytes: usize,
}

impl Ingress {
    pub(super) fn new(grants: CreditGrants) -> Self {
        Self {
            state: Mutex::new(IngressState {
                queue: VecDeque::new(),
                grants,
                call_bytes: 0,
                log_count: 0,
                log_bytes: 0,
                open: HashMap::new(),
                open_replies: 0,
                delivery: None,
            }),
            notifier: Mutex::new(None),
        }
    }

    /// Lock order: `inbound`, then `ingress`, then the pool's own lock.
    fn lock(&self) -> MutexGuard<'_, IngressState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Attach the generation's delivery pool. Returns the units and request
    /// bytes to grant to the child: what the pool has free now.
    pub(super) fn attach(&self, pool: DeliveryPool) -> Result<(usize, usize), String> {
        let mut state = self.lock();
        if state.delivery.is_some() {
            return Err("a delivery pool is already attached".to_string());
        }
        let (slots, request_bytes) = pool.free();
        state.delivery = Some(Delivery {
            pool,
            slots,
            request_bytes,
            used_slots: 0,
            used_bytes: 0,
        });
        Ok((slots, request_bytes))
    }

    /// Validate and queue one `HostCall` frame of `frame_bytes`. `Err` is a
    /// protocol violation.
    pub(super) fn host_call(
        &self,
        frame: HostCallFrame,
        frame_bytes: usize,
    ) -> Result<CallAdmission, String> {
        let mut guard = self.lock();
        let state = &mut *guard;
        if state.open.contains_key(&frame.call_id) {
            return Err(format!("call id {} is already open", frame.call_id));
        }
        let kind = match frame.kind {
            HostCallKindFrame::Call { max_result_bytes } => {
                let Some(delivery) = &mut state.delivery else {
                    return Err("a Call before any delivery credit was granted".to_string());
                };
                if state
                    .call_bytes
                    .checked_add(frame_bytes)
                    .is_none_or(|used| used > state.grants.ingress_bytes)
                {
                    return Err(format!(
                        "a Call of {frame_bytes} bytes exceeds the ingress credit"
                    ));
                }
                if delivery.used_slots >= delivery.slots {
                    return Err(format!(
                        "Call {}: every delivery unit is in use",
                        frame.call_id
                    ));
                }
                if delivery
                    .used_bytes
                    .checked_add(max_result_bytes)
                    .is_none_or(|used| used > delivery.request_bytes)
                {
                    return Err(format!(
                        "Call {}: the declared result size exceeds the free request bytes",
                        frame.call_id
                    ));
                }
                match delivery
                    .pool
                    .accept_call(CallId(frame.call_id), max_result_bytes)
                {
                    Ok(()) => {}
                    Err(PoolOverdraw::Closed) => return Ok(CallAdmission::Dropped),
                    Err(overdraw) => {
                        return Err(format!("Call {}: {overdraw}", frame.call_id));
                    }
                }
                delivery.used_slots += 1;
                delivery.used_bytes += max_result_bytes;
                state.call_bytes += frame_bytes;
                state.open.insert(
                    frame.call_id,
                    Open::Call {
                        declared: max_result_bytes,
                    },
                );
                PluginHostCallKind::Call { max_result_bytes }
            }
            HostCallKindFrame::Reply => {
                if frame_bytes > state.grants.reply_bytes {
                    return Err(format!(
                        "a Reply of {frame_bytes} bytes exceeds the reply allowance"
                    ));
                }
                if state.open_replies >= state.grants.reply_count {
                    return Err("a Reply without reply credit".to_string());
                }
                state.open_replies += 1;
                state
                    .open
                    .insert(frame.call_id, Open::Reply { released: false });
                PluginHostCallKind::Reply
            }
        };
        state
            .queue
            .push_back(PluginIngress::HostCall(PluginHostCall {
                kind,
                call_id: CallId(frame.call_id),
                invocation_request_id: frame.invocation_request_id,
                body: frame.body,
                frame_bytes,
            }));
        Ok(CallAdmission::Queued)
    }

    /// Validate and queue one `Log` frame of `frame_bytes`. `Err` is a
    /// protocol violation.
    pub(super) fn log(&self, frame: LogFrame, frame_bytes: usize) -> Result<(), String> {
        let mut state = self.lock();
        let fits = state.log_count < state.grants.log_count
            && state
                .log_bytes
                .checked_add(frame_bytes)
                .is_some_and(|used| used <= state.grants.log_bytes);
        if !fits {
            return Err(format!("a Log of {frame_bytes} bytes without log credit"));
        }
        state.log_count += 1;
        state.log_bytes += frame_bytes;
        state.queue.push_back(PluginIngress::Log(PluginLog {
            dropped_since_last: frame.dropped_since_last,
            body: frame.body,
            frame_bytes,
        }));
        Ok(())
    }

    /// Take at most `max_items` items, in order, whose frame bytes sum to at
    /// most `max_bytes`. The first item that does not fit stays, and so do
    /// the items behind it. Returns the `Call` and `Log` credit to send back;
    /// the account keeps it spent until the writer takes that `Credit`. A
    /// drained reply keeps its credit until it is released.
    pub(super) fn drain(
        &self,
        max_items: usize,
        max_bytes: usize,
    ) -> (Vec<PluginIngress>, Returned) {
        let mut state = self.lock();
        let mut items = Vec::new();
        let mut bytes = 0usize;
        let mut returned = Returned::default();
        while items.len() < max_items {
            let Some(next) = state.queue.front() else {
                break;
            };
            let size = next.frame_bytes();
            if bytes
                .checked_add(size)
                .is_none_or(|total| total > max_bytes)
            {
                break;
            }
            bytes += size;
            let Some(item) = state.queue.pop_front() else {
                break;
            };
            match &item {
                PluginIngress::HostCall(PluginHostCall {
                    kind: PluginHostCallKind::Call { .. },
                    ..
                }) => returned.ingress_bytes += size,
                PluginIngress::HostCall(_) => {}
                PluginIngress::Log(_) => {
                    returned.log_count += 1;
                    returned.log_bytes += size;
                }
            }
            items.push(item);
        }
        (items, returned)
    }

    /// Release a reply. Returns true once per open reply; its credit stays
    /// spent until the writer takes the returning `Credit`.
    pub(super) fn release_reply(&self, call_id: CallId) -> bool {
        match self.lock().open.get_mut(&call_id.0) {
            Some(Open::Reply { released }) if !*released => {
                *released = true;
                true
            }
            _ => false,
        }
    }

    /// The writer took `credit` to send it: the child may spend it from now
    /// on, so restore it in the account.
    pub(super) fn credit_taken(&self, credit: &CreditFrame) {
        let mut guard = self.lock();
        let state = &mut *guard;
        match *credit {
            CreditFrame::DeliveryPool { .. } => {}
            CreditFrame::Delivery { call_id } => {
                if let (Some(Open::Call { declared }), Some(delivery)) =
                    (state.open.remove(&call_id), &mut state.delivery)
                {
                    delivery.used_slots -= 1;
                    delivery.used_bytes -= declared;
                }
            }
            CreditFrame::Reply { call_id } => {
                if let Some(Open::Reply { .. }) = state.open.remove(&call_id) {
                    state.open_replies -= 1;
                }
            }
            CreditFrame::IngressBytes { bytes } => {
                state.call_bytes = state.call_bytes.saturating_sub(bytes);
            }
            CreditFrame::Log { count, bytes } => {
                state.log_count = state.log_count.saturating_sub(count);
                state.log_bytes = state.log_bytes.saturating_sub(bytes);
            }
        }
    }

    pub(super) fn install_notifier(&self, notifier: PluginIngressNotifier) {
        *self.notifier.lock().unwrap_or_else(PoisonError::into_inner) = Some(notifier);
    }

    /// Run the notifier. The caller holds no lock.
    pub(super) fn notify(&self) {
        let notifier = self
            .notifier
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        if let Some(notifier) = notifier {
            notifier();
        }
    }

    /// Spent ingress bytes, log count and bytes, and open replies.
    #[cfg(test)]
    pub(super) fn held(&self) -> (usize, usize, usize, usize) {
        let state = self.lock();
        (
            state.call_bytes,
            state.log_count,
            state.log_bytes,
            state.open_replies,
        )
    }
}

#[cfg(test)]
#[path = "ingress_test.rs"]
mod ingress_test;
