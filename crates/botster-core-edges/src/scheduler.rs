//! The scheduler edge and the production policy (plan 2.4).
//!
//! A driver asks the scheduler at every choice point of A5-2. The production policy never varies anything: control is
//! handled first (the driver lists control inputs first among the ready work), sessions are visited round-robin, and a bound
//! is used in full. The seeded policy lives in the testkit.

/// A choice point. The first group is the list of A5-2; the last two are the file completion of plan 2.3b and the
/// round-robin visit of plan 2.4.
///
/// Clause: Core A5-2, Core OR-1, Core OR-3, Core DP-5b.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ChoicePoint {
    /// Which ready work runs first in a `pump`.
    ReadyWork,
    /// Whether the progress of an operation is deferred to a later `pump`. Index 0 is "not deferred".
    OperationDeferral,
    /// How many pumps pass between `Start` and `Running`.
    PumpsToRunning,
    /// How many pumps pass between `Stop` and `Exited`.
    PumpsToExited,
    /// The bound on the work of one `pump`.
    PumpBound,
    /// The size of the non-empty part of the queue that `poll_events` returns.
    PollBatch,
    /// How many updates of a class K key happen before the host polls.
    ClassKUpdates,
    /// The size of one write of the program edge.
    ProgramWriteSize,
    /// The size of one read of the route-transport edge.
    RouteReadSize,
    /// Whether a wake is spurious. Index 0 is "no".
    SpuriousWake,
    /// The place of a due deadline's event among the other events of its `pump`.
    DeadlineEventPlace,
    /// When a file completion is delivered.
    FileCompletion,
    /// Which session the host engine visits next.
    Session,
}

/// The scheduling policy of a driver.
pub trait Scheduler {
    /// Chooses one of `candidates` (at least 1) and returns its index. A policy that does not vary the order returns 0, so
    /// the caller lists the candidate that it would run without variation first.
    fn pick(&mut self, point: ChoicePoint, candidates: usize) -> usize;

    /// Chooses a count from `1..=max` (a work bound, a batch size or a chunk size). Returns 0 only when `max` is 0.
    fn bound(&mut self, point: ChoicePoint, max: usize) -> usize;
}

/// The production policy: no variation, and a round-robin visit of sessions.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Production {
    next_session: usize,
}

impl Production {
    pub fn new() -> Production {
        Production::default()
    }
}

impl Scheduler for Production {
    fn pick(&mut self, point: ChoicePoint, candidates: usize) -> usize {
        if candidates == 0 {
            return 0;
        }
        match point {
            ChoicePoint::Session => {
                let index = self.next_session % candidates;
                self.next_session = index + 1;
                index
            }
            _ => 0,
        }
    }

    fn bound(&mut self, _point: ChoicePoint, max: usize) -> usize {
        max
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Plan 2.4: "round-robin over sessions".
    #[test]
    fn sessions_are_visited_round_robin() {
        let mut policy = Production::new();
        let visited: Vec<usize> = (0..7)
            .map(|_| policy.pick(ChoicePoint::Session, 3))
            .collect();
        assert_eq!(visited, [0, 1, 2, 0, 1, 2, 0]);
    }

    /// A session that ends leaves the cursor above the new count: the visit wraps and stays in range.
    #[test]
    fn the_visit_stays_in_range_when_the_session_count_shrinks() {
        let mut policy = Production::new();
        let visited: Vec<(usize, usize)> = [5, 5, 5, 2, 2, 1]
            .into_iter()
            .map(|count| (count, policy.pick(ChoicePoint::Session, count)))
            .collect();
        assert_eq!(visited, [(5, 0), (5, 1), (5, 2), (2, 1), (2, 0), (1, 0)]);
    }

    /// Plan 2.4: control first. The driver lists control inputs first, so the production policy always picks index 0.
    #[test]
    fn every_other_point_takes_the_first_candidate() {
        let mut policy = Production::new();
        for point in [
            ChoicePoint::ReadyWork,
            ChoicePoint::OperationDeferral,
            ChoicePoint::PumpsToRunning,
            ChoicePoint::PumpsToExited,
            ChoicePoint::DeadlineEventPlace,
            ChoicePoint::SpuriousWake,
            ChoicePoint::FileCompletion,
        ] {
            assert_eq!(policy.pick(point, 4), 0, "{point:?}");
        }
        // The session cursor is not moved by another point.
        assert_eq!(policy.pick(ChoicePoint::Session, 4), 0);
    }

    /// Plan 2.4: the pump bound is the 9B bound, used in full.
    #[test]
    fn a_bound_is_used_in_full() {
        let mut policy = Production::new();
        for point in [
            ChoicePoint::PumpBound,
            ChoicePoint::PollBatch,
            ChoicePoint::ClassKUpdates,
            ChoicePoint::ProgramWriteSize,
            ChoicePoint::RouteReadSize,
        ] {
            assert_eq!(policy.bound(point, 4096), 4096, "{point:?}");
        }
        assert_eq!(policy.bound(ChoicePoint::PumpBound, 0), 0);
    }

    #[test]
    fn no_candidate_gives_index_zero() {
        assert_eq!(Production::new().pick(ChoicePoint::Session, 0), 0);
    }
}
