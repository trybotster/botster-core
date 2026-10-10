//! The session worker machine of Core (plan 2.2): [`worker::Worker`], with the worker protocol constants.
//!
//! No compatibility promise; use `botster-core`.
//!
//! `xtask` reads [`WORKER_PROTOCOL`] and [`WORKER_FEATURES_BY_PROTOCOL`] to validate `conformance/core-deferred.toml`
//! (plan section 5, rule 4). They live in this crate because the plan names it, and P3 grows the worker around them.

use botster_core_contract::prelude::Feature;

pub mod drain;
pub mod worker;

pub use drain::Drain;
pub use worker::{
    Action, CandidateId, DescriptorId, Input, PayloadSpec, SpawnFailure, Worker, WorkerConfig,
};

/// The worker protocol number `T` of this Core. The first v1 worker protocol number is 1.
///
/// Clause: Core AD-4, Core A6-2.
pub const WORKER_PROTOCOL: u8 = 1;

/// The per-worker features that each worker protocol adds or keeps, by protocol number, ascending.
///
/// Protocol 1 has `focus_report` (Core A6-2: "`focus_report` is part of protocol 1"). P3 completes the protocol 1 row with the
/// per-worker features of A2-6 and A3-3 that the worker implements; a later protocol repeats every feature that it keeps.
///
/// Clause: Core AD-4, Core A2-6, Core A6-2.
pub const WORKER_FEATURES_BY_PROTOCOL: &[(u8, &[Feature])] = &[(1, &[Feature::FocusReport])];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_protocol_is_one_and_the_table_has_a_row_for_every_protocol() {
        assert_eq!(WORKER_PROTOCOL, 1);
        let numbers: Vec<u8> = WORKER_FEATURES_BY_PROTOCOL
            .iter()
            .map(|(p, _)| *p)
            .collect();
        assert_eq!(numbers, (1..=WORKER_PROTOCOL).collect::<Vec<u8>>());
    }
}
