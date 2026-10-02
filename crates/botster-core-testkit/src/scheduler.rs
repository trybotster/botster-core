//! The seeded scheduler (plan 2.4, Core A5-2).
//!
//! One ChaCha8 stream, seeded by `with_seed(n)`. Every choice point is a named function that cites its item of A5-2. The
//! functions are the only place that draws from the stream, so a seed fixes every order that the contract leaves open and
//! nothing else. The production policy stays P0's: the session visit is round-robin and is not varied (not an item of A5-2).

use botster_core_edges::scheduler::{ChoicePoint, Production};
use botster_core_edges::Scheduler;
use rand_chacha::ChaCha8Rng;
use rand_core::{RngCore, SeedableRng};
use std::sync::{Arc, Mutex};

/// The most pumps that the program and process edges spend between two states (A5-2: "a seed-chosen number of pumps").
pub const MAX_PUMPS_BETWEEN_STATES: usize = 4;

/// The seeded policy of A5-2.
#[derive(Debug, Clone)]
pub struct SeededScheduler {
    rng: ChaCha8Rng,
    sessions: Production,
}

impl SeededScheduler {
    /// Clause: Core A5-2 (`with_seed(n)`).
    pub fn with_seed(seed: u64) -> SeededScheduler {
        SeededScheduler {
            rng: ChaCha8Rng::seed_from_u64(seed),
            sessions: Production::new(),
        }
    }

    /// A uniform index below `n`. A choice among 0 or 1 candidates draws nothing.
    fn below(&mut self, n: usize) -> usize {
        if n <= 1 {
            return 0;
        }
        sample_below(n as u64, || self.rng.next_u64()) as usize
    }

    /// A uniform count in `1..=max`; 0 only when `max` is 0.
    fn count(&mut self, max: usize) -> usize {
        if max == 0 {
            return 0;
        }
        1 + self.below(max)
    }

    /// Which ready work runs first in a `pump` (A5-2, "event order across sessions, and completion order of concurrent
    /// operations"; Core OR-3). Returns an index into `ready`.
    pub fn ready_work(&mut self, ready: usize) -> usize {
        self.below(ready)
    }

    /// Whether the progress of an operation moves to a later `pump` (A5-2, "how many `pump` calls an operation takes"; Core OR-1).
    pub fn defer_operation(&mut self) -> bool {
        self.below(2) == 1
    }

    /// The pumps between `Start` and `Running` (A5-2, "the number of pumps between `Start` and `Running`"). At least 1.
    pub fn pumps_to_running(&mut self) -> usize {
        self.count(MAX_PUMPS_BETWEEN_STATES)
    }

    /// The pumps between `Stop` and `Exited` (A5-2, "between `Stop` and `Exited`"). At least 1.
    pub fn pumps_to_exited(&mut self) -> usize {
        self.count(MAX_PUMPS_BETWEEN_STATES)
    }

    /// The work that one `pump` does, at most `max` (A5-2, "partial progress per `pump`").
    pub fn pump_bound(&mut self, max: usize) -> usize {
        self.count(max)
    }

    /// The size of the non-empty part of the queue that `poll_events` returns, at most `max` (A5-2, "the batching of events per
    /// `poll_events`").
    pub fn poll_batch(&mut self, max: usize) -> usize {
        self.count(max)
    }

    /// How many updates of a class K key happen before the host polls, at most `max` (A5-2, "coalescing of class K events").
    pub fn class_k_updates(&mut self, max: usize) -> usize {
        self.count(max)
    }

    /// The size of one write of the program edge, at most `max` (A5-2, "output chunking and frame boundaries on a route").
    pub fn program_write_size(&mut self, max: usize) -> usize {
        self.count(max)
    }

    /// The size of one read of the route-transport edge, at most `max` (A5-2, "the order of route delivery across routes").
    pub fn route_read_size(&mut self, max: usize) -> usize {
        self.count(max)
    }

    /// Whether the wake fires with no work (A5-2, "spurious wakes"; Core OU-6, Core TH-2).
    pub fn spurious_wake(&mut self) -> bool {
        self.below(2) == 1
    }

    /// The place of a due deadline's event among `events` events of one `pump`, as an index in `0..=events` (A5-2, "the order,
    /// inside one `pump`, of a due deadline's event"; Core TM-3: the event is processed in this `pump` whatever its place).
    pub fn deadline_event_place(&mut self, events: usize) -> usize {
        self.below(events + 1)
    }

    /// Which of `pending` file completions is delivered next (plan 2.3b and 2.4; Core DP-5b gives completions of different routes
    /// no order). Returns an index into the pending list.
    pub fn file_completion(&mut self, pending: usize) -> usize {
        self.below(pending)
    }
}

impl Scheduler for SeededScheduler {
    fn pick(&mut self, point: ChoicePoint, candidates: usize) -> usize {
        match point {
            ChoicePoint::ReadyWork => self.ready_work(candidates),
            // Index 1 is "deferred" or "spurious". With one candidate only index 0 exists, and nothing is drawn.
            ChoicePoint::OperationDeferral if candidates >= 2 => {
                usize::from(self.defer_operation())
            }
            ChoicePoint::SpuriousWake if candidates >= 2 => usize::from(self.spurious_wake()),
            ChoicePoint::DeadlineEventPlace => {
                self.deadline_event_place(candidates.saturating_sub(1))
            }
            ChoicePoint::FileCompletion => self.file_completion(candidates),
            ChoicePoint::Session => self.sessions.pick(point, candidates),
            // A count point asked as a pick, and a point of a later revision, are not varied.
            _ => 0,
        }
    }

    fn bound(&mut self, point: ChoicePoint, max: usize) -> usize {
        match point {
            ChoicePoint::PumpsToRunning => self.pumps_to_running().min(max),
            ChoicePoint::PumpsToExited => self.pumps_to_exited().min(max),
            ChoicePoint::PumpBound => self.pump_bound(max),
            ChoicePoint::PollBatch => self.poll_batch(max),
            ChoicePoint::ClassKUpdates => self.class_k_updates(max),
            ChoicePoint::ProgramWriteSize => self.program_write_size(max),
            ChoicePoint::RouteReadSize => self.route_read_size(max),
            // A pick point asked as a bound, and a point of a later revision, use the bound in full.
            _ => max,
        }
    }
}

/// A uniform value below `n` (at least 2) from the 64-bit draws of `draw`. Rejection sampling: the accepted range is a whole
/// number of lengths of `n`, so the result has no bias.
fn sample_below(n: u64, mut draw: impl FnMut() -> u64) -> u64 {
    let limit = u64::MAX - u64::MAX % n;
    loop {
        let x = draw();
        if x < limit {
            return x % n;
        }
    }
}

/// The scheduler as the `Sim` and its edges share it: one stream, so one seed fixes one order.
#[derive(Debug, Clone)]
pub struct SchedulerHandle(Arc<Mutex<SeededScheduler>>);

impl SchedulerHandle {
    pub fn with_seed(seed: u64) -> SchedulerHandle {
        SchedulerHandle(Arc::new(Mutex::new(SeededScheduler::with_seed(seed))))
    }

    /// Runs `f` with the scheduler.
    pub fn with<R>(&self, f: impl FnOnce(&mut SeededScheduler) -> R) -> R {
        // The testkit has no panic that leaves the stream half drawn, so a poisoned lock still holds a usable stream.
        let mut guard = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        f(&mut guard)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draws(seed: u64) -> Vec<usize> {
        let mut s = SeededScheduler::with_seed(seed);
        (0..64).map(|_| s.ready_work(1000)).collect()
    }

    /// A5-1: the same seed gives the same stream. A5-2: another seed gives another one.
    #[test]
    fn a_seed_fixes_the_stream() {
        assert_eq!(draws(7), draws(7));
        assert_ne!(draws(7), draws(8));
    }

    /// A5-2: every choice stays in its range.
    #[test]
    fn choices_stay_in_range() {
        let mut s = SeededScheduler::with_seed(1);
        for _ in 0..500 {
            assert!(s.ready_work(5) < 5);
            assert!((1..=MAX_PUMPS_BETWEEN_STATES).contains(&s.pumps_to_running()));
            assert!((1..=MAX_PUMPS_BETWEEN_STATES).contains(&s.pumps_to_exited()));
            assert!((1..=9).contains(&s.pump_bound(9)));
            assert!((1..=9).contains(&s.poll_batch(9)));
            assert!((1..=9).contains(&s.class_k_updates(9)));
            assert!((1..=9).contains(&s.program_write_size(9)));
            assert!((1..=9).contains(&s.route_read_size(9)));
            assert!(s.deadline_event_place(3) <= 3);
            assert!(s.file_completion(4) < 4);
        }
        assert_eq!(s.pump_bound(0), 0);
        assert_eq!(s.ready_work(0), 0);
    }

    /// Core OR-1: an operation is never done in zero pumps, so a count point never gives 0 for a non-zero bound.
    #[test]
    fn a_count_is_never_zero_for_a_nonzero_bound() {
        let mut s = SeededScheduler::with_seed(3);
        assert!((0..500).all(|_| s.poll_batch(1) == 1 && s.route_read_size(2) >= 1));
    }

    /// A5-2 "Not varied": the session visit stays round-robin, and a choice of one candidate draws nothing.
    #[test]
    fn sessions_stay_round_robin_and_a_single_candidate_draws_nothing() {
        let mut s = SeededScheduler::with_seed(5);
        let order: Vec<usize> = (0..4).map(|_| s.pick(ChoicePoint::Session, 3)).collect();
        assert_eq!(order, [0, 1, 2, 0]);
        let mut a = SeededScheduler::with_seed(5);
        let mut b = SeededScheduler::with_seed(5);
        assert_eq!(a.pick(ChoicePoint::ReadyWork, 1), 0);
        assert_eq!(a.ready_work(100), b.ready_work(100));
    }

    /// Each choice point of the trait reaches its named function, so a seed varies it (A5-2 list and plan 2.3b).
    #[test]
    fn the_trait_reaches_every_choice_point() {
        let mut s = SeededScheduler::with_seed(11);
        let mut seen_picks = std::collections::BTreeSet::new();
        let mut seen_bounds = std::collections::BTreeSet::new();
        for _ in 0..400 {
            seen_picks.insert((0, s.pick(ChoicePoint::ReadyWork, 4)));
            seen_picks.insert((1, s.pick(ChoicePoint::OperationDeferral, 2)));
            seen_picks.insert((2, s.pick(ChoicePoint::SpuriousWake, 2)));
            seen_picks.insert((3, s.pick(ChoicePoint::DeadlineEventPlace, 4)));
            seen_picks.insert((4, s.pick(ChoicePoint::FileCompletion, 4)));
            for (i, point) in [
                ChoicePoint::PumpsToRunning,
                ChoicePoint::PumpsToExited,
                ChoicePoint::PumpBound,
                ChoicePoint::PollBatch,
                ChoicePoint::ClassKUpdates,
                ChoicePoint::ProgramWriteSize,
                ChoicePoint::RouteReadSize,
            ]
            .into_iter()
            .enumerate()
            {
                seen_bounds.insert((i, s.bound(point, 4)));
            }
        }
        // Every pick point gives each of its candidates, and every bound point gives each of 1 to 4.
        assert_eq!(seen_picks.len(), 4 + 2 + 2 + 4 + 4);
        assert_eq!(seen_bounds.len(), 7 * 4);
    }

    /// The `Scheduler` trait gives an index below the candidate count, also for the binary points (Scheduler::pick contract).
    #[test]
    fn a_pick_stays_below_the_candidate_count() {
        let mut s = SeededScheduler::with_seed(13);
        for _ in 0..200 {
            for point in [
                ChoicePoint::ReadyWork,
                ChoicePoint::OperationDeferral,
                ChoicePoint::SpuriousWake,
                ChoicePoint::DeadlineEventPlace,
                ChoicePoint::FileCompletion,
                ChoicePoint::Session,
            ] {
                assert_eq!(s.pick(point, 1), 0, "{point:?}");
                assert_eq!(s.pick(point, 0), 0, "{point:?}");
                assert!(s.pick(point, 2) < 2, "{point:?}");
            }
        }
    }

    /// The sampler has no modulo bias: a draw at or above the largest whole multiple of `n` is rejected, and a draw below it is
    /// used. For `n = 7` the limit is `u64::MAX - 1`.
    #[test]
    fn the_sampler_rejects_the_biased_tail() {
        let mut draws = [u64::MAX - 1, u64::MAX, u64::MAX - 2].into_iter();
        // The first two draws are at or above the limit and are rejected; the third is used.
        assert_eq!(
            sample_below(7, || draws.next().unwrap()),
            (u64::MAX - 2) % 7
        );
        let mut draws = [0u64].into_iter();
        assert_eq!(sample_below(7, || draws.next().unwrap()), 0);
    }

    /// The binary choice points use the draw as "index 1 is yes". The values of seed 0 are fixed, so a change of the stream
    /// or of the meaning of a draw shows up here.
    #[test]
    fn the_binary_points_follow_the_stream() {
        let mut s = SeededScheduler::with_seed(0);
        let deferred: Vec<bool> = (0..16).map(|_| s.defer_operation()).collect();
        let mut s = SeededScheduler::with_seed(0);
        let spurious: Vec<bool> = (0..16).map(|_| s.spurious_wake()).collect();
        assert_eq!(deferred, spurious, "one stream, one draw each");
        let expected = [0u8, 1, 0, 0, 1, 1, 0, 0, 0, 0, 0, 1, 1, 0, 0, 1];
        assert_eq!(deferred, expected.map(|b| b == 1));
    }
}
