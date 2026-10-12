//! Condition 3 of the lead's E1 ruling (steward ruling R-88, group E): with no hold, the driver asks the same choices, in the
//! same order, and gets the same answers as before `Scheduler::held` and `Scheduler::admitted` existed.
//!
//! A seeded policy varies every choice and records each one: the choice point, the candidate count (or the bound's `max`),
//! and its answer. Two sessions start, run and stop under it. The test compares each seed's trace, as its length and an
//! FNV-1a digest, with `BEFORE_E1`: the values that this same file gave on v1 `1b984f06`, the parent of E1, where the trait
//! had only `pick` and `bound`. The policy implements only those two, so the file compiles on both.

use super::deadlines::to_launch;
use super::*;
use botster_core_edges::scheduler::ChoicePoint;

/// Every choice that the driver asks: the point, the candidate count or `max`, and the answer.
type Trace = Arc<Mutex<Vec<(ChoicePoint, usize, usize)>>>;

/// The seeds of the comparison.
const SEEDS: u64 = 16;

/// `(trace length, FNV-1a digest)` per seed `0..SEEDS`, from `print_the_seeded_traces` on v1 `1b984f06` (pool evidence job
/// on E1 `1d1d7fc9`, which ran the same file on both commits; log
/// `botster-core-stage1-p6-e1-scheduler-hold-1d1d7fc9-pool-20261011-200116-36766.log`).
const BEFORE_E1: [(usize, u64); SEEDS as usize] = [
    (207, 16081904804330880812),
    (193, 14999279856648310390),
    (230, 10601161564193167726),
    (229, 8461361969394252691),
    (208, 13394400034927130325),
    (248, 4822766342596046137),
    (219, 1766076094699244872),
    (293, 4616665827199621369),
    (221, 1538455162258642356),
    (213, 12098497657580199005),
    (173, 11502032415553900377),
    (206, 641723253192062702),
    (231, 5545640914891194933),
    (261, 7983905030523437726),
    (201, 5681368860853910093),
    (235, 13479727771670637969),
];

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

/// The trace of one seed, as its length and its FNV-1a digest over the `Debug` text of each entry.
fn trace(seed: u64) -> (usize, u64) {
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
    let trace = trace.lock().unwrap();
    let mut digest = 0xcbf2_9ce4_8422_2325_u64;
    for entry in trace.iter() {
        for byte in format!("{entry:?};").bytes() {
            digest ^= u64::from(byte);
            digest = digest.wrapping_mul(0x0100_0000_01b3);
        }
    }
    (trace.len(), digest)
}

#[test]
fn with_no_hold_the_seeded_choices_are_those_before_e1() {
    for seed in 0..SEEDS {
        assert_eq!(trace(seed), BEFORE_E1[seed as usize], "seed {seed}");
    }
}

/// Prints the traces, so that an evidence job can record them on another commit.
#[test]
#[ignore = "evidence: prints the seeded traces"]
fn print_the_seeded_traces() {
    for seed in 0..SEEDS {
        println!("seeded-trace {seed} {:?}", trace(seed));
    }
}
