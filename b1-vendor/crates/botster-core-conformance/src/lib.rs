//! The Core conformance runner over `CoreApi` (Core v1.17 with A2, A3 and A4; ledger prefix `core`; design 5.5, 6.2).
//!
//! The suite is data: one JSON transcript per test id in `conformance/core/`, embedded here. A Phase 1 repo calls
//! [`conformance_tests!`] once with a factory that builds its [`CoreHarness`] for a seed, and gets one `#[test]` per id:
//!
//! ```ignore
//! // The full suite against the real Core (needed for acceptance).
//! botster_core_conformance::conformance_tests!(|_seed| Box::new(RealCoreHarness::new()), botster_core_conformance::CoreSchemas);
//! // Core section 11 faithfulness (R-1.5): the same suite against FakeCore.
//! mod fake {
//!     botster_core_conformance::conformance_tests!(|seed| Box::new(botster_core_conformance::fake::FakeCoreHarness::new(seed)), botster_core_conformance::CoreSchemas);
//! }
//! ```
//!
//! [`from_make`] wraps a constructor of Core section 11's form. It runs only the transcripts that use one default handle; every other
//! transcript gives `unsupported_control`, which is not a pass.

// Failure values carry whole JSON documents for the report, as in `botster-conformance`. They occur on a cold path.
#![allow(clippy::result_large_err)]

mod driver;
#[cfg(feature = "fake")]
pub mod fake;
mod harness;

pub use driver::{
    a20_1_proven_ids_are_active, a20_1_testkit_proven, a6_2_deferred_set, normalize_instances,
    normalize_op, normalize_op_env, route_ended, run_script_events, strict_woken_wait, CoreDriver,
    CoreSchemas, ROUTE_CLOSED_HEALTHY, ROUTE_ENDED, TESTKIT_PROVEN,
};
pub use harness::{
    from_make, ControlError, CoreHarness, DataDirRef, MakeHarness, OpenSpec, RouteClient,
    WorkerBuild, WorkerRef,
};

use include_dir::{include_dir, Dir};

/// The Core transcripts (`conformance/core`), embedded.
pub static CORE_TRANSCRIPTS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../conformance/core");

/// F of steward ruling R-74: the length of the write that a held-write transcript holds. It is twice the largest measured
/// capacity of a non-canonical PTY with no reader (Linux 6.12: 20480 bytes; macOS: 1022 bytes), and at most the default
/// `max_paste_bytes`. A held-write transcript writes F bytes (`repeat`) and fences with `pty_queued` in `[1, F]`. F is not a
/// kernel count, and no transcript asserts one (R-43 item 7). The test `held_write` keeps the transcripts on this value.
pub const HELD_WRITE_FILL_BYTES: u64 = 40960;

/// Builds the driver for one seed from a harness factory.
pub fn driver_for(harness: Box<dyn CoreHarness>) -> Box<dyn botster_conformance::StepDriver> {
    Box::new(CoreDriver::new(harness))
}

include!(concat!(env!("OUT_DIR"), "/core_tests.rs"));
