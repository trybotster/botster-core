//! The real operating-system edges of Core (plan 2.3, 2.6): the code that machines never contain.
//!
//! No compatibility promise; use `botster-core`. This crate is exempt from the machine-crate lints (plan 2.3c).
//! Modules are added by the packages that need them. The first is [`lock`].

pub mod entropy;
pub mod lock;
pub mod process;
pub mod storage;
