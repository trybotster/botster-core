//! The session size (SZ-1 to SZ-3): the host's `Resize` sets the PTY size and the model's size.
//!
//! **One PTY resize is out at a time (SZ-3).** A `Resize` that arrives while one is out waits; a later one replaces the
//! waiting one, which completes `Superseded{by}`. On the control link `by` carries the host's request number of the
//! replacing `Resize`, and the host maps it to that operation's `OpId`. When the PTY took a size, the model takes the same
//! size, `model_rev` advances (ST-1), `Size` is reported (the host's `SizeChanged`, after the PTY resize) and the `Resize`
//! completes `Applied{actual}`. A `Resize` of the current size with nothing out completes `Applied` with no report (SZ-2).

use super::{Action, Worker};
use botster_core_contract::prelude::*;
use botster_core_edges::edges::WindowSize;
use botster_core_link::msg::{Observation, WorkerMsg};

/// The resizes of the session: the one out on the PTY and the one that waits for it.
#[derive(Debug, Default)]
pub(super) struct Resizes {
    out: Option<(u64, Size)>,
    waiting: Option<(u64, Size)>,
}

/// The PTY size of a session size: rows and columns as the PTY holds them (at most `u16::MAX`), and the pixel size of the
/// screen from the cell size when one is given, else zero.
pub fn window_size(size: &Size) -> WindowSize {
    let clamp = |n: u32| u16::try_from(n).unwrap_or(u16::MAX);
    let (rows, cols) = (clamp(size.rows), clamp(size.cols));
    let (width_px, height_px) = size.cell_px.map_or((0, 0), |px| {
        (
            clamp(u32::from(cols).saturating_mul(px.width)),
            clamp(u32::from(rows).saturating_mul(px.height)),
        )
    });
    WindowSize {
        cols,
        rows,
        width_px,
        height_px,
    }
}

/// The size that the PTY takes for `size`: its rows and columns as the PTY holds them, with the cell size.
fn actual(size: &Size) -> Size {
    let window = window_size(size);
    Size {
        rows: u32::from(window.rows),
        cols: u32::from(window.cols),
        cell_px: size.cell_px,
    }
}

impl Worker {
    /// `Resize` (SZ-1): out on the PTY now, or waiting behind the one that is out.
    pub(super) fn on_resize(&mut self, req: u64, size: Size) {
        if self.model.is_none() || !self.group_live() {
            self.report(&WorkerMsg::Done {
                req,
                result: OpResult::Err(CoreError::new(
                    ErrorCode::SessionEnded,
                    "the session has no PTY to resize",
                )),
            });
            return;
        }
        let size = actual(&size);
        if self.resizes.out.is_some() {
            if let Some((replaced, _)) = self.resizes.waiting.replace((req, size)) {
                self.report(&WorkerMsg::Done {
                    req: replaced,
                    result: OpResult::Ok(OpOutput::Resize(ResizeResult::Superseded {
                        by: OpId(req),
                    })),
                });
            }
            return;
        }
        if self.size == Some(size) {
            // SZ-2: the current size is applied already; nothing changed, so nothing is reported.
            self.report(&WorkerMsg::Done {
                req,
                result: OpResult::Ok(OpOutput::Resize(ResizeResult::Applied { actual: size })),
            });
            return;
        }
        self.resize_pty(req, size);
    }

    fn resize_pty(&mut self, req: u64, size: Size) {
        self.resizes.out = Some((req, size));
        self.actions
            .push_back(Action::ResizePty(window_size(&size)));
    }

    /// The answer to `ResizePty`: the model takes the size the PTY took, then the waiting resize goes out.
    pub(super) fn on_pty_resized(&mut self, result: Result<(), i32>) {
        let Some((req, size)) = self.resizes.out.take() else {
            return;
        };
        let result = match result {
            Ok(()) => {
                self.apply_size(size);
                OpResult::Ok(OpOutput::Resize(ResizeResult::Applied { actual: size }))
            }
            Err(errno) => OpResult::Err(CoreError::new(
                ErrorCode::Internal,
                format!("the PTY resize failed (errno {errno})"),
            )),
        };
        self.report(&WorkerMsg::Done { req, result });
        if let Some((next, size)) = self.resizes.waiting.take() {
            if self.group_live() {
                self.resize_pty(next, size);
            } else {
                self.report(&WorkerMsg::Done {
                    req: next,
                    result: OpResult::Err(CoreError::new(
                        ErrorCode::SessionEnded,
                        "the session has no PTY to resize",
                    )),
                });
            }
        }
    }

    /// The model takes the size that the PTY took: a read-visible change (ST-1) that the host reports as `SizeChanged`.
    fn apply_size(&mut self, size: Size) {
        self.size = Some(size);
        let Some(model) = self.model.as_mut() else {
            return;
        };
        // The library refuses only a size it cannot hold; the PTY holds it already, so the model keeps its old size then.
        let _ = model.term.resize(&size);
        self.input.model_rev = self.input.model_rev.wrapping_add(1);
        self.after_step();
        let model_rev = ModelRev(self.input.model_rev);
        self.report(&WorkerMsg::Observed {
            observation: Observation::Size { size, model_rev },
        });
    }
}
