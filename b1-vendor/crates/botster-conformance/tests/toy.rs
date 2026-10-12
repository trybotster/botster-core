//! The `conformance_tests!` pattern end to end: a build script writes one `#[test]` per toy transcript, and each runs through the engine.

use botster_conformance::{Bindings, Deadline, Poll, StepDriver, StepError};
use include_dir::{include_dir, Dir};
use serde_json::{json, Value};
use std::collections::VecDeque;

pub static TOY_TRANSCRIPTS: Dir<'_> =
    include_dir!("$CARGO_MANIFEST_DIR/tests/fixtures/transcripts");

/// An echo subject: `send` queues the value, `next` gives it back.
struct Echo {
    queue: VecDeque<Value>,
    seed: u64,
}

impl StepDriver for Echo {
    fn has_feature(&self, name: &str) -> bool {
        name == "echo"
    }
    fn has_control(&self, op: &str) -> bool {
        op == "noop"
    }
    fn setup(&mut self, _: &Value) -> Result<(), StepError> {
        Ok(())
    }
    fn exec(&mut self, step: &Value, _: &mut Bindings, _: &Deadline) -> Result<(), StepError> {
        match step.get("send") {
            Some(v) => {
                self.queue
                    .push_back(json!({ "echo": v, "seed_is_set": self.seed < 1000 }));
                Ok(())
            }
            None => Err(StepError::Unsupported {
                op: "unknown".into(),
            }),
        }
    }
    fn next(&mut self, _: &Value, _: &Deadline) -> Poll {
        self.queue.pop_front().map_or(Poll::Idle, Poll::Item)
    }
}

fn make(seed: u64) -> Box<dyn StepDriver> {
    Box::new(Echo {
        queue: VecDeque::new(),
        seed,
    })
}

include!(concat!(env!("OUT_DIR"), "/toy_tests.rs"));

/// Every toy frame is a JSON value that the schema `any` accepts, so the pre-send and received checks run.
fn toy_schemas() -> botster_conformance::SchemaRegistry {
    let mut r = botster_conformance::SchemaRegistry::new();
    r.add("any", &json!({})).unwrap();
    r.set_frame_schema(botster_conformance::Direction::Sent, "any");
    r.set_frame_schema(botster_conformance::Direction::Received, "any");
    r
}

toy_conformance_tests!(make, toy_schemas());

fn as_json(t: &botster_conformance::Transcript) -> Value {
    serde_json::to_value(t).unwrap()
}

#[test]
fn load_one_gives_the_transcript_that_load_dir_gives_for_the_same_id() {
    let all = botster_conformance::load_dir(&TOY_TRANSCRIPTS).unwrap();
    assert!(all.len() >= 3);
    for t in &all {
        let one = botster_conformance::load_one(&TOY_TRANSCRIPTS, &t.id)
            .unwrap()
            .unwrap_or_else(|| panic!("{} is missing", t.id));
        assert_eq!(as_json(&one), as_json(t));
    }
    assert!(
        botster_conformance::load_one(&TOY_TRANSCRIPTS, "conf::no_such_id")
            .unwrap()
            .is_none()
    );
}

#[test]
fn load_one_parses_no_file_but_its_own_skips_fixtures_and_refuses_a_duplicate_name() {
    use include_dir::{DirEntry, File};
    const GOOD: &str =
        r#"{"id":"conf::a","contract":"core","clause":"X-1","subject":"core","steps":[]}"#;
    // `conf::b` does not parse, and a `fixtures` directory holds another file named like `conf::a`.
    const FIXTURES: Dir<'_> = Dir::new(
        "fixtures",
        &[DirEntry::File(File::new(
            "fixtures/conf::a.json",
            b"not json",
        ))],
    );
    const SUB: Dir<'_> = Dir::new(
        "sub",
        &[DirEntry::File(File::new(
            "sub/conf::a.json",
            GOOD.as_bytes(),
        ))],
    );
    static ONE: Dir<'_> = Dir::new(
        "",
        &[
            DirEntry::File(File::new("conf::a.json", GOOD.as_bytes())),
            DirEntry::File(File::new("conf::b.json", b"not json")),
            DirEntry::Dir(FIXTURES),
        ],
    );
    static TWO: Dir<'_> = Dir::new(
        "",
        &[
            DirEntry::File(File::new("conf::a.json", GOOD.as_bytes())),
            DirEntry::Dir(SUB),
        ],
    );
    let a = botster_conformance::load_one(&ONE, "conf::a")
        .unwrap()
        .unwrap();
    assert_eq!(a.id, "conf::a");
    assert!(botster_conformance::load_one(&ONE, "conf::b").is_err());
    let err = botster_conformance::load_one(&TWO, "conf::a").unwrap_err();
    assert!(err.to_string().contains("duplicate id conf::a"), "{err}");
}

// A generated test must fail when its transcript is missing or does not pass, and must pass when it does. Each would pass if `run_one_test` returned at once.
#[test]
#[should_panic(expected = "no transcript with id conf::no_such_id")]
fn run_one_test_panics_for_an_id_with_no_transcript() {
    botster_conformance::run_one_test(
        "toy",
        &TOY_TRANSCRIPTS,
        "conf::no_such_id",
        &make,
        &toy_schemas(),
    );
}

#[test]
#[should_panic(expected = "conf::a")]
fn run_one_test_panics_when_the_transcript_does_not_pass() {
    use include_dir::{DirEntry, File};
    // The subject echoes `{"x":1}`; the transcript expects `{"x":2}`.
    const WRONG: &str = r#"{"id":"conf::a","contract":"core","clause":"X-1","subject":"core","steps":[{"send":{"x":1}},{"expect":{"echo":{"x":2},"seed_is_set":true}}]}"#;
    static DIR: Dir<'_> = Dir::new(
        "",
        &[DirEntry::File(File::new("conf::a.json", WRONG.as_bytes()))],
    );
    botster_conformance::run_one_test("toy", &DIR, "conf::a", &make, &toy_schemas());
}
