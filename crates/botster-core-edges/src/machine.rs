//! The machine interface (plan 2.1).

use std::time::Instant;

/// A sans-IO state machine: it performs no I/O, reads no clock, starts no thread and draws no random number.
///
/// A driver feeds it inputs with the time it chose ([`Machine::handle`]), drains its actions ([`Machine::poll_action`]), and
/// sleeps until its deadline ([`Machine::next_deadline`]). The same machine code runs under the real driver and under the
/// testkit driver (plan 2.1).
///
/// Clause: Core TM-1, Core TM-3 (the host passes `now`; a machine reports its next deadline).
pub trait Machine {
    /// A byte read, a readiness, a child exit, a timer, a host call.
    type Input;
    /// A byte write, a spawn, a signal, a storage write, an event for the host.
    type Action;

    /// Applies one input at `now`. The machine keeps the actions that it causes.
    fn handle(&mut self, now: Instant, input: Self::Input);

    /// Takes the next action, oldest first, or `None` when the machine has none.
    fn poll_action(&mut self) -> Option<Self::Action>;

    /// The earliest instant at which the machine needs `handle` called again with a timer input, or `None`.
    fn next_deadline(&self) -> Option<Instant>;
}
