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
    /// (LC-2), raises the host epoch (DP-8) and reads the ids of the registry's rows (ID-1). It starts no thread, spawns no
    /// process and reads no clock (TM-1).
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
        let identity = botster_terminal_ghostty::terminal_identity();
        let cfg = EngineConfig {
            limits: config.limits,
            features,
            host_epoch,
            worker_path,
            worker_protocol: WORKER_PROTOCOL,
            shadow_answerable: Vec::new(),
            terminal_identity: TerminalIdentity {
                term: identity.term,
                terminfo_source: identity.terminfo_source,
            },
        };
        Ok(Core {
            driver: HostDriver::open(cfg, edges)?,
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
    ) -> Result<AttachResult, AttachRefused> {
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

#[cfg(test)]
#[cfg(feature = "slow")]
mod slow_tests {
    use super::*;
    use std::collections::BTreeMap;

    fn config(dir: &std::path::Path) -> OpenConfig {
        OpenConfig {
            data_dir: dir.join("d"),
            worker_path: Some(std::path::PathBuf::from("/bin/true")),
            limits: CoreLimits::default(),
        }
    }

    fn sid(name: &str) -> SessionId {
        SessionId(name.into())
    }

    fn pump(core: &mut Core) -> Vec<Event> {
        let mut out = Vec::new();
        loop {
            #[allow(clippy::disallowed_methods)] // a test passes a real instant to the pump
            let monotonic = Instant::now();
            let report = core.pump(Now {
                monotonic,
                unix: 1_000_000,
            });
            out.extend(core.poll_events(64));
            if !report.more {
                return out;
            }
        }
    }

    /// Core 2, 9B: every call of the facade reaches the driver and returns what the driver returns: the constants, the
    /// reads, the service calls and the silence threshold.
    #[test]
    fn every_call_reaches_the_driver() {
        let tmp = tempfile::tempdir().unwrap();
        // Limits that are not the defaults: `limits` returns the ones that the host opened with.
        let mut open = config(tmp.path());
        open.limits.pump_events += 1;
        let limits = open.limits.clone();
        let mut core = Core::open(open).expect("open");
        assert_eq!(core.limits(), limits);
        assert_ne!(core.limits(), CoreLimits::default());
        assert_eq!(core.worker_protocol(), WORKER_PROTOCOL);
        assert!(core.adoptable_worker_protocols().contains(&WORKER_PROTOCOL));
        assert_eq!(
            core.worker_protocol_compatibility(Some(WORKER_PROTOCOL)),
            WorkerCompatibility::Compatible
        );
        assert!(core.features().names.contains(&Feature::Silence));
        assert_eq!(core.terminal_identity().term, "xterm-ghostty");
        let identity = botster_terminal_ghostty::terminal_identity();
        assert_eq!(
            core.terminal_identity().terminfo_source,
            identity.terminfo_source
        );
        assert!(core.list().is_empty());
        assert!(core.status().sessions.is_empty());
        assert_eq!(core.diagnostics()["sessions"], 0);
        assert_eq!(core.next_deadline(), None);
        assert_eq!(
            core.snapshot_formats(&sid("nope")).unwrap_err().code,
            ErrorCode::UnknownSession
        );
        assert_eq!(
            core.set_silence_threshold(&sid("nope"), None)
                .unwrap_err()
                .code,
            ErrorCode::UnknownSession
        );
        let service = ServiceId([0; 32]);
        let frame = OutboundFrame {
            frame_type: 0,
            payload: botster_route_codec::prelude::HexBytes(vec![]),
        };
        assert_eq!(
            core.service_send(&service, 0, &frame).unwrap_err(),
            SendError::UnknownService
        );
        assert_eq!(
            core.service_recv(&service, 0).unwrap_err(),
            RecvError::UnknownService
        );
        assert_eq!(
            core.service_report(&service).unwrap_err().code,
            ErrorCode::UnknownService
        );
        assert_eq!(
            core.service_log_tail(&service, 8).unwrap_err().code,
            ErrorCode::UnknownService
        );
        core.release(CaptureId(1));
        core.release_owner(&ClientId("c".into()));
        // `begin`, `pump` and `poll_events` carry a create through, and the reads then see the session.
        let create = core
            .begin(Op::Create {
                session: sid("s1"),
                request: SpawnRequest {
                    argv: vec!["/bin/true".into()],
                    env: BTreeMap::new(),
                    cwd: "/".into(),
                    size: Size {
                        rows: 24,
                        cols: 80,
                        cell_px: None,
                    },
                    labels: BTreeMap::new(),
                    color_profile: None,
                    notification_policy: None,
                    size_policy: None,
                },
            })
            .unwrap();
        let events = pump(&mut core);
        assert!(events.iter().any(
            |e| matches!(e, Event::Completed { op, result: OpResult::Ok(_) } if *op == create)
        ));
        assert_eq!(core.list().len(), 1);
        assert_eq!(core.status().sessions.len(), 1);
        assert_eq!(core.diagnostics()["sessions"], 1);
        assert_eq!(core.get(&sid("s1")).unwrap().state, SessionState::Created);
        assert_eq!(core.cancel(create), CancelResult::TooLate);
        core.set_silence_threshold(&sid("s1"), Some(Duration::from_secs(5)))
            .unwrap();
        assert!(core.terminal_state(&sid("s1")).is_err());
        assert!(core.read_page(CaptureId(1), 0).is_err());
        assert!(core.tap_read(&sid("s1"), 8).is_ok());
    }
}
