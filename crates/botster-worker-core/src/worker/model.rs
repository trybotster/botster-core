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

use super::Worker;
use botster_core_contract::prelude::*;
use botster_core_link::msg::{Observation, WorkerMsg};
use botster_terminal_ghostty::{EncodeError, History, ModeFlags, Terminal, TerminalEvent};

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
    /// A fresh model of `size` with history (ST-2).
    pub(super) fn new(size: &Size) -> Option<Model> {
        let term = Terminal::new(size, History::On).ok()?;
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

    pub(super) fn title(&self) -> Option<String> {
        Some(self.term.title()).filter(|t| !t.is_empty())
    }

    pub(super) fn cwd(&self) -> Option<String> {
        Some(self.term.cwd()).filter(|c| !c.is_empty())
    }
}

/// The snapshot formats of the model (ST-6, `Launched.formats`). The binding does not name its format's name and version
/// yet (asked of P2); the worker invents none, so it names no format until then.
pub(super) fn snapshot_formats() -> Vec<SnapshotFormat> {
    Vec::new()
}

/// The first `model_rev` of an instance (ST-1: tokens from different instances never compare equal). The token is
/// 64 bits and an instance id is any text, so the worker cannot make a range per instance that is certain to be
/// disjoint; it starts each instance at the FNV-1a hash of its host epoch and id. Two instances then share a token only
/// when their starts are within the number of changes that one of them made: about `changes / 2^64`. Inside one
/// instance the token repeats only after 2^64 changes.
pub(super) fn first_rev(instance: &InstanceId, host_epoch: u64) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    host_epoch
        .to_le_bytes()
        .iter()
        .chain(instance.0.as_bytes())
        .fold(OFFSET, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(PRIME)
        })
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

impl Worker {
    /// PTY output reaches the model: it is fed in steps (rule 1: the unconsumed suffix is kept, in order).
    pub(super) fn feed_model(&mut self, bytes: Vec<u8>) {
        let Some(model) = self.model.as_mut() else {
            return;
        };
        model.unfed.extend_from_slice(&bytes);
        let mut stepped = false;
        loop {
            let Some(model) = self.model.as_mut() else {
                return;
            };
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
            self.input.model_rev = self.input.model_rev.wrapping_add(1);
            self.after_step();
            if let Some(query) = step.query {
                // EV-8 (P4b offers it to a route first): with no route, the shadow reply answers it.
                if !query.shadow_reply.is_empty() {
                    self.enqueue_reply(query.shadow_reply);
                }
            }
        }
        if stepped {
            let model_rev = ModelRev(self.input.model_rev);
            self.report(&WorkerMsg::Observed {
                observation: Observation::Output { model_rev },
            });
        }
    }

    /// After a step: the model's events go to the host in their order, a lost event is never silent (EV-2), the model's own
    /// PTY writes are replies, and `ModesChanged` is posted only when the final flags changed (E2-3).
    pub(super) fn after_step(&mut self) {
        let Some(model) = self.model.as_mut() else {
            return;
        };
        let drained = model.term.drain_events();
        let modes = model.term.modes();
        let modes_changed = modes != model.posted_modes;
        if modes_changed {
            model.posted_modes = modes.clone();
        }
        let model_rev = ModelRev(self.input.model_rev);
        let bound = usize::try_from(self.limits.clipboard_bytes).unwrap_or(usize::MAX);
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
                TerminalEvent::ClipboardWrite { selection, bytes } => {
                    // EV-3: a write over clipboard_bytes is surfaced, never silent, with no bytes.
                    let total_bytes = bytes.len() as u64;
                    if total_bytes > self.limits.clipboard_bytes {
                        Observation::ClipboardWrite {
                            selection,
                            bytes: None,
                            total_bytes,
                            reason: Some(ClipboardReason::TooLarge),
                        }
                    } else {
                        Observation::ClipboardWrite {
                            selection,
                            bytes: Some(bytes),
                            total_bytes,
                            reason: None,
                        }
                    }
                }
                // A plain `vt_write` is never used, so a query never arrives as an event; one that does is answered
                // by its shadow reply, as a step's query is.
                TerminalEvent::Query(query) => {
                    if !query.shadow_reply.is_empty() {
                        self.enqueue_reply(query.shadow_reply);
                    }
                    continue;
                }
            };
            self.report(&WorkerMsg::Observed { observation });
        }
        for kind in drained.dropped_kinds {
            self.report(&WorkerMsg::Observed {
                observation: Observation::Lost {
                    kind,
                    tap_dropped_bytes: 0,
                },
            });
        }
        if !drained.pty_writes.is_empty() {
            self.enqueue_reply(drained.pty_writes);
        }
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
                model_rev: ModelRev(self.input.model_rev),
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
            model_rev: ModelRev(self.input.model_rev),
        }))
    }

    /// `ReadModeFlags` (ST-1).
    pub(super) fn read_modes(&self) -> OpResult {
        let Some(model) = self.model.as_ref() else {
            return no_model(ErrorCode::WrongState);
        };
        OpResult::Ok(OpOutput::Modes(Modes {
            flags: model.modes(),
            model_rev: ModelRev(self.input.model_rev),
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
