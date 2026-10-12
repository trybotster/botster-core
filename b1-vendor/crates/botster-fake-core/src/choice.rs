//! The named choice points of FakeCore: every order that Core section 11 leaves open (design 7.2 rule 3).
//!
//! Each point cites its clause. The seed decides at each point, and the chooser records which outcomes it took, so a test can show
//! that every listed point is reachable.

use rand_chacha::ChaCha8Rng;
use rand_core::{RngCore, SeedableRng};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Choice {
    /// The order of the work of one pump: the events of different sessions, the completions of concurrent ops and the due deadlines.
    /// Clause: Core OR-3, TM-5.
    SessionOrder,
    /// How many pumps an operation takes, never zero. Clause: Core OR-1, 11.
    PumpsPerOp,
    /// The pumps between `Start` and `Running`. Clause: Core 11.
    StartPumps,
    /// The pumps between `Stop` and `Exited`. Clause: Core 11.
    StopPumps,
    /// How many events one `poll_events` returns. Clause: Core 11 (batching).
    PollBatch,
    /// Whether a wait wakes with no work. Clause: Core TH-2, OU-6.
    SpuriousWake,
    /// Whether a class K event is posted in the pump of its cause or one pump later. Clause: Core EV-6, 11 (coalescing).
    KeyedDefer,
}

pub struct Chooser {
    rng: ChaCha8Rng,
    seen: BTreeMap<Choice, BTreeSet<usize>>,
}

impl Chooser {
    pub fn new(seed: u64) -> Self {
        Chooser {
            rng: ChaCha8Rng::seed_from_u64(seed),
            seen: BTreeMap::new(),
        }
    }

    /// One decision with `n` outcomes, in `0..n`.
    pub fn pick(&mut self, point: Choice, n: usize) -> usize {
        let pick = if n <= 1 {
            0
        } else {
            (self.rng.next_u64() % n as u64) as usize
        };
        self.seen.entry(point).or_default().insert(pick);
        pick
    }

    /// A seeded permutation (Fisher–Yates).
    pub fn shuffle<T>(&mut self, point: Choice, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = self.pick(point, i + 1);
            items.swap(i, j);
        }
    }

    /// The outcomes that each point took so far.
    pub fn seen(&self) -> &BTreeMap<Choice, BTreeSet<usize>> {
        &self.seen
    }
}
