//! Core plugin timer scheduler mechanics.
//!
//! The scheduler owns deterministic timer state and bounded repeat delivery.
//! Hosts drive logical time by calling drain methods; plugin callback execution
//! still routes through [`PluginWorkerEngine`].

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use crate::actor::{
    BackpressureRoute, PluginCleanupResult, PluginHandlerKind, PluginInvocationRequest,
    PluginInvocationResult, PluginKey, PluginResourceKind, PluginResourceRef,
    PluginTimerCancellationResult, PluginTimerEvent, PluginTimerId, PluginTimerMode,
    PluginTimerSchedule, PluginWorkerEvent,
};
use crate::engine::plugin_worker::{PluginInvocationOutcome, PluginWorkerEngine};
use crate::session::RequestId;

/// Result of one timer scheduling request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginTimerScheduleOutcome {
    /// Scheduled timer id.
    pub timer_id: PluginTimerId,
    /// Events produced while accepting the schedule request.
    pub events: Vec<PluginTimerEvent>,
}

/// Result of draining due timers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginTimerDrainOutcome {
    /// Timer events produced by the drain.
    pub events: Vec<PluginTimerEvent>,
    /// Worker invocation outcomes produced by delivered timers.
    pub invocations: Vec<PluginInvocationOutcome>,
}

impl PluginTimerDrainOutcome {
    /// Build an empty drain outcome.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            events: Vec::new(),
            invocations: Vec::new(),
        }
    }
}

/// Reusable plugin timer scheduler.
#[derive(Clone, Default)]
pub struct PluginTimerScheduler {
    timers: Arc<Mutex<SchedulerState>>,
}

impl PluginTimerScheduler {
    /// Build an empty scheduler.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Schedule or replace plugin-owned timer work.
    pub fn schedule(&self, schedule: PluginTimerSchedule) -> PluginTimerScheduleOutcome {
        let request_id = schedule.request_id.clone();
        let plugin_key = schedule.handler.plugin_key.clone();
        let timer_id = schedule.timer_id.clone();

        if schedule.handler.kind != PluginHandlerKind::Timer {
            return PluginTimerScheduleOutcome {
                timer_id: timer_id.clone(),
                events: vec![PluginTimerEvent::Rejected {
                    request_id,
                    timer_id,
                    plugin_key,
                    reason: "plugin timer schedules must target timer handlers".to_string(),
                }],
            };
        }

        let mut state = self.timers.lock().expect("plugin timer mutex poisoned");
        let debounce_key = match &schedule.mode {
            PluginTimerMode::Debounce { key } => Some(key.clone()),
            PluginTimerMode::OneShot | PluginTimerMode::Interval { .. } => None,
        };
        let mut events = Vec::new();

        if let Some(previous) = state.timers.remove(&timer_id) {
            state.delivery_pending.remove(&timer_id);
            remove_debounce_index_for(&mut state, &previous);
            events.push(PluginTimerEvent::Cancelled {
                request_id: previous.schedule.request_id.clone(),
                resource: previous.resource_ref(),
                reason: "timer rescheduled".to_string(),
            });
        }

        if let Some(key) = &debounce_key {
            if let Some(previous_id) = state
                .debounce_index
                .insert((plugin_key.clone(), key.clone()), timer_id.clone())
            {
                if let Some(previous) = state.timers.remove(&previous_id) {
                    state.delivery_pending.remove(&previous_id);
                    events.push(PluginTimerEvent::Cancelled {
                        request_id: request_id.clone(),
                        resource: previous.resource_ref(),
                        reason: "debounce replaced pending timer".to_string(),
                    });
                }
            }
        }

        let timer = TimerState::new(schedule);
        let resource = timer.resource_ref();
        state.timers.insert(timer_id.clone(), timer);
        events.push(PluginTimerEvent::Scheduled {
            request_id,
            resource,
        });

        PluginTimerScheduleOutcome { timer_id, events }
    }

    /// Cancel one plugin-scoped timer.
    pub fn cancel(
        &self,
        request_id: RequestId,
        plugin_key: &PluginKey,
        timer_id: &PluginTimerId,
    ) -> PluginTimerCancellationResult {
        let mut state = self.timers.lock().expect("plugin timer mutex poisoned");
        let removed = state.timers.remove(timer_id).and_then(|timer| {
            if &timer.schedule.handler.plugin_key == plugin_key {
                state.delivery_pending.remove(timer_id);
                remove_debounce_index_for(&mut state, &timer);
                Some(timer.resource_ref())
            } else {
                state.timers.insert(timer_id.clone(), timer);
                None
            }
        });

        PluginTimerCancellationResult {
            request_id,
            plugin_key: plugin_key.clone(),
            timer_id: timer_id.clone(),
            cancelled: removed.is_some(),
            removed_resource: removed,
        }
    }

    /// Drain timers due at or before the caller-provided logical time.
    pub fn drain_due(&self, now_ms: u64, workers: &PluginWorkerEngine) -> PluginTimerDrainOutcome {
        let due = {
            let state = self.timers.lock().expect("plugin timer mutex poisoned");
            state
                .timers
                .iter()
                .filter(|(_, timer)| timer.next_due_ms <= now_ms)
                .map(|(timer_id, _)| timer_id.clone())
                .collect::<Vec<_>>()
        };

        let mut outcome = PluginTimerDrainOutcome::empty();

        for timer_id in due {
            let action = self.prepare_due_timer(now_ms, &timer_id);
            match action {
                PreparedTimer::None => {}
                PreparedTimer::Coalesced(event) => outcome.events.push(event),
                PreparedTimer::Invoke(request) => {
                    let invocation = workers.invoke(request.clone());
                    let timer_events = self.finish_invocation(now_ms, &timer_id, &invocation);
                    outcome.events.extend(timer_events);
                    outcome.invocations.push(invocation);
                }
            }
        }

        outcome
    }

    /// Cancel scheduler-owned timers for one plugin and return cleanup evidence.
    pub fn cleanup_plugin(
        &self,
        request_id: RequestId,
        plugin_key: &PluginKey,
    ) -> PluginCleanupResult {
        let mut state = self.timers.lock().expect("plugin timer mutex poisoned");
        let timer_ids = state
            .timers
            .iter()
            .filter(|(_, timer)| timer.schedule.handler.plugin_key == *plugin_key)
            .map(|(timer_id, _)| timer_id.clone())
            .collect::<Vec<_>>();
        let mut removed_resources = Vec::new();

        for timer_id in timer_ids {
            if let Some(timer) = state.timers.remove(&timer_id) {
                state.delivery_pending.remove(&timer_id);
                remove_debounce_index_for(&mut state, &timer);
                removed_resources.push(timer.resource_ref());
            }
        }

        PluginCleanupResult {
            request_id,
            plugin_key: plugin_key.clone(),
            removed_descriptors: Vec::new(),
            removed_resources,
        }
    }

    fn prepare_due_timer(&self, now_ms: u64, timer_id: &PluginTimerId) -> PreparedTimer {
        let mut state = self.timers.lock().expect("plugin timer mutex poisoned");
        let delivery_pending = state.delivery_pending.contains(timer_id);
        let Some(timer) = state.timers.get_mut(timer_id) else {
            return PreparedTimer::None;
        };

        if delivery_pending {
            let skipped_ticks = timer.advance_past(now_ms);
            return PreparedTimer::Coalesced(PluginTimerEvent::Coalesced {
                timer_id: timer_id.clone(),
                plugin_key: timer.schedule.handler.plugin_key.clone(),
                skipped_ticks,
                route: plugin_route(&timer.schedule.handler.plugin_key),
            });
        }

        let request = timer.invocation_request();
        state.delivery_pending.insert(timer_id.clone());
        PreparedTimer::Invoke(request)
    }

    fn finish_invocation(
        &self,
        now_ms: u64,
        timer_id: &PluginTimerId,
        invocation: &PluginInvocationOutcome,
    ) -> Vec<PluginTimerEvent> {
        let mut state = self.timers.lock().expect("plugin timer mutex poisoned");
        let mut events = Vec::new();

        events.push(PluginTimerEvent::Fired {
            timer_id: timer_id.clone(),
            request_id: invocation_request_id(&invocation.result),
            result: invocation.result.clone(),
        });
        events.extend(invocation.events.iter().filter_map(|event| match event {
            PluginWorkerEvent::Backpressure(summary) => Some(PluginTimerEvent::Backpressured {
                timer_id: timer_id.clone(),
                summary: summary.clone(),
            }),
            _ => None,
        }));

        state.delivery_pending.remove(timer_id);

        let Some(timer) = state.timers.get_mut(timer_id) else {
            return events;
        };

        match timer.schedule.mode {
            PluginTimerMode::Interval { .. } => {
                let elapsed_ticks = timer.advance_past(now_ms);
                let skipped_ticks = elapsed_ticks.saturating_sub(1);
                if skipped_ticks > 0 {
                    events.push(PluginTimerEvent::Coalesced {
                        timer_id: timer_id.clone(),
                        plugin_key: timer.schedule.handler.plugin_key.clone(),
                        skipped_ticks,
                        route: plugin_route(&timer.schedule.handler.plugin_key),
                    });
                }
            }
            PluginTimerMode::OneShot | PluginTimerMode::Debounce { .. } => {
                let removed = state.timers.remove(timer_id);
                state.delivery_pending.remove(timer_id);
                if let Some(timer) = removed {
                    remove_debounce_index_for(&mut state, &timer);
                }
            }
        }

        events
    }
}

#[derive(Default)]
struct SchedulerState {
    timers: HashMap<PluginTimerId, TimerState>,
    debounce_index: HashMap<(PluginKey, String), PluginTimerId>,
    delivery_pending: HashSet<PluginTimerId>,
}

struct TimerState {
    schedule: PluginTimerSchedule,
    next_due_ms: u64,
    fire_count: u64,
}

impl TimerState {
    fn new(schedule: PluginTimerSchedule) -> Self {
        Self {
            next_due_ms: schedule.due_at_ms,
            schedule,
            fire_count: 0,
        }
    }

    fn resource_ref(&self) -> PluginResourceRef {
        PluginResourceRef {
            plugin_key: self.schedule.handler.plugin_key.clone(),
            kind: PluginResourceKind::Timer,
            resource_id: self.schedule.timer_id.0.clone(),
        }
    }

    fn invocation_request(&mut self) -> PluginInvocationRequest {
        self.fire_count += 1;
        PluginInvocationRequest {
            request_id: RequestId(format!(
                "{}:fire:{}",
                self.schedule.timer_id.0, self.fire_count
            )),
            handler: self.schedule.handler.clone(),
            timeout_ms: self.schedule.timeout_ms,
            context: self.schedule.context.clone(),
            payload: self.schedule.payload.clone(),
        }
    }

    fn advance_past(&mut self, now_ms: u64) -> u64 {
        let PluginTimerMode::Interval { interval_ms } = self.schedule.mode else {
            return 0;
        };
        let interval_ms = interval_ms.max(1);
        let mut skipped_ticks = 0;

        while self.next_due_ms <= now_ms {
            self.next_due_ms = self.next_due_ms.saturating_add(interval_ms);
            skipped_ticks += 1;
        }

        skipped_ticks
    }
}

enum PreparedTimer {
    None,
    Coalesced(PluginTimerEvent),
    Invoke(PluginInvocationRequest),
}

fn remove_debounce_index_for(state: &mut SchedulerState, timer: &TimerState) {
    if let PluginTimerMode::Debounce { key } = &timer.schedule.mode {
        state
            .debounce_index
            .remove(&(timer.schedule.handler.plugin_key.clone(), key.clone()));
    }
}

fn plugin_route(plugin_key: &PluginKey) -> BackpressureRoute {
    BackpressureRoute {
        session_id: None,
        client_id: None,
        subscription_id: None,
        plugin_key: Some(plugin_key.clone()),
    }
}

fn invocation_request_id(result: &PluginInvocationResult) -> RequestId {
    match result {
        PluginInvocationResult::Completed(success) => success.request_id.clone(),
        PluginInvocationResult::Failed(failure) => failure.request_id.clone(),
    }
}
