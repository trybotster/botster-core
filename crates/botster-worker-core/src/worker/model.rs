//! The terminal model of the session: libghostty through `botster-terminal-ghostty` (BUILD.md: libghostty owns every
//! terminal semantic; the worker parses nothing, tracks no mode and encodes no input itself).
//!
//! **Steps (plan 2.4, revision 19; E2-3, EV-8).** The PTY output that the worker read and did not feed yet is fed in model
//! steps: one completed `vt_write_until_query` that consumes a prefix of it. The rest is kept, in order. After each step the
//! worker drains the model's events, posts `ModesChanged` only when the final flags differ from the last posted value, and
//! hands a query (or the model's own PTY writes) to the admission point as a reply. Input is admitted only between steps,
//! because a step runs inside one input of the machine.
//!
//! The query offer to a route and the held shadow reply (EV-8) belong to the route package (P4b); until a route exists, a
//! query is answered with the model's shadow reply.
//!
//! **Capture (ST-6).** `CaptureSnapshot` is the model's GHOSTSNP snapshot (`botster_terminal_ghostty::snapshot_format`), sent
//! as one page. The host mints the capture and keeps its pages.

use super::Worker;
use botster_core_contract::prelude::*;
use botster_core_link::msg::{Observation, WorkerMsg};
use botster_route_codec::prelude::HexBytes;
use botster_terminal_ghostty::{
    ClipboardLocation, EncodeError, History, ModeFlags, Terminal, TerminalEvent,
};

/// The model of a launched session.
pub(super) struct Model {
    pub(super) term: Terminal,
    /// PTY output that the worker read and the model has not consumed, in order.
    unfed: Vec<u8>,
    /// The flags of the last `ModesChanged` (or of the launch).
    posted_modes: ModeFlags,
}

impl std::fmt::Debug for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Model")
            .field("unfed", &self.unfed.len())
            .finish_non_exhaustive()
    }
}

impl Model {
    /// A fresh model of `size` with history (ST-2). Its clipboard limit is `clipboard_bytes` (A14-2: the model's own
    /// decode limit is the same value).
    pub(super) fn new(size: &Size, clipboard_bytes: u64) -> Option<Model> {
        let mut term = Terminal::new(size, History::On).ok()?;
        term.set_clipboard_limit(usize::try_from(clipboard_bytes).unwrap_or(usize::MAX));
        let posted_modes = term.modes();
        Some(Model {
            term,
            unfed: Vec::new(),
            posted_modes,
        })
    }

    pub(super) fn modes(&self) -> ModeFlags {
        self.term.modes()
    }
}

/// The snapshot formats of the model (ST-6, `Launched.formats`): the binding's GHOSTSNP format.
pub(super) fn snapshot_formats() -> Vec<SnapshotFormat> {
    vec![botster_terminal_ghostty::snapshot_format()]
}

/// A text cut to at most `max` bytes at a UTF-8 character boundary, and whether it was cut (A2-4).
fn bounded(text: String, max: usize) -> (String, bool) {
    if text.len() <= max {
        return (text, false);
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    (text[..end].to_string(), true)
}

/// The `selection` of a clipboard write (A13-1): a non-empty OSC 52 selection string as the program wrote it; otherwise
/// the model's location, in OSC 52's own letters. `None` for a location that this binding does not know: A13-1 names no
/// letter for it.
fn clipboard_selection(selection: Option<String>, location: ClipboardLocation) -> Option<String> {
    if let Some(selection) = selection.filter(|s| !s.is_empty()) {
        return Some(selection);
    }
    match location {
        ClipboardLocation::Standard => Some("c".into()),
        ClipboardLocation::Primary => Some("p".into()),
        ClipboardLocation::Selection => Some("s".into()),
        ClipboardLocation::Other(_) => None,
    }
}

impl Worker {
    /// Keeps `bytes` after the output that waits. False while the spawn's answer is out: the output waits, and it is fed
    /// after `Launched`, so no observation comes before it (the link's rule) and the launch carries a fresh model's state.
    fn hold_output(&mut self, bytes: &[u8]) -> bool {
        let Some(model) = self.model.as_mut() else {
            return false;
        };
        model.unfed.extend_from_slice(bytes);
        self.payload != super::PayloadState::Spawning
    }

    /// PTY output reaches the model: it is fed in steps (rule 1: the unconsumed suffix is kept, in order). Each step that
    /// consumes bytes advances `model_rev` (ST-1). True when a step did. Empty `bytes` steps the output that waits.
    pub(super) fn feed_model(&mut self, bytes: &[u8]) -> bool {
        if !self.hold_output(bytes) {
            return false;
        }
        let mut stepped = false;
        while let Some(model) = self.model.as_mut() {
            if model.unfed.is_empty() {
                break;
            }
            let step = match model.term.vt_write_until_query(&model.unfed) {
                Ok(step) => step,
                // The library refused the step: its bytes cannot be applied, and they are not offered again.
                Err(_) => {
                    model.unfed.clear();
                    break;
                }
            };
            if step.consumed == 0 && step.query.is_none() {
                // The model keeps an ESC that may start the ST of an unfinished string: it waits for more bytes.
                break;
            }
            model.unfed.drain(..step.consumed.min(model.unfed.len()));
            stepped = true;
            // ST-1: output is a read-visible change.
            self.model_rev = ModelRev(self.model_rev.0.wrapping_add(1));
            self.after_step();
            if let Some(query) = step.query {
                // EV-8 (P4b offers it to a route first): with no route, the shadow reply answers it.
                self.enqueue_reply(query.shadow_reply);
            }
        }
        stepped
    }

    /// After a step: the model's events go to the host in their order, a lost event is never silent (EV-2), the model's own
    /// PTY writes are replies, and `ModesChanged` is posted only when the final flags changed (E2-3).
    fn after_step(&mut self) {
        let Some(model) = self.model.as_mut() else {
            return;
        };
        let drained = model.term.drain_events();
        let modes = model.term.modes();
        let modes_changed = modes != model.posted_modes;
        if modes_changed {
            model.posted_modes = modes.clone();
        }
        let model_rev = self.model_rev;
        let bound = usize::try_from(self.limits.clipboard_bytes).unwrap_or(usize::MAX);
        let mut lost = drained.dropped_kinds;
        for event in drained.events {
            let observation = match event {
                TerminalEvent::Title(title) => Observation::Title { title, model_rev },
                TerminalEvent::Cwd(cwd) => Observation::Cwd { cwd, model_rev },
                TerminalEvent::Bell => Observation::Bell,
                TerminalEvent::PromptMark { mark, exit_code } => {
                    Observation::PromptMark { mark, exit_code }
                }
                TerminalEvent::Notification {
                    source,
                    title,
                    body,
                } => {
                    // A2-4: title and body are each bounded by clipboard_bytes, cut at a character boundary.
                    let (title, title_cut) = match title {
                        Some(t) => {
                            let (t, cut) = bounded(t, bound);
                            (Some(t), cut)
                        }
                        None => (None, false),
                    };
                    let (body, body_cut) = bounded(body, bound);
                    Observation::Notification {
                        source,
                        title,
                        body,
                        truncated: title_cut || body_cut,
                    }
                }
                TerminalEvent::ClipboardWrite(write) => {
                    let Some(selection) = clipboard_selection(write.selection, write.location)
                    else {
                        // A location that A13-1 names no letter for: the write is not silent (EV-2), it is lost.
                        lost.insert(LostKind::ClipboardWrite);
                        continue;
                    };
                    // EV-3, A13-1, A14-2: a write that the model did not keep is surfaced with no contents.
                    let contents = if write.too_large {
                        None
                    } else {
                        write.contents.map(|entries| {
                            entries
                                .into_iter()
                                .map(|entry| ClipboardContent {
                                    mime: entry.mime,
                                    bytes: HexBytes(entry.bytes),
                                })
                                .collect()
                        })
                    };
                    let reason = contents.is_none().then_some(ClipboardReason::TooLarge);
                    Observation::ClipboardWrite {
                        selection,
                        contents,
                        total_bytes: write.total_bytes,
                        reason,
                    }
                }
            };
            self.report(&WorkerMsg::Observed { observation });
        }
        for kind in lost {
            self.report(&WorkerMsg::Observed {
                observation: Observation::Lost {
                    kind,
                    tap_dropped_bytes: 0,
                },
            });
        }
        self.enqueue_reply(drained.pty_writes);
        if modes_changed {
            self.report(&WorkerMsg::Observed {
                observation: Observation::Modes {
                    flags: modes,
                    model_rev,
                },
            });
        }
    }

    /// `ReadScreen` (ST-2): the plain text, and whether the history is unavailable.
    pub(super) fn read_screen(&self, history: bool) -> OpResult {
        let Some(model) = self.model.as_ref() else {
            return no_model(ErrorCode::WrongState);
        };
        match model.term.screen_text(history) {
            Ok(screen) => OpResult::Ok(OpOutput::Screen(Screen {
                text: screen.text,
                rows: u32::from(model.term.rows()),
                cols: u32::from(model.term.cols()),
                history_unavailable: screen.history_unavailable,
                model_rev: self.model_rev,
            })),
            Err(_) => OpResult::Err(CoreError::new(
                ErrorCode::Internal,
                "the terminal model could not read its screen",
            )),
        }
    }

    /// `ReadCursor` (ST-3): zero-based cells; the row's text with every trailing empty cell and space removed; the text of
    /// cells `[0, col)` untrimmed, an empty cell read as a space.
    pub(super) fn read_cursor(&self) -> OpResult {
        let Some(model) = self.model.as_ref() else {
            return no_model(ErrorCode::CursorReadUnsupported);
        };
        let cursor = model.term.cursor();
        let cells = model.term.row_cells(cursor.row).unwrap_or_default();
        let cell_text = |cell: &String| {
            if cell.is_empty() {
                " ".to_string()
            } else {
                cell.clone()
            }
        };
        let col = usize::try_from(cursor.col).unwrap_or(usize::MAX);
        let text_before_cursor: String = cells.iter().take(col).map(cell_text).collect();
        let row_text: String = cells.iter().map(cell_text).collect();
        let row_text = row_text.trim_end_matches(' ').to_string();
        OpResult::Ok(OpOutput::Cursor(Cursor {
            row: cursor.row,
            col: cursor.col,
            visible: cursor.visible,
            row_text,
            text_before_cursor,
            model_rev: self.model_rev,
        }))
    }

    /// `ReadModeFlags` (ST-1).
    pub(super) fn read_modes(&self) -> OpResult {
        let Some(model) = self.model.as_ref() else {
            return no_model(ErrorCode::WrongState);
        };
        OpResult::Ok(OpOutput::Modes(Modes {
            flags: model.modes(),
            model_rev: self.model_rev,
        }))
    }

    /// `CaptureSnapshot` (ST-6): the model's snapshot at this point, as one page, then the capture. The host mints the
    /// `CaptureId` and keeps the page. A snapshot over `max_snapshot_bytes` is `SnapshotTooLarge`, and no page is sent (a
    /// page over the bound would not fit the link's frame bound either).
    pub(super) fn capture(&mut self, req: u64) -> OpResult {
        let Some(model) = self.model.as_ref() else {
            return no_model(ErrorCode::WrongState);
        };
        let bytes = match model.term.snapshot() {
            Ok(bytes) => bytes,
            Err(_) => {
                return OpResult::Err(CoreError::new(
                    ErrorCode::Internal,
                    "the terminal model could not make its snapshot",
                ))
            }
        };
        let total_bytes = bytes.len() as u64;
        if total_bytes > self.limits.max_snapshot_bytes {
            return OpResult::Err(CoreError::new(
                ErrorCode::SnapshotTooLarge,
                format!("{total_bytes} bytes are over max_snapshot_bytes"),
            ));
        }
        self.report(&WorkerMsg::Pages {
            req,
            pages: vec![Page {
                index: 0,
                bytes: HexBytes(bytes),
                last: true,
            }],
        });
        OpResult::Ok(OpOutput::Capture(Capture {
            // The host mints the id (`WorkerMsg::Pages`).
            capture: CaptureId(0),
            page_count: 1,
            total_bytes,
            model_rev: self.model_rev,
        }))
    }
}

fn no_model(code: ErrorCode) -> OpResult {
    OpResult::Err(CoreError::new(code, "the session has no terminal model"))
}

/// The encoding of a semantic event at the start of its transaction (IN-9), with the model's modes now. `Err` is the
/// certain zero of IN-9: no encoding (`Unsupported{what}`), or a mode that does not report it (`NotReported`).
pub(super) fn encode(
    model: &Model,
    payload: &InputPayload,
) -> Result<Option<(Vec<u8>, usize, usize)>, NotWrittenReason> {
    let map = |e: EncodeError| match e {
        EncodeError::Unsupported(what) => NotWrittenReason::Unsupported { what },
        EncodeError::NotReported => NotWrittenReason::NotReported,
    };
    let whole = |bytes: Vec<u8>| {
        let len = bytes.len();
        Some((bytes, 0, len))
    };
    Ok(match payload {
        InputPayload::Paste {
            bytes,
            require_bracketed,
        } => match model.term.paste_frame() {
            // IN-8: wrapped when bracketed paste is on now; bare when it is off; refused with exact zero when required.
            Some((start, end)) => {
                let mut all = start.clone();
                all.extend_from_slice(&bytes.0);
                all.extend_from_slice(&end);
                Some((all, start.len(), bytes.0.len()))
            }
            None if *require_bracketed => return Err(NotWrittenReason::ModePreconditionFailed),
            None => whole(bytes.0.clone()),
        },
        InputPayload::Key(key) => {
            // IN-9: `repeat` is that many contiguous events, each encoded with the same modes.
            let times = usize::from(key.repeat.unwrap_or(1).max(1));
            let mut one = key.clone();
            one.repeat = None;
            let bytes = model.term.encode_key(&one).map_err(map)?;
            whole(bytes.repeat(times))
        }
        InputPayload::Mouse(mouse) => whole(model.term.encode_mouse(mouse).map_err(map)?),
        InputPayload::Focus { focused } => match model.term.encode_focus(*focused) {
            Some(bytes) => whole(bytes),
            None => return Err(NotWrittenReason::NotReported),
        },
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A13-1: a non-empty OSC 52 selection string as the program wrote it; otherwise the location in OSC 52's letters; no
    /// letter for a location that the binding does not know.
    #[test]
    fn the_clipboard_selection_is_the_string_or_the_locations_letter() {
        assert_eq!(
            clipboard_selection(Some("s0".into()), ClipboardLocation::Standard),
            Some("s0".into())
        );
        for (location, letter) in [
            (ClipboardLocation::Standard, "c"),
            (ClipboardLocation::Primary, "p"),
            (ClipboardLocation::Selection, "s"),
        ] {
            assert_eq!(clipboard_selection(None, location), Some(letter.into()));
            assert_eq!(
                clipboard_selection(Some(String::new()), location),
                Some(letter.into()),
                "an empty string is no selection"
            );
        }
        assert_eq!(clipboard_selection(None, ClipboardLocation::Other(9)), None);
    }

    /// A2-4: a text over the bound is cut at a character boundary at or below it, and flagged; one at the bound is not.
    #[test]
    fn a_bounded_text_is_cut_at_a_character_boundary() {
        assert_eq!(bounded("abcd".into(), 4), ("abcd".into(), false));
        assert_eq!(bounded("abcde".into(), 4), ("abcd".into(), true));
        // "é" is two bytes: a cut inside it moves down to its start.
        assert_eq!(bounded("aé".into(), 2), ("a".into(), true));
    }
}
