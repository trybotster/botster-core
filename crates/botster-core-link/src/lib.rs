//! The private control-link wire between the host and a worker or a guardian (plan section 3).
//!
//! No compatibility promise; use `botster-core`.
//!
//! This crate has the framing and the hello as types and codecs. It performs no I/O: a driver reads bytes and gives them to
//! a [`frame::FrameDecoder`]. The messages that follow the hello belong to the packages that own them.

pub mod frame;
pub mod hello;
pub mod launch;
pub mod msg;
pub mod proof;
