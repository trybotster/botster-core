//! A synchronously refused `attach` returns the caller's transport untouched (Core DP-2, AM-4, OU-1; steward ruling R-19).
//! The transport is a resource that is not `Clone`, has an identity, and reports its own drop: a refusal that dropped or replaced it
//! would show.

use botster_core_contract::prelude::*;
use botster_fake_core::{FakeCore, FakeStore};
use serde_json::json;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

/// The embedder's own stream: an identity, and a flag that Drop sets.
struct Probe {
    id: u64,
    dropped: Arc<AtomicBool>,
}

impl Drop for Probe {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::SeqCst);
    }
}

fn endpoint(id: u64) -> (RouteTransport, Arc<AtomicBool>) {
    let dropped = Arc::new(AtomicBool::new(false));
    let probe = Probe {
        id,
        dropped: dropped.clone(),
    };
    (RouteTransport::Stream(StreamEndpoint::new(probe)), dropped)
}

fn open(limits: serde_json::Value) -> FakeCore {
    let config = OpenConfig {
        data_dir: "d".into(),
        worker_path: Some("w".into()),
        limits: serde_json::from_value(limits).unwrap(),
    };
    FakeCore::open(&FakeStore::new(), config, 0).unwrap()
}

fn now() -> Now {
    Now {
        monotonic: Instant::now(),
        unix: 1,
    }
}

fn create(core: &mut FakeCore, start: bool) -> SessionId {
    let request: SpawnRequest = serde_json::from_value(json!({
        "argv": ["botster-conformance-probe", json!({"program": [{"hold": {}}]}).to_string()],
        "env": {}, "cwd": "/", "size": {"rows": 24, "cols": 80}
    }))
    .unwrap();
    let id = SessionId("s1".into());
    core.begin(Op::Create {
        session: id.clone(),
        request,
    })
    .unwrap();
    if start {
        core.begin(Op::Start { id: id.clone() }).unwrap();
    }
    for _ in 0..20 {
        core.pump(now());
        core.poll_events(64);
    }
    id
}

fn options(extra: serde_json::Value) -> AttachOptions {
    let mut base = json!({"file_directory": "/tmp", "answers_queries": false});
    base.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    serde_json::from_value(base).unwrap()
}

/// The refusal carries the same endpoint that went in, and it was not dropped before the caller received it.
fn assert_returned(
    refused: AttachRefused,
    id: u64,
    dropped: &AtomicBool,
    code: impl Fn(&ErrorCode) -> bool,
) {
    assert!(
        code(&refused.error.code),
        "refusal code: {:?}",
        refused.error
    );
    assert!(
        !dropped.load(Ordering::SeqCst),
        "the transport was dropped before the caller got it"
    );
    let RouteTransport::Stream(endpoint) = refused.transport else {
        panic!("the transport changed kind");
    };
    let probe = endpoint.downcast::<Probe>().expect("the same stream type");
    assert_eq!(probe.id, id, "the transport is the one that was passed in");
    assert!(!dropped.load(Ordering::SeqCst));
    drop(probe);
    assert!(
        dropped.load(Ordering::SeqCst),
        "the caller now owns it and can close it"
    );
}

#[test]
fn unknown_session_returns_the_transport() {
    let mut core = open(json!({}));
    let (t, dropped) = endpoint(11);
    let refused = core
        .attach(
            ClientId("c".into()),
            SessionId("nope".into()),
            t,
            options(json!({})),
        )
        .unwrap_err();
    assert_returned(refused, 11, &dropped, |c| {
        matches!(c, ErrorCode::UnknownSession)
    });
}

#[test]
fn a_state_refusal_returns_the_transport() {
    // Core A2-1: a Created session does not admit attach.
    let mut core = open(json!({}));
    let s = create(&mut core, false);
    let (t, dropped) = endpoint(12);
    let refused = core
        .attach(ClientId("c".into()), s, t, options(json!({})))
        .unwrap_err();
    assert_returned(refused, 12, &dropped, |c| {
        matches!(c, ErrorCode::WrongState)
    });
}

#[test]
fn invalid_input_returns_the_transport() {
    let mut core = open(json!({}));
    let s = create(&mut core, true);
    // Core EV-8: a route that answers queries needs a positive `query_deadline`.
    let (t, dropped) = endpoint(13);
    let refused = core
        .attach(
            ClientId("c".into()),
            s.clone(),
            t,
            options(json!({"answers_queries": true})),
        )
        .unwrap_err();
    assert_returned(refused, 13, &dropped, |c| {
        matches!(c, ErrorCode::InvalidInput { .. })
    });
    // Core OU-1: a route tag over `max_route_tag_bytes`.
    let (t, dropped) = endpoint(14);
    let long = "x".repeat(10_000);
    let refused = core
        .attach(
            ClientId("c".into()),
            s,
            t,
            options(json!({"route_tag": long})),
        )
        .unwrap_err();
    assert_returned(refused, 14, &dropped, |c| {
        matches!(c, ErrorCode::InvalidInput { .. })
    });
}

#[test]
fn route_limit_returns_the_transport() {
    let mut core = open(json!({"routes_per_session": 1}));
    let s = create(&mut core, true);
    let (first, _) = endpoint(1);
    core.attach(ClientId("c".into()), s.clone(), first, options(json!({})))
        .unwrap();
    let (t, dropped) = endpoint(15);
    let refused = core
        .attach(ClientId("c2".into()), s, t, options(json!({})))
        .unwrap_err();
    assert_returned(refused, 15, &dropped, |c| {
        matches!(c, ErrorCode::RouteLimit)
    });
}
