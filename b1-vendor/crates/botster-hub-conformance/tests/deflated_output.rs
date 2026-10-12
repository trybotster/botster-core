//! The output step reads the inflated bytes of an `output_deflate` frame that the decoder produced (Codec BP-2, A.4 0x0E).
//!
//! No codec transcript negotiates `output_deflate`, so this test runs one transcript of its own, written here, over the CI seeds.
//! The fake chooses per frame between a plain `output` and an `output_deflate` frame (a seeded choice), so some seeds deflate.

use botster_conformance::{run_transcript, Limits, Outcome, SeedSet, Transcript};
use botster_hub_conformance::route::fake::FakeRouteSubject;
use botster_hub_conformance::route::{RouteDriver, RouteSchemas};
use serde_json::json;

#[test]
fn route_expect_output_joins_deflated_and_plain_frames() {
    // 200 bytes that compress: a deflated frame is then a different size than the plain one.
    let bytes_hex = "61".repeat(200);
    let transcript: Transcript = serde_json::from_value(json!({
        "id": "test::deflated_output",
        "contract": "codec",
        "clause": "BP-2",
        "subject": "worker",
        "requires": { "controls": ["pty_output"] },
        "steps": [
            { "route_attach": { "history": "screen", "route_features": ["output_deflate"], "writable": true, "answers_queries": true } },
            { "expect": { "frame": "attached", "terminal_format": { "$type": "string" } }, "on": "r1", "because": "Codec TS-1" },
            { "expect": { "frame": "baseline_begin", "rows": { "$type": "integer" }, "cols": { "$type": "integer" }, "modes": { "$type": "object" } }, "on": "r1", "because": "Codec TS-4" },
            { "expect": { "frame": "screen", "payload": { "$any": true } }, "on": "r1", "because": "Codec TS-4" },
            { "expect_eventually": { "frame": "baseline_end", "history": "not_requested" },
              "ignore": [{ "frame": "history", "payload": { "$any": true } }], "on": "r1", "because": ["Codec TS-1", "Codec TS-4"] },
            { "expect": { "frame": "live" }, "on": "r1", "because": "Codec TS-4b" },
            { "control": { "op": "pty_output", "bytes_hex": bytes_hex } },
            { "route_expect_output": { "bytes_hex": bytes_hex }, "because": "Codec BP-2" }
        ]
    }))
    .expect("the transcript parses");
    let make = |seed: u64| -> Box<dyn botster_conformance::StepDriver> {
        Box::new(RouteDriver::new(FakeRouteSubject::new(seed)))
    };
    let outcome = run_transcript(
        &transcript,
        &make,
        &SeedSet::ci(),
        &RouteSchemas,
        &Limits::default(),
    );
    assert!(matches!(outcome, Outcome::Passed), "{outcome:?}");
}

/// The same transcript with a required frame kind (Core A9-3). One byte of output leaves in one frame, plain or compressed by seed, and a
/// `modes` frame goes with it, so the step meets a frame that is not an output frame. A step whose kind is the frame's passes; a step
/// with the other kind fails with the kind that it required; and the `modes` frame never fails the kind check.
#[test]
fn route_expect_output_requires_the_kind_it_names() {
    // `kind` is "output", "output_deflate", or "probe": the probe states what the frame is, with no kind check of the step under test.
    let run = |kind: &str, seed: u64| -> Outcome {
        let last = if kind == "probe" {
            json!({ "expect_eventually": { "frame": "output_deflate" }, "ignore": [{ "frame": "modes" }, { "frame": "output" }], "on": "r1", "because": "Codec BP-2" })
        } else {
            json!({ "route_expect_output": { "bytes_hex": "61", "kind": kind, "ignore": [{ "frame": "modes" }] }, "because": "Codec BP-2" })
        };
        let transcript: Transcript = serde_json::from_value(json!({
            "id": "test::deflated_output_kind",
            "contract": "codec",
            "clause": "BP-2",
            "subject": "worker",
            "requires": { "controls": ["pty_output"] },
            "steps": [
                { "route_attach": { "history": "screen", "route_features": ["output_deflate"], "writable": true, "answers_queries": true } },
                { "expect": { "frame": "attached", "terminal_format": { "$type": "string" } }, "on": "r1", "because": "Codec TS-1" },
                { "expect": { "frame": "baseline_begin", "rows": { "$type": "integer" }, "cols": { "$type": "integer" }, "modes": { "$type": "object" } }, "on": "r1", "because": "Codec TS-4" },
                { "expect": { "frame": "screen", "payload": { "$any": true } }, "on": "r1", "because": "Codec TS-4" },
                { "expect_eventually": { "frame": "baseline_end", "history": "not_requested" },
                  "ignore": [{ "frame": "history", "payload": { "$any": true } }], "on": "r1", "because": ["Codec TS-1", "Codec TS-4"] },
                { "expect": { "frame": "live" }, "on": "r1", "because": "Codec TS-4b" },
                { "control": { "op": "pty_output", "bytes_hex": "61", "scripted": { "modes": { "cursor_visible": false } } } },
                last
            ]
        }))
        .expect("the transcript parses");
        let make = |seed: u64| -> Box<dyn botster_conformance::StepDriver> {
            Box::new(RouteDriver::new(FakeRouteSubject::new(seed)))
        };
        let only = SeedSet::parse(&seed.to_string()).expect("a seed");
        run_transcript(&transcript, &make, &only, &RouteSchemas, &Limits::default())
    };
    let wrong_kind = |o: &Outcome, kind: &str| {
        let text = format!("{o:?}");
        text.contains("frame_kind") && text.contains(&format!("\"{kind}\""))
    };
    let (mut deflated, mut plain) = (0, 0);
    for seed in 0..32 {
        let (d, p) = (run("output_deflate", seed), run("output", seed));
        let (dp, pp) = (matches!(d, Outcome::Passed), matches!(p, Outcome::Passed));
        // the probe says whether this seed's frame is compressed, independently of the check under test
        let actual = matches!(run("probe", seed), Outcome::Passed);
        assert!(
            dp == actual && pp != actual,
            "seed {seed}: deflated {actual}, deflate step {dp}, plain step {pp}"
        );
        if actual {
            assert!(wrong_kind(&p, "output"), "seed {seed}: {p:?}");
            deflated += 1;
        } else {
            assert!(wrong_kind(&d, "output_deflate"), "seed {seed}: {d:?}");
            plain += 1;
        }
    }
    assert!(
        deflated > 0 && plain > 0,
        "{deflated} deflated, {plain} plain"
    );
}

/// One byte of output leaves in one frame, plain or compressed by seed. The eligibility floor says which of the two is right (Core
/// A9-3: only an eligible payload is compressed, and with the choice forced on it is), and `min_deflate_frames` says that a compressed
/// frame was seen: with none, no eligible payload occurred and the step is inconclusive, never a failure. A probe states what each
/// seed's frame is, independently of the step under test.
#[test]
fn route_expect_output_checks_eligibility_per_frame_and_is_inconclusive_without_an_eligible_payload(
) {
    let run = |last: serde_json::Value, seed: u64| -> Outcome {
        let transcript: Transcript = serde_json::from_value(json!({
            "id": "test::deflated_output_eligible",
            "contract": "codec",
            "clause": "BP-2",
            "subject": "worker",
            "requires": { "controls": ["pty_output"] },
            "steps": [
                { "route_attach": { "history": "screen", "route_features": ["output_deflate"], "writable": true, "answers_queries": true } },
                { "expect": { "frame": "attached", "terminal_format": { "$type": "string" } }, "on": "r1", "because": "Codec TS-1" },
                { "expect": { "frame": "baseline_begin", "rows": { "$type": "integer" }, "cols": { "$type": "integer" }, "modes": { "$type": "object" } }, "on": "r1", "because": "Codec TS-4" },
                { "expect": { "frame": "screen", "payload": { "$any": true } }, "on": "r1", "because": "Codec TS-4" },
                { "expect_eventually": { "frame": "baseline_end", "history": "not_requested" },
                  "ignore": [{ "frame": "history", "payload": { "$any": true } }], "on": "r1", "because": ["Codec TS-1", "Codec TS-4"] },
                { "expect": { "frame": "live" }, "on": "r1", "because": "Codec TS-4b" },
                { "control": { "op": "pty_output", "bytes_hex": "61" } },
                last
            ]
        }))
        .expect("the transcript parses");
        let make = |seed: u64| -> Box<dyn botster_conformance::StepDriver> {
            Box::new(RouteDriver::new(FakeRouteSubject::new(seed)))
        };
        let only = SeedSet::parse(&seed.to_string()).expect("a seed");
        run_transcript(&transcript, &make, &only, &RouteSchemas, &Limits::default())
    };
    let step = |extra: serde_json::Value| {
        let mut spec = json!({ "bytes_hex": "61", "kind": "output_deflate" });
        for (k, v) in extra.as_object().expect("an object") {
            spec[k] = v.clone();
        }
        json!({ "route_expect_output": spec, "because": "Codec BP-2" })
    };
    let passed = |o: &Outcome| matches!(o, Outcome::Passed);
    let inconclusive = |o: &Outcome, limit: &str| matches!(o, Outcome::Inconclusive { limit: l, .. } if l == limit);
    let probe = json!({ "expect_eventually": { "frame": "output_deflate" }, "ignore": [{ "frame": "output" }], "on": "r1", "because": "Codec BP-2" });
    let (mut deflated, mut plain) = (0, 0);
    for seed in 0..32 {
        let actual = passed(&run(probe.clone(), seed));
        // other bytes than expected (62 for 61), below the floor, with the compressed-frame premise unmet: a proven output mismatch is
        // a failure, never an open premise
        let wrong = run(
            step(json!({ "bytes_hex": "62", "eligible_from": 256, "min_deflate_frames": 1 })),
            seed,
        );
        assert!(
            matches!(wrong, Outcome::Failed { .. }),
            "seed {seed}: {wrong:?}"
        );
        // the byte is eligible from 1: it must be compressed (a plain frame is a failure, not an open premise)
        let eligible = run(step(json!({ "eligible_from": 1 })), seed);
        assert_eq!(passed(&eligible), actual, "seed {seed}: {eligible:?}");
        assert!(
            actual || matches!(eligible, Outcome::Failed { .. }),
            "seed {seed}: {eligible:?}"
        );
        // the byte is below 2: it must stay plain
        let below = run(step(json!({ "eligible_from": 2 })), seed);
        assert_eq!(passed(&below), !actual, "seed {seed}: {below:?}");
        // a compressed frame is required: with one, the step passes; with none, the premise is open
        let required = run(
            step(json!({ "eligible_from": 1, "min_deflate_frames": 1 })),
            seed,
        );
        assert_eq!(passed(&required), actual, "seed {seed}: {required:?}");
        // more compressed frames than the one that exists: the premise is open, whatever the frame is
        let more = run(
            step(json!({ "eligible_from": 1, "min_deflate_frames": 2 })),
            seed,
        );
        assert!(!passed(&more), "seed {seed}: {more:?}");
        if actual {
            assert!(
                inconclusive(&more, "no_eligible_payload"),
                "seed {seed}: {more:?}"
            );
        }
        // no payload is eligible: a compressed frame there is a failure, and a plain one leaves the premise open
        let none = run(
            step(json!({ "eligible_from": 2, "min_deflate_frames": 1 })),
            seed,
        );
        assert!(!passed(&none), "seed {seed}: {none:?}");
        assert_eq!(
            inconclusive(&none, "no_eligible_payload"),
            !actual,
            "seed {seed}: {none:?}"
        );
        if actual {
            deflated += 1;
        } else {
            plain += 1;
        }
    }
    assert!(
        deflated > 0 && plain > 0,
        "{deflated} deflated, {plain} plain"
    );
}
