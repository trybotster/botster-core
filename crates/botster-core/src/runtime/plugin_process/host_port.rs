//! Child side: the plugin's credit-checked sender for host calls, replies,
//! and log lines (plan section 5.1).
//!
//! The child debits credit before it sends, so the parent never receives a
//! frame that exceeds its credit. A lack of credit is a local, typed refusal:
//! - a `Call` without delivery or ingress credit is `Backpressured` at once;
//!   the handler must not suspend on it;
//! - a `Reply` waits for reply credit, and the invocation's cancellation (its
//!   deadline or a `Cancel`) ends the wait, so a chain's final result is
//!   either sent or its invocation ends with a typed failure;
//! - a log line without credit is dropped and counted, and never blocks.
//!
//! Every host call names the invocation that is running now. The executor
//! clears that invocation under the sender's lock when it queues the result,
//! and one FIFO carries both, so a host call can never follow its
//! invocation's result. No port call writes to the socket: a writer thread
//! does, so a slow parent never blocks the plugin's thread.

use std::collections::{HashMap, HashSet, VecDeque};
use std::os::fd::AsFd;
use std::os::unix::net::UnixStream;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};

use crate::engine::CallId;
use crate::runtime::{CancelTarget, PluginCancellationToken};
use crate::session::RequestId;

use super::protocol::{
    encode_json_bounded, send_all, CreditFrame, CreditGrants, HostCallFrame, HostCallKindFrame,
    LogFrame, PluginMessageBody, FRAME_HOST_CALL, FRAME_LOG,
};

/// Exit code for a broken channel or protocol.
pub(super) const EXIT_PROTOCOL: i32 = 2;

/// Why the port did not send a host call or reply. Nothing was debited.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum HostPortRefusal {
    /// The credit that the call needs is in use now; it returns when the Hub
    /// drains or answers earlier calls.
    Backpressured(String),
    /// No invocation is running in this worker (for example during load).
    NotInInvocation,
    /// The call can never be sent: it exceeds the frame bound or the reply
    /// allowance.
    TooLarge(String),
    /// The running invocation was cancelled while a reply waited for credit.
    Cancelled,
}

impl std::fmt::Display for HostPortRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Backpressured(reason) => write!(f, "backpressured: {reason}"),
            Self::NotInInvocation => f.write_str("no invocation is running"),
            Self::TooLarge(reason) => write!(f, "too large: {reason}"),
            Self::Cancelled => f.write_str("the invocation was cancelled"),
        }
    }
}

impl std::error::Error for HostPortRefusal {}

/// The child's sending half, shared by the executor, the reader, and the
/// host port. Nothing that the plugin calls writes to the socket: frames
/// enter one FIFO, and a writer thread sends them (plan section 5.2). The
/// FIFO is bounded without a new number: a host call or log line enters it
/// only after its credit is debited, and a result only for an invocation in
/// flight, of which the parent sends at most `max_results`.
pub(super) struct Sender {
    ipc: Arc<UnixStream>,
    wire: Mutex<Wire>,
    ready: Condvar,
    max_frame_bytes: usize,
    max_results: usize,
    /// What the writer does when it ends: exit the worker with the code.
    end: fn(i32),
    /// Test seam: after the next write, report and wait for a release.
    #[cfg(test)]
    after_send: Mutex<Option<(std::sync::mpsc::Sender<()>, std::sync::mpsc::Receiver<()>)>>,
}

struct Wire {
    queue: VecDeque<Outgoing>,
    /// Results queued and not yet taken by the writer.
    results: usize,
    /// The parent accepts host calls and log lines only after `Loaded`.
    serving: bool,
    /// The invocation that the executor is running now.
    current: Option<RequestId>,
    /// `Shutdown` arrived: the writer exits once the FIFO is empty.
    shutdown: bool,
}

struct Outgoing {
    frame: Vec<u8>,
    result: bool,
}

impl Sender {
    /// Start the writer thread on `ipc`. `max_results` is the parent's
    /// invocation bound.
    pub(super) fn start(
        ipc: Arc<UnixStream>,
        max_frame_bytes: usize,
        max_results: usize,
    ) -> std::io::Result<Arc<Self>> {
        Self::start_with(ipc, max_frame_bytes, max_results, |code| {
            std::process::exit(code)
        })
    }

    /// As [`Self::start`], with `end` run when the writer ends (tests end
    /// only the thread).
    pub(super) fn start_with(
        ipc: Arc<UnixStream>,
        max_frame_bytes: usize,
        max_results: usize,
        end: fn(i32),
    ) -> std::io::Result<Arc<Self>> {
        let sender = Arc::new(Self {
            ipc,
            wire: Mutex::new(Wire {
                queue: VecDeque::new(),
                results: 0,
                serving: false,
                current: None,
                shutdown: false,
            }),
            ready: Condvar::new(),
            max_frame_bytes,
            max_results,
            end,
            #[cfg(test)]
            after_send: Mutex::new(None),
        });
        let writer = sender.clone();
        std::thread::Builder::new()
            .name("plugin-worker-writer".to_string())
            .spawn(move || writer.run())?;
        Ok(sender)
    }

    fn lock(&self) -> MutexGuard<'_, Wire> {
        self.wire.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Send queued frames in order. A failed write ends the worker: the
    /// parent can no longer drive it. After `Shutdown`, the worker exits once
    /// every queued frame is written; the parent's shutdown deadline bounds
    /// that.
    fn run(&self) {
        loop {
            let next = {
                let mut wire = self.lock();
                loop {
                    if let Some(next) = wire.queue.pop_front() {
                        // A taken result may reach the parent, which may then
                        // send the next Invoke, before the write call returns.
                        // So it stops counting here, not after the write
                        // (review H5).
                        if next.result {
                            wire.results -= 1;
                        }
                        break next;
                    }
                    if wire.shutdown {
                        (self.end)(0);
                        return;
                    }
                    wire = self
                        .ready
                        .wait(wire)
                        .unwrap_or_else(PoisonError::into_inner);
                }
            };
            if send_all(self.ipc.as_fd(), &next.frame).is_err() {
                (self.end)(EXIT_PROTOCOL);
                return;
            }
            #[cfg(test)]
            {
                let seam = self.after_send.lock().expect("after-send seam").take();
                if let Some((reached, release)) = seam {
                    let _ = reached.send(());
                    let _ = release.recv();
                }
            }
        }
    }

    /// Queue `frame` under the caller's lock. Every queued result belongs to
    /// an invocation that the parent still holds in flight (it has not
    /// received the result), so more queued results than the parent's bound
    /// means the parent broke the protocol.
    fn enqueue(&self, wire: &mut Wire, frame: Vec<u8>, result: bool) {
        if result {
            if wire.results >= self.max_results {
                eprintln!("botster plugin worker: more results than invocations in flight");
                (self.end)(EXIT_PROTOCOL);
                return;
            }
            wire.results += 1;
        }
        wire.queue.push_back(Outgoing { frame, result });
        self.ready.notify_one();
    }

    /// Queue one invocation result that is not the running invocation's (a
    /// queued invocation answered at once).
    pub(super) fn send_result<T: serde::Serialize>(&self, frame_type: u8, value: &T) {
        let frame = encode_or_exit(frame_type, value, self.max_frame_bytes);
        let mut wire = self.lock();
        self.enqueue(&mut wire, frame, true);
    }

    /// `Loaded` is sent: host calls and log lines may follow.
    pub(super) fn start_serving(&self) {
        self.lock().serving = true;
    }

    /// `Shutdown` arrived: write what is queued, then exit.
    pub(super) fn shutdown(&self) {
        self.lock().shutdown = true;
        self.ready.notify_one();
    }

    /// The executor starts `request_id`.
    pub(super) fn begin(&self, request_id: RequestId) {
        self.lock().current = Some(request_id);
    }

    /// The executor finished its invocation: clear it and queue the result,
    /// under one lock, behind every host call it made.
    pub(super) fn finish<T: serde::Serialize>(&self, frame_type: u8, result: &T) {
        let frame = encode_or_exit(frame_type, result, self.max_frame_bytes);
        let mut wire = self.lock();
        wire.current = None;
        self.enqueue(&mut wire, frame, true);
    }

    fn current(&self) -> Option<RequestId> {
        self.lock().current.clone()
    }

    /// Queue `frame` if `request_id` is still running. Returns false
    /// otherwise. Never blocks on the socket.
    fn send_for(&self, request_id: &RequestId, frame: Vec<u8>) -> bool {
        let mut wire = self.lock();
        if wire.current.as_ref() != Some(request_id) {
            return false;
        }
        self.enqueue(&mut wire, frame, false);
        true
    }

    /// Queue `frame` if the worker is serving. Returns false otherwise.
    /// Never blocks on the socket.
    fn send_if_serving(&self, frame: Vec<u8>) -> bool {
        let mut wire = self.lock();
        if !wire.serving {
            return false;
        }
        self.enqueue(&mut wire, frame, false);
        true
    }
}

fn encode_or_exit<T: serde::Serialize>(frame_type: u8, value: &T, max: usize) -> Vec<u8> {
    match encode_json_bounded(frame_type, value, max) {
        Ok(frame) => frame,
        Err(error) => {
            eprintln!("botster plugin worker: cannot encode frame: {error}");
            std::process::exit(EXIT_PROTOCOL);
        }
    }
}

/// The encoded length of a frame (type byte plus payload), which is what
/// its credit is charged.
fn charged(frame: &[u8]) -> usize {
    frame.len() - 4
}

/// The plugin's credit-checked sender. The Hub's plugin API calls it from
/// the worker's executor thread, inside a running invocation.
#[derive(Clone)]
pub struct HostPort {
    inner: Arc<PortInner>,
}

impl std::fmt::Debug for HostPort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HostPort").finish_non_exhaustive()
    }
}

struct PortInner {
    sender: Arc<Sender>,
    credits: Mutex<Credits>,
    /// Signalled when reply credit returns or a waiting reply is cancelled.
    changed: Condvar,
    /// Test seam: reports each reply that is about to wait for credit,
    /// under the credit lock, so a test can act only once it waits.
    #[cfg(test)]
    reply_waiting: Mutex<Option<std::sync::mpsc::Sender<()>>>,
}

struct Credits {
    grants: CreditGrants,
    next_call: u64,
    /// Free delivery units and request bytes; zero until the pool attaches.
    delivery_granted: bool,
    delivery_slots: usize,
    delivery_bytes: usize,
    /// Open calls and their declared result bytes. At most the pool's slots.
    calls: HashMap<u64, usize>,
    ingress_free: usize,
    reply_free: usize,
    /// Open replies. At most `grants.reply_count`.
    replies: HashSet<u64>,
    log_count_free: usize,
    log_bytes_free: usize,
    dropped_logs: u64,
}

impl HostPort {
    pub(super) fn new(sender: Arc<Sender>, grants: CreditGrants) -> Self {
        Self {
            inner: Arc::new(PortInner {
                sender,
                credits: Mutex::new(Credits {
                    grants,
                    next_call: 0,
                    delivery_granted: false,
                    delivery_slots: 0,
                    delivery_bytes: 0,
                    calls: HashMap::new(),
                    ingress_free: grants.ingress_bytes,
                    reply_free: grants.reply_count,
                    replies: HashSet::new(),
                    log_count_free: grants.log_count,
                    log_bytes_free: grants.log_bytes,
                    dropped_logs: 0,
                }),
                changed: Condvar::new(),
                #[cfg(test)]
                reply_waiting: Mutex::new(None),
            }),
        }
    }

    #[cfg(test)]
    pub(super) fn report_reply_waiting(&self, waiting: std::sync::mpsc::Sender<()>) {
        *self.inner.reply_waiting.lock().expect("reply waiting seam") = Some(waiting);
    }

    fn lock(&self) -> MutexGuard<'_, Credits> {
        self.inner
            .credits
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    fn next_call(&self) -> u64 {
        let mut credits = self.lock();
        let call = credits.next_call;
        credits.next_call += 1;
        call
    }

    /// Send a host call that the Hub answers with one result invocation of
    /// at most `max_result_bytes`. The call holds one delivery unit until
    /// that result's completion drains or the Hub releases the call.
    ///
    /// # Errors
    ///
    /// Refuses at once, without sending, when no invocation is running, when
    /// the call exceeds the frame bound, or when delivery or ingress credit
    /// is short.
    pub fn call(
        &self,
        max_result_bytes: usize,
        body: PluginMessageBody,
    ) -> Result<CallId, HostPortRefusal> {
        let Some(request_id) = self.inner.sender.current() else {
            return Err(HostPortRefusal::NotInInvocation);
        };
        let call_id = self.next_call();
        let frame = self.encode_call(
            HostCallKindFrame::Call { max_result_bytes },
            call_id,
            &request_id,
            body,
        )?;
        let cost = charged(&frame);
        {
            let mut credits = self.lock();
            if credits.delivery_slots == 0 {
                return Err(HostPortRefusal::Backpressured(
                    "every delivery unit is in use".to_string(),
                ));
            }
            if credits.delivery_bytes < max_result_bytes {
                return Err(HostPortRefusal::Backpressured(
                    "the declared result size exceeds the free delivery bytes".to_string(),
                ));
            }
            if credits.ingress_free < cost {
                return Err(HostPortRefusal::Backpressured(
                    "the call exceeds the free ingress bytes".to_string(),
                ));
            }
            credits.delivery_slots -= 1;
            credits.delivery_bytes -= max_result_bytes;
            credits.ingress_free -= cost;
            credits.calls.insert(call_id, max_result_bytes);
        }
        if self.inner.sender.send_for(&request_id, frame) {
            return Ok(CallId(call_id));
        }
        let mut credits = self.lock();
        credits.calls.remove(&call_id);
        credits.delivery_slots += 1;
        credits.delivery_bytes += max_result_bytes;
        credits.ingress_free += cost;
        Err(HostPortRefusal::NotInInvocation)
    }

    /// Send a chain's final result. It holds one reply credit until the Hub
    /// releases it. When no reply credit is free, this waits for one; the
    /// running invocation's `cancellation` ends the wait.
    ///
    /// # Errors
    ///
    /// Refuses without sending when no invocation is running, when the reply
    /// exceeds the reply allowance, or when `cancellation` fires first.
    pub fn reply(
        &self,
        body: PluginMessageBody,
        cancellation: &PluginCancellationToken,
    ) -> Result<CallId, HostPortRefusal> {
        self.send_reply(body, Some(cancellation))
    }

    /// Send a chain's final result without waiting.
    ///
    /// # Errors
    ///
    /// As [`Self::reply`], and `Backpressured` when no reply credit is free.
    pub fn try_reply(&self, body: PluginMessageBody) -> Result<CallId, HostPortRefusal> {
        self.send_reply(body, None)
    }

    fn send_reply(
        &self,
        body: PluginMessageBody,
        cancellation: Option<&PluginCancellationToken>,
    ) -> Result<CallId, HostPortRefusal> {
        let Some(request_id) = self.inner.sender.current() else {
            return Err(HostPortRefusal::NotInInvocation);
        };
        let call_id = self.next_call();
        let frame = self.encode_call(HostCallKindFrame::Reply, call_id, &request_id, body)?;
        let cost = charged(&frame);
        {
            let allowance = self.lock().grants.reply_bytes;
            if cost > allowance {
                return Err(HostPortRefusal::TooLarge(format!(
                    "a reply of {cost} bytes exceeds the {allowance}-byte allowance"
                )));
            }
        }
        // Subscribe before the first check, so a cancel cannot fall between
        // the check and the wait.
        let _subscription = cancellation.map(|token| {
            token.subscribe(Arc::new(ReplyWake {
                inner: self.inner.clone(),
            }))
        });
        {
            let mut credits = self.lock();
            loop {
                if credits.reply_free > 0 {
                    break;
                }
                match cancellation {
                    None => {
                        return Err(HostPortRefusal::Backpressured(
                            "every reply credit is in use".to_string(),
                        ))
                    }
                    Some(token) if token.is_cancelled() => {
                        return Err(HostPortRefusal::Cancelled);
                    }
                    // No timer: returned reply credit or the cancel wakes it.
                    Some(_) => {
                        #[cfg(test)]
                        if let Some(waiting) = &*self.inner.reply_waiting.lock().expect("seam") {
                            let _ = waiting.send(());
                        }
                        credits = self
                            .inner
                            .changed
                            .wait(credits)
                            .unwrap_or_else(PoisonError::into_inner);
                    }
                }
            }
            credits.reply_free -= 1;
            credits.replies.insert(call_id);
        }
        if self.inner.sender.send_for(&request_id, frame) {
            return Ok(CallId(call_id));
        }
        let mut credits = self.lock();
        credits.replies.remove(&call_id);
        credits.reply_free += 1;
        drop(credits);
        self.inner.changed.notify_all();
        Err(HostPortRefusal::NotInInvocation)
    }

    fn encode_call(
        &self,
        kind: HostCallKindFrame,
        call_id: u64,
        request_id: &RequestId,
        body: PluginMessageBody,
    ) -> Result<Vec<u8>, HostPortRefusal> {
        encode_json_bounded(
            FRAME_HOST_CALL,
            &HostCallFrame {
                kind,
                call_id,
                invocation_request_id: request_id.clone(),
                body,
            },
            self.inner.sender.max_frame_bytes,
        )
        .map_err(|error| HostPortRefusal::TooLarge(error.to_string()))
    }

    /// Send one log line. Without log credit, or before the worker serves,
    /// the line is dropped and counted; the count travels with the next sent
    /// line. Never blocks on credit. Returns whether the line was sent.
    pub fn log(&self, body: PluginMessageBody) -> bool {
        // The dropped count is read and cleared under the same lock that
        // debits the credit, so each dropped line is reported exactly once.
        let (frame, dropped, cost) = {
            let mut credits = self.lock();
            let dropped = credits.dropped_logs;
            let frame = encode_json_bounded(
                FRAME_LOG,
                &LogFrame {
                    dropped_since_last: dropped,
                    body,
                },
                self.inner.sender.max_frame_bytes,
            );
            let Ok(frame) = frame else {
                credits.dropped_logs += 1;
                return false;
            };
            let cost = charged(&frame);
            if credits.log_count_free == 0 || credits.log_bytes_free < cost {
                credits.dropped_logs += 1;
                return false;
            }
            credits.log_count_free -= 1;
            credits.log_bytes_free -= cost;
            credits.dropped_logs = 0;
            (frame, dropped, cost)
        };
        if self.inner.sender.send_if_serving(frame) {
            return true;
        }
        let mut credits = self.lock();
        credits.log_count_free += 1;
        credits.log_bytes_free += cost;
        credits.dropped_logs += dropped + 1;
        false
    }

    /// Apply credit from the parent. `Err` means the parent returned credit
    /// that the child never debited, which breaks the protocol.
    pub(super) fn credit(&self, credit: CreditFrame) -> Result<(), String> {
        let mut credits = self.lock();
        match credit {
            CreditFrame::DeliveryPool {
                slots,
                request_bytes,
            } => {
                if credits.delivery_granted {
                    return Err("a second delivery pool grant".to_string());
                }
                credits.delivery_granted = true;
                credits.delivery_slots = slots;
                credits.delivery_bytes = request_bytes;
            }
            CreditFrame::Delivery { call_id } => {
                let Some(declared) = credits.calls.remove(&call_id) else {
                    return Err(format!(
                        "delivery credit for call {call_id}, which is not open"
                    ));
                };
                credits.delivery_slots += 1;
                credits.delivery_bytes += declared;
            }
            CreditFrame::Reply { call_id } => {
                if !credits.replies.remove(&call_id) {
                    return Err(format!(
                        "reply credit for call {call_id}, which is not open"
                    ));
                }
                credits.reply_free += 1;
                drop(credits);
                self.inner.changed.notify_all();
            }
            CreditFrame::IngressBytes { bytes } => {
                let free = credits.ingress_free.checked_add(bytes);
                match free {
                    Some(free) if free <= credits.grants.ingress_bytes => {
                        credits.ingress_free = free;
                    }
                    _ => return Err(format!("{bytes} ingress bytes that were never debited")),
                }
            }
            CreditFrame::Log { count, bytes } => {
                let count_free = credits.log_count_free.checked_add(count);
                let bytes_free = credits.log_bytes_free.checked_add(bytes);
                match (count_free, bytes_free) {
                    (Some(count_free), Some(bytes_free))
                        if count_free <= credits.grants.log_count
                            && bytes_free <= credits.grants.log_bytes =>
                    {
                        credits.log_count_free = count_free;
                        credits.log_bytes_free = bytes_free;
                    }
                    _ => return Err("log credit that was never debited".to_string()),
                }
            }
        }
        Ok(())
    }
}

/// Wakes a reply that waits for credit when its invocation is cancelled.
/// It takes only the credit lock, a leaf.
struct ReplyWake {
    inner: Arc<PortInner>,
}

impl CancelTarget for ReplyWake {
    fn cancelled(&self) {
        // Take the lock so the wake cannot fall between the waiter's check
        // and its wait.
        drop(
            self.inner
                .credits
                .lock()
                .unwrap_or_else(PoisonError::into_inner),
        );
        self.inner.changed.notify_all();
    }
}

#[cfg(test)]
#[path = "host_port_test.rs"]
mod host_port_test;
