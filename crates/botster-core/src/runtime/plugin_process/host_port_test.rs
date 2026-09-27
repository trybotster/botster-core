//! The child's credit-checked port: refusals, exact debits and returns,
//! reply waits that end on credit or cancel, and dropped-log counting.

use std::io::Read;
use std::os::unix::net::UnixStream;
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::Duration;

use serde_json::json;

use super::{HostPort, HostPortRefusal, Sender};
use crate::contract::session_protocol::{Frame, FrameDecoder};
use crate::engine::CallId;
use crate::runtime::plugin_process::protocol::{
    decode_json, CreditFrame, CreditGrants, HostCallFrame, LogFrame, PluginMessageBody,
    FRAME_HOST_CALL, FRAME_LOG,
};
use crate::runtime::PluginCancellationToken;
use crate::session::RequestId;

/// Bound for each event these tests wait for; expiry fails the test.
const EVENT_DEADLINE: Duration = Duration::from_secs(10);

struct Harness {
    sender: Arc<Sender>,
    port: HostPort,
    peer: UnixStream,
    decoder: FrameDecoder,
    pending: Vec<Frame>,
}

impl Harness {
    fn new(grants: CreditGrants) -> Self {
        Self::with_writer(grants, 4, |_| {})
    }

    /// `end` runs where the worker would exit; the writer thread then ends.
    fn with_writer(grants: CreditGrants, max_results: usize, end: fn(i32)) -> Self {
        let (child, peer) = UnixStream::pair().expect("socketpair");
        let sender =
            Sender::start_with(Arc::new(child), 1024 * 1024, max_results, end).expect("writer");
        let port = HostPort::new(sender.clone(), grants);
        Self {
            sender,
            port,
            peer,
            decoder: FrameDecoder::with_max_len(1024 * 1024),
            pending: Vec::new(),
        }
    }

    fn serving_in(grants: CreditGrants, request_id: &str) -> Self {
        let harness = Self::new(grants);
        harness.sender.start_serving();
        harness.sender.begin(RequestId(request_id.to_string()));
        harness
    }

    /// The next frame the child sent.
    fn frame(&mut self) -> Frame {
        let mut buf = [0u8; 64 * 1024];
        while self.pending.is_empty() {
            let read = self.peer.read(&mut buf).expect("read the peer");
            assert!(read > 0, "the child end closed");
            self.pending
                .extend(self.decoder.feed(&buf[..read]).expect("valid frames"));
        }
        self.pending.remove(0)
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        // Wake an idle writer so its thread ends with the test.
        self.sender.shutdown();
    }
}

fn grants() -> CreditGrants {
    CreditGrants {
        ingress_bytes: 1000,
        reply_count: 1,
        reply_bytes: 500,
        log_count: 1,
        log_bytes: 500,
        max_in_flight_invokes: 4,
    }
}

fn body(value: serde_json::Value) -> PluginMessageBody {
    serde_json::from_value(value).expect("a message body")
}

fn charged(frame: &Frame) -> usize {
    frame.payload.len() + 1
}

#[test]
fn nothing_is_sent_outside_an_invocation_or_before_serving() {
    let mut harness = Harness::new(grants());
    assert_eq!(
        harness.port.call(1, body(json!(1))),
        Err(HostPortRefusal::NotInInvocation)
    );
    assert_eq!(
        harness.port.try_reply(body(json!(1))),
        Err(HostPortRefusal::NotInInvocation)
    );
    assert!(!harness.port.log(body(json!("before serving"))));

    harness.sender.start_serving();
    assert!(harness.port.log(body(json!("serving"))));
    let frame = harness.frame();
    assert_eq!(frame.frame_type, FRAME_LOG);
    let log: LogFrame = decode_json(&frame).expect("log frame");
    assert_eq!(log.dropped_since_last, 1, "the line dropped before serving");
}

#[test]
fn a_call_debits_its_unit_and_frame_and_each_credit_returns_once() {
    let mut harness = Harness::serving_in(grants(), "r");
    assert!(
        matches!(
            harness.port.call(50, body(json!(1))),
            Err(HostPortRefusal::Backpressured(_))
        ),
        "no delivery credit before the pool is granted"
    );

    harness
        .port
        .credit(CreditFrame::DeliveryPool {
            slots: 1,
            request_bytes: 100,
        })
        .expect("grant");
    assert!(
        harness
            .port
            .credit(CreditFrame::DeliveryPool {
                slots: 1,
                request_bytes: 100,
            })
            .is_err(),
        "one grant per process"
    );
    assert!(
        matches!(
            harness.port.call(101, body(json!(1))),
            Err(HostPortRefusal::Backpressured(_))
        ),
        "over the free request bytes"
    );

    let call = harness.port.call(50, body(json!(1))).expect("sent");
    let frame = harness.frame();
    assert_eq!(frame.frame_type, FRAME_HOST_CALL);
    let sent: HostCallFrame = decode_json(&frame).expect("host call frame");
    assert_eq!(CallId(sent.call_id), call);
    assert_eq!(sent.invocation_request_id, RequestId("r".to_string()));
    assert!(
        matches!(
            harness.port.call(1, body(json!(2))),
            Err(HostPortRefusal::Backpressured(_))
        ),
        "the only unit is in use"
    );

    let cost = charged(&frame);
    harness
        .port
        .credit(CreditFrame::IngressBytes { bytes: cost })
        .expect("return the ingress bytes");
    assert!(
        harness
            .port
            .credit(CreditFrame::IngressBytes { bytes: 1 })
            .is_err(),
        "ingress bytes that were never debited"
    );
    harness
        .port
        .credit(CreditFrame::Delivery { call_id: call.0 })
        .expect("return the unit");
    assert!(
        harness
            .port
            .credit(CreditFrame::Delivery { call_id: call.0 })
            .is_err(),
        "a unit returns once"
    );
    harness
        .port
        .call(100, body(json!(3)))
        .expect("the unit is free again");
}

#[test]
fn a_finished_invocation_accepts_no_more_calls() {
    let harness = Harness::serving_in(grants(), "r");
    harness
        .port
        .credit(CreditFrame::DeliveryPool {
            slots: 4,
            request_bytes: 100,
        })
        .expect("grant");
    harness.sender.finish(0x85, &json!("result"));
    assert_eq!(
        harness.port.call(1, body(json!(1))),
        Err(HostPortRefusal::NotInInvocation)
    );
}

#[test]
fn a_reply_over_its_allowance_is_refused_as_too_large() {
    let harness = Harness::serving_in(grants(), "r");
    assert!(matches!(
        harness.port.try_reply(body(json!("x".repeat(600)))),
        Err(HostPortRefusal::TooLarge(_))
    ));
}

#[test]
fn a_waiting_reply_sends_when_reply_credit_returns() {
    let mut harness = Harness::serving_in(grants(), "r");
    let first = harness.port.try_reply(body(json!(1))).expect("the credit");
    harness.frame();
    assert!(matches!(
        harness.port.try_reply(body(json!(2))),
        Err(HostPortRefusal::Backpressured(_))
    ));

    let (waiting_tx, waiting_rx) = mpsc::channel();
    harness.port.report_reply_waiting(waiting_tx);
    let (tx, rx) = mpsc::channel();
    let port = harness.port.clone();
    thread::spawn(move || {
        let _ = tx.send(port.reply(body(json!(3)), &PluginCancellationToken::new()));
    });
    // timer: deadline — the reply reaches its wait; expiry fails the test
    waiting_rx
        .recv_timeout(EVENT_DEADLINE)
        .expect("the reply waits for credit");
    harness
        .port
        .credit(CreditFrame::Reply { call_id: first.0 })
        .expect("release");
    // timer: deadline — the waiting reply wakes on the returned credit; expiry fails the test
    let sent = rx.recv_timeout(EVENT_DEADLINE).expect("the reply returns");
    assert!(sent.is_ok(), "{sent:?}");
    assert_eq!(harness.frame().frame_type, FRAME_HOST_CALL);
}

#[test]
fn a_cancel_ends_a_waiting_reply_without_taking_credit() {
    let harness = Harness::serving_in(grants(), "r");
    let first = harness.port.try_reply(body(json!(1))).expect("the credit");
    let cancellation = PluginCancellationToken::new();
    let (waiting_tx, waiting_rx) = mpsc::channel();
    harness.port.report_reply_waiting(waiting_tx);
    let (tx, rx) = mpsc::channel();
    let port = harness.port.clone();
    let token = cancellation.clone();
    thread::spawn(move || {
        let _ = tx.send(port.reply(body(json!(2)), &token));
    });
    // Cancel only once the reply waits, so only the cancel's wake can end it.
    // timer: deadline — the reply reaches its wait; expiry fails the test
    waiting_rx
        .recv_timeout(EVENT_DEADLINE)
        .expect("the reply waits for credit");
    cancellation.cancel();
    // timer: deadline — the cancel wakes the waiting reply; expiry fails the test
    let ended = rx.recv_timeout(EVENT_DEADLINE).expect("the reply returns");
    assert_eq!(ended, Err(HostPortRefusal::Cancelled));
    harness
        .port
        .credit(CreditFrame::Reply { call_id: first.0 })
        .expect("release");
    harness
        .port
        .try_reply(body(json!(3)))
        .expect("the cancelled reply took no credit");
}

#[test]
fn dropped_log_lines_are_reported_once_with_the_next_sent_line() {
    let mut harness = Harness::serving_in(grants(), "r");
    assert!(harness.port.log(body(json!("sent"))));
    assert!(!harness.port.log(body(json!("dropped"))));
    assert!(!harness.port.log(body(json!("dropped"))));
    let first = harness.frame();
    harness
        .port
        .credit(CreditFrame::Log {
            count: 1,
            bytes: charged(&first),
        })
        .expect("return the log credit");
    assert!(
        harness
            .port
            .credit(CreditFrame::Log { count: 1, bytes: 1 })
            .is_err(),
        "log credit that was never debited"
    );
    assert!(harness.port.log(body(json!("sent"))));
    let second_frame = harness.frame();
    let second: LogFrame = decode_json(&second_frame).expect("log frame");
    assert_eq!(second.dropped_since_last, 2);

    harness
        .port
        .credit(CreditFrame::Log {
            count: 1,
            bytes: charged(&second_frame),
        })
        .expect("return the log credit");
    assert!(harness.port.log(body(json!("sent"))));
    let third: LogFrame = decode_json(&harness.frame()).expect("log frame");
    assert_eq!(third.dropped_since_last, 0, "each drop is reported once");
}

#[test]
fn port_calls_return_while_the_parent_is_not_reading() {
    // Far more credited bytes than a socket buffer holds.
    let mut harness = Harness::serving_in(
        CreditGrants {
            ingress_bytes: 8 * 1024 * 1024,
            reply_count: 1,
            reply_bytes: 500,
            log_count: 64,
            log_bytes: 8 * 1024 * 1024,
            max_in_flight_invokes: 4,
        },
        "r",
    );
    harness
        .port
        .credit(CreditFrame::DeliveryPool {
            slots: 64,
            request_bytes: 64,
        })
        .expect("grant");
    let port = harness.port.clone();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let line = "x".repeat(64 * 1024);
        let logged = (0..64).filter(|_| port.log(body(json!(line)))).count();
        let called = (0..64)
            .filter(|_| port.call(1, body(json!(line))).is_ok())
            .count();
        let _ = tx.send((logged, called));
    });
    // The peer reads nothing until the plugin's calls have returned.
    // timer: deadline — the port calls return without the peer reading; expiry fails the test
    let sent = rx
        .recv_timeout(EVENT_DEADLINE)
        .expect("no port call blocks on the socket");
    assert_eq!(sent, (64, 64));
    for _ in 0..128 {
        harness.frame();
    }
}

static WRITER_END: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(-1);

fn record_end(code: i32) {
    WRITER_END.store(code, std::sync::atomic::Ordering::SeqCst);
}

/// A result stops counting when the writer takes it, not after its write
/// returns: the parent may receive it and send the next Invoke first.
#[test]
fn a_result_the_parent_received_frees_its_count_before_the_write_returns() {
    let mut harness = Harness::with_writer(grants(), 1, record_end);
    let (reached_tx, reached) = mpsc::channel();
    let (release, release_rx) = mpsc::channel();
    *harness.sender.after_send.lock().expect("seam") = Some((reached_tx, release_rx));

    harness.sender.begin(RequestId("one".to_string()));
    harness.sender.finish(0x85, &json!("one"));
    assert_eq!(
        harness.frame().frame_type,
        0x85,
        "the parent has the result"
    );
    // timer: deadline — the writer reaches the hold after its write; expiry fails the test
    reached
        .recv_timeout(EVENT_DEADLINE)
        .expect("the writer holds after the write");

    // The parent retired the first invocation and sent the next one.
    harness.sender.begin(RequestId("two".to_string()));
    harness.sender.finish(0x85, &json!("two"));
    assert_eq!(
        WRITER_END.load(std::sync::atomic::Ordering::SeqCst),
        -1,
        "a healthy worker was ended"
    );
    let _ = release.send(());
    assert_eq!(harness.frame().frame_type, 0x85);
}
