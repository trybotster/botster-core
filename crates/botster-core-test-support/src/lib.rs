//! Reusable, version-coupled test support for a specific `botster-core` release.
//!
//! Downstream crates should use this crate as a dev-dependency when they need
//! fixtures, fakes, or conformance helpers tied to the matching core contract
//! release.

pub mod assertions;
pub mod bounded_wait;
#[cfg(feature = "local-runtime")]
pub mod conformance;
pub mod diagnostics;
pub mod fake;
#[cfg(unix)]
pub mod fixture_gate;
pub mod fixtures;
pub mod real_worker;
pub mod route_observer;
pub mod terminal_adapter;
