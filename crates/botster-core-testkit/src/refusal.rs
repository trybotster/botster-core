//! `RefusalScript` (plan 4.2a): scripted synchronous refusals in front of a `CoreApi`, Core A5-3 timing 1.
//!
//! A script entry names a call, the occurrence of that call (the n-th such call from the moment of arming) and a code. When
//! an entry matches, the layer **returns the error from the call before the call reaches the Core behind it**: no `OpId` is
//! minted, no event is posted, no slot is reserved, no state changes and no ownership is taken. Every other call is passed
//! through, and so is every result of the Core: the layer never reorders, delays or rewrites a result that the Core
//! produced (A5-2).
//!
//! **Validation.** An entry loads only when its code is in that call's sync column ([`ROWS`]). Each row cites the clause that
//! fixes the column: the A2-1 operation table (and its `PendingLimit` rule), the sync `attach` paragraph of A2-1, ER-0 and
//! 9.3, A2-5 (`SendError`, `RecvError`) and IN-6 (`CancelResult`). An invalid entry is an error, never a run. A call that has no
//! row cannot be scripted.
//!
//! Asynchronous failures are never scripted here. Only an edge produces them (A5-3 timing 2).

use botster_core_contract::prelude::*;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

/// The type of the value that a row scripts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// A `CoreError`, given as an `ErrorCode`.
    Core,
    /// A `SendError` (`service_send`).
    Send,
    /// A `RecvError` (`service_recv`).
    Recv,
    /// A `CancelResult` that a cancel returns without reaching Core.
    Cancel,
}

/// One row of the sync-column table.
#[derive(Debug, Clone, Copy)]
pub struct Row {
    /// The call: the operation kind of a `begin` (`Create`), or the method name (`attach`).
    pub call: &'static str,
    pub shape: Shape,
    /// The codes of the sync column, by name (`Refused(Full)` for a cancel).
    pub codes: &'static [&'static str],
    /// The clause rows that fix the column.
    pub source: &'static str,
}

const READ: &[&str] = &["UnknownSession", "WrongState", "PendingLimit"];

/// The sync column of every scriptable call. A `begin` row carries `PendingLimit` as well: "every row also has the sync error
/// `PendingLimit`" (A2-1).
pub const ROWS: &[Row] = &[
    Row {
        call: "Create",
        shape: Shape::Core,
        codes: &[
            "IdTooLong",
            "IdInUse",
            "SessionLimit",
            "InvalidInput",
            "PendingLimit",
        ],
        source: "Core A2-1 row Create; A2-1 PendingLimit rule",
    },
    Row {
        call: "Start",
        shape: Shape::Core,
        codes: READ,
        source: "Core A2-1 row Start",
    },
    Row {
        call: "Stop",
        shape: Shape::Core,
        codes: READ,
        source: "Core A2-1 row Stop",
    },
    Row {
        call: "Signal",
        shape: Shape::Core,
        codes: &[
            "UnknownSession",
            "WrongState",
            "Unsupported",
            "PendingLimit",
        ],
        source: "Core A2-1 row Signal",
    },
    Row {
        call: "Remove",
        shape: Shape::Core,
        codes: READ,
        source: "Core A2-1 row Remove",
    },
    Row {
        call: "StopAll",
        shape: Shape::Core,
        codes: &["PendingLimit"],
        source: "Core A2-1 row StopAll (no sync errors) and the PendingLimit rule",
    },
    Row {
        call: "UpdateMetadata",
        shape: Shape::Core,
        codes: &["UnknownSession", "InvalidInput", "PendingLimit"],
        source: "Core A2-1 row UpdateMetadata",
    },
    Row {
        call: "SetSizePolicy",
        shape: Shape::Core,
        codes: &[
            "UnknownSession",
            "WrongState",
            "Unsupported",
            "PendingLimit",
        ],
        source: "Core A2-1 row SetSizePolicy",
    },
    Row {
        call: "SetColorProfile",
        shape: Shape::Core,
        codes: &[
            "UnknownSession",
            "WrongState",
            "InvalidInput",
            "PendingLimit",
        ],
        source: "Core A2-1 row SetColorProfile",
    },
    Row {
        call: "SetOutputTap",
        shape: Shape::Core,
        codes: READ,
        source: "Core A2-1 row SetOutputTap",
    },
    Row {
        call: "ReadScreen",
        shape: Shape::Core,
        codes: READ,
        source: "Core A2-1 row ReadScreen",
    },
    Row {
        call: "ReadCursor",
        shape: Shape::Core,
        codes: READ,
        source: "Core A2-1 row ReadCursor (as ReadScreen)",
    },
    Row {
        call: "ReadModeFlags",
        shape: Shape::Core,
        codes: READ,
        source: "Core A2-1 row ReadModeFlags (as ReadScreen)",
    },
    Row {
        call: "CaptureSnapshot",
        shape: Shape::Core,
        codes: &[
            "UnknownSession",
            "WrongState",
            "CaptureLimit",
            "PendingLimit",
        ],
        source: "Core A2-1 row CaptureSnapshot",
    },
    Row {
        call: "ReadFacts",
        shape: Shape::Core,
        codes: READ,
        source: "Core A2-1 row ReadFacts (as ReadScreen)",
    },
    Row {
        call: "Resize",
        shape: Shape::Core,
        codes: &[
            "UnknownSession",
            "WrongState",
            "InvalidInput",
            "PendingLimit",
        ],
        source: "Core A2-1 row Resize",
    },
    Row {
        call: "WriteInput",
        shape: Shape::Core,
        codes: &[
            "UnknownSession",
            "WrongState",
            "InvalidInput",
            "PayloadTooLarge",
            "LaneFull",
            "Unsupported",
            "PendingLimit",
        ],
        source: "Core A2-1 row WriteInput",
    },
    Row {
        call: "Detach",
        shape: Shape::Core,
        codes: &["UnknownRoute", "PendingLimit"],
        source: "Core A2-1 row Detach",
    },
    Row {
        call: "AttachWebRtc",
        shape: Shape::Core,
        codes: &[
            "UnknownSession",
            "RouteLimit",
            "InvalidInput",
            "Unsupported",
            "WrongState",
            "PendingLimit",
        ],
        source: "Core A2-1 row AttachWebRtc (as attach, plus InvalidInput)",
    },
    Row {
        call: "AdoptAll",
        shape: Shape::Core,
        codes: &["WrongState", "PendingLimit"],
        source: "Core A2-1 row AdoptAll",
    },
    Row {
        call: "Adopt",
        shape: Shape::Core,
        codes: READ,
        source: "Core A2-1 row Adopt",
    },
    Row {
        call: "SpawnService",
        shape: Shape::Core,
        codes: &[
            "IdInUse",
            "ServiceLimit",
            "InvalidConfig",
            "InvalidInput",
            "BoundUnavailable",
            "PendingLimit",
        ],
        source: "Core A2-1 row SpawnService; A2-5",
    },
    Row {
        call: "StopService",
        shape: Shape::Core,
        codes: &["UnknownService", "WrongState", "PendingLimit"],
        source: "Core A2-1 row StopService",
    },
    Row {
        call: "RemoveService",
        shape: Shape::Core,
        codes: &["UnknownService", "WrongState", "PendingLimit"],
        source: "Core A2-1 row RemoveService",
    },
    Row {
        call: "EndEpoch",
        shape: Shape::Core,
        codes: &["UnknownService", "WrongState", "PendingLimit"],
        source: "Core A2-1 row EndEpoch",
    },
    Row {
        call: "SetNotificationPolicy",
        shape: Shape::Core,
        codes: &[
            "UnknownSession",
            "WrongState",
            "Unsupported",
            "PendingLimit",
        ],
        source: "Core A3-3 row SetNotificationPolicy (A2-1 table)",
    },
    Row {
        call: "attach",
        shape: Shape::Core,
        codes: &[
            "UnknownSession",
            "RouteLimit",
            "InvalidInput",
            "Unsupported",
            "WrongState",
        ],
        source: "Core A2-1 sync attach paragraph; OU-1; A7-1",
    },
    Row {
        call: "cancel",
        shape: Shape::Cancel,
        codes: &["UnknownOp", "Refused(Full)", "Refused(NotCancellable)"],
        source: "Core IN-6 (UnknownOp, Refused(Full)); A2-1 cancel (Refused(NotCancellable))",
    },
    Row {
        call: "service_send",
        shape: Shape::Send,
        codes: &[
            "Backpressured",
            "UnknownService",
            "NoSuchLane",
            "NotConnected",
            "ServiceEnded",
            "Oversize",
        ],
        source: "Core A2-5 service_send",
    },
    Row {
        call: "service_recv",
        shape: Shape::Recv,
        codes: &["UnknownService", "NoSuchLane"],
        source: "Core A2-5 service_recv",
    },
    Row {
        call: "get",
        shape: Shape::Core,
        codes: &["UnknownSession"],
        source: "Core LC-9; 9.3 UnknownSession",
    },
    Row {
        call: "terminal_state",
        shape: Shape::Core,
        codes: &["UnknownSession"],
        source: "Core ST-4 (a sync read); 9.3 UnknownSession",
    },
    Row {
        call: "snapshot_formats",
        shape: Shape::Core,
        codes: &["UnknownSession"],
        source: "Core ST-6 (a sync read); 9.3 UnknownSession",
    },
    Row {
        call: "tap_read",
        shape: Shape::Core,
        codes: &["UnknownSession"],
        source: "Core TP-1 (a sync read); 9.3 UnknownSession",
    },
    Row {
        call: "read_page",
        shape: Shape::Core,
        codes: &["UnknownCapture", "PageOutOfRange"],
        source: "Core ST-6; 9.3 UnknownCapture, PageOutOfRange",
    },
    Row {
        call: "set_silence_threshold",
        shape: Shape::Core,
        codes: &["UnknownSession", "Unsupported"],
        source: "Core A2-7 set_silence_threshold",
    },
    Row {
        call: "service_report",
        shape: Shape::Core,
        codes: &["UnknownService"],
        source: "Core A4-1 service_report",
    },
    Row {
        call: "service_log_tail",
        shape: Shape::Core,
        codes: &["UnknownService"],
        source: "Core SV-9 service_log_tail (an unknown id is UnknownService)",
    },
];

/// The row of a call.
pub fn row(call: &str) -> Option<&'static Row> {
    ROWS.iter().find(|r| r.call == call)
}

/// What a matched entry returns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scripted {
    Core(CoreError),
    Send(SendError),
    Recv(RecvError),
    Cancel(CancelResult),
}

/// Why an entry did not load.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptError {
    /// The call has no row in the table: it cannot be scripted.
    UnknownCall(String),
    /// The code is not in the call's sync column (Core A5-3). It is typed, so a transcript matches it (`expect_refused`).
    NotInSyncColumn { call: String, code: String },
    /// The value is not a value of the call's type.
    BadValue { call: String, why: String },
    /// An occurrence counts from 1.
    ZeroOccurrence,
    /// Another entry already refuses this call (the `call_number`-th call of its kind since the script began).
    Conflict { call: String, call_number: usize },
}

/// The name of a code: a bare name, or the key of a one-key object. A cancel refusal reads `Refused(Full)`.
pub fn code_name(value: &Value) -> Option<String> {
    match value {
        Value::String(name) => Some(name.clone()),
        Value::Object(map) if map.len() == 1 => {
            let (key, inner) = map.iter().next()?;
            match (key.as_str(), inner) {
                ("Refused", Value::String(reason)) => Some(format!("Refused({reason})")),
                _ => Some(key.clone()),
            }
        }
        _ => None,
    }
}

#[derive(Debug)]
struct Entry {
    call: String,
    /// The number of the call that this entry refuses, counted over all calls of its kind since the script began.
    at: usize,
    result: Scripted,
}

/// The entries of a script.
#[derive(Debug, Default)]
pub struct RefusalScript {
    /// How many calls of each kind the layer has counted.
    counted: BTreeMap<String, usize>,
    entries: Vec<Entry>,
}

impl RefusalScript {
    pub fn new() -> RefusalScript {
        RefusalScript::default()
    }

    /// Arms an entry: the `occurrence`-th call of `call` from now (1 is the next one) returns `error`.
    ///
    /// An entry fires on exactly that call, never on a later one. Two entries that name the same call are a conflict
    /// ([`ScriptError::Conflict`]), whether they were armed together or at different times: the second would otherwise have to
    /// move to another call.
    ///
    /// Clause: Core A5-3 (only a code in the call's sync column can be scripted).
    pub fn arm(&mut self, call: &str, occurrence: usize, error: &Value) -> Result<(), ScriptError> {
        let Some(row) = row(call) else {
            return Err(ScriptError::UnknownCall(call.to_string()));
        };
        if occurrence == 0 {
            return Err(ScriptError::ZeroOccurrence);
        }
        let bad = |why: String| ScriptError::BadValue {
            call: call.to_string(),
            why,
        };
        let code =
            code_name(error).ok_or_else(|| bad("a code is a name or a one-key object".into()))?;
        if !row.codes.contains(&code.as_str()) {
            return Err(ScriptError::NotInSyncColumn {
                call: call.to_string(),
                code,
            });
        }
        let parse = |e: serde_json::Error| bad(e.to_string());
        let result = match row.shape {
            Shape::Core => {
                let code: ErrorCode = serde_json::from_value(error.clone()).map_err(parse)?;
                Scripted::Core(CoreError::new(
                    code,
                    "refused by the testkit script (Core A5-3)",
                ))
            }
            Shape::Send => Scripted::Send(serde_json::from_value(error.clone()).map_err(parse)?),
            Shape::Recv => Scripted::Recv(serde_json::from_value(error.clone()).map_err(parse)?),
            Shape::Cancel => {
                Scripted::Cancel(serde_json::from_value(error.clone()).map_err(parse)?)
            }
        };
        let at = self.counted.get(call).copied().unwrap_or(0) + occurrence;
        if self.entries.iter().any(|e| e.call == call && e.at == at) {
            return Err(ScriptError::Conflict {
                call: call.to_string(),
                call_number: at,
            });
        }
        self.entries.push(Entry {
            call: call.to_string(),
            at,
            result,
        });
        Ok(())
    }

    /// Counts one call of `call` and returns the scripted result when an entry names this call.
    pub fn take(&mut self, call: &str) -> Option<Scripted> {
        let counted = self.counted.entry(call.to_string()).or_insert(0);
        *counted += 1;
        let this = *counted;
        let index = self
            .entries
            .iter()
            .position(|e| e.call == call && e.at == this)?;
        Some(self.entries.remove(index).result)
    }

    /// True when no entry is armed.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// The script as the layer and the harness share it: the harness arms entries after the Core was built.
#[derive(Debug, Clone, Default)]
pub struct RefusalHandle(Arc<Mutex<RefusalScript>>);

impl RefusalHandle {
    pub fn new() -> RefusalHandle {
        RefusalHandle::default()
    }

    fn lock(&self) -> MutexGuard<'_, RefusalScript> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn arm(&self, call: &str, occurrence: usize, error: &Value) -> Result<(), ScriptError> {
        self.lock().arm(call, occurrence, error)
    }

    pub fn take(&self, call: &str) -> Option<Scripted> {
        self.lock().take(call)
    }

    pub fn is_empty(&self) -> bool {
        self.lock().is_empty()
    }
}

/// The operation kind of an `Op`: the name of its variant.
pub fn op_kind(op: &Op) -> String {
    match serde_json::to_value(op) {
        Ok(Value::String(name)) => name,
        Ok(Value::Object(map)) => map.keys().next().cloned().unwrap_or_default(),
        _ => String::new(),
    }
}

/// A `CoreApi` with a script in front of it.
pub struct RefusalLayer {
    inner: Box<dyn CoreApi>,
    script: RefusalHandle,
}

impl RefusalLayer {
    pub fn new(inner: Box<dyn CoreApi>, script: RefusalHandle) -> RefusalLayer {
        RefusalLayer { inner, script }
    }

    fn core(&self, call: &str) -> Option<CoreError> {
        match self.script.take(call) {
            Some(Scripted::Core(error)) => Some(error),
            _ => None,
        }
    }
}

impl CoreApi for RefusalLayer {
    fn begin(&mut self, op: Op) -> Result<OpId, CoreError> {
        if let Some(error) = self.core(&op_kind(&op)) {
            return Err(error);
        }
        self.inner.begin(op)
    }

    fn pump(&mut self, now: Now) -> PumpReport {
        self.inner.pump(now)
    }

    fn poll_events(&mut self, max: usize) -> Vec<Event> {
        self.inner.poll_events(max)
    }

    fn wake_handle(&self) -> Arc<dyn WakeHandle> {
        self.inner.wake_handle()
    }

    fn next_deadline(&self) -> Option<Instant> {
        self.inner.next_deadline()
    }

    fn cancel(&mut self, op: OpId) -> CancelResult {
        match self.script.take("cancel") {
            Some(Scripted::Cancel(result)) => result,
            _ => self.inner.cancel(op),
        }
    }

    fn get(&self, id: &SessionId) -> Result<SessionRecord, CoreError> {
        match self.core("get") {
            Some(error) => Err(error),
            None => self.inner.get(id),
        }
    }

    fn list(&self) -> Vec<SessionRecord> {
        self.inner.list()
    }

    fn status(&self) -> Status {
        self.inner.status()
    }

    fn diagnostics(&self) -> Value {
        self.inner.diagnostics()
    }

    fn terminal_state(&self, id: &SessionId) -> Result<TerminalState, CoreError> {
        match self.core("terminal_state") {
            Some(error) => Err(error),
            None => self.inner.terminal_state(id),
        }
    }

    fn read_page(&self, capture: CaptureId, page: u32) -> Result<Page, CoreError> {
        match self.core("read_page") {
            Some(error) => Err(error),
            None => self.inner.read_page(capture, page),
        }
    }

    fn release(&mut self, capture: CaptureId) {
        self.inner.release(capture);
    }

    fn release_owner(&mut self, client: &ClientId) {
        self.inner.release_owner(client);
    }

    fn snapshot_formats(&self, session: &SessionId) -> Result<Vec<SnapshotFormat>, CoreError> {
        match self.core("snapshot_formats") {
            Some(error) => Err(error),
            None => self.inner.snapshot_formats(session),
        }
    }

    fn shadow_answerable_kinds(&self) -> Vec<botster_route_codec::prelude::QueryKind> {
        self.inner.shadow_answerable_kinds()
    }

    /// A refused `attach` currently drops the transport that the call took by value. Steward ruling R-19: on every
    /// synchronous refusal the caller gets its transport back. The by-value `CoreApi::attach` of `contracts-v0.1.7` cannot give
    /// it back, which is a defect of that crate; when the fixed tag lands, this method returns the transport on every refusal
    /// path (scripted ones here, and the ones of the Core behind the layer).
    fn attach(
        &mut self,
        client: ClientId,
        session: SessionId,
        transport: RouteTransport,
        options: AttachOptions,
    ) -> Result<AttachResult, CoreError> {
        if let Some(error) = self.core("attach") {
            return Err(error);
        }
        self.inner.attach(client, session, transport, options)
    }

    fn tap_read(&mut self, session: &SessionId, max: usize) -> Result<TapChunk, CoreError> {
        match self.core("tap_read") {
            Some(error) => Err(error),
            None => self.inner.tap_read(session, max),
        }
    }

    fn set_silence_threshold(
        &mut self,
        session: &SessionId,
        threshold: Option<Duration>,
    ) -> Result<(), CoreError> {
        match self.core("set_silence_threshold") {
            Some(error) => Err(error),
            None => self.inner.set_silence_threshold(session, threshold),
        }
    }

    fn service_send(
        &mut self,
        id: &ServiceId,
        lane: u8,
        frame: &OutboundFrame,
    ) -> Result<(), SendError> {
        match self.script.take("service_send") {
            Some(Scripted::Send(error)) => Err(error),
            _ => self.inner.service_send(id, lane, frame),
        }
    }

    fn service_recv(&mut self, id: &ServiceId, lane: u8) -> Result<Option<Frame>, RecvError> {
        match self.script.take("service_recv") {
            Some(Scripted::Recv(error)) => Err(error),
            _ => self.inner.service_recv(id, lane),
        }
    }

    fn service_report(&self, id: &ServiceId) -> Result<SpawnReport, CoreError> {
        match self.core("service_report") {
            Some(error) => Err(error),
            None => self.inner.service_report(id),
        }
    }

    fn service_log_tail(&self, id: &ServiceId, max: usize) -> Result<Vec<u8>, CoreError> {
        match self.core("service_log_tail") {
            Some(error) => Err(error),
            None => self.inner.service_log_tail(id, max),
        }
    }

    fn features(&self) -> Features {
        self.inner.features()
    }

    fn limits(&self) -> CoreLimits {
        self.inner.limits()
    }

    fn worker_protocol(&self) -> u8 {
        self.inner.worker_protocol()
    }

    fn adoptable_worker_protocols(&self) -> BTreeSet<u8> {
        self.inner.adoptable_worker_protocols()
    }

    fn worker_protocol_compatibility(&self, protocol: Option<u8>) -> WorkerCompatibility {
        self.inner.worker_protocol_compatibility(protocol)
    }

    fn terminal_identity(&self) -> TerminalIdentity {
        self.inner.terminal_identity()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A value of a code, as a transcript writes it.
    fn sample(code: &str) -> Value {
        match code {
            "Unsupported" => json!({"Unsupported": {}}),
            "InvalidConfig" => json!({"InvalidConfig": {"field": "x"}}),
            "BoundUnavailable" => json!({"BoundUnavailable": {"bound": "cpu_seconds"}}),
            "Refused(Full)" => json!({"Refused": "Full"}),
            "Refused(NotCancellable)" => json!({"Refused": "NotCancellable"}),
            name => json!(name),
        }
    }

    /// The checks of one row against the types of `botster-core-contract`.
    fn check_row(call: &str, is_op: bool) {
        let row = row(call).unwrap_or_else(|| panic!("{call} has a row"));
        assert!(!row.source.is_empty(), "{call} cites its source");
        for code in row.codes {
            let mut script = RefusalScript::new();
            script
                .arm(call, 1, &sample(code))
                .unwrap_or_else(|e| panic!("{call}: {code}: {e:?}"));
            match script.take(call) {
                Some(Scripted::Core(error)) => {
                    // A sync column holds no code that 9.3 makes async only.
                    assert_ne!(error.code.timing(), Timing::Async, "{call}: {code}");
                }
                Some(Scripted::Send(_) | Scripted::Recv(_)) => {}
                Some(Scripted::Cancel(result)) => {
                    assert!(matches!(
                        result,
                        CancelResult::UnknownOp | CancelResult::Refused(_)
                    ));
                }
                None => panic!("{call}: {code} did not fire"),
            }
        }
        // Every `begin` row has the PendingLimit of A2-1, and `attach` (not a `begin`) does not.
        assert_eq!(row.codes.contains(&"PendingLimit"), is_op, "{call}");
        // A code outside the column is refused with the typed refusal. `Internal` is never in a column; neither is a code that
        // only an edge or the worker produces.
        let outside = match row.shape {
            Shape::Core => json!({"RegistryFailed": {"uncertain": false}}),
            Shape::Send => json!("Other"),
            Shape::Recv => json!("Other"),
            Shape::Cancel => json!("TooLate"),
        };
        let mut script = RefusalScript::new();
        assert!(
            matches!(
                script.arm(call, 1, &outside),
                Err(ScriptError::NotInSyncColumn { .. })
            ),
            "{call}"
        );
        assert!(script.is_empty());
    }

    macro_rules! row_tests {
        ($($name:ident: $call:literal, $op:literal;)*) => {
            $(#[test] fn $name() { check_row($call, $op); })*
        };
    }

    row_tests! {
        row_create: "Create", true;
        row_start: "Start", true;
        row_stop: "Stop", true;
        row_signal: "Signal", true;
        row_remove: "Remove", true;
        row_stop_all: "StopAll", true;
        row_update_metadata: "UpdateMetadata", true;
        row_set_size_policy: "SetSizePolicy", true;
        row_set_color_profile: "SetColorProfile", true;
        row_set_output_tap: "SetOutputTap", true;
        row_read_screen: "ReadScreen", true;
        row_read_cursor: "ReadCursor", true;
        row_read_mode_flags: "ReadModeFlags", true;
        row_capture_snapshot: "CaptureSnapshot", true;
        row_read_facts: "ReadFacts", true;
        row_resize: "Resize", true;
        row_write_input: "WriteInput", true;
        row_detach: "Detach", true;
        row_attach_web_rtc: "AttachWebRtc", true;
        row_adopt_all: "AdoptAll", true;
        row_adopt: "Adopt", true;
        row_spawn_service: "SpawnService", true;
        row_stop_service: "StopService", true;
        row_remove_service: "RemoveService", true;
        row_end_epoch: "EndEpoch", true;
        row_set_notification_policy: "SetNotificationPolicy", true;
        row_attach: "attach", false;
        row_cancel: "cancel", false;
        row_service_send: "service_send", false;
        row_service_recv: "service_recv", false;
        row_get: "get", false;
        row_terminal_state: "terminal_state", false;
        row_read_page: "read_page", false;
        row_snapshot_formats: "snapshot_formats", false;
        row_tap_read: "tap_read", false;
        row_set_silence_threshold: "set_silence_threshold", false;
        row_service_report: "service_report", false;
        row_service_log_tail: "service_log_tail", false;
    }

    /// The table has no duplicate call, and every operation of the contract has a row: the schema of `Op` lists the variants.
    #[test]
    fn every_operation_has_one_row() {
        let calls: BTreeSet<&str> = ROWS.iter().map(|r| r.call).collect();
        assert_eq!(calls.len(), ROWS.len(), "a call has one row");
        let schema = serde_json::to_value(schemars::schema_for!(Op)).expect("a schema");
        let mut variants = BTreeSet::new();
        for branch in schema["oneOf"].as_array().expect("oneOf") {
            if let Some(names) = branch["enum"].as_array() {
                variants.extend(names.iter().filter_map(|n| n.as_str().map(str::to_string)));
            }
            if let Some(props) = branch["properties"].as_object() {
                variants.extend(props.keys().cloned());
            }
        }
        assert!(variants.len() >= 26, "{variants:?}");
        for variant in &variants {
            assert!(calls.contains(variant.as_str()), "Op::{variant} has no row");
        }
    }

    #[test]
    fn a_code_has_a_name() {
        assert_eq!(
            code_name(&json!("WrongState")).as_deref(),
            Some("WrongState")
        );
        assert_eq!(
            code_name(&json!({"Unsupported": {}})).as_deref(),
            Some("Unsupported")
        );
        assert_eq!(
            code_name(&json!({"Refused": "Full"})).as_deref(),
            Some("Refused(Full)")
        );
        assert_eq!(code_name(&json!({"A": 1, "B": 2})), None);
        assert_eq!(code_name(&json!(7)), None);
    }

    #[test]
    fn an_entry_that_cannot_load_is_an_error() {
        let mut script = RefusalScript::new();
        assert_eq!(
            script.arm("nope", 1, &json!("WrongState")),
            Err(ScriptError::UnknownCall("nope".into()))
        );
        assert_eq!(
            script.arm("Start", 0, &json!("WrongState")),
            Err(ScriptError::ZeroOccurrence)
        );
        assert_eq!(
            script.arm("Start", 1, &json!("SessionLimit")),
            Err(ScriptError::NotInSyncColumn {
                call: "Start".into(),
                code: "SessionLimit".into()
            })
        );
        assert!(matches!(
            script.arm("Start", 1, &json!(3)),
            Err(ScriptError::BadValue { .. })
        ));
        // A name of the column that is not a value of the type.
        assert!(matches!(
            script.arm("Start", 1, &json!({"Unsupported": 5})),
            Err(ScriptError::NotInSyncColumn { .. })
        ));
        assert!(script.is_empty(), "no invalid entry stays armed");
    }

    #[test]
    fn the_nth_call_fires_once_and_each_call_counts_its_own_kind() {
        let mut script = RefusalScript::new();
        script.arm("Start", 3, &json!("WrongState")).unwrap();
        script.arm("get", 1, &json!("UnknownSession")).unwrap();
        assert!(script.take("Start").is_none());
        assert!(matches!(script.take("get"), Some(Scripted::Core(_))));
        assert!(script.take("get").is_none(), "an entry fires once");
        assert!(script.take("Stop").is_none(), "another kind does not count");
        assert!(script.take("Start").is_none());
        let fired = script.take("Start").expect("the third Start");
        assert!(matches!(
            fired,
            Scripted::Core(CoreError {
                code: ErrorCode::WrongState,
                ..
            })
        ));
        assert!(script.is_empty());
        assert!(script.take("Start").is_none());
    }

    /// Two entries of one call count from their own arming, and the nearer one fires first.
    #[test]
    fn two_entries_of_one_call_fire_in_their_own_turn() {
        let mut script = RefusalScript::new();
        script.arm("Start", 2, &json!("WrongState")).unwrap();
        script.arm("Start", 1, &json!("UnknownSession")).unwrap();
        let code = |s: Option<Scripted>| match s {
            Some(Scripted::Core(e)) => Some(e.code),
            _ => None,
        };
        assert_eq!(code(script.take("Start")), Some(ErrorCode::UnknownSession));
        assert_eq!(code(script.take("Start")), Some(ErrorCode::WrongState));
        assert_eq!(code(script.take("Start")), None);
    }

    #[test]
    fn an_operation_kind_is_its_variant_name() {
        assert_eq!(op_kind(&Op::StopAll), "StopAll");
        assert_eq!(
            op_kind(&Op::Start {
                id: SessionId("s".into())
            }),
            "Start"
        );
    }

    /// The `CoreApi` behind the layer, for these tests only: it counts the calls that reach it. It stands in for no Core in a
    /// conformance run.
    struct Behind {
        reached: Arc<Mutex<Vec<String>>>,
    }

    impl Behind {
        fn note(&self, call: &str) {
            self.reached.lock().unwrap().push(call.to_string());
        }
    }

    impl CoreApi for Behind {
        fn begin(&mut self, op: Op) -> Result<OpId, CoreError> {
            self.note(&op_kind(&op));
            Ok(OpId(7))
        }
        fn pump(&mut self, _now: Now) -> PumpReport {
            self.note("pump");
            PumpReport {
                more: true,
                events_posted: 3,
            }
        }
        fn poll_events(&mut self, max: usize) -> Vec<Event> {
            self.note(&format!("poll_events {max}"));
            Vec::new()
        }
        fn wake_handle(&self) -> Arc<dyn WakeHandle> {
            unreachable!("not used")
        }
        fn next_deadline(&self) -> Option<Instant> {
            None
        }
        fn cancel(&mut self, _op: OpId) -> CancelResult {
            self.note("cancel");
            CancelResult::TooLate
        }
        fn get(&self, _id: &SessionId) -> Result<SessionRecord, CoreError> {
            self.note("get");
            Err(CoreError::new(ErrorCode::UnknownSession, "behind"))
        }
        fn list(&self) -> Vec<SessionRecord> {
            Vec::new()
        }
        fn status(&self) -> Status {
            Status {
                sessions: Vec::new(),
            }
        }
        fn diagnostics(&self) -> Value {
            json!({})
        }
        fn terminal_state(&self, _id: &SessionId) -> Result<TerminalState, CoreError> {
            self.note("terminal_state");
            Err(CoreError::new(ErrorCode::UnknownSession, "behind"))
        }
        fn read_page(&self, _capture: CaptureId, _page: u32) -> Result<Page, CoreError> {
            self.note("read_page");
            Err(CoreError::new(ErrorCode::UnknownCapture, "behind"))
        }
        fn release(&mut self, _capture: CaptureId) {}
        fn release_owner(&mut self, _client: &ClientId) {}
        fn snapshot_formats(&self, _session: &SessionId) -> Result<Vec<SnapshotFormat>, CoreError> {
            self.note("snapshot_formats");
            Ok(Vec::new())
        }
        fn shadow_answerable_kinds(&self) -> Vec<botster_route_codec::prelude::QueryKind> {
            Vec::new()
        }
        fn attach(
            &mut self,
            _client: ClientId,
            _session: SessionId,
            _transport: RouteTransport,
            _options: AttachOptions,
        ) -> Result<AttachResult, CoreError> {
            self.note("attach");
            Err(CoreError::new(ErrorCode::UnknownSession, "behind"))
        }
        fn tap_read(&mut self, _session: &SessionId, _max: usize) -> Result<TapChunk, CoreError> {
            self.note("tap_read");
            Err(CoreError::new(ErrorCode::UnknownSession, "behind"))
        }
        fn set_silence_threshold(
            &mut self,
            _session: &SessionId,
            _threshold: Option<Duration>,
        ) -> Result<(), CoreError> {
            self.note("set_silence_threshold");
            Ok(())
        }
        fn service_send(
            &mut self,
            _id: &ServiceId,
            _lane: u8,
            _frame: &OutboundFrame,
        ) -> Result<(), SendError> {
            self.note("service_send");
            Ok(())
        }
        fn service_recv(&mut self, _id: &ServiceId, _lane: u8) -> Result<Option<Frame>, RecvError> {
            self.note("service_recv");
            Ok(None)
        }
        fn service_report(&self, _id: &ServiceId) -> Result<SpawnReport, CoreError> {
            self.note("service_report");
            Err(CoreError::new(ErrorCode::UnknownService, "behind"))
        }
        fn service_log_tail(&self, _id: &ServiceId, _max: usize) -> Result<Vec<u8>, CoreError> {
            self.note("service_log_tail");
            Ok(Vec::new())
        }
        fn features(&self) -> Features {
            unreachable!("not used")
        }
        fn limits(&self) -> CoreLimits {
            CoreLimits::default()
        }
        fn worker_protocol(&self) -> u8 {
            1
        }
        fn adoptable_worker_protocols(&self) -> BTreeSet<u8> {
            BTreeSet::from([1])
        }
        fn worker_protocol_compatibility(&self, _protocol: Option<u8>) -> WorkerCompatibility {
            WorkerCompatibility::Compatible
        }
        fn terminal_identity(&self) -> TerminalIdentity {
            TerminalIdentity {
                term: "t".into(),
                terminfo_source: "s".into(),
            }
        }
    }

    fn layer() -> (RefusalLayer, RefusalHandle, Arc<Mutex<Vec<String>>>) {
        let reached = Arc::new(Mutex::new(Vec::new()));
        let handle = RefusalHandle::new();
        let behind = Behind {
            reached: Arc::clone(&reached),
        };
        (
            RefusalLayer::new(Box::new(behind), handle.clone()),
            handle,
            reached,
        )
    }

    fn start() -> Op {
        Op::Start {
            id: SessionId("s1".into()),
        }
    }

    /// Core A5-3 timing 1: a scripted refusal is returned from the call, and the call never reaches the Core behind the layer.
    #[test]
    fn a_scripted_refusal_never_reaches_the_core() {
        let (mut layer, handle, reached) = layer();
        handle.arm("Start", 1, &json!("WrongState")).unwrap();
        let error = layer.begin(start()).unwrap_err();
        assert_eq!(error.code, ErrorCode::WrongState);
        assert!(reached.lock().unwrap().is_empty(), "the Core saw nothing");
        // The next call passes and returns exactly what the Core returned.
        assert_eq!(layer.begin(start()), Ok(OpId(7)));
        assert_eq!(*reached.lock().unwrap(), ["Start"]);
    }

    #[test]
    fn the_nth_matching_call_is_refused_and_the_others_pass() {
        let (mut layer, handle, reached) = layer();
        handle.arm("Start", 2, &json!("WrongState")).unwrap();
        assert_eq!(layer.begin(start()), Ok(OpId(7)));
        assert!(
            layer
                .begin(Op::Stop {
                    id: SessionId("s1".into())
                })
                .is_ok(),
            "another kind"
        );
        assert_eq!(
            layer.begin(start()).unwrap_err().code,
            ErrorCode::WrongState
        );
        assert_eq!(layer.begin(start()), Ok(OpId(7)));
        assert_eq!(*reached.lock().unwrap(), ["Start", "Stop", "Start"]);
    }

    /// Every other call, and every result of the Core, goes through unchanged: the layer neither delays nor rewrites it.
    #[test]
    fn everything_else_passes_through_unchanged() {
        let (mut layer, handle, reached) = layer();
        let now = Now {
            monotonic: Instant::now(),
            unix: 5,
        };
        assert_eq!(
            layer.pump(now),
            PumpReport {
                more: true,
                events_posted: 3
            }
        );
        assert!(layer.poll_events(9).is_empty());
        assert_eq!(layer.cancel(OpId(1)), CancelResult::TooLate);
        assert_eq!(
            layer.get(&SessionId("s".into())).unwrap_err().detail,
            "behind"
        );
        assert_eq!(layer.worker_protocol(), 1);
        assert_eq!(layer.terminal_identity().term, "t");
        assert_eq!(
            *reached.lock().unwrap(),
            ["pump", "poll_events 9", "cancel", "get"]
        );
        assert!(handle.is_empty());
    }

    #[test]
    fn the_sync_calls_with_a_row_are_scripted_in_the_layer() {
        let (mut layer, handle, reached) = layer();
        let (session, service) = (SessionId("s".into()), ServiceId([1; 32]));
        let frame = OutboundFrame {
            frame_type: 1,
            payload: Default::default(),
        };
        handle
            .arm("cancel", 1, &json!({"Refused": "Full"}))
            .unwrap();
        assert_eq!(
            layer.cancel(OpId(1)),
            CancelResult::Refused(CancelRefusal::Full)
        );
        handle
            .arm("service_send", 1, &json!("Backpressured"))
            .unwrap();
        assert_eq!(
            layer.service_send(&service, 0, &frame),
            Err(SendError::Backpressured)
        );
        handle.arm("service_recv", 1, &json!("NoSuchLane")).unwrap();
        assert_eq!(layer.service_recv(&service, 0), Err(RecvError::NoSuchLane));
        handle.arm("get", 1, &json!("UnknownSession")).unwrap();
        assert_eq!(
            layer.get(&session).unwrap_err().detail,
            "refused by the testkit script (Core A5-3)"
        );
        handle
            .arm("terminal_state", 1, &json!("UnknownSession"))
            .unwrap();
        assert_eq!(
            layer.terminal_state(&session).unwrap_err().code,
            ErrorCode::UnknownSession
        );
        assert_ne!(
            layer.terminal_state(&session).unwrap_err().detail,
            "refused by the testkit script (Core A5-3)"
        );
        handle
            .arm("read_page", 1, &json!("PageOutOfRange"))
            .unwrap();
        assert_eq!(
            layer.read_page(CaptureId(1), 0).unwrap_err().code,
            ErrorCode::PageOutOfRange
        );
        handle
            .arm("set_silence_threshold", 1, &json!({"Unsupported": {}}))
            .unwrap();
        assert!(layer.set_silence_threshold(&session, None).is_err());
        handle
            .arm("service_report", 1, &json!("UnknownService"))
            .unwrap();
        assert_eq!(
            layer.service_report(&service).unwrap_err().detail,
            "refused by the testkit script (Core A5-3)"
        );
        handle
            .arm("service_log_tail", 1, &json!("UnknownService"))
            .unwrap();
        assert!(layer.service_log_tail(&service, 1).is_err());
        let transport = RouteTransport::WebRtc {
            offer: String::new(),
            expected_fingerprint: String::new(),
        };
        let options: AttachOptions =
            serde_json::from_value(json!({"file_directory": "/tmp"})).unwrap();
        handle.arm("attach", 1, &json!("RouteLimit")).unwrap();
        let error = layer
            .attach(ClientId("c".into()), session, transport, options)
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::RouteLimit);
        // Only the unscripted second `terminal_state` reached the Core.
        assert_eq!(*reached.lock().unwrap(), ["terminal_state"]);
        assert!(handle.is_empty());
    }

    /// A refusal fires before delegation for `snapshot_formats` and `tap_read` too, and the next call reaches the Core.
    #[test]
    fn snapshot_formats_and_tap_read_are_refused_before_delegation() {
        let (mut layer, handle, reached) = layer();
        let session = SessionId("s".into());
        handle
            .arm("snapshot_formats", 1, &json!("UnknownSession"))
            .unwrap();
        handle.arm("tap_read", 1, &json!("UnknownSession")).unwrap();
        let scripted = "refused by the testkit script (Core A5-3)";
        assert_eq!(
            layer.snapshot_formats(&session).unwrap_err().detail,
            scripted
        );
        assert_eq!(layer.tap_read(&session, 1).unwrap_err().detail, scripted);
        assert!(reached.lock().unwrap().is_empty(), "the Core saw nothing");
        assert_eq!(layer.snapshot_formats(&session), Ok(Vec::new()));
        assert_eq!(layer.tap_read(&session, 1).unwrap_err().detail, "behind");
        assert_eq!(*reached.lock().unwrap(), ["snapshot_formats", "tap_read"]);
    }

    /// Each `attach` call is counted once: occurrence 2 refuses the second call, and the calls around it reach the Core. (The
    /// transport that a refusal returns to the caller waits for the fixed `CoreApi::attach`, steward ruling R-19.)
    #[test]
    fn attach_counts_each_call_once() {
        let (mut layer, handle, reached) = layer();
        handle.arm("attach", 2, &json!("RouteLimit")).unwrap();
        let attach = |layer: &mut RefusalLayer| {
            let transport = RouteTransport::WebRtc {
                offer: String::new(),
                expected_fingerprint: String::new(),
            };
            let options: AttachOptions =
                serde_json::from_value(json!({"file_directory": "/tmp"})).unwrap();
            layer
                .attach(
                    ClientId("c".into()),
                    SessionId("s".into()),
                    transport,
                    options,
                )
                .unwrap_err()
        };
        assert_eq!(
            attach(&mut layer).detail,
            "behind",
            "the first call is delegated"
        );
        assert_eq!(
            attach(&mut layer).code,
            ErrorCode::RouteLimit,
            "the second call is refused"
        );
        assert_eq!(
            attach(&mut layer).detail,
            "behind",
            "the third call is delegated"
        );
        assert_eq!(*reached.lock().unwrap(), ["attach", "attach"]);
        assert!(handle.is_empty());
    }

    /// Entries that name the same call conflict, armed together or at different times. No entry moves to another call.
    #[test]
    fn two_entries_for_one_call_are_a_conflict() {
        let mut script = RefusalScript::new();
        script.arm("Start", 1, &json!("WrongState")).unwrap();
        assert_eq!(
            script.arm("Start", 1, &json!("PendingLimit")),
            Err(ScriptError::Conflict {
                call: "Start".into(),
                call_number: 1
            })
        );
        // Entries armed at different times that converge on one call.
        let mut script = RefusalScript::new();
        script.arm("Start", 2, &json!("WrongState")).unwrap();
        assert!(script.take("Start").is_none());
        assert_eq!(
            script.arm("Start", 1, &json!("PendingLimit")),
            Err(ScriptError::Conflict {
                call: "Start".into(),
                call_number: 2
            })
        );
        // The refused entry left no trace: the first entry fires on its own call, once, and the call after it passes.
        assert!(matches!(
            script.take("Start"),
            Some(Scripted::Core(CoreError {
                code: ErrorCode::WrongState,
                ..
            }))
        ));
        assert!(script.take("Start").is_none());
        // Different calls, or different call numbers, do not conflict.
        script.arm("Start", 1, &json!("WrongState")).unwrap();
        script.arm("Stop", 1, &json!("WrongState")).unwrap();
        script.arm("Start", 2, &json!("PendingLimit")).unwrap();
    }

    /// An entry fires on its own call and not on a later one, whatever other entries do.
    #[test]
    fn an_entry_never_moves_to_a_later_call() {
        let mut script = RefusalScript::new();
        script.arm("Start", 1, &json!("WrongState")).unwrap();
        script.arm("Start", 3, &json!("PendingLimit")).unwrap();
        let code = |s: Option<Scripted>| match s {
            Some(Scripted::Core(e)) => Some(e.code),
            _ => None,
        };
        assert_eq!(code(script.take("Start")), Some(ErrorCode::WrongState));
        assert_eq!(code(script.take("Start")), None);
        assert_eq!(code(script.take("Start")), Some(ErrorCode::PendingLimit));
        assert_eq!(code(script.take("Start")), None);
    }
}
