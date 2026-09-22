use std::fmt;
use std::io;
use std::thread::JoinHandle;

/// An opaque resource funded by the host before its allocation.
///
/// For an attached worker, Core drops the resource after its join returns, including a
/// returned worker panic. If Core loses the unjoined record or unwinds before
/// joining, Core retains the resource until process exit without running its
/// destructor. A resource for a worker that never starts drops normally.
///
/// Core frees the outer box before dropping the supplied value. The host owns
/// admission and sizing, including that box. Core does not inspect the value.
pub struct PluginWorkerResource {
    resource: Option<Box<dyn WorkerResourceRelease>>,
}

impl PluginWorkerResource {
    /// Allocate storage for a resource that the host has already funded.
    pub fn new<T: Send + 'static>(resource: T) -> Self {
        Self {
            resource: Some(Box::new(resource)),
        }
    }
}

impl Drop for PluginWorkerResource {
    fn drop(&mut self) {
        if let Some(resource) = self.resource.take() {
            resource.release();
        }
    }
}

trait WorkerResourceRelease: Send {
    fn release(self: Box<Self>);
}

// This function boundary frees the outer allocation before the returned resource drops.
#[allow(clippy::boxed_local)]
fn unbox<T>(boxed: Box<T>) -> T {
    *boxed
}

impl<T: Send + 'static> WorkerResourceRelease for T {
    fn release(self: Box<Self>) {
        let resource = unbox(self);
        drop(resource);
    }
}

/// A fixed-capacity batch with a host-funded metadata resource.
///
/// Metadata covers the input buffer, join buffer, executor allocation, and its
/// own resource box. The host funds these allocations before construction.
/// Core releases metadata after destroying its covered owners. If that cleanup
/// fails to return, Core retains the metadata resource until process exit.
/// Executor clones can keep metadata alive after unload returns.
pub struct PluginWorkerResources {
    resources: Option<Vec<PluginWorkerResource>>,
    capacity: usize,
    metadata: WorkerMetadataGuard,
}

impl PluginWorkerResources {
    /// Reserve the input buffer while the metadata resource is already guarded.
    pub fn with_capacity(capacity: usize, metadata: PluginWorkerResource) -> Self {
        let mut batch = Self {
            resources: Some(Vec::new()),
            capacity,
            metadata: WorkerMetadataGuard(Some(metadata)),
        };
        batch
            .resources
            .as_mut()
            .expect("worker resource input buffer")
            .reserve_exact(capacity);
        batch
    }

    /// Insert without growth, or return the original resource when full.
    pub fn try_push(&mut self, resource: PluginWorkerResource) -> Result<(), PluginWorkerResource> {
        let resources = self
            .resources
            .as_mut()
            .expect("worker resource input buffer");
        if resources.len() == self.capacity {
            return Err(resource);
        }
        resources.push(resource);
        Ok(())
    }

    pub(super) fn len(&self) -> usize {
        self.resources
            .as_ref()
            .expect("worker resource input buffer")
            .len()
    }
}

impl Drop for PluginWorkerResources {
    fn drop(&mut self) {
        drop(self.resources.take());
        self.metadata.release();
    }
}

#[derive(Default)]
pub(super) struct WorkerMetadataGuard(Option<PluginWorkerResource>);

impl WorkerMetadataGuard {
    pub(super) fn release(&mut self) {
        drop(self.0.take());
    }
}

impl Drop for WorkerMetadataGuard {
    fn drop(&mut self) {
        if let Some(resource) = self.0.take() {
            std::mem::forget(resource);
        }
    }
}

pub(super) struct WorkerResourceConstruction {
    input: Option<std::vec::IntoIter<PluginWorkerResource>>,
    join_handles: Option<Vec<WorkerJoinRecord>>,
    metadata: WorkerMetadataGuard,
}

impl WorkerResourceConstruction {
    pub(super) fn new(resources: Option<PluginWorkerResources>, capacity: usize) -> Self {
        let mut construction = match resources {
            Some(mut batch) => Self {
                input: batch.resources.take().map(Vec::into_iter),
                join_handles: Some(Vec::new()),
                metadata: std::mem::take(&mut batch.metadata),
            },
            None => Self {
                input: None,
                join_handles: Some(Vec::new()),
                metadata: WorkerMetadataGuard::default(),
            },
        };
        construction
            .join_handles
            .as_mut()
            .expect("worker join buffer")
            .reserve_exact(capacity);
        construction
    }

    pub(super) fn next_resource(&mut self) -> Option<PluginWorkerResource> {
        self.input.as_mut().map(|input| {
            input
                .next()
                .expect("validated plugin worker resource count")
        })
    }

    pub(super) fn push(&mut self, record: WorkerJoinRecord) {
        self.join_handles
            .as_mut()
            .expect("worker join buffer")
            .push(record);
    }

    pub(super) fn destroy_input(&mut self) {
        // Keep retention active if an unused resource destructor panics.
        let metadata = std::mem::take(&mut self.metadata);
        let input_iter = self.input.take();
        drop(input_iter);
        self.metadata = metadata;
    }

    pub(super) fn take_join_handles(&mut self) -> Option<Vec<WorkerJoinRecord>> {
        self.join_handles.take()
    }

    pub(super) fn take_metadata(&mut self) -> WorkerMetadataGuard {
        std::mem::take(&mut self.metadata)
    }
}

impl Drop for WorkerResourceConstruction {
    fn drop(&mut self) {
        drop(self.input.take());
        drop(self.join_handles.take());
        self.metadata.release();
    }
}

/// The host supplied a resource count different from the executor width.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PluginWorkerResourceCountMismatch {
    /// Number of resources required before loading this plugin.
    pub expected: usize,
    /// Number of resources supplied by the host.
    pub actual: usize,
}

impl fmt::Display for PluginWorkerResourceCountMismatch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "plugin worker resource count mismatch: expected {}, received {}",
            self.expected, self.actual
        )
    }
}

impl std::error::Error for PluginWorkerResourceCountMismatch {}

/// Retention starts before spawn can create a thread. Only a returned spawn
/// failure or a returned join can release the resource.
pub(super) struct WorkerJoinRecord {
    handle: Option<JoinHandle<()>>,
    resource: Option<PluginWorkerResource>,
}

impl WorkerJoinRecord {
    pub(super) fn spawn(
        resource: Option<PluginWorkerResource>,
        start: impl FnOnce() -> io::Result<JoinHandle<()>>,
    ) -> io::Result<Self> {
        let mut record = Self {
            handle: None,
            resource,
        };
        match start() {
            Ok(handle) => {
                record.handle = Some(handle);
                Ok(record)
            }
            Err(error) => {
                // No thread started. Earlier records keep their own resources.
                drop(record.resource.take());
                Err(error)
            }
        }
    }

    pub(super) fn join(mut self) -> std::thread::Result<()> {
        let handle = self.handle.take().expect("plugin worker join handle");
        let result = handle.join();
        // A returned Err also proves that this exact thread has exited.
        drop(self.resource.take());
        result
    }
}

impl Drop for WorkerJoinRecord {
    fn drop(&mut self) {
        if let Some(resource) = self.resource.take() {
            // Detachment and unwinding do not prove thread exit. The host
            // approved retaining this reservation until process exit.
            std::mem::forget(resource);
        }
    }
}
