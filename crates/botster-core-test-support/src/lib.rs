//! Reusable test support for consumers of a specific `botster-core` version.
//!
//! Downstream crates should use this crate as a dev-dependency when they need
//! fixtures or conformance helpers tied to the matching core contract release.

pub mod assertions;
pub mod fake;
pub mod fixtures;
