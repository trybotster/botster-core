//! Pure multi-client subscription routing engine.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::actor::{
    BackpressureRoute, BackpressureSummary, ClientControlFrame, DeliveryLag, MailboxSendFailure,
    MailboxSendFailureReason, QueueSource, SessionIoEvent, SessionIoRequest, TerminalAttachState,
};
use crate::client::ClientId;
use crate::client_stream::{ClientStreamHarness, ClientStreamObservation, ClientStreamOutcome};
use crate::session::SessionId;
use crate::session::SubscriptionId;
use crate::transport::{TransportEgress, TransportIngress};

/// Observable multiplexer routing decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SubscriptionMultiplexerObservation {
    /// A per-client stream emitted an observation.
    ClientStream {
        /// Client whose stream emitted the observation.
        client_id: ClientId,
        /// Observation from the per-client stream harness.
        observation: ClientStreamObservation,
    },
    /// A delivery was accepted but the route is lagging.
    DeliveryLagged {
        /// Client whose delivery route is lagging.
        client_id: ClientId,
        /// Typed lag summary for the affected route.
        lag: DeliveryLag,
    },
    /// A delivery failed at the caller-owned queue boundary.
    DeliveryFailed {
        /// Client whose delivery route failed.
        client_id: ClientId,
        /// Typed failure summary for the affected route.
        failure: MailboxSendFailure,
    },
    /// A session event is intentionally not broadcast by the multiplexer.
    SessionEventNotBroadcast {
        /// Session that emitted the event.
        session_id: SessionId,
        /// Stable event kind.
        event_kind: String,
    },
}

/// Deterministic result of handling one multiplexer command.
///
/// The multiplexer only classifies and batches per-client outputs. It performs
/// no transport writes; callers enqueue each `client_egress` item into the
/// receiving client's worker or adapter boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubscriptionMultiplexerOutcome {
    /// Frames emitted to client transports, paired with the receiving client.
    pub client_egress: Vec<(ClientId, TransportEgress)>,
    /// Requests emitted to session I/O workers, paired with their target session.
    pub session_requests: Vec<(SessionId, SessionIoRequest)>,
    /// Client-side control frames, paired with the receiving client.
    pub client_control_frames: Vec<(ClientId, ClientControlFrame)>,
    /// Testable diagnostic observations.
    pub observations: Vec<SubscriptionMultiplexerObservation>,
}

impl SubscriptionMultiplexerOutcome {
    /// Build an empty multiplexer outcome.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            client_egress: Vec::new(),
            session_requests: Vec::new(),
            client_control_frames: Vec::new(),
            observations: Vec::new(),
        }
    }

    fn append_client_outcome(&mut self, client_id: &ClientId, outcome: ClientStreamOutcome) {
        self.client_egress.extend(
            outcome
                .egress
                .into_iter()
                .map(|egress| (client_id.clone(), egress)),
        );
        self.session_requests.extend(outcome.session_requests);
        self.client_control_frames.extend(
            outcome
                .control_frames
                .into_iter()
                .map(|frame| (client_id.clone(), frame)),
        );
        self.observations
            .extend(outcome.observations.into_iter().map(|observation| {
                SubscriptionMultiplexerObservation::ClientStream {
                    client_id: client_id.clone(),
                    observation,
                }
            }));
    }
}

impl Default for SubscriptionMultiplexerOutcome {
    fn default() -> Self {
        Self::empty()
    }
}

/// Synchronous, transport-neutral subscription multiplexer.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SubscriptionMultiplexer {
    clients: HashMap<ClientId, ClientStreamHarness>,
    session_subscribers: HashMap<SessionId, Vec<ClientId>>,
}

impl SubscriptionMultiplexer {
    /// Build an empty multiplexer.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Handle one client ingress frame.
    pub fn handle_client_ingress(
        &mut self,
        client_id: ClientId,
        ingress: TransportIngress,
    ) -> SubscriptionMultiplexerOutcome {
        let routed_session_id = routed_session_id(&ingress);
        let outcome = self
            .clients
            .entry(client_id.clone())
            .or_insert_with(|| ClientStreamHarness::new(client_id.clone()))
            .handle_ingress(ingress);

        if let Some(session_id) = routed_session_id {
            self.sync_client_subscription(&client_id, &session_id);
        }

        let mut multiplexer_outcome = SubscriptionMultiplexerOutcome::empty();
        multiplexer_outcome.append_client_outcome(&client_id, outcome);
        multiplexer_outcome
    }

    /// Fan out a session-wide pushed event to current subscribers.
    pub fn handle_session_event(
        &mut self,
        event: SessionIoEvent,
    ) -> SubscriptionMultiplexerOutcome {
        match event {
            SessionIoEvent::TerminalBytes { session_id, data } => {
                self.fanout_session_event(&session_id, |session_id| SessionIoEvent::TerminalBytes {
                    session_id,
                    data: data.clone(),
                })
            }
            SessionIoEvent::ProcessExited {
                session_id,
                payload,
            } => {
                self.fanout_session_event(&session_id, |session_id| SessionIoEvent::ProcessExited {
                    session_id,
                    payload: payload.clone(),
                })
            }
            // These variants are intentionally matched one by one so adding a
            // new SessionIoEvent forces an explicit broadcast decision.
            SessionIoEvent::InitialSnapshotReady(snapshot) => {
                self.route_initial_snapshot_ready(snapshot)
            }
            SessionIoEvent::SnapshotReady(snapshot) => {
                Self::not_broadcast(snapshot.session_id, "snapshot_ready")
            }
            SessionIoEvent::SendFileWritten(written) => {
                Self::not_broadcast(written.session_id, "send_file_written")
            }
            SessionIoEvent::SendFileFailed(failed) => {
                Self::not_broadcast(failed.session_id, "send_file_failed")
            }
            SessionIoEvent::PreparedSnapshotReady(snapshot) => {
                Self::not_broadcast(snapshot.session_id, "prepared_snapshot_ready")
            }
            // ModeFlagsReady is still a targeted request-response contract; it
            // is not a session-wide pushed event for this multiplexer.
            SessionIoEvent::ModeFlagsReady(mode_flags) => {
                Self::not_broadcast(mode_flags.session_id, "mode_flags_ready")
            }
            SessionIoEvent::ScreenReady(screen) => {
                Self::not_broadcast(screen.session_id, "screen_ready")
            }
            SessionIoEvent::TitleChanged { session_id, .. } => {
                Self::not_broadcast(session_id, "title_changed")
            }
            SessionIoEvent::CwdChanged { session_id, .. } => {
                Self::not_broadcast(session_id, "cwd_changed")
            }
            SessionIoEvent::PromptMark { session_id, .. } => {
                Self::not_broadcast(session_id, "prompt_mark")
            }
            SessionIoEvent::Bell { session_id } => Self::not_broadcast(session_id, "bell"),
            SessionIoEvent::Notification { session_id, .. } => {
                Self::not_broadcast(session_id, "notification")
            }
            SessionIoEvent::Shutdown { session_id, .. } => {
                Self::not_broadcast(session_id, "shutdown")
            }
        }
    }

    /// Fan out session attach state to current subscribers.
    pub fn handle_attach_state(
        &mut self,
        session_id: SessionId,
        state: TerminalAttachState,
    ) -> SubscriptionMultiplexerOutcome {
        let subscribers = self.subscribers_for(&session_id);
        let mut multiplexer_outcome = SubscriptionMultiplexerOutcome::empty();
        for client_id in subscribers {
            if let Some(harness) = self.clients.get_mut(&client_id) {
                let outcome = harness.handle_attach_state(session_id.clone(), state.clone());
                multiplexer_outcome.append_client_outcome(&client_id, outcome);
            }
        }
        multiplexer_outcome
    }

    /// Report client-side backpressure for an active subscription route.
    pub fn report_backpressure(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        source: QueueSource,
        capacity: usize,
        depth: usize,
    ) -> SubscriptionMultiplexerOutcome {
        let subscription_id = self
            .clients
            .get(&client_id)
            .and_then(|harness| harness.active_subscription(&session_id))
            .cloned();
        let summary = BackpressureSummary {
            source,
            capacity,
            depth,
            route: BackpressureRoute {
                session_id: Some(session_id),
                client_id: Some(client_id.clone()),
                subscription_id,
                plugin_key: None,
            },
        };
        let outcome = self
            .clients
            .entry(client_id.clone())
            .or_insert_with(|| ClientStreamHarness::new(client_id.clone()))
            .report_backpressure(summary);

        let mut multiplexer_outcome = SubscriptionMultiplexerOutcome::empty();
        multiplexer_outcome.append_client_outcome(&client_id, outcome);
        multiplexer_outcome
    }

    /// Report accepted-but-slow delivery for a subscription route.
    ///
    /// This is an observation-only hub signal. It does not emit client health
    /// frames or mutate subscriptions because the caller-owned queue accepted
    /// the delivery.
    pub fn report_delivery_lag(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        source: QueueSource,
        capacity: usize,
        depth: usize,
    ) -> SubscriptionMultiplexerOutcome {
        let lag = DeliveryLag {
            source,
            capacity,
            depth,
            route: self.route_for_subscription(&client_id, session_id, subscription_id),
        };

        let mut outcome = SubscriptionMultiplexerOutcome::empty();
        outcome
            .observations
            .push(SubscriptionMultiplexerObservation::DeliveryLagged { client_id, lag });
        outcome
    }

    /// Report a failed delivery attempt at a caller-owned queue boundary.
    ///
    /// `QueueFull` is an observation-only hub signal and keeps the route
    /// subscribed. `QueueClosed` removes only the matching active route.
    pub fn report_delivery_failure(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        source: QueueSource,
        reason: MailboxSendFailureReason,
    ) -> SubscriptionMultiplexerOutcome {
        let route = self.route_for_subscription(&client_id, session_id.clone(), subscription_id);
        let failure = MailboxSendFailure {
            source,
            route: route.clone(),
            reason,
        };

        if reason == MailboxSendFailureReason::QueueClosed {
            if let Some(route_subscription_id) = route.subscription_id.as_ref() {
                let mut multiplexer_outcome = SubscriptionMultiplexerOutcome::empty();
                if self
                    .clients
                    .get(&client_id)
                    .and_then(|harness| harness.active_subscription(&session_id))
                    == Some(route_subscription_id)
                {
                    let harness = self
                        .clients
                        .get_mut(&client_id)
                        .expect("checked client stream exists");
                    let outcome = harness.handle_ingress(TransportIngress::UnsubscribeSession {
                        client_id: client_id.clone(),
                        session_id: session_id.clone(),
                        subscription_id: route_subscription_id.clone(),
                    });
                    multiplexer_outcome.append_client_outcome(&client_id, outcome);
                    self.sync_client_subscription(&client_id, &session_id);
                }
                multiplexer_outcome.observations.push(
                    SubscriptionMultiplexerObservation::DeliveryFailed { client_id, failure },
                );
                return multiplexer_outcome;
            }
        }

        let mut outcome = SubscriptionMultiplexerOutcome::empty();
        outcome
            .observations
            .push(SubscriptionMultiplexerObservation::DeliveryFailed { client_id, failure });
        outcome
    }

    fn fanout_session_event(
        &mut self,
        session_id: &SessionId,
        build: impl Fn(SessionId) -> SessionIoEvent,
    ) -> SubscriptionMultiplexerOutcome {
        let subscribers = self.subscribers_for(session_id);
        let mut multiplexer_outcome = SubscriptionMultiplexerOutcome::empty();
        for client_id in subscribers {
            if let Some(harness) = self.clients.get_mut(&client_id) {
                let outcome = harness.handle_session_event(build(session_id.clone()));
                multiplexer_outcome.append_client_outcome(&client_id, outcome);
            }
        }
        multiplexer_outcome
    }

    fn route_initial_snapshot_ready(
        &mut self,
        snapshot: crate::InitialSnapshotReady,
    ) -> SubscriptionMultiplexerOutcome {
        let client_id = snapshot.client_id.clone();
        let mut multiplexer_outcome = SubscriptionMultiplexerOutcome::empty();
        if let Some(harness) = self.clients.get_mut(&client_id) {
            let outcome =
                harness.handle_session_event(SessionIoEvent::InitialSnapshotReady(snapshot));
            multiplexer_outcome.append_client_outcome(&client_id, outcome);
        }
        multiplexer_outcome
    }

    fn sync_client_subscription(&mut self, client_id: &ClientId, session_id: &SessionId) {
        let active_subscription = self
            .clients
            .get(client_id)
            .and_then(|harness| harness.active_subscription(session_id));
        match active_subscription {
            Some(_) => self.add_subscriber(client_id, session_id),
            None => self.remove_subscriber(client_id, session_id),
        }
    }

    fn add_subscriber(&mut self, client_id: &ClientId, session_id: &SessionId) {
        let subscribers = self
            .session_subscribers
            .entry(session_id.clone())
            .or_default();
        if !subscribers.contains(client_id) {
            subscribers.push(client_id.clone());
        }
    }

    fn remove_subscriber(&mut self, client_id: &ClientId, session_id: &SessionId) {
        if let Some(subscribers) = self.session_subscribers.get_mut(session_id) {
            subscribers.retain(|subscriber_id| subscriber_id != client_id);
            if subscribers.is_empty() {
                self.session_subscribers.remove(session_id);
            }
        }
    }

    fn subscribers_for(&self, session_id: &SessionId) -> Vec<ClientId> {
        self.session_subscribers
            .get(session_id)
            .cloned()
            .unwrap_or_default()
    }

    fn route_for_subscription(
        &self,
        client_id: &ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
    ) -> BackpressureRoute {
        BackpressureRoute {
            session_id: Some(session_id),
            client_id: Some(client_id.clone()),
            subscription_id: Some(subscription_id),
            plugin_key: None,
        }
    }

    fn not_broadcast(
        session_id: SessionId,
        event_kind: &'static str,
    ) -> SubscriptionMultiplexerOutcome {
        let mut outcome = SubscriptionMultiplexerOutcome::empty();
        outcome.observations.push(
            SubscriptionMultiplexerObservation::SessionEventNotBroadcast {
                session_id,
                event_kind: event_kind.to_string(),
            },
        );
        outcome
    }
}

fn routed_session_id(ingress: &TransportIngress) -> Option<SessionId> {
    match ingress {
        TransportIngress::SubscribeSession { session_id, .. }
        | TransportIngress::UnsubscribeSession { session_id, .. }
        | TransportIngress::TerminalInput { session_id, .. }
        | TransportIngress::Resize { session_id, .. }
        | TransportIngress::RequestSnapshot { session_id, .. }
        | TransportIngress::SendFile { session_id, .. }
        | TransportIngress::Focus { session_id, .. } => Some(session_id.clone()),
        TransportIngress::Heartbeat { .. }
        | TransportIngress::BoundaryPayload { .. }
        | TransportIngress::ClientState { .. }
        | TransportIngress::Ping { .. } => None,
    }
}
