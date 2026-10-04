//! Independent terminal observations for Core ST-2, ST-3, ST-4, A2-4 and IN-9 (R-7).
//!
//! The driver feeds this handle only the output that the worker model consumed, at each completed model step.
//! The handle survives the move of the program edge into the worker. Observations never pump the subject.

use botster_core_contract::prelude::{KeyInput, MouseInput, Size};
use botster_route_codec::prelude::ModeFlags;
use botster_terminal_ghostty::{
    encode_key_with_modes, encode_mouse_with_modes, EncodeError, Error, History, Terminal,
    TerminalEvent,
};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

struct State {
    terminal: Terminal,
    notification: Option<Value>,
}

/// A separate libghostty terminal behind a driver handle.
#[derive(Clone)]
pub struct OracleHandle(Arc<Mutex<State>>);

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
    pub fn consumed_output(&self, bytes: &[u8]) -> Result<(), Error> {
        let mut state = self.lock();
        state.terminal.vt_write(bytes);
        let drained = state.terminal.drain_events();
        if drained.dropped != 0 {
            return Err(Error::OutOfMemory);
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
}
