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
    a6_2_deferred_set, normalize_instances, normalize_op, run_script_events, CoreDriver,
    CoreSchemas,
};
pub use harness::{
    from_make, ControlError, CoreHarness, DataDirRef, MakeHarness, OpenSpec, RouteClient,
    WorkerBuild, WorkerRef,
};

use include_dir::{include_dir, Dir};

/// The Core transcripts (`conformance/core`), embedded.
pub static CORE_TRANSCRIPTS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../conformance/core");

/// Builds the driver for one seed from a harness factory.
pub fn driver_for(harness: Box<dyn CoreHarness>) -> Box<dyn botster_conformance::StepDriver> {
    Box::new(CoreDriver::new(harness))
}

include!(concat!(env!("OUT_DIR"), "/core_tests.rs"));
