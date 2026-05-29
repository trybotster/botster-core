//! In-memory transport-neutral client stream contract harness.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::actor::{
    BackpressureSummary, ClientControlFrame, PasteFileRequest, SessionIoEvent, SessionIoRequest,
    SnapshotReady, TerminalAttachState,
};
use crate::client::ClientId;
use crate::session::{SessionId, SubscriptionId};
use crate::transport::{TransportEgress, TransportIngress};

/// Monotonic client stream generation used to reject stale reconnect deliveries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ClientStreamGeneration(pub u64);

/// Observable stream decision made by the in-memory harness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientStreamObservation {
    /// A session subscription became active.
    Subscribed {
        /// Subscribed session.
        session_id: SessionId,
        /// Active subscription route.
        subscription_id: SubscriptionId,
    },
    /// A duplicate subscribe request left the active route unchanged.
    DuplicateSubscription {
        /// Subscribed session.
        session_id: SessionId,
        /// Existing subscription route.
        subscription_id: SubscriptionId,
    },
    /// A new subscription id replaced the previous route for a session.
    ReplacedSubscription {
        /// Subscribed session.
        session_id: SessionId,
        /// Previous subscription route.
        old_subscription_id: SubscriptionId,
        /// New active subscription route.
        new_subscription_id: SubscriptionId,
    },
    /// A matching subscription route was removed.
    Unsubscribed {
        /// Unsubscribed session.
        session_id: SessionId,
        /// Removed subscription route.
        subscription_id: SubscriptionId,
    },
    /// An unsubscribe request did not match the active route.
    UnsubscribeIgnored {
        /// Requested session.
        session_id: SessionId,
        /// Requested subscription route.
        subscription_id: SubscriptionId,
    },
    /// Terminal input was dropped because the session has no active route.
    DroppedUnsubscribedInput {
        /// Target session.
        session_id: SessionId,
    },
    /// Paste input was dropped because the session has no active route.
    DroppedUnsubscribedPaste {
        /// Target session.
        session_id: SessionId,
    },
    /// Resize was dropped because the session has no active route.
    DroppedUnsubscribedResize {
        /// Target session.
        session_id: SessionId,
    },
    /// Snapshot request was dropped because the session has no active route.
    DroppedUnsubscribedSnapshot {
        /// Target session.
        session_id: SessionId,
    },
    /// Focus update was dropped because the session has no active route.
    DroppedUnsubscribedFocus {
        /// Target session.
        session_id: SessionId,
    },
    /// Session output was dropped because the session has no active route.
    DroppedUnsubscribedDelivery {
        /// Source session.
        session_id: SessionId,
    },
    /// Delivery or ingress was stale for the current client stream generation.
    GenerationStale {
        /// Current stream generation.
        current: ClientStreamGeneration,
        /// Generation attached to the stale command.
        received: ClientStreamGeneration,
    },
    /// Backpressure was observed on the client side.
    Backpressure(BackpressureSummary),
    /// The stream shut down.
    Shutdown {
        /// Human-readable shutdown reason.
        reason: String,
    },
    /// A command was ignored because the stream is closed.
    Closed,
}

/// Deterministic result of handling one stream command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientStreamOutcome {
    /// Frames emitted to the concrete client transport.
    pub egress: Vec<TransportEgress>,
    /// Requests emitted to session I/O workers, paired with their target session.
    pub session_requests: Vec<(SessionId, SessionIoRequest)>,
    /// Client-side control frames emitted by the stream.
    pub control_frames: Vec<ClientControlFrame>,
    /// Testable diagnostic observations.
    pub observations: Vec<ClientStreamObservation>,
}

impl ClientStreamOutcome {
    /// Build an empty stream outcome.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            egress: Vec::new(),
            session_requests: Vec::new(),
            control_frames: Vec::new(),
            observations: Vec::new(),
        }
    }
}

impl Default for ClientStreamOutcome {
    fn default() -> Self {
        Self::empty()
    }
}

/// Synchronous in-memory harness for per-client stream routing semantics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientStreamHarness {
    client_id: ClientId,
    subscriptions: HashMap<SessionId, SubscriptionId>,
    generation: ClientStreamGeneration,
    closed: bool,
}

impl ClientStreamHarness {
    /// Build a harness for one connected client.
    #[must_use]
    pub fn new(client_id: ClientId) -> Self {
        Self {
            client_id,
            subscriptions: HashMap::new(),
            generation: ClientStreamGeneration(0),
            closed: false,
        }
    }

    /// Return the current stream generation.
    #[must_use]
    pub const fn generation(&self) -> ClientStreamGeneration {
        self.generation
    }

    /// Return the client that owns this stream.
    #[must_use]
    pub const fn client_id(&self) -> &ClientId {
        &self.client_id
    }

    /// Advance the stream generation and return the new value.
    pub fn advance_generation(&mut self) -> ClientStreamGeneration {
        self.generation = ClientStreamGeneration(self.generation.0 + 1);
        self.generation
    }

    /// Return the active subscription for a session.
    #[must_use]
    pub fn active_subscription(&self, session_id: &SessionId) -> Option<&SubscriptionId> {
        self.subscriptions.get(session_id)
    }

    /// Handle client ingress for the current generation.
    pub fn handle_ingress(&mut self, ingress: TransportIngress) -> ClientStreamOutcome {
        self.handle_ingress_for_generation(self.generation, ingress)
    }

    /// Handle client ingress if the supplied generation is current.
    pub fn handle_ingress_for_generation(
        &mut self,
        generation: ClientStreamGeneration,
        ingress: TransportIngress,
    ) -> ClientStreamOutcome {
        if let Some(outcome) = self.reject_if_unroutable(generation) {
            return outcome;
        }

        match ingress {
            TransportIngress::SubscribeSession {
                client_id: _,
                session_id,
                subscription_id,
            } => self.subscribe(session_id, subscription_id),
            TransportIngress::UnsubscribeSession {
                client_id: _,
                session_id,
                subscription_id,
            } => self.unsubscribe(session_id, subscription_id),
            TransportIngress::TerminalInput { session_id, data } => self.route_session_request(
                session_id.clone(),
                SessionIoRequest::PtyInput { session_id, data },
                |session_id| ClientStreamObservation::DroppedUnsubscribedInput { session_id },
            ),
            TransportIngress::Resize {
                session_id,
                rows,
                cols,
            } => self.route_session_request(
                session_id.clone(),
                SessionIoRequest::Resize {
                    session_id,
                    rows,
                    cols,
                },
                |session_id| ClientStreamObservation::DroppedUnsubscribedResize { session_id },
            ),
            TransportIngress::RequestSnapshot {
                request_id,
                session_id,
            } => self.route_session_request(
                session_id.clone(),
                SessionIoRequest::GetSnapshot {
                    request_id,
                    session_id,
                },
                |session_id| ClientStreamObservation::DroppedUnsubscribedSnapshot { session_id },
            ),
            TransportIngress::Paste {
                request_id,
                session_id,
                data,
            } => self.route_session_request(
                session_id.clone(),
                SessionIoRequest::PasteFile(PasteFileRequest {
                    request_id,
                    session_id,
                    filename: "paste".to_string(),
                    data,
                }),
                |session_id| ClientStreamObservation::DroppedUnsubscribedPaste { session_id },
            ),
            TransportIngress::Focus {
                session_id,
                focused: _,
            } => {
                if self.subscriptions.contains_key(&session_id) {
                    ClientStreamOutcome::empty()
                } else {
                    let mut outcome = ClientStreamOutcome::empty();
                    outcome
                        .observations
                        .push(ClientStreamObservation::DroppedUnsubscribedFocus { session_id });
                    outcome
                }
            }
            TransportIngress::Heartbeat { request_id } | TransportIngress::Ping { request_id } => {
                let mut outcome = ClientStreamOutcome::empty();
                outcome.egress.push(TransportEgress::Pong { request_id });
                outcome
            }
            TransportIngress::ClientState { state, .. } => {
                let mut outcome = ClientStreamOutcome::empty();
                outcome
                    .control_frames
                    .push(ClientControlFrame::State { state });
                outcome
            }
            TransportIngress::BoundaryPayload { route_id, payload } => {
                let mut outcome = ClientStreamOutcome::empty();
                outcome
                    .egress
                    .push(TransportEgress::BoundaryPayload { route_id, payload });
                outcome
            }
        }
    }

    /// Handle a session I/O event for the current generation.
    pub fn handle_session_event(&mut self, event: SessionIoEvent) -> ClientStreamOutcome {
        self.handle_session_event_for_generation(self.generation, event)
    }

    /// Handle a session I/O event if the supplied generation is current.
    pub fn handle_session_event_for_generation(
        &mut self,
        generation: ClientStreamGeneration,
        event: SessionIoEvent,
    ) -> ClientStreamOutcome {
        if let Some(outcome) = self.reject_if_unroutable(generation) {
            return outcome;
        }

        match event {
            SessionIoEvent::TerminalBytes { session_id, data } => {
                self.route_delivery(session_id, |session_id, subscription_id| {
                    TransportEgress::TerminalOutput {
                        session_id,
                        subscription_id,
                        data,
                    }
                })
            }
            SessionIoEvent::SnapshotReady(snapshot) => self.route_snapshot(snapshot),
            SessionIoEvent::PasteFileFailed(_) => ClientStreamOutcome::empty(),
            SessionIoEvent::ProcessExited {
                session_id,
                payload,
            } => self.route_delivery(session_id, |session_id, subscription_id| {
                TransportEgress::ProcessExit {
                    session_id,
                    subscription_id,
                    code: payload.exit_code,
                }
            }),
            SessionIoEvent::InitialSnapshotReady(_)
            | SessionIoEvent::PasteFileWritten(_)
            | SessionIoEvent::PreparedSnapshotReady(_)
            | SessionIoEvent::ModeFlagsReady(_)
            | SessionIoEvent::ScreenReady(_)
            | SessionIoEvent::PromptMark { .. }
            | SessionIoEvent::Bell { .. }
            | SessionIoEvent::Notification { .. }
            | SessionIoEvent::Shutdown { .. } => ClientStreamOutcome::empty(),
        }
    }

    /// Report attach state for a subscribed session.
    pub fn handle_attach_state(
        &self,
        session_id: SessionId,
        state: TerminalAttachState,
    ) -> ClientStreamOutcome {
        if self.closed {
            return Self::closed_outcome();
        }

        self.route_delivery(session_id, |session_id, subscription_id| {
            TransportEgress::AttachState {
                session_id,
                subscription_id,
                state,
            }
        })
    }

    /// Report scrollback for a subscribed session.
    pub fn handle_scrollback(&self, session_id: SessionId, data: Vec<u8>) -> ClientStreamOutcome {
        if self.closed {
            return Self::closed_outcome();
        }

        self.route_delivery(session_id, |session_id, subscription_id| {
            TransportEgress::Scrollback {
                session_id,
                subscription_id,
                data,
            }
        })
    }

    /// Surface a client-side backpressure report.
    pub fn report_backpressure(&self, summary: BackpressureSummary) -> ClientStreamOutcome {
        if self.closed {
            return Self::closed_outcome();
        }

        let mut outcome = ClientStreamOutcome::empty();
        outcome
            .control_frames
            .push(ClientControlFrame::Backpressure(summary.clone()));
        outcome
            .observations
            .push(ClientStreamObservation::Backpressure(summary));
        outcome
    }

    /// Close the client stream and stop future routing.
    pub fn shutdown(&mut self, reason: impl Into<String>) -> ClientStreamOutcome {
        let reason = reason.into();
        self.closed = true;
        self.subscriptions.clear();

        let mut outcome = ClientStreamOutcome::empty();
        outcome.egress.push(TransportEgress::Close {
            reason: reason.clone(),
        });
        outcome
            .observations
            .push(ClientStreamObservation::Shutdown { reason });
        outcome
    }

    fn subscribe(
        &mut self,
        session_id: SessionId,
        subscription_id: SubscriptionId,
    ) -> ClientStreamOutcome {
        let mut outcome = ClientStreamOutcome::empty();
        match self.subscriptions.get(&session_id) {
            Some(existing) if existing == &subscription_id => {
                outcome
                    .observations
                    .push(ClientStreamObservation::DuplicateSubscription {
                        session_id,
                        subscription_id,
                    });
            }
            Some(existing) => {
                let old_subscription_id = existing.clone();
                self.subscriptions
                    .insert(session_id.clone(), subscription_id.clone());
                outcome
                    .observations
                    .push(ClientStreamObservation::ReplacedSubscription {
                        session_id,
                        old_subscription_id,
                        new_subscription_id: subscription_id,
                    });
            }
            None => {
                self.subscriptions
                    .insert(session_id.clone(), subscription_id.clone());
                outcome
                    .observations
                    .push(ClientStreamObservation::Subscribed {
                        session_id,
                        subscription_id,
                    });
            }
        }
        outcome
    }

    fn unsubscribe(
        &mut self,
        session_id: SessionId,
        subscription_id: SubscriptionId,
    ) -> ClientStreamOutcome {
        let mut outcome = ClientStreamOutcome::empty();
        if self.subscriptions.get(&session_id) == Some(&subscription_id) {
            self.subscriptions.remove(&session_id);
            outcome.session_requests.push((
                session_id.clone(),
                SessionIoRequest::UnsubscribeTerminal {
                    session_id: session_id.clone(),
                    subscription_id: subscription_id.clone(),
                },
            ));
            outcome
                .observations
                .push(ClientStreamObservation::Unsubscribed {
                    session_id,
                    subscription_id,
                });
        } else {
            outcome
                .observations
                .push(ClientStreamObservation::UnsubscribeIgnored {
                    session_id,
                    subscription_id,
                });
        }
        outcome
    }

    fn route_session_request(
        &self,
        session_id: SessionId,
        request: SessionIoRequest,
        dropped: impl FnOnce(SessionId) -> ClientStreamObservation,
    ) -> ClientStreamOutcome {
        if self.subscriptions.contains_key(&session_id) {
            let mut outcome = ClientStreamOutcome::empty();
            outcome.session_requests.push((session_id, request));
            outcome
        } else {
            let mut outcome = ClientStreamOutcome::empty();
            outcome.observations.push(dropped(session_id));
            outcome
        }
    }

    fn route_snapshot(&self, snapshot: SnapshotReady) -> ClientStreamOutcome {
        self.route_delivery(snapshot.session_id.clone(), |_, subscription_id| {
            TransportEgress::Snapshot {
                session_id: snapshot.session_id,
                subscription_id,
                data: snapshot.data,
            }
        })
    }

    fn route_delivery(
        &self,
        session_id: SessionId,
        build: impl FnOnce(SessionId, SubscriptionId) -> TransportEgress,
    ) -> ClientStreamOutcome {
        let mut outcome = ClientStreamOutcome::empty();
        if let Some(subscription_id) = self.subscriptions.get(&session_id) {
            outcome
                .egress
                .push(build(session_id, subscription_id.clone()));
        } else {
            outcome
                .observations
                .push(ClientStreamObservation::DroppedUnsubscribedDelivery { session_id });
        }
        outcome
    }

    fn reject_if_unroutable(
        &self,
        generation: ClientStreamGeneration,
    ) -> Option<ClientStreamOutcome> {
        if self.closed {
            Some(Self::closed_outcome())
        } else if generation != self.generation {
            let mut outcome = ClientStreamOutcome::empty();
            outcome
                .observations
                .push(ClientStreamObservation::GenerationStale {
                    current: self.generation,
                    received: generation,
                });
            Some(outcome)
        } else {
            None
        }
    }

    fn closed_outcome() -> ClientStreamOutcome {
        let mut outcome = ClientStreamOutcome::empty();
        outcome.observations.push(ClientStreamObservation::Closed);
        outcome
    }
}
