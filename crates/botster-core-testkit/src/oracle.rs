//! Independent terminal observations for Core ST-2, ST-3, ST-4, A2-4 and IN-9 (R-7).
//!
//! The driver feeds this handle only the output that the worker model consumed, at each completed model step.
//! The handle survives the move of the program edge into the worker. Observations never pump the subject.

use botster_core_contract::prelude::{ColorProfile, KeyInput, MouseInput, Size};
use botster_route_codec::prelude::ModeFlags;
use botster_terminal_ghostty::{
    encode_focus_with_modes, encode_key_with_modes, encode_mouse_with_modes,
    paste_frame_with_modes, EncodeError, Error, History, Terminal, TerminalEvent,
};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

struct State {
    terminal: Terminal,
    notification: Option<Value>,
}

/// A failure while the driver feeds an oracle step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OracleError {
    Library(Error),
    EventsLost { count: u64 },
    ConsumptionMismatch { offered: usize, consumed: usize },
}

/// A separate libghostty terminal behind a driver handle.
#[derive(Clone)]
pub struct OracleHandle(Arc<Mutex<State>>);

/// `oracle_query_reply`: ask a fresh native shadow terminal, independently of the subject (EV-8).
/// The adapter resolves the session size and decodes request and prefix with the contract codec.
pub fn oracle_query_reply(
    size: &Size,
    request: &[u8],
    prefix: &[u8],
    profile: Option<&ColorProfile>,
) -> Result<Value, Error> {
    let mut terminal = Terminal::new(size, History::Off)?;
    terminal.vt_write(prefix);
    terminal.drain_events();
    if let Some(profile) = profile {
        terminal.set_color_profile(profile)?;
    }
    let step = terminal.vt_write_until_query(request)?;
    let reply = step
        .query
        .map(|query| query.shadow_reply)
        .unwrap_or_default();
    if reply.is_empty() {
        Ok(json!({"answerable": false}))
    } else {
        Ok(
            json!({"answerable": true, "reply_hex": botster_route_codec::prelude::hex_encode(&reply)}),
        )
    }
}

impl OracleHandle {
    pub fn new(size: &Size, history: History) -> Result<Self, Error> {
        Ok(Self(Arc::new(Mutex::new(State {
            terminal: Terminal::new(size, history)?,
            notification: None,
        }))))
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Feed one completed model step. The bytes are stimulus, never an expected terminal value.
    pub fn consumed_output(&self, bytes: &[u8]) -> Result<(), OracleError> {
        let mut state = self.lock();
        let step = state
            .terminal
            .vt_write_until_query(bytes)
            .map_err(OracleError::Library)?;
        let drained = state.terminal.drain_events();
        if drained.dropped != 0 {
            return Err(OracleError::EventsLost {
                count: drained.dropped,
            });
        }
        if step.consumed != bytes.len() {
            return Err(OracleError::ConsumptionMismatch {
                offered: bytes.len(),
                consumed: step.consumed,
            });
        }
        for event in drained.events {
            if let TerminalEvent::Notification {
                source,
                title,
                body,
            } = event
            {
                state.notification = Some(json!({"source": source, "title": title, "body": body}));
            }
        }
        Ok(())
    }

    /// Apply the size that the worker model accepted.
    pub fn resize(&self, size: &Size) -> Result<(), Error> {
        self.lock().terminal.resize(size)
    }

    /// `oracle_state`: read the oracle, without the subject's cached state.
    pub fn state(&self) -> Value {
        let state = self.lock();
        json!({"modes": state.terminal.modes(), "title": state.terminal.title(), "cwd": state.terminal.cwd()})
    }

    /// `oracle_modes`: read the current flags from libghostty.
    pub fn modes(&self) -> Value {
        json!({"flags": self.lock().terminal.modes()})
    }

    /// `oracle_screen`: read text and dimensions from libghostty.
    pub fn screen(&self, history: bool) -> Result<Value, Error> {
        let state = self.lock();
        Ok(json!({"text": state.terminal.screen_text(history)?.text,
            "rows": state.terminal.rows(), "cols": state.terminal.cols()}))
    }

    /// `oracle_cursor`: preserve empty cells and spaces in both raw strings.
    pub fn cursor(&self) -> Result<Value, Error> {
        let state = self.lock();
        let cursor = state.terminal.cursor();
        let cells = state
            .terminal
            .row_cells(cursor.row)
            .ok_or(Error::InvalidValue)?;
        let before: String = cells
            .iter()
            .take(cursor.col as usize)
            .map(String::as_str)
            .collect();
        Ok(
            json!({"row": cursor.row, "col": cursor.col, "visible": cursor.visible,
            "row_raw": cells.concat(), "before_raw": before}),
        )
    }

    /// `oracle_notification`: return the last full notification. A read does not consume it.
    pub fn notification(&self) -> Option<Value> {
        self.lock().notification.clone()
    }

    /// `oracle_encode` for a key. Without explicit modes, use the oracle's current modes.
    pub fn encode_key(
        &self,
        input: &KeyInput,
        modes: Option<&ModeFlags>,
    ) -> Result<Vec<u8>, EncodeError> {
        match modes {
            Some(modes) => encode_key_with_modes(modes, input),
            None => self.lock().terminal.encode_key(input),
        }
    }

    /// `oracle_encode` for a mouse. Explicit modes use the supplied size.
    pub fn encode_mouse(
        &self,
        input: &MouseInput,
        explicit: Option<(&ModeFlags, &Size)>,
    ) -> Result<Vec<u8>, EncodeError> {
        match explicit {
            Some((modes, size)) => encode_mouse_with_modes(modes, size, input),
            None => self.lock().terminal.encode_mouse(input),
        }
    }

    /// Get a focus report from libghostty. No report is `None`.
    pub fn encode_focus(&self, focused: bool, modes: Option<&ModeFlags>) -> Option<Vec<u8>> {
        match modes {
            Some(modes) => encode_focus_with_modes(modes, focused),
            None => self.lock().terminal.encode_focus(focused),
        }
    }

    /// Get paste markers from libghostty. No markers is `None`.
    pub fn paste_frame(&self, modes: Option<&ModeFlags>) -> Option<(Vec<u8>, Vec<u8>)> {
        match modes {
            Some(modes) => paste_frame_with_modes(modes),
            None => self.lock().terminal.paste_frame(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn size() -> Size {
        Size {
            rows: 3,
            cols: 12,
            cell_px: None,
        }
    }

    #[test]
    fn query_replies_come_from_a_fresh_native_shadow_with_prefix_and_profile() {
        let profile = ColorProfile {
            foreground: botster_core_contract::prelude::Rgb {
                r: 11,
                g: 22,
                b: 33,
            },
            background: botster_core_contract::prelude::Rgb {
                r: 44,
                g: 55,
                b: 66,
            },
            cursor: None,
            palette: None,
        };
        for profile in [None, Some(&profile)] {
            for (prefix, request) in [
                (&b"line\r\ntext"[..], &b"\x1b[6n"[..]),
                (&b""[..], &b"\x1b]10;?\x1b\\"[..]),
                (&b""[..], &b"\x1b]52;;?\x1b\\"[..]),
                (&b""[..], &b"plain"[..]),
            ] {
                let mut reference = Terminal::new(&size(), History::Off).unwrap();
                reference.vt_write(prefix);
                if let Some(profile) = profile {
                    reference.set_color_profile(profile).unwrap();
                }
                let reply = reference
                    .vt_write_until_query(request)
                    .unwrap()
                    .query
                    .map(|query| query.shadow_reply)
                    .unwrap_or_default();
                let actual = oracle_query_reply(&size(), request, prefix, profile).unwrap();
                assert_eq!(actual["answerable"], !reply.is_empty());
                if reply.is_empty() {
                    assert!(actual.get("reply_hex").is_none());
                } else {
                    assert_eq!(
                        actual["reply_hex"],
                        botster_route_codec::prelude::hex_encode(&reply)
                    );
                }
            }
        }
        let mut invalid = size();
        invalid.rows = 0;
        assert!(oracle_query_reply(&invalid, b"", b"", None).is_err());
    }

    #[test]
    fn observations_follow_consumed_output_and_resize() {
        let oracle = OracleHandle::new(&size(), History::On).unwrap();
        let driver = oracle.clone();
        let mut reference = Terminal::new(&size(), History::On).unwrap();
        let bytes = b"wide: \xe7\x95\x8c\x1b[2;4H\x1b[?25l\x1b]2;oracle title\x07\x1b]7;file:///tmp/oracle\x07";
        driver.consumed_output(bytes).unwrap();
        reference.vt_write(bytes);
        assert_eq!(
            oracle.state(),
            json!({"modes": reference.modes(), "title": reference.title(), "cwd": reference.cwd()})
        );
        assert_eq!(oracle.modes(), json!({"flags": reference.modes()}));
        let cursor = reference.cursor();
        let cells = reference.row_cells(cursor.row).unwrap();
        assert_eq!(
            oracle.cursor().unwrap(),
            json!({"row": cursor.row, "col": cursor.col, "visible": cursor.visible,
            "row_raw": cells.concat(), "before_raw": cells[..cursor.col as usize].concat()})
        );
        let resized = Size {
            rows: 4,
            cols: 14,
            cell_px: None,
        };
        driver.resize(&resized).unwrap();
        reference.resize(&resized).unwrap();
        for history in [false, true] {
            assert_eq!(
                oracle.screen(history).unwrap(),
                json!({"text": reference.screen_text(history).unwrap().text,
                "rows": reference.rows(), "cols": reference.cols()})
            );
        }
    }

    #[test]
    fn notification_reads_keep_the_last_full_event() {
        let oracle = OracleHandle::new(&size(), History::Off).unwrap();
        assert_eq!(oracle.notification(), None);
        let mut reference = Terminal::new(&size(), History::Off).unwrap();
        for bytes in [
            b"\x1b]9;first\x07".as_slice(),
            b"\x1b]777;notify;title;second\x07",
        ] {
            oracle.consumed_output(bytes).unwrap();
            reference.vt_write(bytes);
            let event = reference
                .drain_events()
                .events
                .into_iter()
                .find_map(|event| {
                    if let TerminalEvent::Notification {
                        source,
                        title,
                        body,
                    } = event
                    {
                        Some(json!({"source": source, "title": title, "body": body}))
                    } else {
                        None
                    }
                })
                .unwrap();
            assert_eq!(oracle.notification(), Some(event.clone()));
            assert_eq!(oracle.notification(), Some(event));
        }
        oracle.consumed_output(b"plain text").unwrap();
        assert!(oracle.notification().is_some());
    }

    #[test]
    fn invalid_sizes_return_the_library_error() {
        let invalid = Size { rows: 0, ..size() };
        assert!(OracleHandle::new(&invalid, History::Off).is_err());
        assert!(OracleHandle::new(&size(), History::Off)
            .unwrap()
            .resize(&invalid)
            .is_err());
    }

    #[test]
    fn encoders_use_live_modes_or_explicit_modes() {
        use botster_route_codec::prelude::{Key, KeyEvent, MouseAction, MouseButton, NamedKey};
        let oracle = OracleHandle::new(&size(), History::Off).unwrap();
        let mut reference = Terminal::new(&size(), History::Off).unwrap();
        let plain = reference.modes();
        let bytes = b"\x1b[?1h\x1b[?1000h\x1b[?1006h\x1b[?1004h\x1b[?2004h";
        oracle.consumed_output(bytes).unwrap();
        reference.vt_write(bytes);
        let key = KeyInput {
            key: Key::Named(NamedKey("arrow_up".into())),
            mods: vec![],
            shifted_key: None,
            base_layout_key: None,
            event: KeyEvent::Press,
            text: None,
            repeat: None,
        };
        let mouse = MouseInput {
            action: MouseAction::Press,
            button: MouseButton::Left,
            row: 1,
            col: 2,
            x: None,
            y: None,
            mods: vec![],
            notches: None,
        };
        assert_eq!(oracle.encode_key(&key, None), reference.encode_key(&key));
        assert_eq!(
            oracle.encode_key(&key, Some(&plain)),
            encode_key_with_modes(&plain, &key)
        );
        assert_eq!(
            oracle.encode_mouse(&mouse, None),
            reference.encode_mouse(&mouse)
        );
        assert_eq!(
            oracle.encode_mouse(&mouse, Some((&plain, &size()))),
            encode_mouse_with_modes(&plain, &size(), &mouse)
        );
        for focused in [false, true] {
            assert_eq!(
                oracle.encode_focus(focused, None),
                reference.encode_focus(focused)
            );
            assert_eq!(
                oracle.encode_focus(focused, Some(&plain)),
                encode_focus_with_modes(&plain, focused)
            );
        }
        assert_eq!(oracle.paste_frame(None), reference.paste_frame());
        assert_eq!(
            oracle.paste_frame(Some(&plain)),
            paste_frame_with_modes(&plain)
        );
    }

    #[test]
    fn an_incomplete_step_reports_the_library_consumption() {
        let oracle = OracleHandle::new(&size(), History::Off).unwrap();
        let mut reference = Terminal::new(&size(), History::Off).unwrap();
        let bytes = b"\x1b[6ntail";
        let step = reference.vt_write_until_query(bytes).unwrap();
        assert_eq!(
            oracle.consumed_output(bytes),
            Err(OracleError::ConsumptionMismatch {
                offered: bytes.len(),
                consumed: step.consumed
            })
        );
    }

    #[test]
    fn a_lost_notification_is_an_error() {
        let oracle = OracleHandle::new(&size(), History::Off).unwrap();
        let mut reference = Terminal::new(&size(), History::Off).unwrap();
        let bytes = vec![7; botster_terminal_ghostty::MAX_BUFFERED_EVENTS + 1];
        reference.vt_write_until_query(&bytes).unwrap();
        let lost = reference.drain_events().dropped;
        assert!(lost > 0);
        assert_eq!(
            oracle.consumed_output(&bytes),
            Err(OracleError::EventsLost { count: lost })
        );
    }
}
