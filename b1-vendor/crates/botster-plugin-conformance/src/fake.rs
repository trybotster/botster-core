//! The adapters that wrap the fakes as `HpSubject`s (design section 4: the fake never depends on the runner).

use crate::subject::{Backpressured, HpSubject, Unsupported};
use botster_plugin_fakes::{FakeEndpoint, FakeHub, FakePlugin};
use serde_json::Value;

/// A fake endpoint as the subject of a transcript.
pub struct FakeSubject<E: FakeEndpoint>(pub E);

impl<E: FakeEndpoint> HpSubject for FakeSubject<E> {
    fn is_fake(&self) -> bool {
        true
    }

    fn has_feature(&self, _name: &str) -> bool {
        false
    }

    fn has_control(&self, op: &str) -> bool {
        self.0.controls().contains(&op)
    }

    fn configure(&mut self, setup: &Value) -> Result<(), String> {
        self.0.configure(setup)
    }

    fn write(&mut self, conn: &str, bytes: &[u8]) -> Result<(), Backpressured> {
        self.0.write(conn, bytes).map_err(|_| Backpressured)
    }

    fn read(&mut self, conn: &str) -> Option<Vec<u8>> {
        self.0.read(conn)
    }

    fn conns(&self) -> Vec<String> {
        self.0.conns()
    }

    fn closed(&self, conn: &str) -> Option<String> {
        self.0.closed(conn).map(String::from)
    }

    fn close_channel(&mut self, conn: &str) {
        self.0.close_channel(conn);
    }

    fn advance_clock(&mut self, ms: u64) {
        self.0.advance_clock(ms);
    }

    fn control(&mut self, op: &Value) -> Result<Value, Unsupported> {
        self.0.control(op).map_err(|u| Unsupported(u.0))
    }
}

/// The plugin-side FakeHub for one seed: the subject of `plugin_hub` transcripts (design 7.1).
pub fn hub(seed: u64) -> Box<dyn HpSubject> {
    Box::new(FakeSubject(FakeHub::new(seed)))
}

/// FakePlugin for one seed: the subject of `plugin_host` transcripts (design 7.1).
pub fn host(seed: u64) -> Box<dyn HpSubject> {
    Box::new(FakeSubject(FakePlugin::new(seed)))
}
