//! The reads of section 6.1 as scripted facts, and captures (Core ST-1 to ST-7, A2-1).
//!
//! FakeCore models no terminal (R-5): the screen, the history and the modes are script data.

use crate::pump::{fail, ok, Step};
use crate::state::*;
use crate::Inner;
use botster_core_contract::prelude::*;
use botster_route_codec::prelude::HexBytes;

impl Sess {
    pub fn new(id: SessionId, instance: InstanceId, request: SpawnRequest) -> Sess {
        Sess {
            id,
            instance,
            state: SessionState::Created,
            labels: request.labels.clone(),
            size: request.size,
            script: crate::script::of(&request).unwrap_or_default(),
            request,
            exit: None,
            protocol: None,
            worker: None,
            payload: Payload::None,
            program_exit_in: None,
            program_exit_started: false,
            control_broken: false,
            children: 0,
            pc: 0,
            input_read: 0,
            seen_modes: Default::default(),
            held: vec![],
            kill_at: None,
            routes: Default::default(),
            reported: (0, 0, 0),
        }
    }

    /// The record of `get` (Core LC-9). `worker_protocol` is absent until Core learned it from the worker's hello (Core AD-4).
    pub fn record(&self) -> SessionRecord {
        SessionRecord {
            id: self.id.clone(),
            state: self.state,
            size: self.size,
            labels: self.labels.clone(),
            exit: self.exit,
            worker_protocol: self.protocol,
            worker_features: self.protocol.map(|_| [Feature::FocusReport].into()),
        }
    }

    /// The cached state of `terminal_state` (Core ST-4): the revisions are as last reported, so they may lag (Core IN-10).
    pub fn terminal_state(&self) -> TerminalState {
        let modes = self
            .worker
            .as_ref()
            .map(|w| w.modes().clone())
            .unwrap_or_default();
        let (client, host, model) = self.reported;
        TerminalState {
            size: self.size,
            modes,
            title: None,
            cwd: None,
            last_output_at: None,
            focused: None,
            model_rev: ModelRev(model),
            input_rev: InputRevs {
                client: InputRev(client),
                host: InputRev(host),
            },
        }
    }
}

impl Inner {
    /// A read completes with the script data of the session (Core A2-1, ST-5: the worker serves its final model after exit).
    pub(crate) fn read_op(&mut self, op: &Op, now: Now) -> Step {
        let (sid, owner) = match op {
            Op::ReadScreen { session, .. }
            | Op::ReadCursor { session }
            | Op::ReadModeFlags { session }
            | Op::ReadFacts { session, .. } => (session, None),
            Op::CaptureSnapshot { session, owner } => (session, Some(owner)),
            _ => return fail(ErrorCode::Internal, "not a read"),
        };
        let Some(s) = self.sessions.get(sid) else {
            return Step::Retry;
        };
        // Core A2-1: a read of a `Created` or `Lost` session is `WrongState`; there is no model.
        if matches!(s.state, SessionState::Created | SessionState::Lost(_)) {
            return fail(ErrorCode::WrongState, "no worker model for this session");
        }
        // Core A2-1: a read that needs the worker fails `WorkerLinkFailed` when the control link is broken.
        if s.control_broken {
            return fail(ErrorCode::WorkerLinkFailed, "the control link is broken");
        }
        let Some(w) = s.worker.as_ref() else {
            return Step::Retry;
        };
        let model_rev = ModelRev(w.model_rev());
        let fake = &s.script.fake;
        let (rows, cols) = w.size();
        match op {
            Op::ReadScreen { history, .. } => {
                let mut text = String::new();
                // Core ST-2: with history unavailable the text is the visible screen and the flag says so.
                let unavailable = *history && fake.history_pages.is_empty();
                if *history {
                    for page in &fake.history_pages {
                        text.push_str(&String::from_utf8_lossy(page));
                    }
                }
                text.push_str(&String::from_utf8_lossy(&fake.screen));
                ok(OpOutput::Screen(Screen {
                    text,
                    rows,
                    cols,
                    history_unavailable: unavailable,
                    model_rev,
                }))
            }
            Op::ReadCursor { .. } => ok(OpOutput::Cursor(Cursor {
                row: 0,
                col: 0,
                visible: w.modes().cursor_visible,
                row_text: String::new(),
                text_before_cursor: String::new(),
                model_rev,
            })),
            Op::ReadModeFlags { .. } => ok(OpOutput::Modes(Modes {
                flags: w.modes().clone(),
                model_rev,
            })),
            Op::ReadFacts { .. } => ok(OpOutput::Facts(Facts {
                model_rev,
                modes: w.modes().clone(),
                size: s.size,
                cursor: FactsCursor {
                    row: 0,
                    col: 0,
                    visible: w.modes().cursor_visible,
                },
                input: InputFacts {
                    client: ClientFacts {
                        rev: InputRev(w.client_rev()),
                        age_ms: 0,
                    },
                    host: HostFacts {
                        rev: InputRev(w.host_rev()),
                    },
                    client_tail: HexBytes(vec![]),
                    tail_complete: true,
                    input_records: vec![],
                    next_cursor: InputCursor(0),
                    gap: false,
                },
            })),
            _ => {
                // Core ST-6: a capture is a frozen copy at one `model_rev`, read in pages.
                let mut pages = vec![fake.screen.clone()];
                pages.extend(fake.history_pages.iter().cloned());
                let total: u64 = pages.iter().map(|p| p.len() as u64).sum();
                if total > self.limits.max_snapshot_bytes {
                    return fail(ErrorCode::SnapshotTooLarge, "over max_snapshot_bytes");
                }
                let capture = CaptureId(self.next_capture);
                self.next_capture += 1;
                let result = Capture {
                    capture,
                    page_count: pages.len() as u32,
                    total_bytes: total,
                    model_rev,
                };
                let instance = s.instance.clone();
                self.captures.insert(
                    capture,
                    crate::state::StoredCapture {
                        owner: owner.expect("owner").clone(),
                        instance,
                        taken: now.monotonic,
                        pages,
                        model_rev,
                    },
                );
                ok(OpOutput::Capture(result))
            }
        }
    }

    /// Core ST-6: `read_page` is sync. A page beyond the last is `PageOutOfRange`.
    pub(crate) fn read_page(&self, capture: CaptureId, page: u32) -> Result<Page, CoreError> {
        let c = self
            .captures
            .get(&capture)
            .ok_or_else(|| CoreError::new(ErrorCode::UnknownCapture, "no such capture"))?;
        let bytes = c
            .pages
            .get(page as usize)
            .ok_or_else(|| CoreError::new(ErrorCode::PageOutOfRange, "no such page"))?;
        Ok(Page {
            index: page,
            bytes: HexBytes(bytes.clone()),
            last: page as usize + 1 == c.pages.len(),
        })
    }
}
