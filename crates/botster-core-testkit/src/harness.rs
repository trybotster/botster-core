//! `TestkitHarness` (plan 4.2, default tier): the `CoreHarness` over the testkit.
//!
//! `open` builds the real `Core`: the host driver of `botster-core-host` over in-memory edges (`crate::core`). The harness holds
//! no stand-in for Core (BUILD.md: no hand-written stand-in for a real component). A transcript that needs a control or an
//! edge that does not exist yet gets `unsupported_control`, which is never a pass; an id stays in `conformance/core-pending.txt`
//! until it passes.

use crate::controls::ControlRegistry;
use crate::core::{core_features, Directories, RunInputs};
use crate::refusal::{CoreRecord, RefusalHandle, RefusalLayer};
use crate::resume_controls::CaptureLog;
use crate::scheduler::SchedulerHandle;
use crate::worker::{ProcessTable, TestkitCore, Workers};
use botster_core_conformance::{
    ControlError, CoreHarness, DataDirRef, OpenSpec, RouteClient, WorkerBuild, WorkerRef,
};
use botster_core_contract::prelude::*;
use botster_core_host::driver::HostWake;
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// The default-tier harness for one seed (foundation design 6.1: seeds 0 to 31).
#[derive(Debug)]
pub struct TestkitHarness {
    controls: ControlRegistry,
    seed: u64,
    directories: Directories,
    /// Every in-process worker of the run, in one `Sim` with the run's seeded stream (plan 4.1, A5-2).
    workers: Workers,
    /// The scripted synchronous refusals of each handle (plan 4.2a). The harness arms them; the layer in front of the handle's
    /// Core consumes them.
    refusals: BTreeMap<String, RefusalHandle>,
    /// What each handle's Core reported in its diagnostics, as the layer in front of it records it (`parked_work`, `op_posted`).
    records: BTreeMap<String, CoreRecord>,
    /// The data directory and the process table of each handle that `open` built, for the controls.
    handles: BTreeMap<String, HandleEdges>,
    /// Core TH-1 for the facade's `Core` type, as the runner checked it at compile time (`with_core_type`).
    core_send_not_sync: Option<bool>,
}

/// What the controls reach of one handle: its data directory and the process table of its host.
/// The edge failures of one host and its wake object, which a control that releases held work signals.
pub(crate) type HostFaults = (Arc<Mutex<crate::core::Faults>>, Arc<dyn HostWake>);
/// The record of the worker's route reports that one host consumed (`route_reports`, R-66), and its host's wake object.
pub(crate) type HostReports = (
    Arc<Mutex<crate::route_reports::RouteReports>>,
    Arc<dyn HostWake>,
);

struct HandleEdges {
    dir: String,
    processes: ProcessTable,
    /// The captures that its host completed (`oracle_resume`).
    captures: CaptureLog,
    /// The route-ended causes of its routes (A2-3).
    route_ends: crate::route_client::RouteEnds,
    /// The session instance of each of its routes (`route_fill`).
    route_instances: crate::route_client::RouteInstances,
    /// The failures that a control injects at its host's edges (`fail_handoff`, `hold_handoff`).
    faults: Arc<Mutex<crate::core::Faults>>,
    /// The worker's route reports that its host consumed (`route_reports`, R-66).
    reports: Arc<Mutex<crate::route_reports::RouteReports>>,
    /// The wake object of its host: a control that releases held work signals it.
    wake: Arc<dyn HostWake>,
    /// The ops that `scheduler_hold` holds on its host (Core A5-2, R-88 group E).
    holds: crate::op_holds::SharedHolds,
}

impl std::fmt::Debug for HandleEdges {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HandleEdges")
            .field("dir", &self.dir)
            .finish_non_exhaustive()
    }
}

impl TestkitHarness {
    pub fn new(seed: u64) -> TestkitHarness {
        let start = Instant::now();
        TestkitHarness {
            controls: crate::controls::registered_controls(),
            seed,
            directories: Directories::default(),
            workers: Workers::new(SchedulerHandle::with_seed(seed), start),
            refusals: BTreeMap::new(),
            records: BTreeMap::new(),
            handles: BTreeMap::new(),
            core_send_not_sync: None,
        }
    }

    /// Core TH-1: the runner names the facade's `Core` type, which this crate does not see. It checks the type at compile
    /// time and passes the answer here; `type_check: send_not_sync` reads it.
    pub fn with_core_type(mut self, send_not_sync: bool) -> TestkitHarness {
        self.core_send_not_sync = Some(send_not_sync);
        self
    }

    /// The seed of this run. `with_seed` of every `Sim` that `open` builds takes it (Core A5-2).
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Puts the refusal layer of the handle in front of its Core. `open` calls this with the Core that it built, so every Core
    /// that a transcript sees can be scripted (Core A5-3 timing 1).
    pub fn with_refusals(&mut self, handle: &str, core: Box<dyn CoreApi>) -> Box<dyn CoreApi> {
        let script = self.refusals.entry(handle.to_string()).or_default().clone();
        let record = CoreRecord::default();
        self.records.insert(handle.to_string(), record.clone());
        Box::new(RefusalLayer::new(core, script, record))
    }

    /// The record of the Core of `handle`, once `open` built it (`parked_work`, `op_posted`).
    pub(crate) fn record_of(&self, handle: &str) -> Option<&CoreRecord> {
        self.records.get(handle)
    }

    /// Returns the refusal script for a handle. The refusal module arms this script.
    pub(crate) fn refusal_script(&mut self, handle: &str) -> RefusalHandle {
        self.refusals.entry(handle.to_string()).or_default().clone()
    }

    /// The data directory of `handle`, once `open` built it.
    pub(crate) fn directory_of(&self, handle: &str) -> Option<&str> {
        self.handles.get(handle).map(|h| h.dir.as_str())
    }

    /// The captures that the host of `handle` completed, once `open` built it.
    pub(crate) fn captures_of(&self, handle: &str) -> Option<CaptureLog> {
        self.handles.get(handle).map(|h| h.captures.clone())
    }

    /// The edge failures and the wake object of the host of `handle`, once `open` built it.
    pub(crate) fn faults_of(&self, handle: &str) -> Option<HostFaults> {
        self.handles
            .get(handle)
            .map(|h| (Arc::clone(&h.faults), Arc::clone(&h.wake)))
    }

    /// The worker's route reports that the host of `handle` consumed, and its wake object, once `open` built it.
    pub(crate) fn reports_of(&self, handle: &str) -> Option<HostReports> {
        self.handles
            .get(handle)
            .map(|h| (Arc::clone(&h.reports), Arc::clone(&h.wake)))
    }

    /// The holds of `scheduler_hold` on the host of `handle`, and its host's wake object, which a release signals.
    pub(crate) fn holds_of(
        &self,
        handle: &str,
    ) -> Option<(crate::op_holds::SharedHolds, Arc<dyn HostWake>)> {
        self.handles
            .get(handle)
            .map(|h| (Arc::clone(&h.holds), Arc::clone(&h.wake)))
    }

    /// The process table of the host of `handle`, once `open` built it.
    pub(crate) fn processes_of(&self, handle: &str) -> Option<&ProcessTable> {
        self.handles.get(handle).map(|h| &h.processes)
    }

    /// The data directories of the run.
    pub(crate) fn directories(&self) -> &Directories {
        &self.directories
    }

    /// The in-process workers of the run.
    pub(crate) fn workers(&self) -> &Workers {
        &self.workers
    }
}

/// The `CoreLimits` of an `OpenSpec` (Core 9B). Both harnesses read them the same way.
pub(crate) fn limits_of(spec: &OpenSpec) -> Result<CoreLimits, CoreError> {
    serde_json::from_value(spec.limits.clone()).map_err(|error| {
        CoreError::new(
            ErrorCode::InvalidConfig {
                field: "limits".to_string(),
            },
            format!("the limits are not CoreLimits: {error}"),
        )
    })
}

impl CoreHarness for TestkitHarness {
    /// Builds the real `Core` over an in-memory data directory (LC-1, LC-2, 9B).
    fn open(&mut self, spec: &OpenSpec) -> Result<Box<dyn CoreApi>, CoreError> {
        let limits = limits_of(spec)?;
        let config = OpenConfig {
            data_dir: spec.data_dir.0.clone().into(),
            worker_path: spec.worker.as_ref().map(|w| {
                w.file_name
                    .clone()
                    .unwrap_or_else(|| "botster-worker".to_string())
                    .into()
            }),
            limits,
        };
        let spawner = self.workers.spawner(&spec.data_dir.0);
        let table = spawner.table();
        let opened = self.directories.open(
            &spec.data_dir.0,
            &config,
            RunInputs {
                seed: self.seed,
                scheduler: self.workers.scheduler(),
            },
            core_features(),
            Some(Box::new(spawner)),
        )?;
        table.set_wake(Arc::clone(&opened.wake));
        let (faults, reports, wake) = (opened.faults, opened.reports, Arc::clone(&opened.wake));
        let holds = opened.holds;
        let core = Box::new(TestkitCore::new(
            opened.driver,
            opened.wake,
            self.workers.clone(),
        ));
        self.handles.insert(
            spec.handle.clone(),
            HandleEdges {
                dir: spec.data_dir.0.clone(),
                processes: table,
                captures: core.captures(),
                route_ends: core.route_ends(),
                route_instances: core.route_instances(),
                faults,
                reports,
                wake,
                holds,
            },
        );
        Ok(self.with_refusals(&spec.handle, core))
    }

    /// The harness passes the clock, so `advance_clock` moves it (Core TM-1, A5-1).
    fn injects_clock(&self) -> bool {
        true
    }

    /// A data directory is a name here: the in-memory registry of a `Sim` is keyed by it, and survives a drop and a reopen
    /// (Core LC-12, AD-1) once P1 provides the `Storage` user.
    fn data_dir(&mut self, name: &str) -> DataDirRef {
        DataDirRef(name.to_string())
    }

    /// `Current` is the in-process `Worker`. There is no `Previous` while the worker protocol is 1 (Core A6-2: no old
    /// protocol is fabricated).
    fn worker(&self, which: WorkerBuild) -> Option<WorkerRef> {
        (which == WorkerBuild::Current).then_some(WorkerRef {
            build: which,
            file_name: None,
        })
    }

    /// The in-process worker has no file; Core receives a path and fixes no name for it (Core E1-1).
    fn worker_named(&self, file_name: &str) -> Option<WorkerRef> {
        Some(WorkerRef {
            build: WorkerBuild::Current,
            file_name: Some(file_name.to_string()),
        })
    }

    /// The runner dropped the handle's Core (LC-12: its workers and its rows stay). The handle is gone, so a control that
    /// names it is `Bad`, the record of its Core included (`parked_work`, `op_posted`; R297-F1); a later `open` of the same
    /// directory, under any handle, reaches the rows and the workers again.
    fn drop_handle(&mut self, handle: &str) {
        self.handles.remove(handle);
        self.records.remove(handle);
    }

    /// The controls that the testkit builds (design 6.3, `docs/core-testkit-controls.md`). The others come with the machines
    /// and edges that they need.
    fn has_control(&self, op: &str) -> bool {
        self.controls.contains(op) && crate::alloc_controls::supports(op)
    }

    /// Core TH-1: the runner's compile-time answer for the facade's `Core` (`with_core_type`); `None` without one.
    fn core_is_send_not_sync(&self) -> Option<bool> {
        self.core_send_not_sync
    }

    fn control(&mut self, handle: &str, op: &str, args: &Value) -> Result<Value, ControlError> {
        let handler = self.controls.handler(op).ok_or(ControlError::Unsupported)?;
        handler(self, handle, args)
    }

    /// The statement steps that need no Core: `check_crates` (Core A5-1). The others wait for a Core and the real harness.
    fn statement(&mut self, kind: &str, spec: &Value) -> Result<Value, ControlError> {
        match kind {
            "check_crates" => {
                crate::statements::check_crates(crate::statements::workspace_root(), spec)
            }
            _ => Err(ControlError::Unsupported),
        }
    }

    /// `argv[0]` of a session program: the probe binary. The scripted program edge interprets the script of `argv[1]`
    /// in-process (Core A5-1).
    fn probe_binary(&self) -> String {
        botster_probe_script::PROBE_BINARY.to_string()
    }

    /// A connected in-memory stream: one end goes to `Core::attach`, which hands it to the worker (DP-2); the other end is
    /// the client's.
    fn attach_stream(
        &mut self,
        handle: &str,
        core: &mut dyn CoreApi,
        client: ClientId,
        session: &SessionId,
        options: AttachOptions,
    ) -> Result<(AttachResult, Box<dyn RouteClient>), CoreError> {
        let (worker_end, client_end) = crate::net::stream_pair(
            &self.workers.scheduler(),
            crate::route_client::ROUTE_STREAM_BYTES,
        );
        let result = core
            .attach(
                client,
                session.clone(),
                RouteTransport::Stream(StreamEndpoint::new(worker_end)),
                options,
            )
            .map_err(|refused| refused.error)?;
        let ends = self
            .handles
            .get(handle)
            .map(|h| h.route_ends.clone())
            .unwrap_or_default();
        let reports = self.handles.get(handle).map(|h| Arc::clone(&h.reports));
        // The route belongs to the session instance that the host held at the attach, which Core recorded: the attach can come
        // before the row is stored (Create and Start begun, not pumped), and the worker later still. So a fill reads the row
        // when it runs, and refuses a row of another instance.
        let instance = self
            .handles
            .get(handle)
            .and_then(|h| h.route_instances.of(result.route));
        let session_row = self
            .directory_of(handle)
            .and_then(|dir| {
                self.directories
                    .row_reader(dir, &botster_core_host::session::row_key(session))
            })
            .zip(instance)
            .map(|(rows, instance)| crate::route_client::SessionRow {
                rows,
                session: session.clone(),
                instance,
            });
        let route = crate::route_client::TestkitRoute::new(
            client_end,
            self.workers.clone(),
            result.route,
            ends,
        )
        .with_fill(crate::route_client::RouteFill {
            session: session_row,
            max_frame_bytes: usize::try_from(result.limits.max_frame_bytes).unwrap_or(usize::MAX),
        });
        let route = match reports {
            Some(reports) => route.with_reports(reports),
            None => route,
        };
        Ok((result, Box::new(route)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use botster_hub_conformance::route::RouteRead;
    use serde_json::json;

    /// R-87, R-88: `record_of` gives the record that the handle's refusal layer keeps, so the `parked_work` and `op_posted`
    /// controls read what
    /// the Core behind that layer reported.
    #[test]
    fn record_of_is_the_record_of_the_handles_layer() {
        let mut harness = TestkitHarness::new(0);
        assert!(harness.record_of("a").is_none(), "no layer yet");
        let (behind, _reached) = crate::refusal::tests::behind();
        let diagnostics = behind.diagnostics_slot();
        let mut core = harness.with_refusals("a", Box::new(behind));
        let s1 = SessionId("s1".into());
        assert!(!harness
            .record_of("a")
            .expect("the layer's record")
            .parked(&s1));
        *diagnostics.lock().unwrap() = json!({"parked_work": ["s1"]});
        core.pump(Now {
            monotonic: Instant::now(),
            unix: 0,
        });
        assert!(harness
            .record_of("a")
            .expect("the layer's record")
            .parked(&s1));
        // R297-F1: the record goes with the handle.
        harness.drop_handle("a");
        assert!(harness.record_of("a").is_none(), "a dropped handle");
    }

    fn spec() -> OpenSpec {
        OpenSpec {
            handle: "h".into(),
            data_dir: DataDirRef("d".into()),
            worker: Some(WorkerRef {
                build: WorkerBuild::Current,
                file_name: None,
            }),
            limits: json!({}),
        }
    }

    /// A Core opened with `limits`, a running session `s`, and a stream route attached to it after the one pump that hands
    /// the stream to the worker (Core OU-9, OR-1).
    fn attached_route(
        limits: serde_json::Value,
    ) -> (TestkitHarness, Box<dyn CoreApi>, Box<dyn RouteClient>) {
        let (harness, core, route, _) = attached_route_and_result(limits);
        (harness, core, route)
    }

    /// `attached_route`, and the result of the attach.
    fn attached_route_and_result(
        limits: serde_json::Value,
    ) -> (
        TestkitHarness,
        Box<dyn CoreApi>,
        Box<dyn RouteClient>,
        AttachResult,
    ) {
        let mut harness = TestkitHarness::new(0);
        let mut core = harness
            .open(&OpenSpec { limits, ..spec() })
            .expect("a Core");
        let session = SessionId("s".into());
        let now = || Now {
            monotonic: Instant::now(),
            unix: 1,
        };
        let pump_until = |core: &mut Box<dyn CoreApi>, state: SessionState| {
            for _ in 0..64 {
                core.pump(now());
                core.poll_events(64);
                if core.get(&SessionId("s".into())).map(|r| r.state) == Ok(state) {
                    return;
                }
            }
            panic!("the session is not {state:?}");
        };
        core.begin(Op::Create {
            session: session.clone(),
            request: SpawnRequest {
                argv: vec!["/bin/sh".into()],
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
        pump_until(&mut core, SessionState::Created);
        core.begin(Op::Start {
            id: session.clone(),
        })
        .unwrap();
        pump_until(&mut core, SessionState::Running);
        let options: AttachOptions = serde_json::from_value(
            json!({"file_directory": "/tmp", "answers_queries": false, "input": true}),
        )
        .unwrap();
        let (result, route) = harness
            .attach_stream("h", core.as_mut(), ClientId("c".into()), &session, options)
            .expect("attached");
        // The one pump that hands the stream to the worker (Core OU-9, OR-1).
        core.pump(now());
        (harness, core, route, result)
    }

    /// Core TH-3, OU-9: a stream route's frames reach the client with no host pump after the one that hands the stream over:
    /// a read runs the workers' ready work.
    #[test]
    fn a_stream_route_reads_its_frames_with_no_host_pump_after_the_handoff() {
        let (_harness, _core, mut route) = attached_route(json!({}));
        let deadline = botster_conformance::Deadline::after(None);
        let mut bytes = Vec::new();
        while bytes.len() < 5 {
            match route.read(64, &deadline) {
                RouteRead::Bytes(more) => bytes.extend(more),
                other => panic!("the frames come with no pump: {other:?} after {bytes:?}"),
            }
        }
        assert_eq!(
            bytes[4],
            botster_route_codec::prelude::TYPE_ATTACHED,
            "the first frame is attached"
        );
    }

    /// Core A2-3, OU-2b: the client end of a harness route reports the cause of a failed close that Core recorded: the
    /// route's worker write fails, and after the host's pump the client's end of stream names `write_failed`.
    #[test]
    fn a_harness_route_reports_the_cause_that_core_recorded() {
        let (_harness, mut core, mut route) = attached_route(json!({}));
        route.control("fail_writes", &json!({"on": true})).unwrap();
        let deadline = botster_conformance::Deadline::after(None);
        let now = || Now {
            monotonic: Instant::now(),
            unix: 1,
        };
        for _ in 0..64 {
            match route.read(1 << 20, &deadline) {
                RouteRead::Eof { ended: Some(ended) } => {
                    assert_eq!(ended, "write_failed");
                    return;
                }
                _ => {
                    core.pump(now());
                    core.poll_events(64);
                }
            }
        }
        panic!("the client end never reports the cause of the close");
    }

    /// The frames that a route reads until `live` (the stream prefix is 4 bytes, big-endian), then the read after it.
    fn read_baseline(route: &mut dyn RouteClient) -> (Vec<Vec<u8>>, RouteRead) {
        let deadline = botster_conformance::Deadline::after(None);
        let mut bytes = Vec::new();
        let mut frames = Vec::new();
        loop {
            while bytes.len() >= 4 {
                let len = u32::from_be_bytes(bytes[..4].try_into().unwrap()) as usize;
                if bytes.len() < 4 + len {
                    break;
                }
                let frame: Vec<u8> = bytes.drain(..4 + len).skip(4).collect();
                let live = frame[0] == botster_route_codec::prelude::TYPE_LIVE;
                frames.push(frame);
                if live {
                    assert!(bytes.is_empty(), "no frame after live: {bytes:?}");
                    return (frames, route.read(1 << 20, &deadline));
                }
            }
            match route.read(1 << 20, &deadline) {
                RouteRead::Bytes(more) => bytes.extend(more),
                other => panic!(
                    "the baseline comes with no pump: {other:?} after {} frames",
                    frames.len()
                ),
            }
        }
    }

    /// Steward ruling R-45, OU-9 (`conf::ou_9_baseline_then_live_no_gap` at the edge of 9B): with `route_queue_bytes` and
    /// `max_snapshot_bytes` both exactly the snapshot's size, the route attaches and delivers the whole baseline and `live`,
    /// with no resync. The baseline sequence does not count against the queue threshold.
    #[test]
    fn a_queue_of_exactly_the_snapshot_delivers_the_baseline_and_live() {
        use botster_route_codec::prelude::{
            TYPE_ATTACHED, TYPE_BASELINE_BEGIN, TYPE_BASELINE_END, TYPE_LIVE, TYPE_SCREEN,
        };
        let (_h, _core, mut probe) = attached_route(json!({}));
        let (frames, _) = read_baseline(probe.as_mut());
        let size = frames[2].len() - 1;
        let (_h, _core, mut route) =
            attached_route(json!({"max_snapshot_bytes": size, "route_queue_bytes": size}));
        let (frames, after) = read_baseline(route.as_mut());
        let kinds: Vec<u8> = frames.iter().map(|f| f[0]).collect();
        assert_eq!(
            kinds,
            vec![
                TYPE_ATTACHED,
                TYPE_BASELINE_BEGIN,
                TYPE_SCREEN,
                TYPE_BASELINE_END,
                TYPE_LIVE
            ]
        );
        assert_eq!(
            frames[2].len() - 1,
            size,
            "the snapshot is exactly the limit"
        );
        assert!(
            matches!(after, RouteRead::Empty),
            "no resync and no close: {after:?}"
        );
    }

    /// Core OU-1, DP-4 (lead, plans 23aw and 23bd): `pty_input` on a route answers the input that reached the PTY of the
    /// route's session (here a host write), the bytes that the session control gives, in one segment with the unattributed op.
    #[test]
    fn pty_input_on_a_route_is_the_input_log_of_its_session() {
        let (mut harness, mut core, mut route, _) = attached_route_and_result(json!({}));
        let session = SessionId("s".into());
        let log = |harness: &mut TestkitHarness| {
            harness
                .control("h", "pty_input", &json!({"session": "s"}))
                .unwrap()
        };
        assert_eq!(
            route.control("pty_input", &Value::Null).unwrap(),
            json!({"bytes": {"$bytes_hex": ""}, "segments": []}),
            "nothing reached the PTY yet"
        );
        let write = core
            .begin(Op::WriteInput {
                session: session.clone(),
                payload: InputPayload::Bytes {
                    bytes: botster_route_codec::prelude::HexBytes(b"xy".to_vec()),
                },
                guard: None,
            })
            .unwrap();
        let now = Now {
            monotonic: Instant::now(),
            unix: 1,
        };
        let mut done = false;
        for _ in 0..64 {
            core.pump(now);
            done |= core
                .poll_events(64)
                .iter()
                .any(|e| matches!(e, Event::Completed { op, .. } if *op == write));
            if done {
                break;
            }
        }
        assert!(done, "the write completed");
        assert_eq!(log(&mut harness), json!({"bytes": {"$bytes_hex": "7879"}}));
        assert_eq!(
            route.control("pty_input", &Value::Null).unwrap(),
            json!({"bytes": {"$bytes_hex": "7879"}, "segments": [
                {"op": crate::route_client::UNATTRIBUTED_OP, "bytes": {"$bytes_hex": "7879"}}
            ]})
        );
        assert!(route.has_control("pty_input"));
    }

    /// R-47 item 3: `route_fill` writes N = C + F bytes of the pattern at the session's program edge. C is the bytes
    /// that the stream takes from the worker now (0 while gated with no accept limit, the `route_accept` limit, or the
    /// free room of the queue); F is the route's applied `max_frame_bytes`. The client reads the pattern, k from 0 for
    /// each fill, as output.
    #[test]
    fn route_fill_writes_the_room_and_one_frame_of_the_pattern() {
        use botster_route_codec::prelude::TYPE_OUTPUT;
        let (_h, _core, mut route, result) = attached_route_and_result(json!({}));
        let frame = usize::try_from(result.limits.max_frame_bytes).unwrap();
        read_baseline(route.as_mut());
        let deadline = botster_conformance::Deadline::after(None);
        // The output that the client reads until it has `bytes` bytes of it.
        let read_output = |route: &mut Box<dyn RouteClient>, bytes: usize| {
            let (mut stream, mut output) = (Vec::<u8>::new(), Vec::<u8>::new());
            while output.len() < bytes {
                match route.read(1 << 20, &deadline) {
                    RouteRead::Bytes(more) => stream.extend(more),
                    other => panic!("{other:?} after {} output bytes", output.len()),
                }
                while stream.len() >= 4 {
                    let len = u32::from_be_bytes(stream[..4].try_into().unwrap()) as usize;
                    if stream.len() < 4 + len {
                        break;
                    }
                    let frame: Vec<u8> = stream.drain(..4 + len).skip(4).collect();
                    assert_eq!(frame[0], TYPE_OUTPUT, "only output: {frame:?}");
                    output.extend(&frame[1..]);
                }
            }
            output
        };
        let pattern =
            |bytes: usize| -> Vec<u8> { (0..bytes).map(|k| b'a' + (k % 26) as u8).collect() };
        let fill = |route: &mut Box<dyn RouteClient>| {
            route
                .control("route_fill", &json!({"op": "route_fill"}))
                .unwrap()["bytes"]
                .as_u64()
                .unwrap() as usize
        };
        route.control("route_gate", &json!({"on": true})).unwrap();
        assert_eq!(fill(&mut route), frame, "gated: C is 0");
        route.control("route_gate", &json!({"on": false})).unwrap();
        assert_eq!(read_output(&mut route, frame), pattern(frame));
        route
            .control("route_accept", &json!({"bytes": 10}))
            .unwrap();
        assert_eq!(fill(&mut route), 10 + frame, "C is the accept limit");
        route.control("route_gate", &json!({"on": false})).unwrap();
        assert_eq!(read_output(&mut route, 10 + frame), pattern(10 + frame));
        let open = crate::route_client::ROUTE_STREAM_BYTES + frame;
        assert_eq!(
            fill(&mut route),
            open,
            "C is the free room of the empty queue"
        );
        assert_eq!(read_output(&mut route, open), pattern(open));
    }

    /// R-90 follow-up, Q2: `route_reports {progressed}` is false while a filled, gated route waits, and true once the host
    /// consumed the worker's progress report after `route_accept` let the stream take bytes.
    #[test]
    fn route_accept_sets_the_base_of_progressed() {
        let (mut harness, mut core, mut route, result) = attached_route_and_result(json!({}));
        read_baseline(route.as_mut());
        let args = json!({"session": "s", "route": result.route.0});
        let progressed = |harness: &mut TestkitHarness, core: &mut Box<dyn CoreApi>| {
            for _ in 0..16 {
                core.pump(Now {
                    monotonic: Instant::now(),
                    unix: 1,
                });
                core.poll_events(64);
            }
            harness.control("h", "route_reports", &args).unwrap()["progressed"] == json!(true)
        };
        route.control("route_gate", &json!({"on": true})).unwrap();
        route
            .control("route_fill", &json!({"op": "route_fill"}))
            .unwrap();
        assert!(!progressed(&mut harness, &mut core), "no accept yet");
        route
            .control("route_accept", &json!({"bytes": 10}))
            .unwrap();
        assert!(
            progressed(&mut harness, &mut core),
            "the stream took 10 bytes"
        );
        route.control("route_accept", &json!({"bytes": 0})).unwrap();
        assert!(
            !progressed(&mut harness, &mut core),
            "a new accept with no bytes"
        );
    }

    /// Core LC-1, LC-2, 9B: `open` builds the real Core, refuses a second open of the directory while the first lives, and a
    /// reopen after the drop works.
    #[test]
    fn open_builds_a_core_and_the_directory_is_exclusive_until_the_drop() {
        let mut harness = TestkitHarness::new(0);
        let first = harness.open(&spec()).expect("a Core");
        assert_eq!(first.list(), vec![]);
        let second = harness.open(&spec()).err().expect("the directory is held");
        assert_eq!(second.code, ErrorCode::DataDirInUse);
        drop(first);
        assert!(harness.open(&spec()).is_ok());
    }

    /// Core LC-1, 9B: no worker path is `MissingWorkerPath`, and a zero limit is `InvalidConfig`.
    #[test]
    fn open_checks_the_worker_path_and_the_limits() {
        let mut harness = TestkitHarness::new(0);
        let mut no_worker = spec();
        no_worker.worker = None;
        assert_eq!(
            harness.open(&no_worker).err().expect("refused").code,
            ErrorCode::MissingWorkerPath
        );
        let mut zero = spec();
        zero.limits = json!({"max_sessions": 0});
        assert!(matches!(
            harness.open(&zero).err().expect("refused").code,
            ErrorCode::InvalidConfig { .. }
        ));
    }

    /// Core A6-2: no previous worker is fabricated while the protocol is 1.
    #[test]
    fn only_the_current_worker_is_offered() {
        let harness = TestkitHarness::new(1);
        assert_eq!(
            harness.worker(WorkerBuild::Current),
            Some(WorkerRef {
                build: WorkerBuild::Current,
                file_name: None
            })
        );
        assert_eq!(harness.worker(WorkerBuild::Previous), None);
        assert_eq!(
            harness.worker_named("renamed-worker"),
            Some(WorkerRef {
                build: WorkerBuild::Current,
                file_name: Some("renamed-worker".into()),
            })
        );
    }

    #[test]
    fn the_harness_names_the_probe_binary_and_holds_no_control() {
        let mut harness = TestkitHarness::new(2);
        assert_eq!(harness.probe_binary(), "botster-conformance-probe");
        assert_eq!(harness.data_dir("a"), DataDirRef("a".into()));
        assert!(!harness.has_control("kill_worker"));
        assert_eq!(
            harness.control("h", "kill_worker", &json!({})),
            Err(ControlError::Unsupported)
        );
        assert_eq!(harness.core_is_send_not_sync(), None);
        for answer in [true, false] {
            assert_eq!(
                TestkitHarness::new(2)
                    .with_core_type(answer)
                    .core_is_send_not_sync(),
                Some(answer)
            );
        }
        assert!(!harness.is_fake());
        assert!(harness.injects_clock());
        assert_eq!(harness.seed(), 2);
    }

    /// `fail_next` arms a refusal in the handle's script, and a code outside the sync column is refused with the typed value
    /// that `expect_refused` matches (Core A5-3).
    #[test]
    fn fail_next_arms_the_script_and_refuses_a_code_outside_the_sync_column() {
        let mut harness = TestkitHarness::new(0);
        assert!(harness.has_control("fail_next"));
        let armed = harness.control(
            "h",
            "fail_next",
            &json!({"target": "Create", "error": "SessionLimit"}),
        );
        assert_eq!(armed, Ok(Value::Null));
        assert!(!harness.refusals["h"].is_empty());
        let refused = harness.control(
            "h",
            "fail_next",
            &json!({"target": "Start", "error": {"StartFailed": {"ExecFailed": {"errno": 2}}}}),
        );
        assert_eq!(
            refused,
            Err(ControlError::Refused(
                json!({"call": "Start", "code": "StartFailed", "reason": "not_in_sync_column"})
            ))
        );
        // The refused entry is not armed, and another handle has its own script.
        assert!(harness.refusals["h"].take("Start").is_none());
        assert!(!harness.refusals.contains_key("other"));
    }

    #[test]
    fn fail_next_reports_bad_arguments() {
        let mut harness = TestkitHarness::new(0);
        for args in [
            json!({"error": "SessionLimit"}),
            json!({"target": "Create"}),
            json!({"target": "Create", "error": "SessionLimit", "occurrence": "x"}),
            json!({"target": "Nope", "error": "SessionLimit"}),
            json!({"target": "Create", "error": "SessionLimit", "occurrence": 0}),
        ] {
            assert!(
                matches!(
                    harness.control("h", "fail_next", &args),
                    Err(ControlError::Bad(_))
                ),
                "{args}"
            );
        }
        assert_eq!(
            harness.control("h", "host_crash", &json!({})),
            Err(ControlError::Unsupported)
        );
        let nth = harness.control(
            "h",
            "fail_next",
            &json!({"target": "Create", "error": "SessionLimit", "occurrence": 3}),
        );
        assert_eq!(nth, Ok(Value::Null));
    }

    /// `check_crates` is answered from the workspace: this testkit is in no facade, and no listed crate depends on it. Another
    /// statement is not answered yet, and that is never a pass.
    #[test]
    fn the_harness_answers_check_crates_and_nothing_else_yet() {
        let mut harness = TestkitHarness::new(0);
        let spec = json!({"crate": "botster-core-testkit", "not_in": ["botster_core::prelude"], "no_dependency_of": ["botster-core-ffi"]});
        assert_eq!(
            harness.statement("check_crates", &spec),
            Ok(json!({"found_in": [], "depended_on_by": []}))
        );
        let missing = harness.statement("check_crates", &json!({}));
        assert!(matches!(missing, Err(ControlError::Bad(_))));
        for kind in [
            "run_suite",
            "error_codes_reachable",
            "run_deterministic",
            "other",
        ] {
            assert_eq!(
                harness.statement(kind, &spec),
                Err(ControlError::Unsupported),
                "{kind}"
            );
        }
    }
}
