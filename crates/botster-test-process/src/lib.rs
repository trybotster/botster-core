//! The one owner of real-process test code in botster-core (brief-p6-test-process). A dev-dependency only: no production
//! crate depends on it.
//!
//! - [`OwnedChild`]: a child that the test starts and owns; bounded status; its drop ends and reaps it (and, in group mode,
//!   every member of its group).
//! - [`run_to_completion`]: a short-lived tool run to its exit with its output, as `Command::output` but bounded.
//! - [`Guard`]: the anchor of a process that production starts and reaps; its drop ends the process's group. One design for
//!   every guard (the double-forked anchor, `anchor.rs`).
//! - [`Bounded`], [`first_line`], [`eof`]: reads of pipes, FIFOs and sockets, bounded by deadlines.
//! - [`Blocker`]: the blocked fixture child (`/bin/cat` on a FIFO), which waits without CPU and ends when the test is gone.
//! - [`alone`]: a test that changes state of its whole process runs alone, in a new process of its test binary.
//! - [`Deadline`] and [`CLEANUP`]: the only timer. Every wait blocks on a real event and fails at a deadline derived from a
//!   named limit.
//!
//! Outside this crate, test code does not wait for a child, read a child's pipe without a deadline, or sleep: `cargo xtask
//! ci` checks it. Design and prior art: `DESIGN.md`.

pub mod alone;
pub mod anchor;
pub mod built_in;
mod child;
mod deadline;
mod fixture;
pub mod platform;
mod read;
pub mod rounds;

pub use anchor::Guard;
pub use child::{reap_within, run_to_completion, OwnedChild};
pub use deadline::{Deadline, CLEANUP};
pub use fixture::{quoted, Blocker};
pub use read::{eof, first_line, Bounded, ReadError};

/// A failure of an owner's cleanup: it fails the test, or it is reported when the test already panics (a second panic would
/// abort the test process before the other owners clean up).
pub(crate) fn fail(report: String) {
    if std::thread::panicking() {
        eprintln!("{report}");
    } else {
        panic!("{report}");
    }
}

/// The compile-time manifest directory of this crate, for the build-root check of the test binaries (bug catalog B13,
/// `built_in`).
#[doc(hidden)]
pub const BUILT_IN: &str = env!("CARGO_MANIFEST_DIR");

#[cfg(test)]
crate::built_in_test!();
