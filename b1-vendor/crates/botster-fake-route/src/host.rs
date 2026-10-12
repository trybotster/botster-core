//! The host lane of the worker: host writes join the one admission point with the route input (Core AM-2, IN-1).
//!
//! A host write is submitted with a `decide` closure. The worker calls it when the write STARTS (Core IN-8, IN-10), with the
//! revisions and modes of that moment, so a guard or a mode precondition is checked at the admission point and never earlier.
//! The closure belongs to the caller: the worker knows no guard rule and no encoding (R-5). It supplies the bytes to write and
//! the payload count, or an opaque refusal code that the caller maps back.
//!
//! The worker serializes the transactions (Core AM-2): while a host write is in progress, route input waits in the input
//! queue. The order between sources follows arrival, with the fairness bound `admission_fairness_frames`.

use crate::*;
use botster_route_codec::prelude::ModeFlags;

/// The state of the worker at the start of a host write (Core IN-8, IN-10).
pub struct StartView<'a> {
    pub modes: &'a ModeFlags,
    /// The input revision of the `Client` source class: complete client input frames received (Core IN-4, IN-10).
    pub client_rev: u64,
    /// The input revision of the `Host` source class: host writes admitted (Core IN-10).
    pub host_rev: u64,
    /// The revision of the read-visible state (Core ST-1).
    pub model_rev: u64,
    pub exited: bool,
}

/// What a host write puts on the PTY: the script supplies the bytes (R-5).
pub struct Planned {
    pub bytes: Vec<u8>,
    /// What `payload_bytes_written` counts when the write completes.
    pub payload_bytes: u64,
}

/// Chooses at the start of the write: the bytes to write, or a refusal code that the caller owns.
pub type Decide = Box<dyn FnOnce(&StartView<'_>) -> Result<Planned, u32> + Send>;

/// How a host write ended, with the exact counts (Core IN-2, IN-6, IN-7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostOutcome {
    Written {
        pty: u64,
        payload: u64,
    },
    /// The `decide` closure refused it.
    Refused(u32),
    /// The PTY write failed after `pty` bytes.
    Failed {
        pty: u64,
        payload: u64,
    },
    /// A cancel was honored after `pty` bytes.
    Cancelled {
        pty: u64,
        payload: u64,
    },
    /// The worker link failed after the write started and before it was acknowledged.
    Unknown {
        max_payload: u64,
    },
    /// The write never started and will not (the worker link failed).
    NeverSent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostDone {
    pub token: u64,
    pub outcome: HostOutcome,
}

/// A route that closed, as the worker tells the host (Core OU-2, A2-3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteEvent {
    /// A healthy close: `route_closed` was queued (Codec TS-8).
    Closed {
        route: RouteId,
        reason: CloseReason,
        exit: Option<Exit>,
    },
    /// A failed route (Codec TS-9).
    Ended {
        route: RouteId,
        reason: RouteEndReason,
    },
}

pub(crate) struct HostTxn {
    token: u64,
    bytes: Vec<u8>,
    written: usize,
    payload_bytes: u64,
}

impl FakeWorker {
    /// A host write joins the input queue. It starts when the admission point reaches it (Core AM-2).
    pub fn host_submit(&mut self, token: u64, decide: Decide) {
        // Core IN-7: a write that reaches a worker whose link failed was never sent.
        if self.link_lost {
            self.host_done.push_back(HostDone {
                token,
                outcome: HostOutcome::NeverSent,
            });
            return;
        }
        self.pty_pending.push_back(PendingWrite {
            route: HOST_SOURCE,
            op: None,
            action: Action::Host {
                token,
                decide: Some(decide),
            },
            silent: true,
        });
        self.flush_pty();
    }

    /// One step of progress: the transaction in progress writes its next piece (Core ST-1 partial progress, AM-2).
    pub fn pty_step(&mut self) {
        self.host_advance();
        self.flush_pty();
    }

    /// At most `n` bytes of a host write reach the PTY per step: short PTY writes (Core AM-2). `None` writes the whole transaction.
    pub fn set_pty_chunk(&mut self, n: Option<usize>) {
        self.pty_chunk = n;
    }

    /// Cancels a host write that has not completed. Returns false when the write is not here (Core IN-6).
    pub fn host_cancel(&mut self, token: u64) -> bool {
        if let Some(t) = self.host_txn.take_if(|t| t.token == token) {
            // Core IN-6: the completion reports exactly what reached the PTY.
            let outcome = HostOutcome::Cancelled {
                pty: t.written as u64,
                payload: (t.written as u64).min(t.payload_bytes),
            };
            self.host_done.push_back(HostDone { token, outcome });
            self.flush_pty();
            return true;
        }
        let at = self
            .pty_pending
            .iter()
            .position(|w| matches!(&w.action, Action::Host { token: t, .. } if *t == token));
        let Some(at) = at else { return false };
        self.pty_pending.remove(at);
        self.host_done.push_back(HostDone {
            token,
            outcome: HostOutcome::Cancelled { pty: 0, payload: 0 },
        });
        true
    }

    /// The worker link failed (Core IN-7): a started write is unknown, a write that never started will not.
    pub fn link_lost(&mut self) {
        self.link_lost = true;
        if let Some(t) = self.host_txn.take() {
            self.host_done.push_back(HostDone {
                token: t.token,
                outcome: HostOutcome::Unknown {
                    max_payload: t.payload_bytes,
                },
            });
        }
        for w in std::mem::take(&mut self.pty_pending) {
            if let Action::Host { token, .. } = w.action {
                self.host_done.push_back(HostDone {
                    token,
                    outcome: HostOutcome::NeverSent,
                });
            }
        }
    }

    /// True while the worker has something to report to the host or a host write in progress (Core AM-2, TM-6): the host should pump again.
    pub fn pty_busy(&self) -> bool {
        self.host_txn.is_some()
            || !self.host_done.is_empty()
            || !self.route_events.is_empty()
            || !self.activity_routes.is_empty()
            || self.output_activity
    }

    /// True once since the last call when the program wrote output (Core 6.2, `Activity{source: Output}`).
    pub fn take_output_activity(&mut self) -> bool {
        std::mem::take(&mut self.output_activity)
    }

    /// The routes whose client sent input since the last call (Core IN-4).
    pub fn take_activity(&mut self) -> Vec<RouteId> {
        std::mem::take(&mut self.activity_routes)
    }

    /// The host writes that ended since the last call.
    pub fn host_done(&mut self) -> Vec<HostDone> {
        self.host_done.drain(..).collect()
    }

    /// The route closes that the host has not seen yet (Core OU-2).
    pub fn take_route_events(&mut self) -> Vec<RouteEvent> {
        self.route_events.drain(..).collect()
    }

    /// The scripted modes of the program after a change that the script names (Core ST-1, IN-8).
    pub fn set_modes(&mut self, modes: ModeFlags) {
        self.modes = modes;
        self.model_rev += 1;
        let ids: Vec<RouteId> = self.routes.keys().copied().collect();
        for id in ids {
            self.send_modes(id);
        }
    }

    /// The host sets the size (Core SZ-1): the latest of a host resize and a route's size applies (Core DP-6).
    pub fn host_resize(&mut self, rows: u32, cols: u32) {
        if (rows, cols) != (self.rows, self.cols) {
            (self.rows, self.cols) = (rows, cols);
            self.model_rev += 1;
            let ids: Vec<RouteId> = self.routes.keys().copied().collect();
            for id in ids {
                self.send_modes(id);
            }
        }
    }

    pub fn modes(&self) -> &ModeFlags {
        &self.modes
    }

    pub fn size(&self) -> (u32, u32) {
        (self.rows, self.cols)
    }

    pub fn client_rev(&self) -> u64 {
        self.client_rev
    }

    pub fn host_rev(&self) -> u64 {
        self.host_rev
    }

    pub fn model_rev(&self) -> u64 {
        self.model_rev
    }

    /// The bytes that reached the PTY, in admission order, one entry per write (Core AM-2).
    pub fn pty_bytes(&self) -> Vec<(Option<Op>, Vec<u8>)> {
        self.pty_log.clone()
    }

    /// Every byte that reached the PTY, in admission order (Core AM-2).
    pub fn pty_stream(&self) -> Vec<u8> {
        self.pty_log.iter().flat_map(|(_, b)| b.clone()).collect()
    }

    /// True once the final output tail is queued for every open route (Core OU-7).
    pub fn exit_delivered(&self) -> bool {
        self.exited.is_some() && !self.exit_pending && self.source.is_empty()
    }

    /// The program exits (Core OU-7, Codec TS-8).
    pub fn program_exit(&mut self, exit: Exit) {
        self.exited = Some(exit);
        self.exit_pending = true;
        self.pump_source();
    }

    /// The next write of the admission order: first in, with the fairness bound across sources (Core AM-2).
    pub(crate) fn next_source(&self) -> Option<usize> {
        let head = self.pty_pending.front()?.route;
        let (last, run) = self.streak;
        if head == last && run >= self.cfg.admission_fairness_frames {
            // Core AM-2: a source that has had its share yields to the oldest write of another source.
            if let Some(other) = self.pty_pending.iter().position(|w| w.route != head) {
                return Some(other);
            }
        }
        Some(0)
    }

    /// Admits one write: a route write is written at once, and a host write starts its transaction (Core AM-2).
    pub(crate) fn admit(&mut self, w: PendingWrite) {
        let source = w.route;
        self.streak = if self.streak.0 == source {
            (source, self.streak.1 + 1)
        } else {
            (source, 1)
        };
        match w.action {
            Action::Host { token, decide } => self.host_start(token, decide.expect("decide")),
            _ => self.do_write(w),
        }
    }

    fn host_start(&mut self, token: u64, decide: Decide) {
        let view = StartView {
            modes: &self.modes,
            client_rev: self.client_rev,
            host_rev: self.host_rev,
            model_rev: self.model_rev,
            exited: self.exited.is_some(),
        };
        match decide(&view) {
            Err(code) => self.host_done.push_back(HostDone {
                token,
                outcome: HostOutcome::Refused(code),
            }),
            Ok(plan) => {
                // Core IN-10: the host revision advances when the worker admits a host write.
                self.host_rev += 1;
                self.host_txn = Some(HostTxn {
                    token,
                    bytes: plan.bytes,
                    written: 0,
                    payload_bytes: plan.payload_bytes,
                });
                self.host_advance();
            }
        }
    }

    fn host_advance(&mut self) {
        let Some(mut t) = self.host_txn.take() else {
            return;
        };
        let remaining = t.bytes.len() - t.written;
        let piece = self.pty_chunk.map_or(remaining, |c| c.min(remaining));
        // Codec TS-5b: a PTY write that fails does so after n bytes.
        let (piece, failed) = match self.pty_fail_after {
            Some(k) if k < t.written + piece => {
                self.pty_fail_after = None;
                (k.saturating_sub(t.written), true)
            }
            _ => (piece, false),
        };
        self.log_write(None, &t.bytes[t.written..t.written + piece]);
        t.written += piece;
        let (pty, payload) = (t.written as u64, (t.written as u64).min(t.payload_bytes));
        let outcome = if failed {
            HostOutcome::Failed { pty, payload }
        } else if t.written == t.bytes.len() {
            HostOutcome::Written {
                pty,
                payload: t.payload_bytes,
            }
        } else {
            self.host_txn = Some(t);
            return;
        };
        self.host_done.push_back(HostDone {
            token: t.token,
            outcome,
        });
    }
}

/// The source of a host write in the admission order. Route ids start at 1.
pub(crate) const HOST_SOURCE: RouteId = 0;
