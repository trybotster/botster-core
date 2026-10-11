//! The oracle controls of both tiers (`docs/core-testkit-controls.md`): `oracle_query_reply` (Core EV-8), `oracle_screen`
//! (Core ST-2, A2-1) and `oracle_screen_payload` (Core OU-9, DP-3). Each expected value comes from a libghostty oracle,
//! never from the subject (R-7). This module holds the argument forms and the oracle reads; each tier supplies only its
//! session lookup (the testkit's handlers are below, the real tier's are in `real/oracle.rs`).
//!
//! - `oracle_query_reply` asks a fresh terminal of the session's current size ([`crate::oracle::oracle_query_reply`]). It
//!   replays no history, so a `Resize` or a `SetColorProfile` does not change it: the profile is its argument.
//! - `oracle_screen` reads O_raw, the anchor of `oracle_resume`: on the testkit the output that the worker's model
//!   consumed, from the model log (R-67); on the real tier the session's known output history (R-62). It replays the
//!   history, so condition 3 ([`Changed`]) holds for it.
//! - `oracle_screen_payload` loads the payload of a route's `screen` frame in a fresh oracle terminal. It has no session.

use crate::controls::{parse, session_row, ControlRegistry};
use crate::harness::TestkitHarness;
use botster_core_conformance::ControlError;
use botster_core_contract::prelude::{ColorProfile, Op, SessionId, Size};
use botster_route_codec::prelude::{hex_decode, HexBytes};
use botster_terminal_ghostty::{History, Terminal};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeSet;

/// The controls of this module, on both tiers.
pub(crate) const CONTROLS: [&str; 3] = [
    "oracle_query_reply",
    "oracle_screen",
    "oracle_screen_payload",
];

/// The name of the refusal of condition 3.
pub(crate) const SIZE_OR_PROFILE_CHANGED: &str = "size_or_profile_changed";

pub(crate) fn register_controls(registry: &mut ControlRegistry) {
    registry.register("oracle_query_reply", testkit_query_reply);
    registry.register("oracle_screen", testkit_screen);
    registry.register("oracle_screen_payload", testkit_screen_payload);
}

/// Condition 3 (R-62): the sessions that a `Resize` or a `SetColorProfile` reached. O_raw replays the history with the size
/// and the profile of the create request, so a control that replays the history of such a session gives
/// `unsupported_control` `size_or_profile_changed`, never a value. A replay with the changes needs a steward ruling.
///
/// Each tier records at the point where its host begins the op (the testkit's `TestkitCore`, the real tier's capture
/// layer), before Core admits it. The record stays correct when a resize reaches the worker, because the op still enters
/// there.
#[derive(Debug, Clone, Default)]
pub(crate) struct Changed(BTreeSet<SessionId>);

impl Changed {
    /// The host begins `op`.
    pub(crate) fn begun(&mut self, op: &Op) {
        if let Op::Resize { session, .. } | Op::SetColorProfile { session, .. } = op {
            self.0.insert(session.clone());
        }
    }

    /// `Ok` unless a `Resize` or a `SetColorProfile` reached `session`.
    pub(crate) fn check(&self, session: &SessionId) -> Result<(), ControlError> {
        if self.0.contains(session) {
            return Err(ControlError::UnsupportedFor(SIZE_OR_PROFILE_CHANGED.into()));
        }
        Ok(())
    }
}

/// `oracle_query_reply {session, request_hex, prefix_hex?, profile?}`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct QueryReply {
    pub(crate) session: SessionId,
    request_hex: String,
    #[serde(default)]
    prefix_hex: Option<String>,
    #[serde(default)]
    profile: Option<ColorProfile>,
}

impl QueryReply {
    /// `{answerable, reply_hex?}`: what a fresh terminal of `size` answers to the request after the prefix, with the profile
    /// when there is one.
    pub(crate) fn reply(&self, size: &Size) -> Result<Value, ControlError> {
        let bytes = |hex: &str, what: &str| {
            hex_decode(hex).map_err(|e| ControlError::Bad(format!("the {what}: {e}")))
        };
        let request = bytes(&self.request_hex, "request_hex")?;
        let prefix = match &self.prefix_hex {
            Some(hex) => bytes(hex, "prefix_hex")?,
            None => Vec::new(),
        };
        crate::oracle::oracle_query_reply(size, &request, &prefix, self.profile.as_ref())
            .map_err(|e| ControlError::Bad(format!("oracle_query_reply: {e:?}")))
    }
}

/// `oracle_screen {session, history}`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Screen {
    pub(crate) session: SessionId,
    pub(crate) history: bool,
}

/// `{text, rows, cols}` of an oracle terminal: the scrollback and the visible screen with `history`, else the visible screen.
pub(crate) fn screen(terminal: &Terminal, history: bool) -> Result<Value, ControlError> {
    let text = terminal
        .screen_text(history)
        .map_err(|e| ControlError::Bad(format!("the oracle's screen: {e:?}")))?
        .text;
    Ok(json!({"text": text, "rows": terminal.rows(), "cols": terminal.cols()}))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ScreenPayload {
    payload: HexBytes,
}

/// `oracle_screen_payload {payload}` → `{text, rows, cols}` without history: a fresh oracle terminal loads the payload of a
/// route's `screen` frame (DP-3: the worker's native snapshot). A payload that does not load is `Bad`.
pub(crate) fn screen_payload(args: &Value) -> Result<Value, ControlError> {
    let ScreenPayload { payload } = parse(args)?;
    let terminal = Terminal::from_snapshot(&payload.0, History::Off, None).map_err(|e| {
        ControlError::Bad(format!("oracle_screen_payload: the payload does not load: {e:?}"))
    })?;
    screen(&terminal, false)
}

/// `oracle_screen_payload` on the testkit: the same read as on the real tier.
fn testkit_screen_payload(
    _: &mut TestkitHarness,
    _: &str,
    args: &Value,
) -> Result<Value, ControlError> {
    screen_payload(args)
}

/// `oracle_query_reply` on the testkit: the size of the session's stored row.
fn testkit_query_reply(
    harness: &mut TestkitHarness,
    handle: &str,
    args: &Value,
) -> Result<Value, ControlError> {
    let args: QueryReply = parse(args)?;
    let row = session_row(harness, handle, &args.session)?;
    args.reply(&row.request.size)
}

/// `oracle_screen` on the testkit: O_raw from the model log of the session's worker (R-67), after condition 3.
fn testkit_screen(
    harness: &mut TestkitHarness,
    handle: &str,
    args: &Value,
) -> Result<Value, ControlError> {
    let Screen { session, history } = parse(args)?;
    harness
        .captures_of(handle)
        .ok_or_else(|| ControlError::Bad(format!("the handle '{handle}' is not open")))?
        .check(&session)?;
    let row = session_row(harness, handle, &session)?;
    let worker = row.worker.ok_or_else(|| {
        ControlError::Bad(format!("the session {} has no worker process", session.0))
    })?;
    let raw = harness
        .workers()
        .model_log(worker.identity())
        .map_err(ControlError::Bad)?
        .raw(&session, row.request.color_profile.as_ref())?;
    screen(&raw, history)
}

#[cfg(test)]
mod tests;
