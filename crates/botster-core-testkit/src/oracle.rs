//! The terminal oracles of the testkit (`docs/core-testkit-controls.md`, steward rule R-7): an independent libghostty
//! terminal, built fresh for each control, replays what reached the session's PTY (the output the worker read and the sizes
//! it set, in order) and answers. The subject's own model is never read: expected terminal values come from libghostty, never
//! from the code under test and never from a transcript.

use crate::program::ModelLogEntry;
use botster_core_conformance::ControlError;
use botster_core_contract::prelude::*;
use botster_route_codec::prelude::{HexBytes, ModeFlags};
use botster_terminal_ghostty::{
    encode_focus_with_modes, encode_key_with_modes, encode_mouse_with_modes,
    paste_frame_with_modes, History, Terminal, TerminalEvent,
};
use serde_json::{json, Value};

fn bad(why: impl Into<String>) -> ControlError {
    ControlError::Bad(why.into())
}

fn parse<T: serde::de::DeserializeOwned>(v: &Value) -> Result<T, ControlError> {
    serde_json::from_value(v.clone()).map_err(|e| bad(format!("input: {e}")))
}

fn size_of(window: &botster_core_edges::edges::WindowSize) -> Size {
    Size {
        rows: u32::from(window.rows),
        cols: u32::from(window.cols),
        cell_px: None,
    }
}

/// The oracle terminal of a session, and the last notification it classified.
pub struct Oracle {
    term: Terminal,
    notification: Option<(NotificationSource, Option<String>, String)>,
}

impl Oracle {
    /// Replays `log` into a fresh terminal of its first size, with the steps the worker uses (each step stops after a query).
    pub fn replay(log: &[ModelLogEntry]) -> Result<Oracle, ControlError> {
        let first = log
            .iter()
            .find_map(|e| match e {
                ModelLogEntry::Size(size) => Some(size_of(size)),
                ModelLogEntry::Output(_) => None,
            })
            .ok_or_else(|| bad("the session's PTY has no size yet"))?;
        let term = Terminal::new(&first, History::On).map_err(|e| bad(format!("{e:?}")))?;
        let mut oracle = Oracle {
            term,
            notification: None,
        };
        let mut unfed: Vec<u8> = Vec::new();
        let mut sized = false;
        for entry in log {
            match entry {
                ModelLogEntry::Size(size) => {
                    if sized {
                        oracle
                            .term
                            .resize(&size_of(size))
                            .map_err(|e| bad(format!("{e:?}")))?;
                    }
                    sized = true;
                }
                ModelLogEntry::Output(bytes) => {
                    unfed.extend_from_slice(bytes);
                    oracle.feed(&mut unfed)?;
                }
            }
        }
        Ok(oracle)
    }

    fn feed(&mut self, unfed: &mut Vec<u8>) -> Result<(), ControlError> {
        while !unfed.is_empty() {
            let step = self
                .term
                .vt_write_until_query(unfed)
                .map_err(|e| bad(format!("{e:?}")))?;
            if step.consumed == 0 && step.query.is_none() {
                break;
            }
            unfed.drain(..step.consumed.min(unfed.len()));
            for event in self.term.drain_events().events {
                if let TerminalEvent::Notification {
                    source,
                    title,
                    body,
                } = event
                {
                    self.notification = Some((source, title, body));
                }
            }
        }
        Ok(())
    }

    /// `oracle_modes`: `{flags}`.
    pub fn modes(&self) -> Value {
        json!({ "flags": self.term.modes() })
    }

    /// `oracle_state`: `{modes, title, cwd}`.
    pub fn state(&self) -> Value {
        let text = |s: String| if s.is_empty() { Value::Null } else { json!(s) };
        json!({
            "modes": self.term.modes(),
            "title": text(self.term.title()),
            "cwd": text(self.term.cwd()),
        })
    }

    /// `oracle_cursor`: `{row, col, visible, row_raw, before_raw}`, untrimmed, an empty cell read as a space.
    pub fn cursor(&self) -> Value {
        let cursor = self.term.cursor();
        let cells: Vec<String> = self
            .term
            .row_cells(cursor.row)
            .unwrap_or_default()
            .into_iter()
            .map(|c| if c.is_empty() { " ".to_string() } else { c })
            .collect();
        let col = usize::try_from(cursor.col).unwrap_or(usize::MAX);
        json!({
            "row": cursor.row,
            "col": cursor.col,
            "visible": cursor.visible,
            "row_raw": cells.concat(),
            "before_raw": cells.iter().take(col).cloned().collect::<String>(),
        })
    }

    /// `oracle_screen`: `{text, rows, cols}`.
    pub fn screen(&self, history: bool) -> Result<Value, ControlError> {
        let screen = self
            .term
            .screen_text(history)
            .map_err(|e| bad(format!("{e:?}")))?;
        Ok(json!({
            "text": screen.text,
            "rows": self.term.rows(),
            "cols": self.term.cols(),
        }))
    }

    /// `oracle_notification`: the last notification with its full text.
    pub fn notification(&self) -> Result<Value, ControlError> {
        let (source, title, body) = self
            .notification
            .clone()
            .ok_or_else(|| bad("the session has no notification"))?;
        let mut out = json!({ "source": source, "body": body });
        if let Some(title) = title {
            out["title"] = json!(title);
        }
        Ok(out)
    }

    /// `oracle_encode`: the bytes of an event (`kind` key, mouse, focus or paste), with `modes` laid over the oracle's flags,
    /// or with the oracle's own state when `modes` is absent. The result is a binary value; no encoding is empty bytes.
    pub fn encode(
        &self,
        kind: &str,
        input: &Value,
        modes: Option<&Value>,
    ) -> Result<Value, ControlError> {
        let flags: Option<ModeFlags> = match modes {
            None => None,
            Some(partial) => {
                let mut base =
                    serde_json::to_value(self.term.modes()).map_err(|e| bad(e.to_string()))?;
                let (Some(base_map), Some(over)) = (base.as_object_mut(), partial.as_object())
                else {
                    return Err(bad("'modes' is an object"));
                };
                for (k, v) in over {
                    base_map.insert(k.clone(), v.clone());
                }
                Some(serde_json::from_value(base).map_err(|e| bad(format!("modes: {e}")))?)
            }
        };
        let bytes: Vec<u8> = match kind {
            "key" => {
                let key: KeyInput = parse(input)?;
                let encoded = match &flags {
                    Some(f) => encode_key_with_modes(f, &key),
                    None => self.term.encode_key(&key),
                };
                encoded.unwrap_or_default()
            }
            "mouse" => {
                let mouse: MouseInput = parse(input.get("mouse").unwrap_or(input))?;
                let encoded = match &flags {
                    Some(f) => {
                        let size = Size {
                            rows: u32::from(self.term.rows()),
                            cols: u32::from(self.term.cols()),
                            cell_px: None,
                        };
                        encode_mouse_with_modes(f, &size, &mouse)
                    }
                    None => self.term.encode_mouse(&mouse),
                };
                encoded.unwrap_or_default()
            }
            "focus" => {
                let focused = input
                    .get("focused")
                    .and_then(Value::as_bool)
                    .ok_or_else(|| bad("focus input needs 'focused'"))?;
                match &flags {
                    Some(f) => encode_focus_with_modes(f, focused),
                    None => self.term.encode_focus(focused),
                }
                .unwrap_or_default()
            }
            "paste" => {
                let text = input
                    .get("text")
                    .and_then(Value::as_str)
                    .ok_or_else(|| bad("paste input needs 'text'"))?;
                let frame = match &flags {
                    Some(f) => paste_frame_with_modes(f),
                    None => self.term.paste_frame(),
                };
                match frame {
                    Some((start, end)) => [start, text.as_bytes().to_vec(), end].concat(),
                    None => text.as_bytes().to_vec(),
                }
            }
            other => return Err(bad(format!("unknown kind {other}"))),
        };
        Ok(json!({ "$bytes_hex": HexBytes(bytes).to_hex() }))
    }
}
