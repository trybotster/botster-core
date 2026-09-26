//! Record retirement: a slot and an id return only after the outcome is
//! settled and every frame the record queued has been written.

use std::cell::RefCell;

use serde_json::json;

use super::{Admission, Invocation, Invocations};
use crate::actor::{
    PluginHandlerKind, PluginHandlerRef, PluginInvocationContext, PluginInvocationRequest,
    PluginInvocationResult, PluginInvocationSuccess, PluginKey,
};
use crate::runtime::plugin_process::outbound::Lane;
use crate::session::RequestId;

fn request(id: &str) -> PluginInvocationRequest {
    PluginInvocationRequest {
        request_id: RequestId(id.to_string()),
        handler: PluginHandlerRef {
            plugin_key: PluginKey("table".to_string()),
            kind: PluginHandlerKind::Command,
            handler_id: "run".to_string(),
        },
        timeout_ms: 1_000,
        context: PluginInvocationContext {
            client_id: None,
            session_id: None,
            subscription_id: None,
            surface_id: None,
            origin: None,
            metadata: None,
        },
        payload: serde_json::from_value(json!({})).expect("payload"),
    }
}

fn completed(request: &PluginInvocationRequest) -> PluginInvocationResult {
    PluginInvocationResult::Completed(PluginInvocationSuccess {
        request_id: request.request_id.clone(),
        handler: request.handler.clone(),
        payload: None,
    })
}

/// Records the frames the table queues.
#[derive(Default)]
struct Frames(RefCell<Vec<(Lane, RequestId)>>);

impl Frames {
    fn sink(&self) -> impl Fn(Lane, Vec<u8>, RequestId) -> bool + '_ {
        move |lane, _frame, owner| {
            self.0.borrow_mut().push((lane, owner));
            true
        }
    }

    fn take(&self) -> Vec<(Lane, RequestId)> {
        std::mem::take(&mut self.0.borrow_mut())
    }
}

#[test]
fn a_settled_record_keeps_its_slot_until_its_frames_are_written() {
    let table = Invocations::new(1);
    let frames = Frames::default();
    let sink = frames.sink();
    let first_request = request("first");
    let first = Invocation::new(&first_request, vec![1]);
    assert!(matches!(table.admit(&first, &sink), Admission::Admitted));
    let second = Invocation::new(&request("second"), vec![2]);
    assert!(matches!(table.admit(&second, &sink), Admission::Waiting));

    table
        .settle_result(completed(&first_request), &sink)
        .expect("a valid result");
    assert!(
        !second.lock().admitted,
        "the first Invoke is still unwritten"
    );

    table.frame_written(first.request_id(), &sink);
    assert!(second.lock().admitted);
    assert_eq!(
        frames.take(),
        [
            (Lane::Invoke, RequestId("first".to_string())),
            (Lane::Invoke, RequestId("second".to_string())),
        ]
    );
}

#[test]
fn a_reused_id_waits_until_the_earlier_record_retires() {
    let table = Invocations::new(2);
    let frames = Frames::default();
    let sink = frames.sink();
    let old_request = request("same");
    let old = Invocation::new(&old_request, vec![1]);
    assert!(matches!(table.admit(&old, &sink), Admission::Admitted));
    table.frame_written(old.request_id(), &sink);
    // The old invocation is cancelled; its Cancel is queued and unwritten.
    assert!(table.cancel(&old, vec![9], &sink));
    table
        .settle_result(completed(&old_request), &sink)
        .expect("a valid result");

    let new = Invocation::new(&request("same"), vec![2]);
    assert!(
        matches!(table.admit(&new, &sink), Admission::Waiting),
        "a same-id record is still retiring"
    );
    // A stale cancel through the old invocation never reaches the new one.
    assert!(!table.cancel(&old, vec![9], &sink));

    table.frame_written(old.request_id(), &sink);
    assert!(new.lock().admitted);
    assert!(
        !table.cancel(&old, vec![9], &sink),
        "the old record is gone"
    );
    assert!(table.cancel(&new, vec![9], &sink));
}

#[test]
fn a_result_must_match_the_whole_identity_and_come_once() {
    let table = Invocations::new(1);
    let frames = Frames::default();
    let sink = frames.sink();
    let valid = request("id");
    let invocation = Invocation::new(&valid, vec![1]);
    assert!(matches!(
        table.admit(&invocation, &sink),
        Admission::Admitted
    ));

    let mut forged = valid.clone();
    forged.handler.plugin_key = PluginKey("someone-else".to_string());
    assert!(table.settle_result(completed(&forged), &sink).is_err());
    assert!(
        invocation.lock().outcome.is_none(),
        "a forged result settles nothing"
    );

    table
        .settle_result(completed(&valid), &sink)
        .expect("valid");
    assert!(
        table.settle_result(completed(&valid), &sink).is_err(),
        "a second result is a violation"
    );
}
