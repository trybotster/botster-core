//! Pure routed envelope routing engine.

use std::collections::{HashMap, VecDeque};

use crate::SessionId;

use crate::contract::routed_envelope::{
    EnvelopeCursor, EnvelopeDeliveryState, EnvelopeDeliveryStatus, EnvelopeId, EnvelopeTarget,
    RoutedEnvelope, RoutedEnvelopeDrainOutcome, RoutedEnvelopeObservation,
    RoutedEnvelopePublishOutcome, RoutedEnvelopeQueueConfig,
};

/// In-memory, policy-free, at-least-once routed envelope router.
///
/// A drained envelope stays queued until its target acknowledges it, so a
/// target that drains and then fails before acking receives it again on a
/// later drain from an earlier cursor. `per_target_capacity` bounds queued
/// plus delivered-but-unacknowledged envelopes: a target that never acks
/// fills its queue, and publishing to it reports `Backpressured`. An ack
/// removes the envelope and its delivery record, so memory stays bounded by
/// what is outstanding. An envelope id is unique per target while it is
/// outstanding: publishing it again for that target enqueues nothing and
/// reports the existing copy; after its acknowledgement the id is free.
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
            // An envelope identity is unique per target while it is
            // outstanding: publishing the same id again is idempotent and
            // reports the existing copy, so each queued copy has the one
            // record its acknowledgement removes.
            if let Some(existing) = self.deliveries.get(&(target.clone(), envelope.id.clone())) {
                outcome.deliveries.push(existing.clone());
                continue;
            }
            let cursor = self.next_envelope_cursor();
            let mut target_envelope = envelope.clone();
            target_envelope.targets = vec![target.clone()];
            target_envelope.cursor = Some(cursor);

            let queue = self.queues.entry(target.clone()).or_default();
            if queue.len() >= self.config.per_target_capacity {
                // Reported, not recorded: nothing about it stays queued.
                let state = EnvelopeDeliveryState {
                    envelope_id: envelope.id.clone(),
                    target: target.clone(),
                    cursor,
                    status: EnvelopeDeliveryStatus::Backpressured,
                };
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

    /// Deliver the queued envelopes after an optional cursor, up to `limit`.
    ///
    /// Delivery does not remove them: each stays queued, and is delivered
    /// again by a later drain from an earlier cursor, until the target
    /// acknowledges it. `after` pages past envelopes the caller has already
    /// seen.
    pub fn drain(
        &mut self,
        target: &EnvelopeTarget,
        after: Option<EnvelopeCursor>,
        limit: usize,
    ) -> RoutedEnvelopeDrainOutcome {
        let mut outcome = RoutedEnvelopeDrainOutcome::default();
        let Some(queue) = self.queues.get(target) else {
            return outcome;
        };
        let after = after.map(|cursor| cursor.0).unwrap_or(0);
        for envelope in queue {
            if outcome.envelopes.len() >= limit {
                break;
            }
            let cursor = envelope.cursor.expect("queued envelope has cursor");
            if cursor.0 <= after {
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
            outcome.envelopes.push(envelope.clone());
        }
        outcome
    }

    /// Acknowledge one envelope for one target: remove it from the queue
    /// and forget its delivery record. Returns the final state
    /// (`Acknowledged`) once; `None` for an unknown or already acknowledged
    /// envelope.
    pub fn acknowledge(
        &mut self,
        target: &EnvelopeTarget,
        envelope_id: &EnvelopeId,
    ) -> Option<EnvelopeDeliveryState> {
        let mut state = self
            .deliveries
            .remove(&(target.clone(), envelope_id.clone()))?;
        if let Some(queue) = self.queues.get_mut(target) {
            if let Some(index) = queue.iter().position(|queued| &queued.id == envelope_id) {
                queue.remove(index);
            }
            if queue.is_empty() {
                self.queues.remove(target);
            }
        }
        state.status = EnvelopeDeliveryStatus::Acknowledged;
        Some(state)
    }

    /// Forget a target that is gone: drop its queue, its delivery records,
    /// and its subscriptions to every route. When a target is gone is host
    /// policy.
    pub fn forget_target(&mut self, target: &EnvelopeTarget) {
        self.forget_matching(|candidate| candidate == target);
    }

    /// Forget every target of one session: [`EnvelopeTarget::Session`] and
    /// each [`EnvelopeTarget::Subscription`] on it, as
    /// [`Self::forget_target`] forgets one target.
    pub fn forget_session_targets(&mut self, session_id: &SessionId) {
        self.forget_matching(|candidate| match candidate {
            EnvelopeTarget::Session { session_id: id }
            | EnvelopeTarget::Subscription { session_id: id, .. } => id == session_id,
            _ => false,
        });
    }

    fn forget_matching(&mut self, gone: impl Fn(&EnvelopeTarget) -> bool) {
        self.queues.retain(|target, _| !gone(target));
        self.deliveries
            .retain(|(delivery_target, _), _| !gone(delivery_target));
        self.subscriptions.retain(|_, subscribers| {
            subscribers.retain(|subscriber| !gone(subscriber));
            !subscribers.is_empty()
        });
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::routed_envelope::RoutedEnvelopePayload;
    use crate::EndpointId;

    fn endpoint(id: &str) -> EnvelopeTarget {
        EnvelopeTarget::Endpoint {
            endpoint_id: EndpointId(id.to_string()),
        }
    }

    fn envelope(id: &str, target: &EnvelopeTarget) -> RoutedEnvelope {
        RoutedEnvelope::new(
            EnvelopeId(id.to_string()),
            EndpointId("source".to_string()),
            vec![target.clone()],
            RoutedEnvelopePayload {
                content_type: "application/octet-stream".to_string(),
                body: Vec::new(),
                extension: None,
            },
            1,
        )
    }

    /// Records exist only for outstanding envelopes: acknowledged ones and
    /// backpressured copies leave none, so the map does not grow.
    #[test]
    fn delivery_records_do_not_grow_after_acks_or_backpressure() {
        let mut router = RoutedEnvelopeRouter::with_config(RoutedEnvelopeQueueConfig::new(1));
        let reader = endpoint("reader");
        for index in 0..100 {
            let id = format!("env-{index}");
            router.publish(envelope(&id, &reader));
            // The queue is full: this copy is refused and not recorded.
            router.publish(envelope(&format!("{id}-refused"), &reader));
            let _ = router.drain(&reader, None, 10);
            router
                .acknowledge(&reader, &EnvelopeId(id))
                .expect("ack the drained envelope");
        }
        assert!(
            router.deliveries.is_empty(),
            "{} records",
            router.deliveries.len()
        );
        assert!(router.queues.is_empty());
    }
}
