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
//! - `oracle_encode` calls the pinned encoder with explicit modes (R-84); it reads no session model, only the stored size.
//! - `oracle_modes` reads the modes of O_raw, the same terminal that `oracle_screen` reads (plan 23ax: one source of consumed
//!   output; the retained `OracleHandle` feed is not wired).
//! - `oracle_state` reads the modes of O_raw and the last title and cwd that the replay of O_raw reported (R-84 (5)).
//! - `oracle_screen_payload` loads the payload of a route's `screen` frame in a fresh oracle terminal. It has no session.

use crate::controls::{parse, session_row, ControlRegistry};
use crate::harness::TestkitHarness;
use botster_core_conformance::ControlError;
use botster_core_contract::prelude::{ColorProfile, Op, SessionId, Size};
use botster_route_codec::prelude::{hex_decode, hex_encode, HexBytes};
use botster_terminal_ghostty::{
    encode_focus_with_modes, encode_key_with_modes, encode_mouse_with_modes,
    paste_frame_with_modes, EncodeError, History, KeyInput, ModeFlags, MouseInput, Terminal,
    TerminalEvent,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeSet;

/// The controls of this module, on both tiers. The real harness (the `slow` feature) lists them.
#[cfg_attr(not(feature = "slow"), allow(dead_code))]
pub(crate) const CONTROLS: [&str; 6] = [
    "oracle_encode",
    "oracle_modes",
    "oracle_query_reply",
    "oracle_screen",
    "oracle_screen_payload",
    "oracle_state",
];

/// The name of the refusal of condition 3.
pub(crate) const SIZE_OR_PROFILE_CHANGED: &str = "size_or_profile_changed";

pub(crate) fn register_controls(registry: &mut ControlRegistry) {
    registry.register("oracle_encode", testkit_encode);
    registry.register("oracle_modes", testkit_modes);
    registry.register("oracle_query_reply", testkit_query_reply);
    registry.register("oracle_screen", testkit_screen);
    registry.register("oracle_screen_payload", testkit_screen_payload);
    registry.register("oracle_state", testkit_state);
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

/// `oracle_encode {session, kind, input, modes?}` (steward ruling R-84): the bytes that the pinned libghostty encoder gives
/// for one input with explicit modes, as a hex string. Core's write path encodes with the same functions (R-7), so the
/// testkit adds no encoding: it parses the input with the contract's types and calls the binding.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Encode {
    pub(crate) session: SessionId,
    kind: EncodeKind,
    input: Value,
    #[serde(default)]
    modes: Option<serde_json::Map<String, Value>>,
}

/// The input kinds of `oracle_encode`, each with its `input` form.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum EncodeKind {
    /// `input` is a `KeyInput`.
    Key,
    /// `input` is `{mouse: MouseInput}`.
    Mouse,
    /// `input` is `{focused}`.
    Focus,
    /// `input` is `{text}`.
    Paste,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MouseArg {
    mouse: MouseInput,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FocusArg {
    focused: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PasteArg {
    text: String,
}

impl Encode {
    /// The encoded bytes as a hex string, with `size` (the session's stored size) for a mouse event (R-84):
    /// - `NotReported`, and a focus event with focus reporting off, give `""`: the PTY gets nothing;
    /// - `Unsupported{what}` is refused `{reason: "unsupported", what}`, which a transcript matches with `expect_refused`;
    /// - a paste with bracketed paste off is the text alone (Core IN-8).
    ///
    /// Without `modes` (note (1): the modes of the session's model) the call is `unsupported_control`: no transcript does
    /// this, and the testkit has no session model for it here.
    pub(crate) fn encode(&self, size: &Size) -> Result<Value, ControlError> {
        let partial = self
            .modes
            .as_ref()
            .ok_or_else(|| ControlError::UnsupportedFor("oracle_encode_without_modes".into()))?;
        let modes = modes_over_fresh(size, partial)?;
        let bytes = match self.kind {
            EncodeKind::Key => {
                let key: KeyInput = input_of(&self.input, "key")?;
                // `repeat` is the host's count of events (Core IN-9), not part of one event's encoding.
                if key.repeat.is_some() {
                    return Err(ControlError::Bad(
                        "oracle_encode: a key input has no `repeat`".into(),
                    ));
                }
                reported(encode_key_with_modes(&modes, &key))?
            }
            EncodeKind::Mouse => {
                let MouseArg { mouse } = input_of(&self.input, "mouse")?;
                reported(encode_mouse_with_modes(&modes, size, &mouse))?
            }
            EncodeKind::Focus => {
                let FocusArg { focused } = input_of(&self.input, "focus")?;
                encode_focus_with_modes(&modes, focused).unwrap_or_default()
            }
            EncodeKind::Paste => {
                let PasteArg { text } = input_of(&self.input, "paste")?;
                match paste_frame_with_modes(&modes) {
                    Some((start, end)) => [start, text.into_bytes(), end].concat(),
                    None => text.into_bytes(),
                }
            }
        };
        Ok(Value::String(hex_encode(&bytes)))
    }
}

/// R-84 (1), as the steward corrected it: a partial `modes` object is laid over the modes of a fresh terminal of the pinned
/// libghostty, never over the session's modes. The base comes from the binding at run time, never from a literal: which
/// modes a new terminal has on (`dec_1035` and `dec_1036`, for example) is terminal semantics (R-7). `other_modes` merges
/// key by key, so a partial `other_modes` keeps the fresh terminal's other entries. A key that is not a mode, an
/// `other_modes` key that a fresh terminal does not report, or a value of the wrong type, is `Bad`.
fn modes_over_fresh(
    size: &Size,
    partial: &serde_json::Map<String, Value>,
) -> Result<ModeFlags, ControlError> {
    let bad = |what: String| ControlError::Bad(format!("oracle_encode: {what}"));
    let fresh = Terminal::new(size, History::Off)
        .map_err(|e| bad(format!("no fresh terminal: {e:?}")))?
        .modes();
    let Value::Object(mut all) =
        serde_json::to_value(fresh).map_err(|e| bad(format!("the fresh modes: {e}")))?
    else {
        return Err(bad("the fresh modes are not an object".into()));
    };
    for (name, value) in partial {
        match (all.get_mut(name), value) {
            (None, _) => return Err(bad(format!("`{name}` is not a mode"))),
            (Some(Value::Object(base)), Value::Object(over)) => {
                for (other, value) in over {
                    if !base.contains_key(other) {
                        return Err(bad(format!("`{other}` is not one of the `{name}`")));
                    }
                    base.insert(other.clone(), value.clone());
                }
            }
            (Some(slot), value) => *slot = value.clone(),
        }
    }
    serde_json::from_value(Value::Object(all)).map_err(|e| bad(format!("the modes: {e}")))
}

/// The `input` of `kind`, in the contract's form.
fn input_of<T: serde::de::DeserializeOwned>(input: &Value, kind: &str) -> Result<T, ControlError> {
    serde_json::from_value(input.clone())
        .map_err(|e| ControlError::Bad(format!("oracle_encode: the {kind} input: {e}")))
}

/// R-84 (2): the bytes of a reported event, `[]` for `NotReported`, and the refusal of `Unsupported{what}`.
fn reported(encoded: Result<Vec<u8>, EncodeError>) -> Result<Vec<u8>, ControlError> {
    match encoded {
        Ok(bytes) => Ok(bytes),
        Err(EncodeError::NotReported) => Ok(Vec::new()),
        Err(EncodeError::Unsupported(what)) => Err(ControlError::Refused(
            json!({"reason": "unsupported", "what": what}),
        )),
    }
}

/// `oracle_modes {session}`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SessionArg {
    pub(crate) session: SessionId,
}

/// `{flags}` (note (1) of the controls doc): the `ModeFlags` of an oracle terminal, with cursor visibility (Core ST-4).
pub(crate) fn modes(terminal: &Terminal) -> Value {
    json!({ "flags": terminal.modes() })
}

/// `{modes, title?, cwd?}` (R-84 (5)): the modes of O_raw (as `oracle_modes`), and the last `Title` and `Cwd` events that
/// the replay of O_raw reported. The oracle keeps the last event itself, as Core's host and worker do; it never reads
/// libghostty's getters, which give `""` both for no title and for an empty title. So an empty title that the program set
/// is `""`, and a field is absent only when no event came (the `Option` shape of `TerminalState`).
///
/// A replay that dropped events (the binding's buffer was full) is `Bad`: the last title or cwd can be among them.
pub(crate) fn state(terminal: &mut Terminal) -> Result<Value, ControlError> {
    let drained = terminal.drain_events();
    if drained.dropped > 0 {
        return Err(ControlError::Bad(format!(
            "oracle_state: the replay dropped {} events",
            drained.dropped
        )));
    }
    let mut state = json!({ "modes": terminal.modes() });
    for event in drained.events {
        match event {
            TerminalEvent::Title(title) => state["title"] = Value::String(title),
            TerminalEvent::Cwd(cwd) => state["cwd"] = Value::String(cwd),
            // The other events say nothing about the title or the cwd.
            _ => {}
        }
    }
    Ok(state)
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
        ControlError::Bad(format!(
            "oracle_screen_payload: the payload does not load: {e:?}"
        ))
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

/// `oracle_encode` on the testkit: the size of the session's stored row.
fn testkit_encode(
    harness: &mut TestkitHarness,
    handle: &str,
    args: &Value,
) -> Result<Value, ControlError> {
    let args: Encode = parse(args)?;
    let row = session_row(harness, handle, &args.session)?;
    args.encode(&row.request.size)
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
    screen(&testkit_raw(harness, handle, &session)?, history)
}

/// `oracle_modes` on the testkit: the modes of O_raw, as `oracle_screen` reads it.
fn testkit_modes(
    harness: &mut TestkitHarness,
    handle: &str,
    args: &Value,
) -> Result<Value, ControlError> {
    let SessionArg { session } = parse(args)?;
    Ok(modes(&testkit_raw(harness, handle, &session)?))
}

/// `oracle_state` on the testkit: see [`state`].
fn testkit_state(
    harness: &mut TestkitHarness,
    handle: &str,
    args: &Value,
) -> Result<Value, ControlError> {
    let SessionArg { session } = parse(args)?;
    state(&mut testkit_raw(harness, handle, &session)?)
}

/// O_raw of `session` on the testkit: the model log of the session's worker (R-67), after condition 3.
fn testkit_raw(
    harness: &mut TestkitHarness,
    handle: &str,
    session: &SessionId,
) -> Result<Terminal, ControlError> {
    harness
        .captures_of(handle)
        .ok_or_else(|| ControlError::Bad(format!("the handle '{handle}' is not open")))?
        .check(session)?;
    let row = session_row(harness, handle, session)?;
    let worker = row.worker.ok_or_else(|| {
        ControlError::Bad(format!("the session {} has no worker process", session.0))
    })?;
    harness
        .workers()
        .model_log(worker.identity())
        .map_err(ControlError::Bad)?
        .raw(session, row.request.color_profile.as_ref())
}

#[cfg(test)]
pub(crate) mod tests;
