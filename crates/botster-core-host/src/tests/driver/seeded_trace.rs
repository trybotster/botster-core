//! The seeded choice trace is deterministic: one seed gives the same choices, in the same order, with the same answers, in
//! every run on one head (Core A5-1: deterministic under a script and a seed).
//!
//! A seeded policy varies every choice and records each one: the choice point, the candidate count (or the bound's `max`),
//! and its answer. Two sessions start, run and stop under it.
//!
//! History: condition 3 of the lead's E1 ruling (steward ruling R-88, group E) asked for a one-time proof that E1
//! (`Scheduler::held` and `Scheduler::admitted`) changed no choice. This file then compared each seed's trace with the
//! values that it gave on v1 `1b984f06`, the parent of E1. The gate on merge train 2's head `08098782`, which merged #300's
//! head `484912ac`, passed with that comparison (`botster-core-stage1-train-2-08098782-pool-20261011-234830-6797.log`), so
//! the proof is settled. A
//! recorded trace changes with every legitimate change of the driver's steps, so the test now checks determinism only.

use super::deadlines::to_launch;
use super::*;
use botster_core_edges::scheduler::ChoicePoint;

/// Every choice that the driver asks: the point, the candidate count or `max`, and the answer.
type Trace = Arc<Mutex<Vec<(ChoicePoint, usize, usize)>>>;

/// The seeds of the comparison.
const SEEDS: u64 = 16;

/// A policy that varies every choice from a seeded xorshift64* stream.
struct Seeded {
    state: u64,
    trace: Trace,
}

impl Seeded {
    fn next(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
}

impl Scheduler for Seeded {
    fn pick(&mut self, point: ChoicePoint, candidates: usize) -> usize {
        let answer = match candidates {
            0 => 0,
            n => (self.next() % n as u64) as usize,
        };
        self.trace.lock().unwrap().push((point, candidates, answer));
        answer
    }

    fn bound(&mut self, point: ChoicePoint, max: usize) -> usize {
        let answer = match max {
            0 => 0,
            n => 1 + (self.next() % n as u64) as usize,
        };
        self.trace.lock().unwrap().push((point, max, answer));
        answer
    }
}

/// The choice trace of one seed.
fn trace(seed: u64) -> Vec<(ChoicePoint, usize, usize)> {
    let trace = Trace::default();
    let policy = Seeded {
        state: seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1,
        trace: Arc::clone(&trace),
    };
    let mut rig = Rig::with_scheduler(CoreLimits::default(), Box::new(policy));
    for (name, link) in [("s1", LinkId(1)), ("s2", LinkId(2))] {
        to_launch(&mut rig, name, link);
        rig.worker_says(link, launched());
        rig.settle();
    }
    for name in ["s1", "s2"] {
        rig.driver.begin(Op::Stop { id: sid(name) }).unwrap();
    }
    rig.run_out();
    drop(rig);
    Arc::try_unwrap(trace)
        .expect("the rig held the only other handle")
        .into_inner()
        .unwrap()
}

#[test]
fn one_seed_gives_the_same_choices_in_every_run() {
    let traces: Vec<_> = (0..SEEDS).map(trace).collect();
    for (seed, first) in traces.iter().enumerate() {
        assert!(!first.is_empty(), "seed {seed} asked no choice");
        assert_eq!(&trace(seed as u64), first, "seed {seed}");
    }
    // The seeds vary the choices, so the comparison is not of one fixed trace.
    assert!(traces.iter().any(|t| t != &traces[0]));
}
