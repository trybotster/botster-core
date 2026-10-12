//! Shared conformance engine: the transcript model, matchers, order constructs, seeds, results and report (design sections 5 and 6.1; ledger prefixes of every suite).
//!
//! This crate holds no contract type. A runner crate supplies a [`StepDriver`] for the suite steps that the engine does not own.
//!
//! **One `#[test]` per id (design 6.1).** A runner crate has a `build.rs` that lists the ids of its embedded transcripts and writes
//! [`testgen::render_test_macro`] (macro name `conformance_tests`) to `$OUT_DIR`. The crate includes that file, so a Phase 1 repo calls
//! `conformance_tests!(make_subject)` once and gets one test per id. `tests/toy.rs` runs this pattern end to end.

// Failure values carry whole JSON documents for the report. They occur on a cold path, so boxing them would only complicate the driver API.
#![allow(clippy::result_large_err)]

pub mod citation;
pub mod client;
pub mod driver;
pub mod load;
pub mod matcher;
pub mod order;
pub mod report;
pub mod result;
pub mod run;
pub mod seeds;
pub mod stimulus;
pub mod template;
pub mod testgen;
pub mod transcript;
pub mod vectors;

pub use client::{client_step_schema_json, parse_client_steps, ClientStep};
pub use driver::{input_log_0d, input_log_0d_defect, Deadline, Poll, StepDriver, StepError};
pub use load::{load_dir, load_one, parse_transcript, LoadError};
pub use matcher::{
    bytes_value, is_variable, substitute, Bindings, Direction, FrameSchema, MatchFailure,
    NoSchemas, SchemaRegistry, SchemaResolver,
};
pub use order::{order_kind, run_step, run_steps, Limits, StepFailure};
pub use report::Report;
pub use result::Outcome;
pub use run::{run_one_test, run_transcript, DriverFactory};
pub use seeds::{driver_rng, SeedSet, Selection};
pub use transcript::{
    transcript_schema_json, Contract, FakeMode, Requires, Subject, Transcript, TranscriptKind,
};
