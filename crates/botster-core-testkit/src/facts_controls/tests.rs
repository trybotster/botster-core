use super::*;
use botster_core_conformance::{
    normalize_op, CoreHarness, DataDirRef, OpenSpec, WorkerBuild, WorkerRef,
};
use botster_core_contract::prelude::*;
use std::time::Instant;

/// The most pumps that a test gives Core to reach an event: far above what this session needs.
const PUMPS: usize = 50;

fn spec() -> OpenSpec {
    OpenSpec {
        handle: "a".into(),
        data_dir: DataDirRef("d".into()),
        worker: Some(WorkerRef {
            build: WorkerBuild::Current,
            file_name: None,
        }),
        limits: json!({}),
    }
}

fn now(at: Instant) -> Now {
    Now {
        monotonic: at,
        unix: 1_000_000,
    }
}

fn sid() -> SessionId {
    SessionId("s1".into())
}

/// Pumps until `op` completes, and returns its result.
fn complete(core: &mut dyn CoreApi, at: Instant, op: OpId) -> OpResult {
    for _ in 0..PUMPS {
        core.pump(now(at));
        for event in core.poll_events(64) {
            if let Event::Completed { op: o, result } = event {
                if o == op {
                    return result;
                }
            }
        }
    }
    panic!("the op {op:?} did not complete");
}

/// A Core on handle `a` with the session `s1` running a payload that holds.
fn running(harness: &mut TestkitHarness) -> (Box<dyn CoreApi>, Instant) {
    let mut core = harness.open(&spec()).expect("open");
    let at = Instant::now();
    let mut create = json!({"Create": {"session": "s1", "request": {"program": [{"hold": {}}]}}});
    normalize_op(&mut create, &harness.probe_binary());
    let create = core.begin(serde_json::from_value(create).unwrap()).unwrap();
    complete(core.as_mut(), at, create);
    let start = core.begin(Op::Start { id: sid() }).unwrap();
    complete(core.as_mut(), at, start);
    (core, at)
}

/// Core ST-7, 9B: `input_records_cost {session}` is the JSON byte size of the `input_records` list of the last `ReadFacts`
/// answer that the session's worker sent; before any answer, it is an error.
#[test]
fn input_records_cost_is_the_size_of_the_last_facts_answer() {
    let mut harness = TestkitHarness::new(0);
    assert!(harness.has_control("input_records_cost"));
    let (mut core, at) = running(&mut harness);
    let cost = |harness: &mut TestkitHarness| {
        harness.control("a", "input_records_cost", &json!({"session": "s1"}))
    };
    assert!(cost(&mut harness).is_err(), "no ReadFacts answer yet");
    let write = core
        .begin(Op::WriteInput {
            session: sid(),
            payload: InputPayload::Text { text: "hi".into() },
            guard: None,
        })
        .unwrap();
    complete(core.as_mut(), at, write);
    let read = core
        .begin(Op::ReadFacts {
            session: sid(),
            after: None,
        })
        .unwrap();
    let OpResult::Ok(OpOutput::Facts(facts)) = complete(core.as_mut(), at, read) else {
        panic!("ReadFacts answers");
    };
    assert!(!facts.input.input_records.is_empty());
    let expected = serde_json::to_vec(&facts.input.input_records)
        .unwrap()
        .len();
    assert_eq!(cost(&mut harness).unwrap(), json!({ "bytes": expected }));
}
