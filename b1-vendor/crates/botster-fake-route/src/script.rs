//! The session script of the fake worker: the data that a real worker gets from its terminal library and its PTY program.
//!
//! The fake has no terminal. It carries opaque script bytes only (steward rulings R-5.1, R-5.2, R-7). A test author keeps the script
//! consistent with the bytes that the test writes. Nothing here is an expected terminal value: a real subject ignores the script,
//! and transcripts check script-fed values by relation to the script (`written_by_op`), never by a hand-written literal.

use botster_route_codec::prelude::*;
use serde::{Deserialize, Deserializer};
use serde_json::Value;

/// A partial `ModeFlags` (Codec A.6): the fields it names replace those of the modes it is applied to.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ModePatch(Value);

impl ModePatch {
    /// The modes after the patch. A field that does not fit `ModeFlags` is a script error.
    pub fn apply(&self, base: &ModeFlags) -> Result<ModeFlags, String> {
        let mut value = serde_json::to_value(base).map_err(|e| e.to_string())?;
        if let (Some(into), Some(patch)) = (value.as_object_mut(), self.0.as_object()) {
            into.extend(patch.clone());
        }
        serde_json::from_value(value).map_err(|e| e.to_string())
    }
}

/// Opaque bytes in a script, written as a hex string.
fn hex<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
    let text = String::deserialize(d)?;
    hex_decode(&text).map_err(serde::de::Error::custom)
}

fn hex_list<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<Vec<u8>>, D::Error> {
    Vec::<String>::deserialize(d)?
        .iter()
        .map(|t| hex_decode(t).map_err(serde::de::Error::custom))
        .collect()
}

/// The complete outcome of one semantic input, as the terminal library decides it (Core IN-9, Codec TS-5b, A.5). The fake never decides
/// it from modes. In JSON it is a hex string (the bytes written), `{"not_written": "not_reported"}` (the modes do not report the input:
/// nothing is written and no frame is sent) or `{"not_written": {"unsupported": "<what>"}}` (a refusal `unsupported { what }`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Bytes(Vec<u8>),
    NotReported,
    Unsupported(String),
}

impl Default for Outcome {
    /// An input that the script does not name is not reported.
    fn default() -> Self {
        Outcome::NotReported
    }
}

impl<'de> Deserialize<'de> for Outcome {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let bad = |m: &str| serde::de::Error::custom(m.to_string());
        match Value::deserialize(d)? {
            Value::String(text) => hex_decode(&text).map(Outcome::Bytes).map_err(|e| bad(&e)),
            Value::Object(o) => match o.get("not_written") {
                Some(Value::String(r)) if r == "not_reported" => Ok(Outcome::NotReported),
                Some(Value::Object(u)) => match u.get("unsupported").and_then(Value::as_str) {
                    Some(what) => Ok(Outcome::Unsupported(what.to_string())),
                    None => Err(bad("not_written takes unsupported: <what>")),
                },
                _ => Err(bad("an outcome is a hex string or {not_written: ..}")),
            },
            _ => Err(bad("an outcome is a hex string or {not_written: ..}")),
        }
    }
}

/// What each semantic input writes to the PTY, and the complete bytes of a paste, a file path paste and a typed reply.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Writes {
    pub key: Outcome,
    pub mouse: Outcome,
    pub focus_in: Outcome,
    /// Also the outcome of the focus-out that a route end writes (Codec TS-10).
    pub focus_out: Outcome,
    /// The complete bytes that a committed paste writes (Codec TS-5d, TS-5b). The fake writes exactly these, whatever the body was.
    #[serde(deserialize_with = "hex")]
    pub paste: Vec<u8>,
    /// The complete bytes that a committed file writes to the PTY: the path paste (Codec TS-5f). The fake chooses the path for the
    /// `input_done` frame and the file, and writes exactly these bytes.
    #[serde(deserialize_with = "hex")]
    pub file: Vec<u8>,
    /// Written for a typed `reply_terminal_query` variant (Codec A.5).
    #[serde(deserialize_with = "hex")]
    pub reply: Vec<u8>,
}

/// The explicit worst-case admission size of each semantic input, paste and file, in bytes (Codec TS-5b, TS-5f, Core IN-9). The fake
/// compares these numbers with the limits. It never computes an encoding or a size.
#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Admission {
    pub key: u64,
    pub mouse: u64,
    pub focus: u64,
    /// The worst-case size of a committed paste write, for the input queue (Codec TS-5, TS-7).
    pub paste: u64,
    /// The size of the path paste of a committed file. The fake compares it with `max_paste_bytes` before it creates the file, and
    /// counts it in the input queue (Codec TS-5f).
    pub file: u64,
}

/// What the terminal library and the PTY program supply to the worker.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct SessionScript {
    /// The `screen` payload of every baseline (Codec TS-4, A.4 `screen`). Opaque.
    #[serde(deserialize_with = "hex")]
    pub screen: Vec<u8>,
    /// The `history` pages of a `full` baseline (Codec TS-4, A.4 `history`). Opaque.
    #[serde(deserialize_with = "hex_list")]
    pub history_pages: Vec<Vec<u8>>,
    /// The modes that the route reports (Codec TS-4d, A.6) and that the paste and focus rules read.
    pub modes: ModePatch,
    pub writes: Writes,
    pub admission: Admission,
}

/// A notification that completes inside a chunk of PTY output (Codec TS-4c). The script names it, because the fake parses nothing.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Notification {
    /// The offset of the byte that completes it. Absent means the last byte of the chunk.
    #[serde(default)]
    pub at: Option<usize>,
    pub source: NotificationSource,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub body: String,
}

/// The script data of one `pty_output` control (`scripted`). A real subject ignores it and derives the same facts from the bytes.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct ScriptedOutput {
    /// The modes after this chunk. Present means that the chunk changes them (Codec TS-4d).
    pub modes: Option<ModePatch>,
    pub notifications: Vec<Notification>,
}
