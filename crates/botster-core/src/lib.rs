//! Botster Core: the facade. It re-exports the contract and holds the `Core` handle.
//!
//! The other crates of the workspace are published with the same version, but they are not part of the API: they carry no
//! compatibility promise (plan 2.6). A host imports this crate only.

use botster_core_contract::prelude::*;
use botster_core_host::driver::{check_open, HostDriver};
use botster_core_host::EngineConfig;
use botster_core_sys::storage::{DataDir, OpenError};
use std::cell::Cell;
use std::collections::BTreeSet;
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::{Duration, Instant};

mod real;

/// The Core contract crate, whole.
pub use botster_core_contract as contract;

/// The names that a user of Core needs: the prelude of the contract.
pub mod prelude {
    pub use botster_core_contract::prelude::*;
}

/// The worker protocol number T of this Core (AD-4, A6-2).
const WORKER_PROTOCOL: u8 = botster_worker_core::WORKER_PROTOCOL;

/// The Core handle. It is `Send` and not `Sync`: one thread at a time owns it, and it can move between threads.
///
/// Clause: Core TH-1, Core LC-1, Core LC-2, Core LC-12.
pub struct Core {
    driver: HostDriver<real::RealEdges>,
    /// `Cell` is `Send` and not `Sync`, so the handle has the same two properties.
    _not_sync: PhantomData<Cell<()>>,
}

fn open_error(error: OpenError) -> CoreError {
    match error {
        OpenError::InUse => CoreError::new(
            ErrorCode::DataDirInUse,
            "another host holds the data directory",
        ),
        OpenError::Unsafe(why) => CoreError::new(
            ErrorCode::InvalidConfig {
                field: "data_dir".into(),
            },
            why,
        ),
        OpenError::Io(error) => CoreError::new(
            ErrorCode::RegistryFailed { uncertain: false },
            format!("the data directory failed: {error}"),
        ),
        OpenError::CorruptEpoch => CoreError::new(
            ErrorCode::RegistryFailed { uncertain: false },
            "the host epoch row is corrupt",
        ),
    }
}

impl Core {
    /// Opens a host over `config.data_dir`: validates the limits (9B), needs a worker path (LC-1), takes the exclusive lock
    /// (LC-2) and raises the host epoch (DP-8). It starts no thread and spawns no process.
    ///
    /// # Errors
    /// `InvalidConfig`, `MissingWorkerPath`, `DataDirInUse`, or `RegistryFailed` when the directory cannot be used.
    pub fn open(config: OpenConfig) -> Result<Core, CoreError> {
        let worker_path = check_open(&config)?;
        let data = DataDir::open(&config.data_dir).map_err(open_error)?;
        let (edges, host_epoch) =
            real::RealEdges::new(data, &config.data_dir).map_err(|error| {
                CoreError::new(
                    ErrorCode::RegistryFailed { uncertain: false },
                    format!("the control socket failed: {error}"),
                )
            })?;
        let features = Features {
            names: BTreeSet::from([Feature::Silence, Feature::NotificationPolicy]),
            service_preamble_versions: vec![1],
        };
        let cfg = EngineConfig {
            limits: config.limits,
            features,
            host_epoch,
            worker_path,
            worker_protocol: WORKER_PROTOCOL,
            shadow_answerable: Vec::new(),
            terminal_identity: TerminalIdentity {
                term: "xterm-ghostty".to_string(),
                // PLACEHOLDER (Core TI-1, A2-8): the terminfo source of the pinned emulator comes from P2's binding
                // (`botster-terminal-ghostty`), which is not on `v1` yet. Core must not invent it. The follow-up PR wires
                // `botster_terminal_ghostty::terminal_identity()` and removes this value; the a2_8 ids stay pending until then.
                terminfo_source: String::new(),
            },
        };
        Ok(Core {
            driver: HostDriver::new(cfg, edges, Instant::now()),
            _not_sync: PhantomData,
        })
    }
}

impl CoreApi for Core {
    fn begin(&mut self, op: Op) -> Result<OpId, CoreError> {
        self.driver.begin(op)
    }

    fn pump(&mut self, now: Now) -> PumpReport {
        self.driver.pump(now)
    }

    fn poll_events(&mut self, max: usize) -> Vec<Event> {
        self.driver.poll_events(max)
    }

    fn wake_handle(&self) -> Arc<dyn WakeHandle> {
        self.driver.wake_handle()
    }

    fn next_deadline(&self) -> Option<Instant> {
        self.driver.next_deadline()
    }

    fn cancel(&mut self, op: OpId) -> CancelResult {
        self.driver.cancel(op)
    }

    fn get(&self, id: &SessionId) -> Result<SessionRecord, CoreError> {
        self.driver.get(id)
    }

    fn list(&self) -> Vec<SessionRecord> {
        self.driver.list()
    }

    fn status(&self) -> Status {
        self.driver.status()
    }

    fn diagnostics(&self) -> serde_json::Value {
        self.driver.diagnostics()
    }

    fn terminal_state(&self, id: &SessionId) -> Result<TerminalState, CoreError> {
        self.driver.terminal_state(id)
    }

    fn read_page(&self, capture: CaptureId, page: u32) -> Result<Page, CoreError> {
        self.driver.read_page(capture, page)
    }

    fn release(&mut self, capture: CaptureId) {
        self.driver.release(capture);
    }

    fn release_owner(&mut self, client: &ClientId) {
        self.driver.release_owner(client);
    }

    fn snapshot_formats(&self, session: &SessionId) -> Result<Vec<SnapshotFormat>, CoreError> {
        self.driver.snapshot_formats(session)
    }

    fn shadow_answerable_kinds(&self) -> Vec<botster_route_codec::prelude::QueryKind> {
        self.driver.shadow_answerable_kinds()
    }

    fn attach(
        &mut self,
        client: ClientId,
        session: SessionId,
        transport: RouteTransport,
        options: AttachOptions,
    ) -> Result<AttachResult, CoreError> {
        self.driver.attach(client, session, transport, options)
    }

    fn tap_read(&mut self, session: &SessionId, max: usize) -> Result<TapChunk, CoreError> {
        self.driver.tap_read(session, max)
    }

    fn set_silence_threshold(
        &mut self,
        session: &SessionId,
        threshold: Option<Duration>,
    ) -> Result<(), CoreError> {
        self.driver.set_silence_threshold(session, threshold)
    }

    fn service_send(
        &mut self,
        id: &ServiceId,
        lane: u8,
        frame: &OutboundFrame,
    ) -> Result<(), SendError> {
        self.driver.service_send(id, lane, frame)
    }

    fn service_recv(&mut self, id: &ServiceId, lane: u8) -> Result<Option<Frame>, RecvError> {
        self.driver.service_recv(id, lane)
    }

    fn service_report(&self, id: &ServiceId) -> Result<SpawnReport, CoreError> {
        self.driver.service_report(id)
    }

    fn service_log_tail(&self, id: &ServiceId, max: usize) -> Result<Vec<u8>, CoreError> {
        self.driver.service_log_tail(id, max)
    }

    fn features(&self) -> Features {
        self.driver.features()
    }

    fn limits(&self) -> CoreLimits {
        self.driver.limits()
    }

    fn worker_protocol(&self) -> u8 {
        self.driver.worker_protocol()
    }

    fn adoptable_worker_protocols(&self) -> BTreeSet<u8> {
        self.driver.adoptable_worker_protocols()
    }

    fn worker_protocol_compatibility(&self, protocol: Option<u8>) -> WorkerCompatibility {
        self.driver.worker_protocol_compatibility(protocol)
    }

    fn terminal_identity(&self) -> TerminalIdentity {
        self.driver.terminal_identity()
    }
}
