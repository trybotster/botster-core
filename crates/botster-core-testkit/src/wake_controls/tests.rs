use super::*;
use botster_core_conformance::{
    normalize_op, CoreHarness, DataDirRef, OpenSpec, WorkerBuild, WorkerRef,
};
use botster_core_contract::prelude::*;
use serde_json::json;
use std::time::{Duration, Instant};

/// The most pumps that a test gives Core to settle: far above what one session needs.
const PUMPS: usize = 50;
/// The seeds that a test searches for one whose idle waits draw a spurious wake. Each idle wait draws one with a chance of
/// one half, so the search ends at the first seeds.
const SEEDS: u64 = 16;
/// The waits that a test makes on an idle host.
const WAITS: usize = 16;

fn spec() -> OpenSpec {
    OpenSpec {
        handle: "a".into(),
        data_dir: DataDirRef("quiet-dir".into()),
        worker: Some(WorkerRef {
            build: WorkerBuild::Current,
            file_name: None,
        }),
        limits: json!({}),
    }
}

fn quiet(harness: &mut TestkitHarness) -> bool {
    harness.control("a", "edges_quiet", &json!({})).unwrap()["quiet"] == json!(true)
}

/// Core A5-2, the fence of `await_quiet`: the edges are not quiet while the new worker's hello waits, and they are quiet
/// once Core has consumed every report and has no runnable work. The control never polls: the events stay queued.
#[test]
fn the_edges_are_quiet_only_when_core_has_consumed_every_report() {
    let mut harness = TestkitHarness::new(0);
    let mut core = harness.open(&spec()).expect("open");
    assert!(
        format!("{harness:?}").contains(r#"HandleEdges { dir: "quiet-dir""#),
        "the harness shows the handle's directory"
    );
    assert!(quiet(&mut harness), "no worker, no report");
    let mut create = json!({"Create": {"session": "s1", "request": {"program": [{"hold": {}}]}}});
    normalize_op(&mut create, &harness.probe_binary());
    core.begin(serde_json::from_value(create).unwrap()).unwrap();
    core.begin(Op::Start {
        id: SessionId("s1".into()),
    })
    .unwrap();
    let at = Instant::now();
    let now = Now {
        monotonic: at,
        unix: 1_000_000,
    };
    let mut seen_busy = false;
    let mut settled = false;
    for _ in 0..PUMPS {
        let report = core.pump(now);
        let is_quiet = quiet(&mut harness);
        seen_busy |= !is_quiet;
        if !report.more && is_quiet {
            settled = true;
            break;
        }
    }
    assert!(
        seen_busy,
        "the spawned worker's hello was a report that Core had not consumed"
    );
    assert!(settled, "Core consumed every report");
    assert_eq!(
        core.get(&SessionId("s1".into())).unwrap().state,
        SessionState::Running
    );
    assert!(
        !core.poll_events(64).is_empty(),
        "the control polled nothing: the events are still queued"
    );
}

/// `edges_quiet` refuses an unknown handle and an argument that it does not take; `no_spurious_wakes` refuses an argument.
#[test]
fn the_wake_controls_refuse_what_they_cannot_take() {
    let mut harness = TestkitHarness::new(0);
    assert!(matches!(
        harness.control("a", "edges_quiet", &json!({})),
        Err(ControlError::Bad(_))
    ));
    let _core = harness.open(&spec()).expect("open");
    assert!(matches!(
        harness.control("a", "edges_quiet", &json!({"session": "s1"})),
        Err(ControlError::Bad(_))
    ));
    assert!(matches!(
        harness.control("a", "no_spurious_wakes", &json!({"session": "s1"})),
        Err(ControlError::Bad(_))
    ));
}

/// A Core of a fresh run of `seed` with no runnable work. With `armed`, `no_spurious_wakes` runs first.
fn idle_core(seed: u64, armed: bool) -> (TestkitHarness, Box<dyn CoreApi>) {
    let mut harness = TestkitHarness::new(seed);
    let mut core = harness.open(&spec()).expect("open");
    if armed {
        assert_eq!(
            harness.control(
                "a",
                "no_spurious_wakes",
                &json!({"op": "no_spurious_wakes"})
            ),
            Ok(Value::Null)
        );
    }
    let now = Now {
        monotonic: Instant::now(),
        unix: 1_000_000,
    };
    let settled = (0..PUMPS).any(|_| !core.pump(now).more);
    assert!(settled, "the host has no runnable work left");
    (harness, core)
}

/// What `WAITS` waits with no timeout give on an idle host.
fn idle_waits(core: &dyn CoreApi) -> Vec<Wake> {
    let wake = core.wake_handle();
    (0..WAITS).map(|_| wake.wait(Duration::ZERO)).collect()
}

/// The first seed whose idle waits draw a spurious wake, with those waits.
fn seed_with_a_spurious_wake() -> (u64, Vec<Wake>) {
    (0..SEEDS)
        .map(|seed| (seed, idle_waits(idle_core(seed, false).1.as_ref())))
        .find(|(_, waits)| waits.contains(&Wake::Woken))
        .unwrap_or_else(|| {
            panic!("no seed below {SEEDS} drew a spurious wake in {WAITS} idle waits")
        })
}

/// Core TH-2 and A5-2: the run's scheduler seeds spurious wakes, so an idle host can be `Woken` with no work. With
/// `no_spurious_wakes`, the same seed gives `TimedOut` at every idle wait, so a transcript's `TimedOut` is sound.
#[test]
fn no_spurious_wakes_ends_the_seeded_spurious_wakes() {
    let (seed, _) = seed_with_a_spurious_wake();
    let (_harness, core) = idle_core(seed, true);
    assert_eq!(
        idle_waits(core.as_ref()),
        vec![Wake::TimedOut; WAITS],
        "seed {seed}"
    );
}

/// The spurious wakes come from the run's one seeded scheduler, so the same seed gives the same waits again: a failing
/// seed replays exactly.
#[test]
fn a_seed_replays_its_spurious_wakes() {
    let (seed, waits) = seed_with_a_spurious_wake();
    assert!(waits.contains(&Wake::TimedOut), "seed {seed}: {waits:?}");
    assert_eq!(
        idle_waits(idle_core(seed, false).1.as_ref()),
        waits,
        "seed {seed}"
    );
}

/// Core TM-6: with `no_spurious_wakes`, a real wake source still wakes the host. A `begin` is one.
#[test]
fn a_real_wake_still_wakes_with_no_spurious_wakes() {
    let (seed, _) = seed_with_a_spurious_wake();
    let (harness, mut core) = idle_core(seed, true);
    let wake = core.wake_handle();
    assert_eq!(
        wake.wait(Duration::ZERO),
        Wake::TimedOut,
        "the host is idle"
    );
    let mut create = json!({"Create": {"session": "s1", "request": {"program": [{"hold": {}}]}}});
    normalize_op(&mut create, &harness.probe_binary());
    core.begin(serde_json::from_value(create).unwrap()).unwrap();
    assert_eq!(wake.wait(Duration::ZERO), Wake::Woken);
}
