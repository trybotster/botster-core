//! The harness of the Core runner (design 6.2): construction, data directories, worker builds and test controls.
//!
//! `Fn() -> Box<dyn CoreApi>` cannot express a failed `open`, two handles on one directory, a drop and a reopen with live workers,
//! or an older worker binary, so the runner takes a harness.

use botster_conformance::Deadline;
use botster_core_contract::prelude::*;
use botster_hub_conformance::route::RouteRead;
use serde_json::Value;

/// A data directory that the harness owns. It is kept across drop and reopen (Core LC-2, LC-12, AD-1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataDirRef(pub String);

/// The worker builds that a harness can offer: the current one, and the previous protocol (Core AD-4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerBuild {
    Current,
    Previous,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerRef {
    pub build: WorkerBuild,
    /// The file name under which the harness offers the worker binary. `None` is the harness's default name (Core E1-1: Core fixes none).
    pub file_name: Option<String>,
}

/// One `open`: an explicit configuration. `limits` is JSON, so a transcript can give a zero, a value out of range, or a cross-limit
/// violation (Core 9B) and expect `InvalidConfig`. `worker: None` tests LC-1.
#[derive(Debug, Clone)]
pub struct OpenSpec {
    /// The name of the handle, for the controls that name it.
    pub handle: String,
    pub data_dir: DataDirRef,
    pub worker: Option<WorkerRef>,
    pub limits: Value,
}

/// Why a control did not run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlError {
    /// The harness cannot cause this event. The result is `unsupported_control`, never a pass.
    Unsupported,
    /// The control is known and its arguments are wrong.
    Bad(String),
    /// The harness refuses to script this control on purpose, and says what it refused (Core A5-3: a code that is not in the call's
    /// sync column cannot be scripted). The value is typed: a transcript matches it with `expect_refused`.
    Refused(Value),
}

/// The client end of a route that the harness handed to Core: bytes go straight between the client and the worker (Core DP-1).
pub trait RouteClient {
    fn write(&mut self, bytes: &[u8]);
    /// Reads at most `max` bytes. A subject that can block gives up at the deadline and returns `Empty`.
    fn read(&mut self, max: usize, deadline: &Deadline) -> RouteRead;
    /// A control of the worker that serves this route (design 6.3).
    fn control(&mut self, op: &str, args: &Value) -> Result<Value, String>;
    fn has_control(&self, op: &str) -> bool;
}

pub trait CoreHarness {
    /// True for a fake. A `structural` transcript is only checked structurally on it (design 7.1).
    fn is_fake(&self) -> bool {
        false
    }

    /// Core LC-1, LC-2, 9B: one open with an explicit config. Errors are results.
    fn open(&mut self, spec: &OpenSpec) -> Result<Box<dyn CoreApi>, CoreError>;

    /// A data directory owned by this harness, kept across drop and reopen.
    fn data_dir(&mut self, name: &str) -> DataDirRef;

    /// The worker binaries the harness can offer: `Current`, and `Previous` (Core AD-4).
    fn worker(&self, which: WorkerBuild) -> Option<WorkerRef>;

    /// A worker binary offered under an arbitrary file name (Core E1-1). `None` when the harness cannot offer one.
    fn worker_named(&self, _file_name: &str) -> Option<WorkerRef> {
        None
    }

    /// True when the host passes the clock, so `advance_clock` moves it (Core TM-1). A fake and the testkit inject it; only a real Core
    /// with real worker processes follows real time.
    fn injects_clock(&self) -> bool {
        self.is_fake()
    }

    /// True when the injected clock controls every source of progress, so an idle wait may jump the clock to Core's next
    /// deadline (steward ruling R-46). A fake and the testkit control all progress. A harness with real worker processes,
    /// PTYs or sockets answers false: its clock moves only by `advance_clock`, and an idle wait waits on the wake handle,
    /// bounded by the step limit. Meaningful only when `injects_clock` is true.
    fn progress_is_injected(&self) -> bool {
        self.injects_clock()
    }

    /// The handle is gone. Dropping a handle never ends a worker (Core LC-12).
    fn drop_handle(&mut self, handle: &str);

    /// True when the harness implements the control (design 6.3). A control that a harness lacks gives `unsupported_control`.
    fn has_control(&self, op: &str) -> bool;

    /// Core TH-1: whether the concrete Core type is `Send` and not `Sync`. `None` when the harness cannot say.
    fn core_is_send_not_sync(&self) -> Option<bool> {
        None
    }

    /// A statement step that the suite runner executes: `run_suite`, `check_crates`, `error_codes_reachable` or `run_deterministic`
    /// (Core A5-1, A5-3, A5-4). The result is a report that the driver checks against the statement; a harness that cannot run the
    /// statement says `Unsupported`, which is never a pass. [`crate::run_script_events`] runs a script on fresh harnesses.
    fn statement(&mut self, _kind: &str, _spec: &Value) -> Result<Value, ControlError> {
        Err(ControlError::Unsupported)
    }

    /// A test control: kill a worker, hold a completion, withhold a feature, and others. A real harness builds each from injected parts.
    fn control(&mut self, handle: &str, op: &str, args: &Value) -> Result<Value, ControlError>;

    /// `argv[0]` of a session program (design 6.2): the probe binary. Its `argv[1]` is the probe script.
    fn probe_binary(&self) -> String;

    /// Core OU-1, DP-2: connects a stream to a session through `attach`. The harness keeps the client end.
    /// The harness builds the endpoint itself, so on a sync refusal it reports the `CoreError` and owns the cleanup of its own stream (R-19 returns an embedder's transport in `AttachRefused`).
    fn attach_stream(
        &mut self,
        handle: &str,
        core: &mut dyn CoreApi,
        client: ClientId,
        session: &SessionId,
        options: AttachOptions,
    ) -> Result<(AttachResult, Box<dyn RouteClient>), CoreError>;
}

/// The harness of the section 11 form: one constructor, one default handle. Every transcript that needs more than that gives
/// `unsupported_control`. It is a partial-suite convenience, not valid for acceptance.
pub struct MakeHarness<F: FnMut() -> Box<dyn CoreApi>> {
    make: F,
    opened: bool,
}

/// Wraps a constructor of Core section 11's form (design 11).
pub fn from_make<F: FnMut() -> Box<dyn CoreApi>>(make: F) -> MakeHarness<F> {
    MakeHarness {
        make,
        opened: false,
    }
}

impl<F: FnMut() -> Box<dyn CoreApi>> CoreHarness for MakeHarness<F> {
    fn open(&mut self, spec: &OpenSpec) -> Result<Box<dyn CoreApi>, CoreError> {
        // One default handle with the default limits, opened once.
        let default = spec.limits.as_object().is_none_or(|m| m.is_empty())
            && spec.worker.is_some()
            && !self.opened;
        if !default {
            return Err(CoreError::new(
                ErrorCode::Unsupported { what: None },
                "unsupported_control: open",
            ));
        }
        self.opened = true;
        Ok((self.make)())
    }

    fn data_dir(&mut self, name: &str) -> DataDirRef {
        DataDirRef(name.to_string())
    }

    fn worker(&self, which: WorkerBuild) -> Option<WorkerRef> {
        (which == WorkerBuild::Current).then_some(WorkerRef {
            build: which,
            file_name: None,
        })
    }

    fn drop_handle(&mut self, _handle: &str) {}

    fn has_control(&self, _op: &str) -> bool {
        false
    }

    fn control(&mut self, _handle: &str, _op: &str, _args: &Value) -> Result<Value, ControlError> {
        Err(ControlError::Unsupported)
    }

    fn probe_binary(&self) -> String {
        "botster-conformance-probe".to_string()
    }

    fn attach_stream(
        &mut self,
        _handle: &str,
        _core: &mut dyn CoreApi,
        _client: ClientId,
        _session: &SessionId,
        _options: AttachOptions,
    ) -> Result<(AttachResult, Box<dyn RouteClient>), CoreError> {
        Err(CoreError::new(
            ErrorCode::Unsupported { what: None },
            "unsupported_control: attach_stream",
        ))
    }
}
