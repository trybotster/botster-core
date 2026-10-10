//! The fake harness keeps routes apart: two cores in two data directories issue the same route id (Core ID-1), and each client end reaches
//! only its own session. A route end does not hold the data directory of its handle (Core LC-2, DP-8).

use botster_conformance::{Bindings, Deadline, StepDriver};
use botster_core_conformance::fake::FakeCoreHarness;
use botster_core_conformance::CoreDriver;
use serde_json::json;

fn run(d: &mut CoreDriver, b: &mut Bindings, step: serde_json::Value) {
    d.exec(&step, b, &Deadline::none())
        .unwrap_or_else(|e| panic!("{step}: {e:?}"));
}

fn session(d: &mut CoreDriver, b: &mut Bindings, handle: &str, route: &str) {
    run(
        d,
        b,
        json!({"begin": {"handle": handle, "op": {"Create": {"session": "s1", "request": {"program": [{"hold": {}}]}}}}}),
    );
    run(
        d,
        b,
        json!({"begin": {"handle": handle, "op": {"Start": {"id": "s1"}}}}),
    );
    run(
        d,
        b,
        json!({"pump_until": {"handle": handle, "event": {"SessionState": {"id": "s1", "state": "Running"}}}}),
    );
    run(
        d,
        b,
        json!({"attach_route": {"handle": handle, "route": route, "session": "s1"}}),
    );
}

#[test]
fn two_directories_that_issue_the_same_route_id_stay_apart() {
    let mut d = CoreDriver::new(Box::new(FakeCoreHarness::new(0)));
    let mut b = Bindings::new();
    run(
        &mut d,
        &mut b,
        json!({"open": {"as": "h", "data_dir": "d"}}),
    );
    run(
        &mut d,
        &mut b,
        json!({"open": {"as": "h2", "data_dir": "e"}}),
    );
    session(&mut d, &mut b, "h", "r1");
    session(&mut d, &mut b, "h2", "r2");
    assert_eq!(b["r1"], b["r2"], "both handles minted the same route id");
    run(&mut d, &mut b, json!({"pump": {"handle": "h"}}));
    run(&mut d, &mut b, json!({"pump": {"handle": "h2"}}));
    run(&mut d, &mut b, json!({"route_drain": true, "route": "r1"}));
    run(&mut d, &mut b, json!({"route_drain": true, "route": "r2"}));
    run(
        &mut d,
        &mut b,
        json!({"route_send": {"frame": "bytes", "op": "1", "bytes": {"$bytes_hex": "61"}}, "route": "r1"}),
    );
    run(
        &mut d,
        &mut b,
        json!({"route_send": {"frame": "bytes", "op": "1", "bytes": {"$bytes_hex": "62"}}, "route": "r2"}),
    );
    run(
        &mut d,
        &mut b,
        json!({"control": {"op": "pty_input", "handle": "h", "session": "s1"}, "expect": {"bytes": {"$bytes_hex": "61"}}}),
    );
    run(
        &mut d,
        &mut b,
        json!({"control": {"op": "pty_input", "handle": "h2", "session": "s1"}, "expect": {"bytes": {"$bytes_hex": "62"}}}),
    );
}

#[test]
fn a_live_route_does_not_hold_the_data_directory() {
    let mut d = CoreDriver::new(Box::new(FakeCoreHarness::new(0)));
    let mut b = Bindings::new();
    run(
        &mut d,
        &mut b,
        json!({"open": {"as": "h", "data_dir": "d"}}),
    );
    session(&mut d, &mut b, "h", "r1");
    run(&mut d, &mut b, json!({"drop": {"handle": "h"}}));
    run(
        &mut d,
        &mut b,
        json!({"open": {"as": "h2", "data_dir": "d"}}),
    );
    // The route still reaches the worker after its host dropped (Core DP-8).
    run(
        &mut d,
        &mut b,
        json!({"route_send": {"frame": "bytes", "op": "1", "bytes": {"$bytes_hex": "61"}}, "route": "r1"}),
    );
}

#[test]
fn a_route_follows_its_session_across_an_adoption() {
    let mut d = CoreDriver::new(Box::new(FakeCoreHarness::new(0)));
    let mut b = Bindings::new();
    run(
        &mut d,
        &mut b,
        json!({"open": {"as": "h", "data_dir": "d"}}),
    );
    session(&mut d, &mut b, "h", "r1");
    run(&mut d, &mut b, json!({"pump": {"handle": "h"}}));
    run(&mut d, &mut b, json!({"route_drain": true, "route": "r1"}));
    run(&mut d, &mut b, json!({"drop": {"handle": "h"}}));
    run(
        &mut d,
        &mut b,
        json!({"open": {"as": "h2", "data_dir": "d"}}),
    );
    run(
        &mut d,
        &mut b,
        json!({"begin": {"handle": "h2", "op": "AdoptAll"}}),
    );
    run(
        &mut d,
        &mut b,
        json!({"pump_until": {"handle": "h2", "event": {"Completed": {"result": {"ok": "unit"}}}}}),
    );
    run(
        &mut d,
        &mut b,
        json!({"route_send": {"frame": "bytes", "op": "1", "bytes": {"$bytes_hex": "61"}}, "route": "r1"}),
    );
    run(
        &mut d,
        &mut b,
        json!({"control": {"op": "pty_input", "handle": "h2", "session": "s1"}, "expect": {"bytes": {"$bytes_hex": "61"}}}),
    );
}

/// Core LC-7, ID-1, ID-2: an old route connection ends with its session. After Stop, Remove and a new Create of the same id, it never writes to
/// or reads from the new instance, through the handle that attached it or through the handle that adopted its session.
#[test]
fn an_old_route_never_reaches_a_new_instance_of_the_same_id() {
    let mut d = CoreDriver::new(Box::new(FakeCoreHarness::new(0)));
    let mut b = Bindings::new();
    run(
        &mut d,
        &mut b,
        json!({"open": {"as": "h", "data_dir": "d"}}),
    );
    session(&mut d, &mut b, "h", "r1");
    run(&mut d, &mut b, json!({"drop": {"handle": "h"}}));
    run(
        &mut d,
        &mut b,
        json!({"open": {"as": "h2", "data_dir": "d"}}),
    );
    run(
        &mut d,
        &mut b,
        json!({"begin": {"handle": "h2", "op": "AdoptAll"}}),
    );
    run(
        &mut d,
        &mut b,
        json!({"pump_until": {"handle": "h2", "event": {"Completed": {"result": {"ok": "unit"}}}}}),
    );
    run(
        &mut d,
        &mut b,
        json!({"begin": {"handle": "h2", "op": {"Stop": {"id": "s1"}}}}),
    );
    run(
        &mut d,
        &mut b,
        json!({"pump_until": {"handle": "h2", "event": {"SessionState": {"state": {"Exited": {"$any": true}}}}}}),
    );
    run(
        &mut d,
        &mut b,
        json!({"begin": {"handle": "h2", "op": {"Remove": {"id": "s1"}}}}),
    );
    run(
        &mut d,
        &mut b,
        json!({"pump_until": {"handle": "h2", "event": {"SessionState": {"state": "Released"}}}}),
    );
    session(&mut d, &mut b, "h2", "r2");
    run(&mut d, &mut b, json!({"pump": {"handle": "h2"}}));
    run(&mut d, &mut b, json!({"route_drain": true, "route": "r2"}));
    run(
        &mut d,
        &mut b,
        json!({"route_send": {"frame": "bytes", "op": "1", "bytes": {"$bytes_hex": "61"}}, "route": "r1"}),
    );
    run(
        &mut d,
        &mut b,
        json!({"control": {"op": "pty_input", "handle": "h2", "session": "s1"}, "expect": {"bytes": {"$bytes_hex": ""}}}),
    );
}

/// The same isolation through the data directory: no handle holds the sessions, and a new instance of the id took the row.
#[test]
fn an_old_route_does_not_reach_a_replacement_row_in_the_data_directory() {
    let mut d = CoreDriver::new(Box::new(FakeCoreHarness::new(0)));
    let mut b = Bindings::new();
    run(
        &mut d,
        &mut b,
        json!({"open": {"as": "h", "data_dir": "d"}}),
    );
    session(&mut d, &mut b, "h", "r1");
    run(&mut d, &mut b, json!({"drop": {"handle": "h"}}));
    run(
        &mut d,
        &mut b,
        json!({"open": {"as": "h2", "data_dir": "d"}}),
    );
    session(&mut d, &mut b, "h2", "r2");
    run(&mut d, &mut b, json!({"pump": {"handle": "h2"}}));
    run(&mut d, &mut b, json!({"route_drain": true, "route": "r2"}));
    run(&mut d, &mut b, json!({"drop": {"handle": "h2"}}));
    run(
        &mut d,
        &mut b,
        json!({"route_send": {"frame": "bytes", "op": "1", "bytes": {"$bytes_hex": "61"}}, "route": "r1"}),
    );
    run(
        &mut d,
        &mut b,
        json!({"open": {"as": "h3", "data_dir": "d"}}),
    );
    run(
        &mut d,
        &mut b,
        json!({"begin": {"handle": "h3", "op": "AdoptAll"}}),
    );
    run(
        &mut d,
        &mut b,
        json!({"pump_until": {"handle": "h3", "event": {"Completed": {"result": {"ok": "unit"}}}}}),
    );
    run(
        &mut d,
        &mut b,
        json!({"control": {"op": "pty_input", "handle": "h3", "session": "s1"}, "expect": {"bytes": {"$bytes_hex": ""}}}),
    );
}
