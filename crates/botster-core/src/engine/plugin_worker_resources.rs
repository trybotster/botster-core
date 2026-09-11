use std::fmt;
use std::io;
use std::thread::JoinHandle;

/// An opaque resource funded by the host before a plugin worker starts.
///
/// Core drops the resource only after that worker's join returns, including a
/// returned worker panic. If Core loses the unjoined record or unwinds before
/// joining, Core retains the resource until process exit without running its
/// destructor. A resource for a worker that never starts drops normally.
///
/// The host owns admission and sizing. Core does not inspect the resource.
pub type PluginWorkerResource = Box<dyn Send + 'static>;

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
