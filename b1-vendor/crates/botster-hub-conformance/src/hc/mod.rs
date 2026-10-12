//! The Hub→client conformance runner (design 6.4; ledger prefix `hc`, and the codec ids whose subject is the Hub).
//!
//! The driver plays a client over a [`HubSubject`] (the real Hub, HC A5-1). The player plays the Hub for a [`ClientSubject`] (HC A5-2). The suite is data: `conformance/hc/` (and the `hub` subject transcripts of
//! `conformance/codec/`). A Phase 1 repo calls [`hc_conformance_tests!`] once with a subject factory and gets one `#[test]` per id.

mod driver;
mod player;
mod schemas;
mod subject;

pub use driver::HubDriver;
pub use player::{ClientPlayer, ClientSubject};
pub use schemas::HcSchemas;
pub use subject::{ConnectSpec, HubRead, HubSubject, Pump};

use include_dir::{include_dir, Dir};
use serde_json::Value;

/// The HC transcripts (`conformance/hc`), embedded.
pub static HC_TRANSCRIPTS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../conformance/hc");

/// The named fixtures of `conformance/hc/fixtures` (design 5.1, `setup.fixture`).
pub fn fixture(name: &str) -> Option<Value> {
    let file = HC_TRANSCRIPTS.get_file(format!("fixtures/{name}.json"))?;
    serde_json::from_slice(file.contents()).ok()
}

/// Builds the driver for a subject.
pub fn driver_for<S: HubSubject + 'static>(subject: S) -> Box<dyn botster_conformance::StepDriver> {
    Box::new(HubDriver::new(subject, fixture))
}

/// Builds the player for a client subject (`subject: client` transcripts).
pub fn player_for<C: ClientSubject + 'static>(
    client: C,
) -> Box<dyn botster_conformance::StepDriver> {
    Box::new(ClientPlayer::new(client))
}
