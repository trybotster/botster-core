//! Real session tests of the prebuilt worker binary.
#![cfg(feature = "slow")]

const DRIVER_OBSERVER: Option<&str> = None;

#[path = "common/session.rs"]
mod session;
