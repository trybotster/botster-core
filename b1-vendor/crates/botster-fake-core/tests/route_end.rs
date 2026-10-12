//! A route that the worker ends reaches the host once, as `RouteClosed` with the mapped reason (Core OU-2b, A2-3).

use botster_core_contract::prelude::*;
use botster_fake_core::{FakeCore, FakeStore};
use serde_json::json;
use std::time::Instant;

fn now() -> Now {
    Now {
        monotonic: Instant::now(),
        unix: 1,
    }
}

fn pump(core: &mut FakeCore) -> Vec<Event> {
    let mut out = vec![];
    for _ in 0..20 {
        core.pump(now());
        out.extend(core.poll_events(64));
    }
    out
}

#[test]
fn a_client_close_in_the_worker_closes_the_route_as_peer_closed() {
    let config = OpenConfig {
        data_dir: "d".into(),
        worker_path: Some("w".into()),
        limits: serde_json::from_value(json!({})).unwrap(),
    };
    let mut core = FakeCore::open(&FakeStore::new(), config, 0).unwrap();
    let request: SpawnRequest = serde_json::from_value(json!({
        "argv": ["botster-conformance-probe", json!({"program": [{"hold": {}}]}).to_string()],
        "env": {}, "cwd": "/", "size": {"rows": 24, "cols": 80}
    }))
    .unwrap();
    let s = SessionId("s1".into());
    core.begin(Op::Create {
        session: s.clone(),
        request,
    })
    .unwrap();
    core.begin(Op::Start { id: s.clone() }).unwrap();
    pump(&mut core);
    let options: AttachOptions =
        serde_json::from_value(json!({"file_directory": "/tmp", "answers_queries": false}))
            .unwrap();
    let transport = RouteTransport::Stream(StreamEndpoint::new(7u64));
    let route = core
        .attach(ClientId("c".into()), s, transport, options)
        .unwrap()
        .route;
    pump(&mut core);
    core.route_end(route)
        .unwrap()
        .control("client_close", &json!({}))
        .unwrap();
    let closed: Vec<_> = pump(&mut core)
        .into_iter()
        .filter_map(|ev| match ev {
            Event::RouteClosed {
                route: r, reason, ..
            } if r == route => Some(reason),
            _ => None,
        })
        .collect();
    assert_eq!(closed, vec![RouteCloseReason::PeerClosed]);
}
