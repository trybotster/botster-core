//! Pure routed envelope routing engine.

use std::collections::{HashMap, VecDeque};

use crate::contract::routed_envelope::{
    EnvelopeCursor, EnvelopeDeliveryState, EnvelopeDeliveryStatus, EnvelopeId, EnvelopeTarget,
    RoutedEnvelope, RoutedEnvelopeDrainOutcome, RoutedEnvelopeObservation,
    RoutedEnvelopePublishOutcome, RoutedEnvelopeQueueConfig,
};

/// In-memory, policy-free routed envelope router.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutedEnvelopeRouter {
    config: RoutedEnvelopeQueueConfig,
    next_cursor: u64,
    queues: HashMap<EnvelopeTarget, VecDeque<RoutedEnvelope>>,
    subscriptions: HashMap<EnvelopeTarget, Vec<EnvelopeTarget>>,
    deliveries: HashMap<(EnvelopeTarget, EnvelopeId), EnvelopeDeliveryState>,
}

impl RoutedEnvelopeRouter {
    /// Build an empty router with default queue settings.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(RoutedEnvelopeQueueConfig::default())
    }

    /// Build an empty router with explicit queue settings.
    #[must_use]
    pub fn with_config(config: RoutedEnvelopeQueueConfig) -> Self {
        Self {
            config,
            next_cursor: 1,
            queues: HashMap::new(),
            subscriptions: HashMap::new(),
            deliveries: HashMap::new(),
        }
    }

    /// Register a target to receive fanout for a route.
    pub fn subscribe(
        &mut self,
        route: EnvelopeTarget,
        subscriber: EnvelopeTarget,
    ) -> RoutedEnvelopeObservation {
        let subscribers = self.subscriptions.entry(route.clone()).or_default();
        if !subscribers.contains(&subscriber) {
            subscribers.push(subscriber.clone());
        }
        RoutedEnvelopeObservation::Subscribed { route, subscriber }
    }

    /// Remove a target from route fanout.
    pub fn unsubscribe(
        &mut self,
        route: &EnvelopeTarget,
        subscriber: &EnvelopeTarget,
    ) -> RoutedEnvelopeObservation {
        if let Some(subscribers) = self.subscriptions.get_mut(route) {
            subscribers.retain(|candidate| candidate != subscriber);
            if subscribers.is_empty() {
                self.subscriptions.remove(route);
            }
        }
        RoutedEnvelopeObservation::Unsubscribed {
            route: route.clone(),
            subscriber: subscriber.clone(),
        }
    }

    /// Publish one envelope to direct targets or current route subscribers.
    pub fn publish(&mut self, envelope: RoutedEnvelope) -> RoutedEnvelopePublishOutcome {
        let mut outcome = RoutedEnvelopePublishOutcome::default();

        for target in self.resolved_targets(&envelope.targets) {
            let cursor = self.next_envelope_cursor();
            let mut target_envelope = envelope.clone();
            target_envelope.targets = vec![target.clone()];
            target_envelope.cursor = Some(cursor);

            let queue = self.queues.entry(target.clone()).or_default();
            if queue.len() >= self.config.per_target_capacity {
                let state = EnvelopeDeliveryState {
                    envelope_id: envelope.id.clone(),
                    target: target.clone(),
                    cursor,
                    status: EnvelopeDeliveryStatus::Backpressured,
                };
                self.deliveries
                    .insert((target.clone(), envelope.id.clone()), state.clone());
                outcome.deliveries.push(state);
                outcome
                    .observations
                    .push(RoutedEnvelopeObservation::Backpressured {
                        envelope_id: envelope.id.clone(),
                        target,
                        capacity: self.config.per_target_capacity,
                        depth: queue.len(),
                    });
                continue;
            }

            queue.push_back(target_envelope);
            let state = EnvelopeDeliveryState {
                envelope_id: envelope.id.clone(),
                target: target.clone(),
                cursor,
                status: EnvelopeDeliveryStatus::Queued,
            };
            self.deliveries
                .insert((target.clone(), envelope.id.clone()), state.clone());
            outcome.deliveries.push(state);
            outcome
                .observations
                .push(RoutedEnvelopeObservation::Queued {
                    envelope_id: envelope.id.clone(),
                    target,
                    cursor,
                });
        }

        outcome
    }

    /// Drain deliverable envelopes after an optional cursor.
    pub fn drain(
        &mut self,
        target: &EnvelopeTarget,
        after: Option<EnvelopeCursor>,
        limit: usize,
    ) -> RoutedEnvelopeDrainOutcome {
        let mut outcome = RoutedEnvelopeDrainOutcome::default();
        let Some(queue) = self.queues.get_mut(target) else {
            return outcome;
        };

        let after = after.map(|cursor| cursor.0).unwrap_or(0);
        let mut retained = VecDeque::new();

        while let Some(envelope) = queue.pop_front() {
            let cursor = envelope.cursor.expect("queued envelope has cursor");
            if cursor.0 <= after || outcome.envelopes.len() >= limit {
                retained.push_back(envelope);
                continue;
            }

            if let Some(state) = self
                .deliveries
                .get_mut(&(target.clone(), envelope.id.clone()))
            {
                state.status = EnvelopeDeliveryStatus::Delivered;
            }
            outcome.next_cursor = Some(cursor);
            outcome
                .observations
                .push(RoutedEnvelopeObservation::Delivered {
                    envelope_id: envelope.id.clone(),
                    target: target.clone(),
                    cursor,
                });
            outcome.envelopes.push(envelope);
        }

        if retained.is_empty() {
            self.queues.remove(target);
        } else {
            *queue = retained;
        }

        outcome
    }

    /// Acknowledge one delivered envelope for one target.
    pub fn acknowledge(
        &mut self,
        target: &EnvelopeTarget,
        envelope_id: &EnvelopeId,
    ) -> Option<EnvelopeDeliveryState> {
        let state = self
            .deliveries
            .get_mut(&(target.clone(), envelope_id.clone()))?;
        state.status = EnvelopeDeliveryStatus::Acknowledged;
        Some(state.clone())
    }

    /// Return the tracked delivery state for one target copy.
    #[must_use]
    pub fn delivery_state(
        &self,
        target: &EnvelopeTarget,
        envelope_id: &EnvelopeId,
    ) -> Option<&EnvelopeDeliveryState> {
        self.deliveries.get(&(target.clone(), envelope_id.clone()))
    }

    fn resolved_targets(&self, requested_targets: &[EnvelopeTarget]) -> Vec<EnvelopeTarget> {
        let mut targets = Vec::new();
        for target in requested_targets {
            if let Some(subscribers) = self.subscriptions.get(target) {
                targets.extend(subscribers.iter().cloned());
            } else {
                targets.push(target.clone());
            }
        }
        targets
    }

    fn next_envelope_cursor(&mut self) -> EnvelopeCursor {
        let cursor = EnvelopeCursor(self.next_cursor);
        self.next_cursor += 1;
        cursor
    }
}

impl Default for RoutedEnvelopeRouter {
    fn default() -> Self {
        Self::new()
    }
}
