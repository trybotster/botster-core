//! The harness of the Core runner (design 6.2): construction, data directories, worker builds and test controls.
//!
//! `Fn() -> Box<dyn CoreApi>` cannot express a failed `open`, two handles on one directory, a drop and a reopen with live workers,
//! or an older worker binary, so the runner takes a harness.

use botster_conformance::Deadline;
use botster_core_contract::prelude::*;
use botster_hub_conformance::route::RouteRead;
use serde_json::Value;
use std::collections::BTreeMap;

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
    /// The harness has the control but cannot run it for this subject, and names the input that it lacks. The result is
    /// `unsupported_control` with that name as its `op`, never a pass. Example: the real-tier `oracle_resume` of a session whose
    /// program wrote output that the transcript does not state gives `unknown_output_history` (steward ruling R-62).
    UnsupportedFor(String),
    /// The control is known and its arguments are wrong.
    Bad(String),
    /// The harness refuses to script this control on purpose, and says what it refused (Core A5-3: a code that is not in the call's
    /// sync column cannot be scripted). The value is typed: a transcript matches it with `expect_refused`.
    Refused(Value),
}

/// The client end of a route that the harness handed to Core: bytes go straight between the client and the worker (Core DP-1).
pub trait RouteClient {
    fn write(&mut self, bytes: &[u8]);
    /// Reads at most `max` bytes. A subject that can block gives up at the deadline and returns `Empty`. A deadline that has
    /// already passed means one attempt that does not block: it returns `Empty` at once when nothing is ready, never an error
    /// (steward ruling R-68). The driver reads only the end of the stream from `Eof`: it ignores `ended`, and takes the cause
    /// from the host's record (steward ruling R-58).
    fn read(&mut self, max: usize, deadline: &Deadline) -> RouteRead;
    /// A control of the worker that serves this route (design 6.3).
    fn control(&mut self, op: &str, args: &Value) -> Result<Value, String>;
    fn has_control(&self, op: &str) -> bool;
    /// Steward ruling R-86: the client is an independent actor. Moves the bytes that the client already wrote and that wait
    /// for room in the stream, as far as the stream takes them now; a full stream keeps them, so `input_blocked` keeps its
    /// meaning. On a harness whose workers run only when the harness runs them (the testkit), it also runs the workers' ready
    /// route work. It writes no new byte, reads nothing from the stream, reads no Core event and polls nothing (the R-61
    /// fences are unchanged). True when it moved a byte: the driver then takes the pass as progress, not as an idle Core.
    /// Work that it runs and that moves no byte does not count: the driver pumps the Core right after this call in the same
    /// pass, and that pump runs and reports the same ready work (on the testkit, its pump runs the workers' ready work and
    /// keeps `more` true while a worker has some). The driver calls it for every live route in every wait loop. No default:
    /// a no-op would compile and stall.
    fn progress(&mut self) -> bool;
    /// A descriptor that is readable when a read of this end would give bytes or the end of the stream: the client end itself,
    /// or a duplicate of it (steward ruling R-68). With it and the handle's wake descriptor ([`CoreHarness::wake_fd`]), the
    /// driver waits on both, so that a pump runs when the host has work while a route read waits. `None`: an end with no
    /// descriptor (an in-memory end), or one whose reads give nothing for now on purpose (a gated end).
    #[cfg(unix)]
    fn readiness(&self) -> Option<std::os::fd::BorrowedFd<'_>> {
        None
    }
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

    /// A duplicate of the descriptor that the `WakeHandle::fd` of the Core now open under `handle` (`OpenSpec::handle`) gives:
    /// the same descriptor, so that a pump's clear of the TM-6 level signal also shows on it, never a separate notifier. `None`
    /// while no Core is open under that name. After a drop or a reopen under the same name it is the new Core's descriptor
    /// (steward ruling R-68). The driver takes it at the start of each route wait, keeps it only for that wait, never caches it
    /// across steps, and never reads or drains it: only `pump` clears the signal.
    #[cfg(unix)]
    fn wake_fd(&self, _handle: &str) -> Option<std::os::fd::OwnedFd> {
        None
    }

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

    /// Variables that the driver adds to the `env` of every `Create` request (design 6.2). A real harness names its probe control
    /// socket here (`botster_probe_script::control::CONTROL_ENV`), so no transcript names it (Core A5-4). None by default.
    fn probe_env(&self) -> BTreeMap<String, String> {
        BTreeMap::new()
    }

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
