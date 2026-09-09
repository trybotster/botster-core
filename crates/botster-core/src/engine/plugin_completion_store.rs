//! One completion owner, including reserved but not yet published results.
//!
//! All links and ready fronts are numeric. Draining never visits the plugin
//! registry, clones a worker, or acquires a worker admission lock.

use std::collections::{BTreeMap, BTreeSet};
use std::mem::size_of;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use super::{
    CompletionReservationPool, EngineShared, PluginCompletion, PluginCompletionItem,
    PluginWorkerEngineMetrics, WorkerMetrics,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct CompletionReservation {
    pub(super) id: u64,
    pub(super) generation: u64,
    pub(super) payload_bytes: usize,
    pub(super) charged_bytes: usize,
}

struct CompletionSlot {
    reservation: CompletionReservation,
    item: Option<PluginCompletionItem>,
    next: Option<u64>,
}

#[derive(Clone, Copy, Default)]
struct Fifo {
    head: Option<u64>,
    tail: Option<u64>,
}

struct CompletionGeneration {
    reserved_count: usize,
    retired: bool,
    published: Fifo,
    metrics: Arc<WorkerMetrics>,
}

#[derive(Default)]
pub(super) struct CompletionStore {
    reservations: CompletionReservationPool,
    next_slot: u64,
    slots: BTreeMap<u64, CompletionSlot>,
    generations: BTreeMap<u64, CompletionGeneration>,
    active_fronts: BTreeSet<(usize, u64)>,
    retired: Fifo,
    engine_metrics: Option<Arc<PluginWorkerEngineMetrics>>,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum StoreAdmissionError {
    Capacity,
    IdentityExhausted,
    Retired,
}

/// Conservative fixed logical allowance per admitted completion. Each slot
/// reserves an entire generation row and front index even when shared with
/// siblings. The root mutex/store wrapper, publication token and shared handle
/// are also covered. Slot/FIFO fields contain every retired link; retirement
/// removes the active index before reusing those links and needs no new index.
/// B-tree allocator node links/padding and unused container capacity are not
/// logical entries. This is not an allocator-resident byte measurement.
pub(super) const fn metadata_bytes() -> usize {
    size_of::<Mutex<CompletionStore>>()
        + size_of::<(u64, CompletionSlot)>()
        + size_of::<(u64, CompletionGeneration)>()
        + size_of::<(usize, u64)>()
        + size_of::<CompletionReservation>()
        + size_of::<Arc<EngineShared>>()
        + size_of::<WorkerMetrics>()
        + 2 * size_of::<usize>()
        + size_of::<PluginWorkerEngineMetrics>()
        + 2 * size_of::<usize>()
}

impl CompletionStore {
    pub(super) fn reserve(
        &mut self,
        generation: u64,
        payload_bytes: usize,
        charged_bytes: usize,
        metrics: &Arc<WorkerMetrics>,
        shared: &EngineShared,
    ) -> Result<CompletionReservation, StoreAdmissionError> {
        if self
            .generations
            .get(&generation)
            .is_some_and(|row| row.retired)
        {
            return Err(StoreAdmissionError::Retired);
        }
        let id = self
            .next_slot
            .checked_add(1)
            .ok_or(StoreAdmissionError::IdentityExhausted)?;
        if self
            .reservations
            .is_at_capacity(charged_bytes, &shared.config)
        {
            return Err(StoreAdmissionError::Capacity);
        }

        // Nothing fallible remains after reservation. These allocations and
        // their map insertion occur under this same guard, before Queued.
        self.reservations.reserve(charged_bytes);
        if self.engine_metrics.is_none() {
            self.engine_metrics = Some(shared.metrics.clone());
        }
        self.next_slot = id;
        let reservation = CompletionReservation {
            id,
            generation,
            payload_bytes,
            charged_bytes,
        };
        let row = self
            .generations
            .entry(generation)
            .or_insert_with(|| CompletionGeneration {
                reserved_count: 0,
                retired: false,
                published: Fifo::default(),
                metrics: metrics.clone(),
            });
        row.reserved_count += 1;
        self.slots.insert(
            id,
            CompletionSlot {
                reservation,
                item: None,
                next: None,
            },
        );
        metrics
            .reserved_completion_count
            .fetch_add(1, Ordering::SeqCst);
        metrics
            .reserved_completion_bytes
            .fetch_add(charged_bytes, Ordering::SeqCst);
        shared
            .metrics
            .reserved_completion_count
            .fetch_add(1, Ordering::SeqCst);
        shared
            .metrics
            .reserved_completion_bytes
            .fetch_add(charged_bytes, Ordering::SeqCst);
        Ok(reservation)
    }

    pub(super) fn publish(
        &mut self,
        reservation: CompletionReservation,
        completion: PluginCompletion,
        encoded_len: usize,
        metrics: &PluginWorkerEngineMetrics,
    ) {
        assert!(
            encoded_len <= reservation.payload_bytes,
            "payload exceeds its allowance"
        );
        let slot = self
            .slots
            .get_mut(&reservation.id)
            .expect("publisher retains reservation");
        assert_eq!(
            slot.reservation, reservation,
            "reservation identity is exact"
        );
        assert!(slot.item.is_none(), "terminal seal publishes exactly once");
        slot.item = Some(PluginCompletionItem {
            completion,
            encoded_len,
        });
        let row = self
            .generations
            .get_mut(&reservation.generation)
            .expect("reserved generation retained");
        let queue = if row.retired {
            &mut self.retired
        } else {
            &mut row.published
        };
        if let Some(tail) = queue.tail {
            self.slots
                .get_mut(&tail)
                .expect("published tail exists")
                .next = Some(reservation.id);
        } else {
            queue.head = Some(reservation.id);
            if !row.retired {
                assert!(self
                    .active_fronts
                    .insert((encoded_len, reservation.generation)));
            }
        }
        queue.tail = Some(reservation.id);
        row.metrics
            .undrained_completions
            .fetch_add(1, Ordering::SeqCst);
        metrics.undrained_completions.fetch_add(1, Ordering::SeqCst);
    }

    /// Called while admission is sealed. A publisher may have won its terminal
    /// seal and still not have appended: its reservation keeps this row alive.
    pub(super) fn retire(&mut self, generation: u64) {
        let Some(row) = self.generations.get_mut(&generation) else {
            return;
        };
        if row.retired {
            return;
        }
        row.retired = true;
        let published = std::mem::take(&mut row.published);
        if let Some(head) = published.head {
            let item = self.slots[&head].item.as_ref().expect("published head");
            assert!(self.active_fronts.remove(&(item.encoded_len, generation)));
            if let Some(tail) = self.retired.tail {
                self.slots.get_mut(&tail).expect("retired tail exists").next = Some(head);
            } else {
                self.retired.head = Some(head);
            }
            self.retired.tail = published.tail;
        }
    }

    pub(super) fn take_fitting(
        &mut self,
        max_bytes: usize,
        metrics: &PluginWorkerEngineMetrics,
    ) -> Option<PluginCompletionItem> {
        let fitting_retired = self.retired.head.filter(|head| {
            self.slots[head]
                .item
                .as_ref()
                .expect("retired head published")
                .encoded_len
                <= max_bytes
        });
        let (id, retired) = if let Some(id) = fitting_retired {
            (id, true)
        } else {
            let &(bytes, generation) = self.active_fronts.first()?;
            if bytes > max_bytes {
                return None;
            }
            let id = self.generations[&generation]
                .published
                .head
                .expect("indexed active head");
            assert!(self.active_fronts.remove(&(bytes, generation)));
            (id, false)
        };
        let slot = self.slots.remove(&id).expect("indexed slot exists");
        let generation = slot.reservation.generation;
        let row = self
            .generations
            .get_mut(&generation)
            .expect("reserved generation exists");
        let queue = if retired {
            &mut self.retired
        } else {
            &mut row.published
        };
        queue.head = slot.next;
        if queue.head.is_none() {
            queue.tail = None;
        }
        if !retired {
            if let Some(head) = queue.head {
                let bytes = self.slots[&head]
                    .item
                    .as_ref()
                    .expect("next head published")
                    .encoded_len;
                assert!(self.active_fronts.insert((bytes, generation)));
            }
        }
        row.reserved_count = row
            .reserved_count
            .checked_sub(1)
            .expect("one slot retires once");
        let charged_bytes = slot.reservation.charged_bytes;
        self.reservations.release(charged_bytes);
        row.metrics
            .reserved_completion_count
            .fetch_sub(1, Ordering::SeqCst);
        row.metrics
            .reserved_completion_bytes
            .fetch_sub(charged_bytes, Ordering::SeqCst);
        row.metrics
            .undrained_completions
            .fetch_sub(1, Ordering::SeqCst);
        metrics
            .reserved_completion_count
            .fetch_sub(1, Ordering::SeqCst);
        metrics
            .reserved_completion_bytes
            .fetch_sub(charged_bytes, Ordering::SeqCst);
        metrics.undrained_completions.fetch_sub(1, Ordering::SeqCst);
        if row.reserved_count == 0 {
            assert!(row.published.head.is_none());
            self.generations.remove(&generation);
        }
        Some(slot.item.expect("selected slot published"))
    }

    #[cfg(test)]
    pub(super) fn last_published_len(&self, generation: u64) -> Option<usize> {
        let tail = self.generations.get(&generation)?.published.tail?;
        Some(self.slots.get(&tail)?.item.as_ref()?.encoded_len)
    }

    #[cfg(test)]
    pub(super) fn counts(&self) -> (usize, usize, usize, usize) {
        (
            self.slots.len(),
            self.generations.len(),
            self.reservations.reserved_count,
            self.reservations.reserved_bytes,
        )
    }

    #[cfg(test)]
    pub(super) fn exhaust_slot_identities(&mut self) {
        self.next_slot = u64::MAX;
    }
}

impl Drop for CompletionStore {
    fn drop(&mut self) {
        let Some(metrics) = &self.engine_metrics else {
            return;
        };
        for slot in self.slots.values() {
            let row = &self.generations[&slot.reservation.generation];
            row.metrics
                .reserved_completion_count
                .fetch_sub(1, Ordering::SeqCst);
            row.metrics
                .reserved_completion_bytes
                .fetch_sub(slot.reservation.charged_bytes, Ordering::SeqCst);
            metrics
                .reserved_completion_count
                .fetch_sub(1, Ordering::SeqCst);
            metrics
                .reserved_completion_bytes
                .fetch_sub(slot.reservation.charged_bytes, Ordering::SeqCst);
            if slot.item.is_some() {
                row.metrics
                    .undrained_completions
                    .fetch_sub(1, Ordering::SeqCst);
                metrics.undrained_completions.fetch_sub(1, Ordering::SeqCst);
            }
            self.reservations.release(slot.reservation.charged_bytes);
        }
        debug_assert_eq!(self.reservations.reserved_count, 0);
        debug_assert_eq!(self.reservations.reserved_bytes, 0);
    }
}
