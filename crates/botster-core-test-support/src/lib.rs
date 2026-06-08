//! Reusable, version-coupled test support for a specific `botster-core` release.
//!
//! Downstream crates should use this crate as a dev-dependency when they need
//! fixtures, fakes, or conformance helpers tied to the matching core contract
//! release.

pub mod assertions;
#[cfg(feature = "local-runtime")]
pub mod conformance;
pub mod fake;
pub mod fixtures;
pub mod ui_conformance;
