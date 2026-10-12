# crate-catchup audit: every amendment item and its crate symbol

Brief Part 1 item 5. One row per item that an amendment or erratum adds or changes: a type, field, variant, error code, limit, default, config key or feature name.
The rows name the exact crate symbol. Part 2 builds its per-amendment symbol declarations from this table.

**Scope.** Every amendment and erratum file in `contracts.toml` that is not a base file of manifest final6, plus UI 1.1 and UI 1.2 (the reviewer asked for them; both were already frozen at final6). The codec revision 36 and Lua binding r9 replace whole files, so their rows list the differences from the final6 files (revision 26, r6).

**Status.**
- **present**: the symbol is in the crate on the base (main `d18b6de`).
- **fixed**: this branch adds or changes it (commit in the row or the section heading).
- **n/a**: no crate symbol, with the reason. "Runtime" (or "Hub runtime") means the contract states a check or a behavior of the real Core, Hub or worker. The type carries any value, so a transcript can send an invalid one and observe the refusal. "Renderer behavior" and "client behavior" mean a rule for a client; "Lua host" means a rule for the Lua host. No contract crate implements these.
- **absent (deleted)** or **absent (withdrawn)**: the amendment deletes or withdraws the name, and no crate has it.

**Method.** Each row was checked against the source, not by a text search alone:
- struct and enum members: a parse of each named type (field and variant lists, with file and line);
- the HC A.9b failures: a dump of every `KindSpec` verb and action with its `reasons`, compared row by row with the A.9b table;
- the HP calls, handler kinds, events and operation states: the generated `schemas/plugin/plugin-calls.schema.json`;
- the HP A2 section 3 keys: each of the 97 keys compared with its `plugin/limits.rs` table and mode.

**Gaps found by this audit, beyond the brief's list:** Core A16 (`ServiceEnd`), HC A2 A.9b verb failures (`VerbSpec::reasons`), HC A2 A.9b `package open_asset` failures, HP A4-1 (`write_pending_full`), HP A5-1 (`EntityRecord`), HP A8-3 (`breaking_not_accepted` in `CODES`), and HP A8-3 / A2-42 (HC A7, pending). Each is **fixed** in the rows below.

**Gap found after the audit:** Core A7-1 `InvalidInput{field}` (also A9-1 and A15-1). The crate had a unit `InvalidInput`. It is **fixed** on stage0/invalid-input-field. A sweep of every `Code{...}` and `Code(...)` in the Core files (base v1.17 and every amendment and erratum) against `ErrorCode` found no other missed payload: `BoundUnavailable{bound}`, `InvalidConfig{field}`, `InvalidOffer{reason}`, `RegistryFailed{uncertain}`, `StartFailed{...}` and `Unsupported{what}` are present. `SessionEnded{exit}` is a route close reason, `Unsupported{what}` of `NotWritten` is `UnsupportedWhat`, and `Unsupported(bound)` is a `SpawnReport` entry; none of them is an `ErrorCode` payload.

**Paths.** `core/` = `crates/botster-core-contract/src/`, `codec/` = `crates/botster-route-codec/src/`, `hub/` = `crates/botster-hub-contract/src/`, `plugin/` = `crates/botster-plugin-contract/src/`, `ui/` = `crates/botster-ui-contract/src/`. `A::B` names variant or field `B` of type `A`.
Every Rust type below also has its generated schema under `schemas/` (codegen); the TS types are generated for HC only (`ts/packages/hub-contract`).

## Core

### Core A2 (`core-contract-v1.17-amendment-2-candidate4.md`)

| Clause | Item | Crate symbol | Status |
|---|---|---|---|
| A2-1 | the 25 operations: `Create`, `Start`, `Stop`, `Signal`, `Remove`, `StopAll`, `UpdateMetadata`, `SetSizePolicy`, `SetColorProfile`, `SetOutputTap`, `ReadScreen`, `ReadCursor`, `ReadModeFlags`, `CaptureSnapshot`, `ReadFacts`, `Resize`, `WriteInput`, `Detach`, `AttachWebRtc`, `AdoptAll`, `Adopt`, `SpawnService`, `StopService`, `RemoveService`, `EndEpoch` | `core/op.rs` `Op::{Create, Start, Stop, Signal, Remove, StopAll, UpdateMetadata, SetSizePolicy, SetColorProfile, SetOutputTap, ReadScreen, ReadCursor, ReadModeFlags, CaptureSnapshot, ReadFacts, Resize, WriteInput, Detach, AttachWebRtc, AdoptAll, Adopt, SpawnService, StopService, RemoveService, EndEpoch}` | present |
| A2-1 | the arguments of each row (`session`, `request`, `id`, `sig`, `labels`, `policy`, `profile`, `enabled`, `history: bool`, `owner`, `after?`, `size`, `payload`, `guard?`, `route`, `reason`, `client`, `offer`, `expected_fingerprint`, `options`, `service_id`, `argv`, `env`, `cwd`, `limits`, `service`) | the fields of each `core/op.rs` `Op` variant | present |
| A2-1 | the `Ok` values: `()`, `SessionRecord`, `SessionEnd`, `Screen`, `Cursor`, `Modes`, `Capture`, `Facts`, `Applied{actual}` / `Superseded{by}`, `InputResult`, `Answer`, `SpawnReport` (`ServiceExit` for `StopService`: see A16) | `core/op.rs` `OpOutput::{Unit, Record, End, Screen, Cursor, Modes, Capture, Facts, Resize, Input, Answer, Spawn}`, `ResizeResult::{Applied, Superseded}` | present |
| A2-1 | `SessionEnd = Exited(Exit) \| Lost(reason)` | `core/types.rs` `SessionEnd::{Exited, Lost}` | present |
| A2-1 | `Screen{text, rows, cols, history_unavailable, model_rev}` | `core/read.rs` `Screen` | present |
| A2-1 | `Cursor{row, col, visible, row_text, text_before_cursor, model_rev}` | `core/read.rs` `Cursor` | present |
| A2-1 | `Modes{flags, model_rev}` | `core/read.rs` `Modes` | present |
| A2-1 | `Answer{route, answer}` | `core/route.rs` `Answer` | present |
| A2-1 | `Size{rows, cols, cell_px?}` | `core/types.rs` `Size`, `CellPx` | present |
| A2-1 | `rows` and `cols` 1 to 65,535; outside is `InvalidInput` | none | n/a: runtime (`InvalidInput`); `Size` keeps `u32` so a transcript can send 0 (`conf::a2_1_size_range_zero_is_invalid_input`) |
| A2-1 | `SpawnRequest{argv, env, cwd, size, labels, color_profile?}` | `core/types.rs` `SpawnRequest::{argv, env, cwd, size, labels, color_profile}` | present |
| A2-1 | `argv` non-empty; `env` exact; `cwd` absolute | none | n/a: runtime (`InvalidInput`, `StartFailed{CwdMissing}`) |
| A2-1 | `ColorProfile{palette?: [Rgb; 256], foreground, background, cursor?}` | `core/types.rs` `ColorProfile` (schema `palette` length 256) | present |
| A2-1 | `Rgb{r, g, b: u8}` | `core/types.rs` `Rgb` | present |
| A2-1 | `StartFailReason` = `ExecFailed{errno}`, `CwdMissing`, `StartupTimeout`, `WorkerFailed`, `GuardianFailed`, `WebRtcFailed` (non-exhaustive) | `core/types.rs` `StartFailReason::{ExecFailed, CwdMissing, StartupTimeout, WorkerFailed, GuardianFailed, WebRtcFailed, Other}` | present |
| A2-1 | session `ExitCause` = `Normal`, `Signal`, `HostStop`, `Killed`, `Other` | `core/types.rs` `ExitCause` | present |
| A2-1 | sync error `PendingLimit` on every row | `core/error.rs` `ErrorCode::PendingLimit` | present |
| A2-1 | codes in the rows: `IdTooLong`, `IdInUse`, `SessionLimit`, `InvalidInput`, `RegistryFailed`, `UnknownSession`, `WrongState`, `StartFailed`, `Unsupported`, `SessionEnded`, `WorkerLinkFailed`, `CursorReadUnsupported`, `CaptureLimit`, `SnapshotTooLarge`, `PayloadTooLarge`, `LaneFull`, `Internal`, `UnknownRoute`, `ServiceLimit`, `InvalidConfig`, `BoundUnavailable` | `core/error.rs` `ErrorCode::{IdTooLong, IdInUse, SessionLimit, InvalidInput, RegistryFailed, UnknownSession, WrongState, StartFailed, Unsupported, SessionEnded, WorkerLinkFailed, CursorReadUnsupported, CaptureLimit, SnapshotTooLarge, PayloadTooLarge, LaneFull, Internal, UnknownRoute, ServiceLimit, InvalidConfig, BoundUnavailable}` | present |
| A2-1 | new code `UnknownService` (caller, sync) | `core/error.rs` `ErrorCode::UnknownService` | present |
| A2-1 | new code `InvalidOffer{reason: bad_offer \| offer_without_candidates}` (async) | `core/error.rs` `ErrorCode::InvalidOffer`, `OfferRefusal` | present |
| A2-1 | `DeadlineExpired` removed | none | absent (deleted); `core/tests.rs` `no_deadline_code_exists` |
| A2-1 | `cancel`: `Refused(NotCancellable)` beside `Full` | `core/input.rs` `CancelResult::Refused`, `CancelRefusal::{Full, NotCancellable}` | present |
| A2-1 | the sync `attach` admitted states | none | n/a: runtime |
| A2-2 | `InputResult{outcome, counts, detail}`; `detail` has no compatibility promise | `core/input.rs` `InputResult::{outcome, payload_bytes_written, pty_bytes_written, detail}` | present |
| A2-2 | `NotWritten(Refused)` | `core/input.rs` `NotWrittenReason::Refused` | present |
| A2-2 | `NotWritten(Unsupported{what})`, `what` = `pixel_position`, `produced_text`, `named_key`, `wheel_direction`, `coordinate`, `release_event` (non-exhaustive) | `core/input.rs` `NotWrittenReason::Unsupported`, `UnsupportedWhat::{PixelPosition, ProducedText, NamedKey, WheelDirection, Coordinate, ReleaseEvent, Other}` | present |
| A2-3 | `RouteCloseReason` = `Detached`, `Replaced`, `Revoked`, `SessionEnded{exit}`, `SessionRemoved`, `SessionLost`, `PeerClosed`, `WriteFailed`, `BadFrame{code}`, `BadPeer`, `NotConnected`, `SnapshotTooLarge`, `StallTimeout`, `HandoffFailed` | `core/route.rs` `RouteCloseReason::{Detached, Replaced, Revoked, SessionEnded{exit}, SessionRemoved, SessionLost, PeerClosed, WriteFailed, BadFrame{code}, BadPeer, NotConnected, SnapshotTooLarge, StallTimeout, HandoffFailed, Other}` | present |
| A2-3 | `BadFrame.code` = `frame_too_large`, `malformed_frame`, `unknown_frame`, `invalid_input` | `codec/` `ProtocolErrorCode` | present |
| A2-3 | the mapping to the codec's route-ended reasons | none | n/a: runtime (worker) |
| A2-4 | `Bell{id, instance, at}` | `core/event.rs` `Event::Bell` | present |
| A2-4 | `PromptMark{id, instance, mark, exit_code?, at}`; `mark` = `PromptStart`, `CommandStart`, `CommandExecuted`, `CommandFinished` | `core/event.rs` `Event::PromptMark`, `PromptMarkKind` | present |
| A2-4 | `Notification{id, instance, source, title?, body, truncated, at}`; `source` = `Osc9`, `Osc777` | `core/event.rs` `Event::Notification`, `NotificationSource` | present |
| A2-4 | `title`/`body` bounded by `clipboard_bytes` (no new limit) | `core/limits.rs` `CoreLimits::clipboard_bytes` | present |
| A2-4 | `EventsLost.kinds` = `Bell`, `PromptMark`, `Notification`, `ClipboardWrite`, `Query`, `Tap` (non-exhaustive) | `core/event.rs` `LostKind` | present |
| A2-5 | `service_send(id, lane, frame) -> Result<(), SendError>` | `core/api.rs` `CoreApi::service_send` | present |
| A2-5 | `SendError` = `Backpressured`, `UnknownService`, `NoSuchLane`, `NotConnected`, `ServiceEnded`, `Oversize` (non-exhaustive) | `core/service.rs` `SendError` | present |
| A2-5 | `service_recv(id, lane) -> Result<Option<Frame>, RecvError>`; `RecvError` = `UnknownService`, `NoSuchLane` | `core/api.rs` `CoreApi::service_recv`, `core/service.rs` `RecvError`, `Frame` | present |
| A2-5 | `ServiceState` = `Started`, `Reconnected{epoch}`, `Exited{code, signal, cause, descendants_may_remain}`, `Lost{reason, payload_may_remain}`, `Released` | `core/service.rs` `ServiceState`, `ServiceExit` | present |
| A2-5 | `LaneEnded.reason` = `BadLane`, `Oversize`, `Truncated`, `PeerClosed`, `EpochEnded`, `Superseded`, `ServiceExited`, `ServiceRemoved` | `core/service.rs` `LaneEndReason` | present |
| A2-5 | `ServiceLimits.peer_identity: Uid` (default: the host's uid) | `core/service.rs` `ServiceLimits::peer_identity` (`None` = the host's uid) | present |
| A2-5 | each rlimit `Option<{value, required}>` | `core/service.rs` `Rlimits`, `RlimitSpec::{value, required}` | present |
| A2-5 | `ServiceWritable{id, lane}` | `core/event.rs` `Event::ServiceWritable` | present |
| A2-5 | `startup` runs until epoch 1 commits | none | n/a: runtime |
| A2-6 | feature names `route_transport:stream`, `route_transport:webrtc`, `service_preamble_versions`, `silence`, `size_policy_other`, `snapshot_graphics`, `focus_report`, `payload_dies_with_guardian`, `bound:cpu_seconds`, `bound:open_files`, `bound:processes`, `bound:address_space_bytes` | `core/features.rs` `Feature::{RouteTransportStream, RouteTransportWebrtc, ServicePreambleVersions, Silence, SizePolicyOther, SnapshotGraphics, FocusReport, PayloadDiesWithGuardian, BoundCpuSeconds, BoundOpenFiles, BoundProcesses, BoundAddressSpaceBytes}` | present |
| A2-7 | `pump(now) -> PumpReport{more, events_posted}` | `core/api.rs` `CoreApi::pump`, `PumpReport` | present |
| A2-7 | `set_silence_threshold(session, Option<Duration>)` | `core/api.rs` `CoreApi::set_silence_threshold` | present |
| A2-8 | `terminal_identity() -> TerminalIdentity{term, terminfo_source}` | `core/api.rs` `CoreApi::terminal_identity`, `core/read.rs` `TerminalIdentity` | present |
| A2-8 | `term` = `xterm-ghostty` | none | n/a: runtime value of real Core |
| A2-9 | file-name shortening (`NAME_MAX`) | none | n/a: runtime (worker); no limit added |
| A2-10 | codec pin moves to revision 30 | see the codec section | present |

### Core A3 (`core-contract-v1.17-amendment-3-candidate10.md`)

| Clause | Item | Crate symbol | Status |
|---|---|---|---|
| A3-1 | `NotificationPolicy` = `All` (default), `None`, `Matching{patterns: [string]}` | `core/types.rs` `NotificationPolicy::{All, None, Matching}` | present |
| A3-1 | `SpawnRequest.notification_policy?` (default `All`) | `core/types.rs` `SpawnRequest::notification_policy` | present |
| A3-1 | `SetNotificationPolicy{session, policy}` | `core/op.rs` `Op::SetNotificationPolicy` | present |
| A3-1 | route frame `0x0F output_dropped{notifications, last}` | `codec/frames.rs` `OutputDropped`, `DroppedNotification`; `codec/wire.rs` `TYPE_OUTPUT_DROPPED` | present |
| A3-1 | `last.truncated` | `codec/frames.rs` `DroppedNotification` | present |
| A3-1 | `route_queue_bytes` charges a deflate frame at its inflated size | none | n/a: runtime (worker); no limit added |
| A3-3 | feature `output_dropped` (route feature) | `core/features.rs` `Feature::OutputDropped`; `core/route.rs` `AttachOptions::route_features` (strings) | present |
| A3-3 | feature `notification_policy` (per worker) | `core/features.rs` `Feature::NotificationPolicy` | present |
| A3-4 | codec pin moves to revision 36 | `codec/lib.rs` (revision 36) | present |

### Core A4 (`core-contract-v1.17-amendment-4-candidate1.md`)

| Clause | Item | Crate symbol | Status |
|---|---|---|---|
| A4-1 | `service_report(id) -> Result<SpawnReport, UnknownService>` | `core/api.rs` `CoreApi::service_report` (returns `CoreError` with `ErrorCode::UnknownService`) | present |
| A4-1 | `SpawnReport{applied, unsupported}` | `core/service.rs` `SpawnReport` | present |

### Core erratum 1 (`core-contract-v1.17-erratum-1-candidate1.md`)

| Clause | Item | Crate symbol | Status |
|---|---|---|---|
| E1-1 | the worker binary has no fixed name in Core | none | n/a: prose (install layout is HC RT-3) |

### Core A5 (`core-contract-v1.17-amendment-5-candidate4.md`)

| Clause | Item | Crate symbol | Status |
|---|---|---|---|
| A5-1 | crate `botster-core-testkit` | none in this repo | n/a: a botster-core crate |
| A5-2 | `with_seed(n)` | none in this repo | n/a: testkit API in botster-core |
| A5-3, A5-4 | reachability of every code; two suite runs | none | n/a: test architecture |

### Core A6 (`core-contract-v1.17-amendment-6-candidate3.md`)

| Clause | Item | Crate symbol | Status |
|---|---|---|---|
| A6-1 | service-lane transport edge | none | n/a: testkit edge |
| A6-2 | first worker protocol is 1 | `core/api.rs` `CoreApi::{worker_protocol, adoptable_worker_protocols, worker_protocol_compatibility}`, `WorkerCompatibility` | present (the value is runtime) |
| A6-3 | `Remove` completes with `RemoveReport` | `core/op.rs` `OpOutput::RemoveReport`, `RemoveReport::uploads` | present |
| A6-3 | `Deleted`, `NotDeleted{reason: delete_failed{paths} \| outcome_unknown}` | `core/op.rs` `UploadsOutcome::{Deleted, NotDeleted}`, `NotDeleted::{DeleteFailed, OutcomeUnknown}` | present |

### Core erratum 2 (`core-contract-v1.17-erratum-2-candidate2.md`)

| Clause | Item | Crate symbol | Status |
|---|---|---|---|
| E2-1 | OSC 1 produces no `TitleChanged` | none | n/a: runtime |
| E2-2 | `other_modes` holds model state only | `codec/types.rs` `ModeFlags::other_modes` | present (meaning only) |
| E2-3 | `ModesChanged` on a change of state | `core/event.rs` `Event::ModesChanged` | present (meaning only) |

### Core A7 (`core-contract-v1.17-amendment-7-candidate4.md`)

| Clause | Item | Crate symbol | Status |
|---|---|---|---|
| A7-1 | an attach option out of range is sync `InvalidInput{field}`; `field` names the option | `core/error.rs` `ErrorCode::InvalidInput::field` (`Option<String>`, absent when no clause names a field); options `core/route.rs` `AttachOptions`, `RouteLimitChoices` | fixed (stage0/invalid-input-field; the check is runtime) |

### Core A8 (`core-contract-v1.17-amendment-8-candidate5.md`)

| Clause | Item | Crate symbol | Status |
|---|---|---|---|
| A8-1 | `CaptureSnapshot` reserves `max_snapshot_bytes` at `begin` | `core/limits.rs` `CoreLimits::{max_snapshot_bytes, snapshot_retained_bytes}`; `ErrorCode::CaptureLimit` | present (the rule is runtime) |
| A8-2 | `SnapshotTooLarge` also for an uncarriable pending state | `core/error.rs` `ErrorCode::SnapshotTooLarge`; `RouteCloseReason::SnapshotTooLarge` | present (meaning only) |
| A8-2 | the `snapshot_format` description names the continuation limit | none | n/a: the published format document of real Core, not a contract type (`SnapshotFormat{name, version}` is unchanged) |

### Core A9 (`core-contract-v1.17-amendment-9-candidate1.md`)

| Clause | Item | Crate symbol | Status |
|---|---|---|---|
| A9-1 | `max_frame_bytes` at least the route's own `attached` frame; else sync `InvalidInput{field: route_limits.max_frame_bytes}` | `core/route.rs` `RouteLimitChoices::max_frame_bytes`; `ErrorCode::InvalidInput::field` | fixed with A7-1 (the bound is runtime; no number). Core A19-1 (final38) replaces this floor with the route's floor (every floor frame at its widest value); the crate has no floor computation, so no symbol changes |
| A9-2 | two A3-1 tests withdrawn | `conformance/withdrawn.txt` | present |
| A9-3 | per-frame compression choice in the testkit | none | n/a: testkit edge |

### Core A10, A11, A12 (`core-contract-v1.17-amendment-10-candidate1.md`, `core-contract-v1.17-amendment-11-candidate1.md`, `core-contract-v1.17-amendment-12-candidate2.md`)

| Clause | Item | Crate symbol | Status |
|---|---|---|---|
| A10-1, A10-2 | impostor at the adopt handshake; corrupted registry row → `Lost(RegistryCorrupt)` | `core/types.rs` `LostReason::RegistryCorrupt` | present; the edges are n/a (testkit) |
| A11-1 | impostor frames are accepted by nothing | none | n/a: runtime |
| A12-1a, A12-1b | held report; cleanup script point | none | n/a: testkit edges |

### Core erratum 3 and erratum 4

| Clause | Item | Crate symbol | Status |
|---|---|---|---|
| E3-1 | a due deadline's effect runs in its `pump`; `pump_events` bounds a `pump` | `core/limits.rs` `CoreLimits::pump_events` | present (the rule is runtime) |
| E4-1 | the reserved cost C of a query reply | none | n/a: runtime |

### Core A13 (`core-contract-v1.17-amendment-13-candidate5.md`) — fixed in 5bc6ded

| Clause | Item | Crate symbol | Status |
|---|---|---|---|
| A13-1, A13-2 | `ClipboardWrite{id, selection, contents, total_bytes, reason?}` (field `bytes` replaced) | `core/event.rs` `Event::ClipboardWrite::contents` | **fixed** |
| A13-1 | `contents: [{mime, bytes}]`; `none` for `TooLarge`; `[]` = clear | `core/event.rs` `ClipboardContent::{mime, bytes}`, `Option<Vec<ClipboardContent>>` | **fixed** |
| A13-1 | `reason: TooLarge` | `core/event.rs` `ClipboardReason::TooLarge` | present |
| A13-1 | `selection` letters `c`, `p`, `s` or the OSC 52 string | `Event::ClipboardWrite::selection: String` | present (value is runtime) |
| A13-1b | the OSC 5522 acknowledgement (`SUCCESS`, `IO_ERROR`) | none | n/a: worker behavior; bytes are libghostty's (R-7) |
| A13 note | OSC 52 read is `ClipboardRead{selection}` | `codec/types.rs` `ClipboardRead` | present (unchanged) |

### Core A14 (`core-contract-v1.17-amendment-14-candidate3.md`)

| Clause | Item | Crate symbol | Status |
|---|---|---|---|
| A14-1, A14-2 | decoded size and contents size; `total_bytes` of each outcome | `core/event.rs` `Event::ClipboardWrite::total_bytes` | n/a: meaning only (type unchanged) |
| A14-3 | the model's limit equals `clipboard_bytes` | `core/limits.rs` `CoreLimits::clipboard_bytes` | present (unchanged default 1 MiB) |

### Core A15 (`core-contract-v1.17-amendment-15-candidate2.md`) — fixed in 5bc6ded

| Clause | Item | Crate symbol | Status |
|---|---|---|---|
| A15-1, A15-2 | `CoreLimits.max_key_text_bytes` | `core/limits.rs` `CoreLimits::max_key_text_bytes` | **fixed** |
| A15-1 | default 256 | `core/limits.rs` `impl Default for CoreLimits` | **fixed** |
| A15-1 | range 1 to 4,096; outside is `InvalidConfig` at `open` | `core/limits.rs` `CoreLimits::check_ranges` (`in_range("max_key_text_bytes", self.max_key_text_bytes.into(), 1, 4096)`), `check_at_least_one` | **fixed** |
| A15-1 | a longer host key `text` is sync `InvalidInput{field: "text"}`; a longer route key `text` is `input_refused{too_large}` | `ErrorCode::InvalidInput::field`; codec `input_refused` reasons | fixed with A7-1 (the checks are runtime) |

### Core A16 (`core-contract-v1.17-amendment-16-candidate3.md`) — fixed in 5bc6ded

| Clause | Item | Crate symbol | Status |
|---|---|---|---|
| A16-1, A16-2 | `ServiceEnd = Exited(ServiceExit) \| Lost{reason, payload_may_remain}` | `core/service.rs` `ServiceEnd::{Exited, Lost}` | **fixed** |
| A16-2 | `StopService` `Ok` value is `ServiceEnd` | `core/op.rs` `OpOutput::ServiceEnd` (wire `service_end`; replaces `OpOutput::ServiceExit`) | **fixed** |
| A16-1 | `Lost{EpochExhausted}` → `Exited` only through `StopService` | none | n/a: runtime |

## Route codec revision 36 (`route-codec-v1-extract-rev36.md`; differences from final6's revision 26)

The file's own revision log (revisions 27 to 36) lists every change. The crate states its revision (`codec/lib.rs` line 1).

| Revision | Item | Crate symbol | Status |
|---|---|---|---|
| 27 (R-1) | A.1 tagged values: a value with no data is a string; with data, a one-key object | `codec/` serde of every reason enum (e.g. `RefusalReason`, `CloseReason`, `HistoryState`) | present |
| 27 (R-2) | `route_closed { reason, exit? }`, `exit` only for `session_ended` | `codec/frames.rs` `RouteClosed::{reason, exit}` | present |
| 27 (R-3) | refusal `wrong_route` | `codec/types.rs` `RefusalReason::WrongRoute` | present |
| 27 (R-4) | `attach` `writable?: bool` (default `true`); refusal `not_writable` | `hub/actions.rs` attach args `writable: Option<bool>`; `codec/types.rs` `RefusalReason::NotWritable` | present |
| 27 (R-5) | paste and file sequencing errors → `input_refused { reason: refused }` | `codec/types.rs` `RefusalReason::Refused` | present |
| 27 (R-6) | resync baseline: `screen` only, `baseline_end.history: not_requested` | `codec/types.rs` `HistoryState::NotRequested` | present (the rule is runtime) |
| 27 (R-7) | `other_modes`: object from mode name to bool | `codec/types.rs` `ModeFlags::other_modes: BTreeMap<String, bool>` | present |
| 27 (R-8) | meanings of `resync.reason` and `terminal_query.answer_within_ms` | `codec/types.rs` `ResyncReason::{Stalled, Overflow}`; `codec/frames.rs` `TerminalQuery::answer_within_ms` | present (meaning only) |
| 27 (R-9) | `paste_begin.total_bytes` advisory | `codec/` paste begin `total_bytes` | present (meaning only) |
| 27 (R-10) | `route.ended` subject `{ route_id }`; reasons `not_connected`, `handoff_failed`, `transport_lost`, `write_failed`, `stalled`, `session_lost` | `hub/events.rs` `RouteEndedSubject::route_id`, `RouteEndedPayload`; `codec/types.rs` `RouteEndReason` | present |
| 28, 29 | `file_begin.name` length refusal removed; the worker shortens the name | none | absent (deleted refusal); shortening is runtime |
| 30 | `baseline_end.history` `unavailable` / `incomplete` carry a reason | `codec/types.rs` `HistoryState::{Unavailable, Incomplete}`, `HistoryUnavailable`, `HistoryIncomplete` | present |
| 32–36 | frame `0x0F output_dropped { notifications, last }` | `codec/frames.rs` `OutputDropped::{notifications, last}`, `ToClient::OutputDropped`; `codec/wire.rs` `TYPE_OUTPUT_DROPPED` | present |
| 33–36 | `last` required: `{ source, title?, body?, truncated? }`; empty title or body omitted | `codec/frames.rs` `DroppedNotification::{source, title, body, truncated}` | present |
| 33, 35 | route feature `output_dropped` | `core/features.rs` `Feature::OutputDropped`; `route_features` strings | present |
| 32 | `lane_full` deleted from the route reasons | none | absent (deleted) |
| (gate 2) | Core's `paste_require_bracketed_if` deleted | none | absent (deleted) |

## Hub→client (HC)

### HC A1 (`hub-client-contract-v1-amendment-1-candidate5.md`)

| Clause | Item | Crate symbol | Status |
|---|---|---|---|
| §6 `package` | `source` = `{local{path}}`, `{git{url, ref?, path?}}`, or the catalog entry | `hub/rows.rs` `PackageSource::{Local{path}, Git{url, git_ref, path}, Catalog{catalog_id, entry_id}}` | present |
| §6 `package` | the lock gains `source_path` | `hub/rows.rs` `PackageLock::source_path` | present |
| §6 `catalog` | entry `source` = `{git url, ref?, path?}` | `hub/rows.rs` `CatalogEntry::source: GitSource`; `GitSource::{url, git_ref, path}` | present |
| §6 `catalog` | entry `kind` ∈ `plugin`, `agent_definition`, `accessory` | `hub/rows.rs` `CatalogEntry::kind: PackageKind` | present |
| A.9 install | `create(package)` git source `{kind: git, url, ref?, path?}` | `hub/actions.rs` `PackageInstallArgs::source: PackageSource` | present |
| A.9 install | `failed` reasons `manifest_missing`, `manifest_ambiguous`, `manifest_malformed{path?}`, `manifest_unknown_key`, `manifest_bad_version`, `manifest_bad_scope`, `manifest_bad_allow` | `plugin/errors.rs` `CODES` (HP owns them; `hub/registry.rs` `OPERATION_KINDS` `package.install` lists only the HC reasons, by its comment) | present |
| PKC-1 | `failed{path_not_found}`, `failed{path_not_contained}` | `hub/registry.rs` `OPERATION_KINDS` `package.install` `failed_reasons` | present |
| PKC-1 | `invalid_arguments{field_errors}` with `detail.code: path_invalid` | `hub/registry.rs` `PACKAGE` verb `create` reasons `invalid_arguments:path_invalid`; `hub/error.rs` `Error::{detail, field_errors}` | present (verb reason fixed in f4557da) |
| PKC-1 | path syntax (relative, `/`, no `.`, `..` or empty segment), bounded by `max_frame_bytes` | none | n/a: runtime check; `path` is a `String` |

### HC A2 (`hub-client-contract-v1-amendment-2-candidate6.md`), part 1: where it lives

| Clause | Item | Crate symbol | Status |
|---|---|---|---|
| RT-1 | app name `botster`; `BOTSTER_CONFIG_DIR`, `BOTSTER_DATA_DIR`; the config root and data root rule | none | n/a: process environment and paths, not a wire type |
| RT-2 | data-root layout: `hub.lock`, `store/`, `packages/`, `logs/hub.log`, `terminfo/`, `sessions/<session_id>/context.json`, `uploads/`; modes 0700 / 0600; CLI `logs/cli/` and `cli/` | none | n/a: file system layout of the Hub and CLI |
| RT-2 | the data root shown to clients | `hub/rows.rs` `HubRow::data_root` | present |
| RT-3 | install directory with the seven sibling binaries | none | n/a: install layout |
| RT-4 | the lock `<data root>/hub.lock`; HS-0 `failed lock_held` | `hub/handshake.rs` `READY_FAILURE_CODES` (`lock_held`) | present (the lock itself: n/a, runtime) |
| RT-5 | runtime directory and socket owner checks; HS-0 `failed socket_unavailable` | `hub/handshake.rs` `READY_FAILURE_CODES` (`socket_unavailable`) | present (the checks: n/a, runtime) |
| CF-1 | `<config root>/hub.toml` (TOML 1.0), refused when another user can write it; `BOTSTER_OFFLINE=1` | none | n/a: the Hub reads the file; no crate parses `hub.toml` |
| CF-1 | `failed config_invalid <key>`, `failed config_unknown_key <key>` | `hub/handshake.rs` `READY_FAILURE_CODES` (`config_invalid`, `config_unknown_key`) | present |
| CF-1 | `invoke(hub, reload_config)` → `{applied, restart_required}` | `hub/actions.rs` `ReloadConfigResult::{applied, restart_required}`; `hub/registry.rs` `HUB` action `reload_config` | present |
| CF-1 | reload refusal `invalid_arguments{config_invalid, detail{key}}` | `hub/registry.rs` `HUB` action `reload_config` reasons (`invalid_arguments:config_invalid`); `hub/error.rs` `Error::detail` | present |
| CF-1 | event `hub/config.changed{keys, restart_required}` | `plugin/events.rs` `ConfigChangedEvent`, `EVENTS` row `hub` `config.changed` (HP A2-52) | present |
| CF-2 | `[hub] offline` bool, default `false`, restart | none | n/a: `hub.toml` key; its effect is the `check_update` result `failed{offline}` (`hub/actions.rs` `CheckUpdateState::Failed`) |
| CF-2 | `[hub] log_level` ∈ `debug`, `info`, `warn`, `error`, default `info`, live | none | n/a: `hub.toml` key |
| CF-2 | `[limits] max_frame_bytes`, `max_chunk_bytes`, `max_outstanding`, `max_watches` (≥ 32), `route_connect_seconds`, `idempotency_window_seconds`, `cursor_idle_seconds`, `operation_retention_seconds`; default: the Hub's, restart | `hub/handshake.rs` `Limits::{max_frame_bytes, max_chunk_bytes, max_outstanding, max_watches, route_connect_seconds, idempotency_window_seconds, cursor_idle_seconds, operation_retention_seconds}` (the values in `welcome.limits`) | present (the keys: n/a, `hub.toml`; the bound ≥ 32: runtime) |
| CF-2 | `[routes] max_paste_bytes`, `max_file_bytes`, `query_client_deadline_ms`, `stall_deadline_ms`, `stall_close_after_ms`; live for new routes | `codec/types.rs` `RouteLimits::{max_paste_bytes, max_file_bytes, query_client_deadline_ms, stall_deadline_ms, stall_close_after_ms}` (the values in `route_limits`) | present (the keys: n/a, `hub.toml`) |
| CF-2 | `[sessions] activity_idle_ms` integer, default `3000`, live | none | n/a: `hub.toml` key; `hub/rows.rs` `Activity` names it in its doc |
| CF-2 | `[sessions] worktree_base` absolute path, default `<data root>/worktrees` | none | n/a: `hub.toml` key |
| CF-2 | `[sessions] term` string, default Core's entry name, restart | none | n/a: `hub.toml` key |
| CF-2 | `[agents] scopes` list, default every client permission, live | none | n/a: `hub.toml` key; the effective set is `hub/actions.rs` `WhoamiResult::scopes` |
| CF-2 | `[audit] retention_days` default `30`; `[audit] max_bytes` default `67108864` | none | n/a: `hub.toml` keys |
| CF-2 | `[network] trusted_hosts` list of `{path, sha256}`, default empty, restart | none | n/a: `hub.toml` key (HP TR-1) |
| CF-2 | `[update] check_on_start` bool, default `true`, restart | none | n/a: `hub.toml` key |
| HS-0 | ready line UTF-8 + LF; `failed` codes `lock_held`, `config_invalid`, `config_unknown_key`, `socket_path_too_long`, `socket_unavailable`, `adoption_failed`, `internal` | `hub/handshake.rs` `READY_FAILURE_CODES` | present |
| TB-L1 | `<app>` is `botster` | none | n/a: path |
| MC-1c | `TERM`, `TERMINFO`, context file path, `PATH` | none | n/a: process environment of a session |
| MC-1c | `session_type_dir` absent for a `create(session_type)` type | none | n/a: a member of the context file, which no crate models (the base MC-1c file has no crate type either) |
| §6 `session_type` `source` | `{repo{target_id}}` | `hub/rows.rs` `TypeSource::Repo(RepoSource{target_id})` | present |
| §6 `session_type` `source` | `create(session_type, {source: {repo}})` → `unsupported{repo_source_is_discovered}` | `hub/registry.rs` `SESSION_TYPE` verb reasons (`unsupported:repo_source_is_discovered`) | fixed (f4557da) |
| SF-5b | `push_public_key` base64url P-256 point | `hub/rows.rs` `HubRow::push_public_key: String` | present (the encoding: runtime) |
| PR-4 | `GET <web origin>/hubs/<hub_id>/ice` → `{servers: [{urls, username?, credential?}]}` | `hub/pairing.rs` `IceServers::servers`, `IceServer::{urls, username, credential}` | present |
| PR-3 | client key file `<config root>/credentials/<hub_id>.json`, 0600 | none | n/a: client file |
| §6 `navigation`, A.9 `resolve_route` | `resolve_route{route_path}` → `{target: {kind: view, id, select?}}`; `not_found` for an unknown path | `hub/actions.rs` `ResolveRouteArgs::route_path`, `ResolveRouteResult::target`; `hub/rows.rs` `NavTarget::{kind, id, select}`, `NavTargetKind::View` | present |
| §6 `navigation` | the Web URL form and the reserved paths `/signal`, `/ice` | none | n/a: URL syntax, runtime |
| §6 `catalog` | fields exactly `id`, `url`, `refreshed_at?`, `entries` | `hub/rows.rs` `CatalogRow::{id, url, refreshed_at, entries}` | present |
| G-2 | `package enable {fs_roots?}` | `hub/actions.rs` `EnableArgs::fs_roots` | present |
| G-2 | `package enable` refusals `invalid_arguments{fs_root_unknown \| path_not_absolute \| path_not_found}`, `wrong_state{fs_root_unbound}` | `hub/registry.rs` `PACKAGE` action `enable` reasons | present |
| G-2 | `package update` `configuration` merge patch, `webhook_secret` shown as `set: bool` | `hub/actions.rs` `PackagePatch::configuration: OpenObject` | present (the secret field is package-defined configuration: n/a) |
| G-2 | `spawn_target rescan_packages` → operation `succeeded{found, removed}`; `wrong_state{not_a_directory}` | `hub/actions.rs` `RescanResult::{found, removed}`; `hub/registry.rs` `OPERATION_KINDS` `spawn_target.rescan_packages`, `SPAWN_TARGET` action `rescan_packages` reasons | present |
| G-2 | `session_type create/update` `agent_scopes?` | `hub/actions.rs` `SessionTypeDefinition::agent_scopes` | present |
| G-2 | `session create` `agent_scopes?` | `hub/actions.rs` `SessionCreateType::agent_scopes`, `SessionCreateCommand::agent_scopes` | present |
| G-2 | `invalid_arguments{scope_widens}` on `session_type` and `session` create | `hub/registry.rs` `SESSION_TYPE` verb reasons, `SESSION` verb `create` reasons | fixed (f4557da) |
| G-2 | `hub reload_config` | see CF-1 above | present |

### HC A2, part 2: every command fully specified

`hub/registry.rs` holds one `KindSpec` per kind (`HUB`, `SESSION`, `SESSION_TYPE`, `SPAWN_TARGET`, `WORKTREE`, `PACKAGE`, `CATALOG`, `QUARANTINE`, `NAVIGATION`, `CLIENT_CREDENTIAL`, `PUSH_SUBSCRIPTION`, `ROUTE`). An action's A.9b failures are `ActionSpec::reasons` (base); a verb's A.9b failures are `VerbSpec::reasons` (added in f4557da). Both are emitted as `x-actions.*.reasons` and `x-verbs.*.reasons` in the kind schemas. A reason is written `code:reason`, or `code` when A.9b gives no reason.

| Clause | Item | Crate symbol | Status |
|---|---|---|---|
| A.1b | an enum value with no data is a string; with data, a one-key object | serde forms of every HC enum (`hub/lib.rs` doc; tests `hub/tests.rs` A.1b) | present |
| A.1b | `Error` gains `detail?: object`; named data (`sessions`, `unknown`, `candidates`, `consumers`, `interface`, `range`, `found`, `name`, `key`, `code`) are members of `detail` | `hub/error.rs` `Error::detail: Option<OpenObject>` | present |
| A.7b | the `{operation}` result is the full `operation` row at admission | `hub/rows.rs` `OperationRow::{id, kind, state, terminal, progress, result, error}`; `hub/actions.rs` `Receipt` | present |
| A.7b | session create keeps `{operation_id, session, state}` | `hub/actions.rs` `SessionCreateReceipt` | present |
| A.7b | generic states `queued`, `running`, `succeeded`, `failed{reason}`, `cancelled` | `hub/registry.rs` `GENERIC`; `hub/rows.rs` `OperationState::{name, reason, data}` | present |
| A.7b | `progress` absent or `{message, fraction?}` (0..1) | `hub/rows.rs` `Progress::{message, fraction}` | present |
| A.7b | `spawn`: `starting`, `installed`, `failed{exec_failed \| cwd_missing \| worktree_failed \| worker_failed \| session_type_unavailable}`, `cancelled`; result the session ref | `hub/registry.rs` `SPAWN`, `OPERATION_KINDS` `spawn` `failed_reasons` | present |
| A.7b | `safe_write` as HP SW-2 | `hub/registry.rs` `OPERATION_KINDS` `safe_write`; `plugin` SW-2 outcomes | present |
| A.7b | `hub.update`: `result{version}`; `failed{download_failed \| verify_failed \| install_failed}` | `hub/actions.rs` `UpdateResult::version`; `OPERATION_KINDS` `hub.update` | present |
| A.7b | `hub.shutdown`, `hub.restart`, `hub.swap` ended by `close{hub_shutdown, lock_released}` | `OPERATION_KINDS` `hub.shutdown`, `hub.restart`, `hub.swap`; `hub/handshake.rs` `HubShutdownData`, `CloseReason` | present |
| A.7b | `hub.upgrade_packages`, `hub.rollback_packages`: `result{packages: [{package, state, reason?}]}` | `hub/actions.rs` `BulkPackagesResult::packages`, `PackageOutcome::{package, state, reason}` | present |
| A.7b | `session.stop`, `worktree.create` (`failed{git_failed}`), `worktree.delete` | `OPERATION_KINDS` `session.stop`, `worktree.create`, `worktree.delete` | present |
| A.7b | `package.install` `failed` with `fetch_failed`, `url_refused`, `too_large`, `hash_mismatch` (+ HP PM-4, PD-6, PK-3, PKC-1 reasons) | `OPERATION_KINDS` `package.install` `failed_reasons` | present |
| A.7b | `package.remove`, `package.enable`, `package.disable`, `package.reload`, `package.entrypoint` | `OPERATION_KINDS` rows of the same names | present |
| A.7b | `package.upgrade`: `result{package, version, resolved_commit}`; `failed{fetch_failed \| hash_mismatch \| activation_failed}` | `hub/actions.rs` `PackageUpgradeResult`; `OPERATION_KINDS` `package.upgrade` | present |
| A.7b | `package.rollback`: `result{package, ref, resolved_commit}`; `failed{hash_mismatch}` | `hub/actions.rs` `PackageRollbackResult::{package, git_ref, resolved_commit}`; `OPERATION_KINDS` `package.rollback` | present |
| A.7b | `catalog.refresh`: `failed{fetch_failed \| url_refused \| malformed \| too_large}` | `OPERATION_KINDS` `catalog.refresh` | present |
| A.7b | `spawn_target.rescan_packages`: `succeeded{found, removed}` | `hub/actions.rs` `RescanResult`; `OPERATION_KINDS` `spawn_target.rescan_packages` | present |
| A.9b | common failures `invalid_arguments`, `permission_denied`, `unsupported`, `not_found`, `remote_hub_unsupported`, `too_many_requests`, `busy`, `response_too_large`, `hub_shutdown`, `cancelled`, `internal`, `stale_descriptor`, `plugin_failed` | `hub/error.rs` `ErrorCode::{InvalidArguments, PermissionDenied, Unsupported, NotFound, RemoteHubUnsupported, TooManyRequests, Busy, ResponseTooLarge, HubShutdown, Cancelled, Internal, StaleDescriptor, PluginFailed}` | present |
| A.9b (H-11) | a missing named resource is `invalid_arguments` with a `field_errors` entry `{code: not_found, pointer}` | `hub/error.rs` `FieldError::{pointer, code}` | present |
| A.9b | `hub check_update`: none | `HUB` action `check_update` (no reasons) | present |
| A.9b | `hub update`: `wrong_state:up_to_date`, `:offline`, `:dev_build`, `:update_in_progress`, `:swap_in_progress` | `HUB` action `update` reasons | present |
| A.9b | `hub shutdown`, `restart`: `wrong_state:swap_in_progress`, `:update_in_progress`, `:package_operation_in_progress` | `HUB` actions `shutdown`, `restart` reasons | present |
| A.9b | `hub reload_config`: `invalid_arguments:config_invalid` | `HUB` action `reload_config` reasons | present |
| A.9b | `hub diagnostics`, `preview_package_upgrades`: common only | `HUB` actions `diagnostics`, `preview_package_upgrades` (no reasons) | present |
| A.9b | `hub swap_preflight`: `invalid_arguments:unknown_worker_protocol` | `HUB` action `swap_preflight` reasons | present |
| A.9b | `hub swap`: `wrong_state:unadoptable_sessions` (`detail{sessions, unknown}`), `wrong_state:swap_in_progress`, `:update_in_progress`, `:package_operation_in_progress`, `invalid_arguments:unknown_worker_protocol` | `HUB` action `swap` reasons; `hub/actions.rs` `Unadoptable`, `UnknownProtocol` | present |
| A.9b | `hub issue_pairing`: `wrong_state:no_web_origin`, `invalid_arguments:ttl_out_of_range` | `HUB` action `issue_pairing` reasons; `hub/actions.rs` `IssuePairingArgs::ttl_seconds` | present |
| A.9b | `hub upgrade_packages`: `conflict:requirement_conflict`, `:ambiguous_provider`, `:requirement_missing`, `:breaking_not_accepted` (`detail{consumers}`), `wrong_state:package_operation_in_progress` | `HUB` action `upgrade_packages` reasons | present |
| A.9b | `hub rollback_packages`: `wrong_state:no_history`, `:package_operation_in_progress` | `HUB` action `rollback_packages` reasons | present |
| A.9b | `session create` (type): `wrong_state:swap_in_progress`, `internal:credential_unavailable`, `invalid_arguments:target_mismatch`, `wrong_state:session_type_unavailable`, `invalid_arguments:scope_widens` | `SESSION` verb `create` reasons | fixed (f4557da) |
| A.9b | `session create` (command): `permission_denied`, `invalid_arguments:argv_empty`, `:cwd_not_allowed`, `wrong_state:swap_in_progress` | `SESSION` verb `create` reasons | fixed (f4557da) |
| A.9b | `session update`: `conflict` (`if_rev`), `invalid_arguments` | `SESSION` verb `update` reasons | fixed (f4557da) |
| A.9b | `session delete`: `wrong_state` for `starting`, `running`, `stopping` | `SESSION` verb `delete` reasons | fixed (f4557da) |
| A.9b | `session attach`: `unsupported:binding_auth_unavailable`, `:terminal_format`, `wrong_state:session_not_running`, `not_found:route`, `unsupported:binding`, `busy` | `SESSION` action `attach` reasons | present |
| A.9b | `session attach_offer`: `permission_denied:bad_signature`, `not_found:route`, `wrong_state:route_connected`, `:route_expired`, `:not_webrtc_route`, `busy` | `SESSION` action `attach_offer` reasons | present |
| A.9b | `session detach`: `not_found:route` | `SESSION` action `detach` reasons | present |
| A.9b | `session stop`: `wrong_state:not_running` | `SESSION` action `stop` reasons | present |
| A.9b | `session restart`: `wrong_state:not_ended`, `:not_restartable`, `:record_unavailable`, `:environment_not_retained`, `:not_ready`, `:swap_in_progress` | `SESSION` action `restart` reasons | present |
| A.9b | `session read_screen`: `wrong_state:no_screen_model` | `SESSION` action `read_screen` reasons | present |
| A.9b | `session clear_attention`: none | `SESSION` action `clear_attention` (no reasons) | present |
| A.9b | `session safe_write`: `wrong_state:session_ended`, `invalid_arguments:text_too_long`, `:text_empty`, `busy` | `SESSION` action `safe_write` reasons | present |
| A.9b | `session write`: `wrong_state:session_ended`, `busy`, `invalid_arguments:bytes_too_long`, `:bytes_empty`, `:bytes_invalid` | `SESSION` action `write` reasons | present |
| A.9b | `session_type create`, `update`: `conflict`, `invalid_arguments:port_env_invalid`, `:scope_widens`, `wrong_state:not_editable`, `unsupported:repo_source_is_discovered` | `SESSION_TYPE` verbs `create`, `update` reasons | fixed (f4557da) |
| A.9b | `session_type delete`: `wrong_state:not_editable` | `SESSION_TYPE` verb `delete` reasons | fixed (f4557da) |
| A.9b | `session_type resolve`, `definition`: common only | `SESSION_TYPE` actions `resolve`, `definition` (no reasons) | present |
| A.9b | `spawn_target create`, `update`: `invalid_arguments:path_not_absolute`, `:path_not_found`, `:not_a_directory`, `conflict:path_exists` | `SPAWN_TARGET` verbs `create`, `update` reasons | fixed (f4557da) |
| A.9b | `spawn_target delete`: `wrong_state:in_use` | `SPAWN_TARGET` verb `delete` reasons | fixed (f4557da) |
| A.9b | `spawn_target validate`: none; `problems[].code` ∈ `path_missing`, `not_a_directory`, `not_git`, `unreadable` | `SPAWN_TARGET` action `validate`; `hub/actions.rs` `ValidateResult::problems: Option<Vec<CodeMessage>>` | present (the codes are an open string) |
| A.9b | `spawn_target browse`: `invalid_arguments:path_not_absolute`, `not_found:path`, `wrong_state:not_a_directory`, `:unreadable` | `SPAWN_TARGET` action `browse` reasons | present |
| A.9b | `spawn_target rescan_packages`: `wrong_state:not_a_directory` | `SPAWN_TARGET` action `rescan_packages` reasons | present |
| A.9b | `worktree create`: `wrong_state:not_git`, `conflict:branch_checked_out`; operation `failed{git_failed}` | `WORKTREE` verb `create` reasons; `OPERATION_KINDS` `worktree.create` | fixed (f4557da) |
| A.9b | `worktree delete`: `wrong_state:dirty`, `:occupied` | `WORKTREE` verb `delete` reasons | fixed (f4557da) |
| A.9b | `package create` (install): `invalid_arguments:path_invalid`, `conflict:already_installed` | `PACKAGE` verb `create` reasons | fixed (f4557da) |
| A.9b | `package delete` (remove): none | `PACKAGE` verb `delete` (no reasons) | present |
| A.9b | `package update` (configuration): `invalid_arguments` | `PACKAGE` verb `update` reasons | fixed (f4557da) |
| A.9b | `package enable`: `wrong_state:staged`, `:quarantined`, `:errored`, `:fs_root_unbound`, `invalid_arguments:fs_root_unknown`, `:path_not_absolute`, `:path_not_found` | `PACKAGE` action `enable` reasons | present |
| A.9b | `package disable`: none | `PACKAGE` action `disable` (no reasons) | present |
| A.9b | `package reload`: `wrong_state:disabled` | `PACKAGE` action `reload` reasons | present |
| A.9b | `package start_entrypoint`, `stop_entrypoint`, `restart_entrypoint`: `wrong_state:disabled` | `PACKAGE` actions of the same names, reasons | present |
| A.9b | `package preview_upgrade`: `wrong_state:local_source`, `:fetch_failed` | `PACKAGE` action `preview_upgrade` reasons | present |
| A.9b | `package upgrade`: `conflict:breaking_not_accepted`, `:ambiguous_provider`, `:requirement_missing`, `:requirement_conflict`, `wrong_state:pinned`, `:local_source`, `:up_to_date`, `:package_operation_in_progress` | `PACKAGE` action `upgrade` reasons | present |
| A.9b | `package pin`: `wrong_state:local_source` | `PACKAGE` action `pin` reasons | present |
| A.9b | `package unpin`: none | `PACKAGE` action `unpin` (no reasons) | present |
| A.9b | `package rollback`: `wrong_state:no_history`, `:local_source` | `PACKAGE` action `rollback` reasons | present |
| A.9b | `package approve_grants`: `conflict:grants_mismatch`, `wrong_state:nothing_pending` | `PACKAGE` action `approve_grants` reasons | present |
| A.9b | `package open_asset`: `not_found`, `wrong_state:assets_origin_unconfigured`, `invalid_arguments` | `PACKAGE` action `open_asset` reasons | **fixed** (753fdaa; the base listed only `wrong_state:assets_origin_unconfigured`; test `open_asset_carries_its_a9b_failures`) |
| A.9b | `catalog create`: `invalid_arguments:url_not_https`, `conflict` | `CATALOG` verb `create` reasons | fixed (f4557da) |
| A.9b | `catalog delete`: none | `CATALOG` verb `delete` (no reasons) | present |
| A.9b | `catalog refresh`: operation failures | `CATALOG` action `refresh`; `OPERATION_KINDS` `catalog.refresh` | present |
| A.9b | `quarantine resolve`: none | `QUARANTINE` action `resolve` (no reasons) | present |
| A.9b | `navigation resolve_route`: `not_found` | `NAVIGATION` action `resolve_route` reasons | present |
| A.9b | `client_credential delete`: none (own credential: `res ok`, then `close{auth_failed}`) | `CLIENT_CREDENTIAL` verb `delete` (no reasons); `hub/handshake.rs` `CloseReason` (`auth_failed`) | present |
| A.9b | `route delete`: `not_found` | `ROUTE` verb `delete` reasons | fixed (f4557da) |
| A.9b | `push_subscription create`: `invalid_arguments:endpoint_not_https`, `conflict` | `PUSH_SUBSCRIPTION` verb `create` reasons | fixed (f4557da) |
| A.9b | `push_subscription delete`: `not_found` | `PUSH_SUBSCRIPTION` verb `delete` reasons | fixed (f4557da) |
| A.9b | plugin kinds: the declared `reasons` | `hub/describe.rs` `ActionDescriptor` reasons (`DescribedReason`) | present |
| H-18 | `wrong_state{swap_in_progress \| update_in_progress \| package_operation_in_progress}` | `HUB` actions `update`, `shutdown`, `restart`, `swap`, `upgrade_packages`, `rollback_packages` reasons; `PACKAGE` action `upgrade` reasons | present |
| H-3 | Hub row `update` = `{state: idle \| checking \| available \| downloading \| installing \| failed{reason}, available?{version}}` | `hub/rows.rs` `HubRow::update`, `UpdateStatus::{state, available}`, `UpdateState::{Idle, Checking, Available, Downloading, Installing, Failed}`, `AvailableUpdate::version` | present |
| H-3 | `check_update` → `{state: up_to_date \| ahead \| available \| failed{check_failed \| offline \| dev_build}, available?}` | `hub/actions.rs` `CheckUpdateResult::{state, available}`, `CheckUpdateState::{Known, Failed}`, `CheckState::{UpToDate, Ahead, Available}` | present (the `failed` reasons are an open string in `ReasonData`) |
| H-3 | reconnect after a self-restart; liveness bound 500 ms | none | n/a: client behavior |
| H-7, H-34 | `worktree.state` ∈ `creating`, `ready`, `deleting`, `failed`, `missing` | `hub/rows.rs` `WorktreeState::{Creating, Ready, Deleting, Failed, Missing}` | present |
| H-7, H-34 | `spawn_target.kind` ∈ `directory`, `git` (non-exhaustive), also a create input | `hub/rows.rs` `TargetKind::{Directory, Git}`, `SpawnTargetRow::kind`; `hub/actions.rs` `SpawnTargetInput::kind` | present |
| H-8 | MC-2 `whoami {}` → `{caller, scopes}` | `hub/actions.rs` `WhoamiResult::{caller, scopes}` | present |
| H-8 | MC-2 `delete {ref, args?, idempotency_key?, operation_id?}` | `hub/mcp.rs` `DeleteTool::{reference, args, request}` | present |
| H-12 | `delete` args: `package{purge?}`, `worktree{force?}` | `hub/actions.rs` `PackageRemoveArgs::purge`, `WorktreeDeleteArgs::force` | present |
| H-13 | DS-1 `reasons: [{code, message}]` | `hub/describe.rs` `DescribedReason::{code, message}` | present |
| H-14 | `watch(event, filter?)` → `{stream_id}` | `hub/envelope.rs` `WatchOpened::stream_id` | present |
| H-15 | `push_subscription create {endpoint, keys{p256dh, auth}}` → the row without the endpoint | `hub/actions.rs` `PushSubscriptionCreateArgs::{endpoint, keys}`, `PushKeys::{p256dh, auth}`; `hub/rows.rs` `PushSubscriptionRow::{id, credential_id, created_at}` | present |
| H-16 | `spawn_target` actions `validate`, `browse` (permission `spawn_target.browse`), `rescan_packages` | `SPAWN_TARGET` actions `validate`, `browse`, `rescan_packages` | present |
| H-9 | §11 mapping: `failed{wrong_state: r}` → `wrong_state{r}`, `invalid_request{r}` → `invalid_arguments{r}`, `backpressured` → `busy`, `effect: none` | none | n/a: Hub mapping at runtime; the result codes are `ErrorCode` |
| H-19 | quarantine `reason` ∈ `crash_loop`, `hash_mismatch`, `admission_failed`, `bootstrap_failed` | `hub/rows.rs` `QuarantineReason::{CrashLoop, HashMismatch, AdmissionFailed, BootstrapFailed}` | present |
| H-20 | session `failure_reason` ∈ `spawn_failed`, `exec_failed`, `cwd_missing`, `worker_lost` (non-exhaustive) | `hub/rows.rs` `SessionRow::failure_reason: Option<String>` | present (open string) |
| H-21 | `session_type create`: `command`, `args?`, `interaction?` (`interactive` default, `noninteractive`), `lifecycle?` (`restartable` default, `one_shot`), `role?`, `traits?` | `hub/actions.rs` `SessionTypeDefinition::{command, args, interaction, lifecycle, role, traits}`, `Lifecycle::{Restartable, OneShot}`; `hub/rows.rs` `Interaction::{Interactive, Noninteractive}` | present |
| H-26 | `target_worker_protocol` is a u16 | `hub/actions.rs` `SwapArgs::target_worker_protocol: u16`, `SwapPreflightArgs::target_worker_protocol: u16` | present |
| H-31 | `history_unavailable?`; `diagnostics?: [{code, message}]`; `problems[].code` | `hub/actions.rs` `ReadScreenResult::history_unavailable`, `ResolveResult::diagnostics`, `ValidateResult::problems`, `CodeMessage` | present |
| H-32 | VB-1 `limit` is a positive integer; non-positive is `invalid_arguments` | `hub/envelope.rs` `ListArgs::limit: Option<u64>` | present (0 is a runtime refusal) |
| H-33 | `plugin_log` `level` ∈ `debug`, `info`, `warn`, `error` | `hub/rows.rs` `LogRecord::level: LogLevel`, `LogLevel::{Debug, Info, Warn, Error}` | present |
| codec pin | revision 30 items R-1 to R-10 (`attach` `writable?`, `wrong_route`, paste and file reasons, `route_closed`, resync without history, `attach_failed` `snapshot_too_large` / `bad_peer`, tagged values) | `hub/actions.rs` `AttachArgs::writable`; the codec rows in the route codec section above | present |

### HC A3 (`hub-client-contract-v1-amendment-3-candidate9.md`)

| Clause | Item | Crate symbol | Status |
|---|---|---|---|
| codec pin | route codec revision 36 | see the route codec section | present |
| SF-5f | route feature `output_dropped`, offered in `attach` `route_features` and echoed in `attached.features` | `hub/actions.rs` `AttachArgs::route_features: Option<Vec<String>>`; `core/features.rs` `Feature::OutputDropped`; `codec/frames.rs` `Attached::features` | present |
| SF-5f | frame `output_dropped{notifications, last{source: bell \| osc9 \| osc777, title?, body?, truncated?}}` | `codec/frames.rs` `OutputDropped`, `DroppedNotification`; `codec/wire.rs` `TYPE_OUTPUT_DROPPED` | present |
| SF-5f | worst-case frame fit, exactly once, before the resync, the policy filter, one surface per client | none | n/a: worker and client behavior |
| SF-5c | the notification policy goes to the worker with the session | `core/types.rs` `SpawnRequest::notification_policy` (Core A3) | present |

### HC A4 (`hub-client-contract-v1-amendment-4-candidate1.md`)

| Clause | Item | Crate symbol | Status |
|---|---|---|---|
| EN-1a | `req{id, verb, kind, target?, action?, args?, idempotency_key?, operation_id?, descriptor_id?}` | `hub/envelope.rs` `Req::action: Option<String>` | present |
| EN-1a | `action` required for `invoke`, absent otherwise; else `invalid_arguments` | `hub/envelope.rs` `Req::check`, `ReqViolation::{ActionRequired, ActionForbidden}` | present |
| EN-1a | MC-2 `invoke` maps `action` | `hub/mcp.rs` `InvokeTool::action` | present |
| EN-8a | the idempotency match includes `action`; else `conflict{idempotency_key_reused}` | none | n/a: Hub runtime; HC keeps no table of request-level reasons (the code is `ErrorCode::Conflict`) |

### HC A5 (`hub-client-contract-v1-amendment-5-candidate2.md`)

| Clause | Item | Crate symbol | Status |
|---|---|---|---|
| A5-1 | the real Hub in-process; the in-memory transport is not a binding | none | n/a: test architecture |
| A5-2 | one transcript player (Rust, wasm for TS) | none in the contract crates | n/a: test tooling, not a contract type |
| A5-3 | transcript kinds `shared` and `client-only` | none in the contract crates | n/a: a transcript header field of the conformance suite |
| A5-4 | seeded interleaving by source | none | n/a: test architecture |
| A5-5 | FakeHub text replaced | none | n/a: prose |

### HC A6 (`hub-client-contract-v1-amendment-6-candidate1.md`) — fixed in d3158fd

| Clause | Item | Crate symbol | Status |
|---|---|---|---|
| A6-1 | `package.install` `result{package, version}` | `hub/actions.rs` `PackageInstallResult::{package, version}`; `hub/registry.rs` `OPERATION_KINDS` `package.install` `result`, `check_result` | **fixed** |
| A6-1 | the result on `succeeded` and on a `staged` end; none on `failed` | none | n/a: Hub runtime |
| A6-2 | `worktree.branch?` (absent, not null, when HEAD is detached) | `hub/rows.rs` `WorktreeRow::branch: Option<String>` (serde: skip when `None`, refuse `null`) | **fixed** |
| A6-2 | `worktree.head` (string, always present) | `hub/rows.rs` `WorktreeRow::head: String`; `hub/registry.rs` `WORKTREE` `core_fields` (`head`) | **fixed** |

### HC A7 (pending; the steward's candidate 2) — fixed in the tip commit of the branch ("hub contract: HC A7 (pending)")

| Clause | Item | Crate symbol | Status |
|---|---|---|---|
| A7-1 | `preview_upgrade` `breaking` always present | `hub/actions.rs` `UpgradePreview::breaking: Vec<BreakingEntry>` | **fixed** |
| A7-1 | entry `{consumer, interface, range, interface_version}` | `hub/actions.rs` `BreakingEntry::{consumer, interface, range, interface_version}` | **fixed** |
| A7-1 | bulk preview top-level `breaking` | `hub/actions.rs` `PreviewPackageUpgradesResult::breaking` | **fixed** |
| A7-2 | `package.static_origin?` (operators only; absent, not null) | `hub/rows.rs` `PackageRow::static_origin: Option<String>`; `hub/registry.rs` `PACKAGE` `core_fields` (`static_origin`) | **fixed** (operator-only visibility: n/a, Hub runtime) |
| A7-2 | HP listener `errored{port_unavailable}` | none | n/a: HP-internal; no HC field in v1 (steward ruling) |

## Hub→plugin (HP)

`CODES` is `plugin/errors.rs` `CODES` (generated as `schemas/plugin/plugin-errors.json`). `LIMITS` is `plugin/limits.rs` (the HP parameters, each with its kind, its `hub.toml` table and its live or restart status).

### HP A1 (`hub-plugin-contract-v1.1.amendment-1-candidate5.md`)

| Clause | Item | Crate symbol | Status |
|---|---|---|---|
| PK-1 | manifest file `botster-plugin.json` at the package root | `plugin/manifest.rs` `MANIFEST_FILE` | present |
| PK-1 | bound `package_bytes_max` (no new cap) | `plugin/limits.rs` `package_bytes_max` (PM-4) | present |
| PK-2 | `path` stage 1 refusal `invalid_request{detail.code: path_invalid}` | `CODES` `path_invalid` | present |
| PK-2 | stage 2 `failed{detail.code: path_not_found}`, `failed{detail.code: path_not_contained}` | `CODES` `path_not_found`, `path_not_contained` | present |
| PK-2 | the path syntax and the containment check | none | n/a: Hub runtime; `path` is a `String` |
| PK-3 | `manifest_missing`, `manifest_malformed{path}` | `CODES` `manifest_missing`, `manifest_malformed` | present |
| PK-3 | the specific codes stay: `manifest_unknown_key`, `manifest_bad_version`, `manifest_bad_scope`, `manifest_bad_allow` | `CODES` `manifest_unknown_key`, `manifest_bad_version`, `manifest_bad_scope`, `manifest_bad_allow` | present |
| PK-4 | schema artifact `plugin-manifest.schema.json` | `schemas/plugin/plugin-manifest.schema.json` (codegen from `plugin/manifest.rs` `Manifest`) | present |
| PK-5 | source `{kind: git, url, ref?, path?}`; `local` has no `path` | `plugin/files.rs` `PackageSource::{Local{path}, Git{url, git_ref, path}, Catalog{catalog_id, entry_id}}` | present |
| PK-5 | catalog entry `source{git url, ref?, path?}` | `plugin/files.rs` `CatalogEntry::source`, `CatalogSource::{url, git_ref, path}` | present |
| PK-5 | lock record gains `source_path` | `plugin/files.rs` `LockRecord::source_path` | present |
| PK-6 | `botster-agent.json`, `botster-accessory.json` | `plugin/manifest.rs` `AGENT_FILE`, `ACCESSORY_FILE`; `plugin/files.rs` `AgentDefinition`, `Accessory` | present |
| PK-6 | `manifest_ambiguous` | `CODES` `manifest_ambiguous` | present |
| PK-6 | schema artifacts `agent-definition.schema.json`, `accessory.schema.json` | `schemas/plugin/agent-definition.schema.json`, `schemas/plugin/accessory.schema.json` | present |
| PM-2, PK-6 | catalog entry `kind` ∈ `plugin`, `agent_definition`, `accessory` | `plugin/files.rs` `PackageKind::{Plugin, AgentDefinition, Accessory}`, `CatalogEntry::kind` | present |
| PK-7 | independent resolution per package; one resolution for bulk `upgrade_packages` | none | n/a: Hub runtime |
| PK-8 | discovery in `target_plugin_dir` | `plugin/manifest.rs` `DEFAULT_TARGET_PLUGIN_DIR` (A2-39) | present (the rule: n/a, Hub runtime) |
| PK-9 | the manifest file checks are the first static step | none | n/a: Hub runtime order |

### HP A2 (`hub-plugin-contract-v1.1.amendment-2-candidate7.md`)

`CALLS` is `plugin/calls.rs` `CALLS` (one `CallSpec` per call, with `failures`; generated into `schemas/plugin/plugin-calls.schema.json` `calls`). A failure is written `kind{code1, code2}`. `COMMON_FAILURES` is `plugin/calls.rs` `COMMON_FAILURES`.

| Clause | Item | Crate symbol | Status |
|---|---|---|---|
| A2-1 | `session_types.spawn`, `session_types.ensure_worktree_and_spawn`: `not_found`; `invalid_request{unknown_context_key, environment_not_allowed, env_too_long, prompt_too_long}`; `failed{target_disabled}`; `capability_denied`; `quota_exceeded{no_session_port}`; `backpressured{no_receipt_capacity}` | `CALLS` `session_types.spawn`, `session_types.ensure_worktree_and_spawn` failures; `CODES` of the same names | present |
| A2-2 | state refusal `failed{detail.code: wrong_state, detail.reason}` | `CODES` `wrong_state` (kind `failed`) | present |
| A2-2 | the closed reasons: `sessions.stop`, `.delete`, `.read_screen`, `.terminal_state`, `.read_cursor`: `starting`, `running`, `stopping`, `ended`, `lost`, `no_screen_model`; `sessions.safe_write`, `.write`: `session_ended`, `not_running`; `sessions.restart`: `not_ended`, `not_restartable`, `record_unavailable`, `environment_not_retained`, `not_ready`; `worktrees.delete`: `occupied` | `plugin/errors.rs` `WRONG_STATE_REASONS` (one row per call) | present |
| A2-3 | `sessions.write` result `state` ∈ `written`, `partial`, `cancelled`, `unknown`, `failed` with the counts | `plugin/call_types.rs` `WriteResult::{state, accepted_bytes, written_bytes, max_written_bytes, reason}`, `WriteState::{Written, Partial, Cancelled, Unknown, Failed}` | present |
| A2-3 | result `reason` ∈ `os_write_failed`, `cancelled`, `link_lost` | `plugin/errors.rs` `WRITE_RESULT_REASONS` | present |
| A2-3 | `failed{not_written, reason: refused \| unsupported \| mode_precondition_failed \| stale \| not_reported}` | `CODES` `not_written`; `plugin/errors.rs` `NOT_WRITTEN_REASONS`; `CALLS` `sessions.write` failures | present |
| A2-3 | `NotWritten(LaneFull)` → `backpressured`; `SessionEnded`/`Stopping` → `wrong_state{session_ended}` | `CALLS` `sessions.write` failures (`backpressured`, `failed{wrong_state}`) | present (the mapping: Hub runtime) |
| A2-4 | operation kinds `spawn`, `safe_write`, `http`, `ws`, `process`, `plugin_call` | `plugin/operations.rs` `OPERATION_KINDS` | present |
| A2-4 | states per kind (`spawn`: `starting`, `installed`, `failed`, `cancelled`, `unknown`; `http`: `queued`, `done`, `failed`, `cancelled`, `unknown`; `ws`: `queued`, `open`, `failed`, `cancelled`, `unknown`; `process`: `queued`, `running`, `done`, `failed`, `cancelled`, `unknown`; `plugin_call`: `queued`, `dispatched`, `done`, `failed`, `cancelled`, `timed_out`, `unknown`; `safe_write` as SW-2) | `plugin/operations.rs` `STATE_TABLES` | present |
| A2-4 | lost record answer `{kind, state: unknown, reason: hub_restarted, terminal: true}` | `plugin/operations.rs` `OperationFields::{kind, state, reason, terminal}`; `SAFE_WRITE_UNKNOWN_REASONS` (`hub_restarted`) | present |
| A2-4 | `spawn` `failed.reason` ∈ `start_failed`, `worker_lost`, `worktree_failed`, `session_type_invalid`, `target_disabled` | `plugin/operations.rs` `SPAWN_FAILED_REASONS` | present |
| A2-4 | `process` failure `error` (`failed(command_not_found)`, `failed(process_bridge_failed)`, `invalid_request(cwd_not_allowed)`, `timed_out`) | `plugin/operations.rs` `OperationFields::error`; `CODES` `command_not_found`, `process_bridge_failed`, `cwd_not_allowed` | present |
| A2-4 | `truncated` = `{stdout: bool, stderr: bool}` | `plugin/operations.rs` `Truncated::{stdout, stderr}`, `OperationFields::truncated` | present |
| A2-5 | the 13 handler kinds `mcp_tool`, `mcp_prompt`, `mcp_resource`, `ui_action`, `event`, `event_gap`, `entity_provider`, `timer`, `store_watch`, `operation`, `webhook`, `relay`, `fs_watch` | `plugin/frames.rs` `HandlerKind::{McpTool, McpPrompt, McpResource, UiAction, Event, EventGap, EntityProvider, Timer, StoreWatch, Operation, Webhook, Relay, FsWatch}`; `plugin/handlers.rs` `HANDLERS` | present |
| A2-5 | descriptors: `mcp_tool{name, description, input_schema}`, `mcp_prompt{name, description, arguments}`, `mcp_resource{uri_template, name, mime_type?}`, `ui_action{action_id}`, `entity_provider{kind}`, `webhook{path}`, the others `{}` | `HANDLERS` `descriptor` of each row (`plugin/handlers.rs` descriptor types, for example `WebhookDescriptor`) | present |
| A2-5 | payload and result of each kind (`mcp_resource` `{uri}` → `{contents}`; `ui_action` `{view_id?, action_id, target?, input, idempotency_key?, operation_id?}` → `{}` or `{operation_id}`; `event` → EV-15 record; `event_gap` → gap record; `entity_provider` `{kind, request, cursor?}` → `{records, cursor?}`; `timer{timer_id}`; `store_watch{watch_id, collection}`; `operation` → CA-16 record; `webhook{path, headers, body_b64}` → `{status, headers?, body_b64?}`; `relay{relay_id, envelope_b64}`; `fs_watch{watch_id, root}`) | `HANDLERS` `payload`, `result`: `McpResourcePayload`, `McpResourceResult`, `McpPromptResult`, `UiActionPayload`, `UiActionResult`, `EventRecord`, `EventGapRecord`, `EntityProviderPayload`, `EntityProviderResult`, `TimerPayload`, `StoreWatchPayload`, `OperationRecord`, `WebhookPayload`, `WebhookResult`, `RelayPayload`, `FsWatchPayload` | present (`records` fixed by HP A5) |
| A2-5 | `command`, `hook`, `session_action` removed | `HandlerKind` has no such variant | absent (deleted) |
| A2-6 | `hub/session.<state>{session, state, exit?, reason?}` for `created`, `starting`, `running`, `stopping`, `ended`, `lost`, `removed` | `plugin/events.rs` `EVENTS` rows `hub` `session.*` | present |
| A2-6 | `hub/session.attached`, `.detached{session, client_kind}`; `.activity{session, source, at}`; `.modes{session, modes}`; `.size{session, rows, cols}`; `.silent{session, since}` | `EVENTS` rows of the same names | present |
| A2-6 | `hub/route.ended`: subject `{route_id}`, payload `{reason}` | `EVENTS` row `hub` `route.ended` | present |
| A2-6 | `hub/plugin.loaded`, `.unloaded`, `.failed{plugin_key, version, reason?}` | `EVENTS` rows of the same names | present |
| A2-6 | `hub/config.changed{keys, restart_required}`; `hub/package.configuration_changed{plugin_key, names}` | `plugin/events.rs` `ConfigChangedEvent`, `PackageConfigurationChangedEvent`; `EVENTS` rows | present |
| A2-6 | `terminal/title{text}`, `cwd{cwd}`, `bell{}`, `notification{source, title?, body}`, `prompt_mark{mark, exit_code?}`, `modes{modes}`, `exit{code?, signal?, cause}` | `EVENTS` rows `terminal` `title`, `cwd`, `bell`, `notification`, `prompt_mark`, `modes`, `exit` | present |
| A2-6 | spawn operations never as `hub/` events | `EVENTS` has no spawn row | present |
| A2-7 | `entity.publish` `patch` is a JSON Merge Patch; `not_found`; `invalid_request{entity_schema}`; `quota_exceeded{entity_bytes}` | `plugin/call_types.rs` `EntityPublishArgs::patch`; `CALLS` `entity.publish` failures; `CODES` `entity_schema`, `entity_bytes` | present |
| A2-7 | `pending_gap`, `resync_scheduled` | `plugin/entities.rs` `PublishStatus::{Accepted, PendingGap, ResyncScheduled}` | present |
| A2-8 | `botster.json.null` sentinel | `plugin/calls.rs` `LUA_LOCAL_NAMES` (`json.null`) | present (its encoding: n/a, Lua host behavior, LB-9) |
| A2-9 | `store.batch` `ops: [{op: put \| delete, collection, id, doc?, expected_revision?}]` | `plugin/call_types.rs` `StoreBatchArgs::ops`, `StoreBatchOp::{op, collection, id, doc, expected_revision}`, `BatchOpKind` | present |
| A2-9 | `conflict{revision_conflict, op_index}`; `invalid_request{expected_revision_required, op_index}` | `CALLS` `store.batch` failures; `CODES` `revision_conflict`, `expected_revision_required` | present (`op_index` is a `detail` member) |
| A2-10 | `plugins.call` admission `conflict(idempotency_key_reuse)`, `conflict(call_depth_exceeded)` | `CALLS` `plugins.call` failures; `CODES` of the same names | present |
| A2-10 | terminal errors `failed(plugin_crashed)`, `timed_out`, `cancelled`, `failed(handler_error)`, `response_too_large` | `CODES` `plugin_crashed`, `handler_error`; `plugin/result.rs` `ErrorKind` (`timed_out`, `cancelled`, `response_too_large`) | present |
| A2-11 | `unavailable{instance_fenced}`; common `failed(wrong_context)` | `COMMON_FAILURES`; `CODES` `instance_fenced`, `wrong_context` | present |
| A2-12 | every row with `operation_id` lists `backpressured(no_receipt_capacity)`: `session_types.spawn`, `session_types.ensure_worktree_and_spawn`, `sessions.create_command`, `sessions.restart`, `sessions.safe_write`, `http.request`, `ws.open`, `process.run`, `plugins.call` | `CALLS` failures of those rows | present |
| A2-12 | every row that starts a session run lists `quota_exceeded(no_session_port)`: `session_types.spawn`, `session_types.ensure_worktree_and_spawn`, `sessions.create_command`, `sessions.restart` | `CALLS` failures of those rows | present |
| A2-13 | `failed{stop_timeout}` deleted | `CALLS` `sessions.stop` has no `stop_timeout`; `CODES` has none | absent (deleted) |
| A2-14 | `equals` array, `range{from, to}`; `invalid_request{index_value_type}`; `failed{incompatible_document, id}`; `store.watch{equals?}` | `plugin/call_types.rs` `StoreQueryArgs::{equals, range}`, `StoreRange::{from, to}`, `StoreWatchArgs::equals`; `CALLS` `store.query` failures | present |
| A2-15 | `after?` / `next_after?` on `spawn_targets.list`, `worktrees.list`, `session_types.list` | `plugin/call_types.rs` `AfterArgs::after`, `SpawnTargetsResult::next_after`, `WorktreesResult::next_after`, `SessionTypesListArgs::after`, `SessionTypesListResult::next_after` | present |
| A2-16 | `session_types.show` → `{definition}` with `context_keys`, `allowed_environment_overrides`, `interaction`, `presentation` | `plugin/call_types.rs` `SessionTypeDefinition::definition: SessionTypeRow` | present |
| A2-16 | `fs` `kind` ∈ `file`, `dir`, `symlink`, `other` | `plugin/call_types.rs` `FsKind::{File, Dir, Symlink, Other}` | present |
| A2-16 | worktree `status` ∈ `creating`, `ready`, `occupied`, `deleting`, `failed` | `plugin/rows.rs` `WorktreeStatus::{Creating, Ready, Occupied, Deleting, Failed}` | present |
| A2-16 | spawn target `kind` ∈ `git`, `directory` | `plugin/rows.rs` `TargetKind::{Git, Directory}` | present |
| A2-16 | `config.get` `diagnostics[]{name, code, message}` | `plugin/call_types.rs` `ConfigValues::diagnostics`, `ConfigDiagnostic::{name, code, message}` | present |
| A2-17 | `method` ∈ `GET`, `HEAD`, `POST`, `PUT`, `PATCH`, `DELETE`, `OPTIONS` | `plugin/call_types.rs` `HttpMethod::{Get, Head, Post, Put, Patch, Delete, Options}` | present |
| A2-17 | `headers` list of `[name, value]` pairs | `plugin/call_types.rs` `HttpRequestArgs::headers: Option<Vec<[String; 2]>>` | present |
| A2-17 | `invalid_request(net_userinfo, net_header_forbidden, net_scheme)` | `CALLS` `http.request`, `ws.open` failures; `CODES` of the same names | present |
| A2-18 | `relay.deliver` `accepted` / `rejected`; `relay` handler returns `{}` | `plugin/call_types.rs` `RelayDeliverResult::status`, `RelayStatus::{Accepted, Rejected}`; `HANDLERS` `relay` result | present |
| A2-19 | allowed `content_type`: `text/html`, `text/plain`, `text/css`, `application/javascript`, `application/json`, `image/png`, `image/jpeg`, `image/gif`, `image/webp`, `image/svg+xml`; else `invalid_request{content_type_not_allowed}` | `plugin/call_types.rs` `AssetContentType::{TextHtml, TextPlain, TextCss, ApplicationJavascript, ApplicationJson, ImagePng, ImageJpeg, ImageGif, ImageWebp, ImageSvgXml}`; `CODES` `content_type_not_allowed` | present |
| A2-20 | `log.write` result `{seq, dropped_before?}` | `plugin/call_types.rs` `LogWriteResult::{seq, dropped_before}` | present |
| A2-21 | `sessions.update` `metadata` replaces the namespace; `null` removes it | `plugin/call_types.rs` `SessionsUpdateArgs::metadata: Option<Option<OpenObject>>` | present |
| A2-22 | SW-2 `failed.reason` ∈ `core_refused`, `session_removed`; `unknown.reason` ∈ `hub_restarted`, `link_lost`, `cancel_after_text` | `plugin/operations.rs` `SAFE_WRITE_FAILED_REASONS`, `SAFE_WRITE_UNKNOWN_REASONS` | present |
| A2-23 | `hub/session.activity`, `.modes`, `.size`, `.silent` need `events_hub` and `session_observe`; `terminal/*` (not `output`) needs `events_terminal` | `EVENTS` `grant` of those rows (`events_hub + session_observe`, `events_terminal`) | present |
| A2-24 | no `events.describe` call | `CALLS` has no `events.describe` | absent (deleted) |
| A2-25 | `Hello.bindings[{interface, interface_version, provider?, provider_version?, interface_hash?}]`, `Hello.priorities[{name, rank}]` | `plugin/frames.rs` `Hello::{bindings, priorities}`, `HelloBinding`, `Priority::{name, rank}` | present |
| A2-25 | `Ready.fs_watches[{root, path, handler_id}]`, `Ready.session_actions[{action_id, handler_id}]`, `Ready.timers[{timer_id, delay_ms, handler_id?}]`, `Ready.watches[{collection, index?, equals?, handler_id?}]` | `plugin/frames.rs` `Ready::{fs_watches, session_actions, timers, watches}`, `FsWatchDecl`, `SessionActionMap`, `TimerDecl`, `WatchDecl` | present |
| A2-26 | withdrawn | none | n/a: withdrawn |
| A2-27 | `failed{api_unsupported \| feature_missing{names} \| ui_contract_unsupported}`; no kind `unsupported` | `CODES` `api_unsupported`, `feature_missing`, `ui_contract_unsupported`; `plugin/result.rs` `ErrorKind` (no `unsupported`) | present |
| A2-28 | EV-6 frames `snapshot_begin{}`, `snapshot_chunk{sessions}`, `snapshot_end{}`, `delta{op: upsert \| remove, session?, session_id}` | `plugin/entities.rs` `SessionFamilyFrame::{SnapshotBegin, SnapshotChunk, SnapshotEnd, Delta}`, `DeltaOp::{Upsert, Remove}` | present |
| A2-29 | the package operation to client verb table | none in HP; the client verbs are `hub/registry.rs` `PACKAGE`, `CATALOG` | n/a in HP (HC symbols) |
| A2-29 | `failed{net_dns \| net_connect \| net_tls \| net_denied}`; `quota_exceeded{package_too_large}`; `failed{unpack_refused, pointer}`; `quota_exceeded{catalog_too_large}`; `invalid_request{catalog_version_unsupported}`; `conflict{grants_mismatch}` | `CODES` `net_dns`, `net_connect`, `net_tls`, `net_denied`, `package_too_large`, `unpack_refused`, `catalog_too_large`, `catalog_version_unsupported`, `grants_mismatch` | present |
| A2-29 | `failed{wrong_state, reason: no_history}` (rollback); `not_found{ref}` (pin) | `hub/registry.rs` `PACKAGE` action `rollback` reasons (`wrong_state:no_history`), action `pin` (`not_found` by H-11) | present on the client side; `WRONG_STATE_REASONS` is per plugin call, and a package operation is not a plugin call: n/a in HP |
| A2-30 | observe reads succeed with a model; else `wrong_state{no_screen_model}`; `unavailable` only when not ready | `WRONG_STATE_REASONS` (`no_screen_model`); `CALLS` `sessions.terminal_state`, `sessions.read_cursor` failures (`unavailable`) | present |
| A2-31 | Appendix A is the list | `CALLS` (module doc of `plugin/calls.rs`) | present |
| A2-32 | admission answers carry `kind` (`spawn`; `safe_write` with `terminal: false`) | `plugin/call_types.rs` `SpawnAdmission::kind`, `SafeWriteAdmission` (`KindSafeWrite`, `terminal: false`) | present |
| A2-33 | `store.watch`, `fs.watch` commit text "commit: the watch is registered (not durable)" | `CALLS` `store.watch`, `fs.watch` `commit` | present |
| A2-34 | host-local names are LN-1 plus `json.array`, `json.empty_array`, `json.null`; none is in LN-2 | `plugin/calls.rs` `LUA_LOCAL_NAMES` (`json.array`, `json.empty_array`, `json.null`); `CALLS` has none of them | present |
| A2-35 | `operations_get_rate` | `plugin/limits.rs` `operations_get_rate` (`plugin_limits`, live) | present |
| A2-35 | `max_timer_delay_ms` withdrawn | not in `plugin/limits.rs` | absent (withdrawn) |
| A2-36 | `manifest_malformed` pointer field is `pointer` | none | n/a: a `detail` member; `detail` is an open object |
| A2-37 | the request bound is `frame_bytes` | `plugin/limits.rs` `frame_bytes` | present |
| A2-38 | `hub.identity` `unavailable` before a `hub_id` exists | `CALLS` `hub.identity` failures (`unavailable{not_ready}`) | present |
| A2-39 | `target_plugin_dir` default `.botster/plugins` (contract) | `plugin/manifest.rs` `DEFAULT_TARGET_PLUGIN_DIR`; `plugin/limits.rs` `target_plugin_dir` (`plugins`, restart) | present |
| A2-40 | `botster-agent.json`: the client `session_type` create fields except `source`, plus `script_file?` | `plugin/files.rs` `AgentDefinition::{schema_ref, id, label, role, interaction, lifecycle, command, args, traits, allowed_environment_overrides, context_keys, port_env, notifications, script, agent_scopes, script_file}` (the same fields as `hub/actions.rs` `SessionTypeDefinition` minus `source`) | present |
| A2-40 | `botster-accessory.json`: the same fields, `role` fixed to `accessory`; `port_forward` → `allocate_port` | `plugin/files.rs` `Accessory::{role: AccessoryRole, port_forward}`, `Accessory::allocate_port` | present |
| A2-41 | `Authorization: Bearer`, `X-Botster-Signature-256: sha256=<hex>`; `webhook_auth: bearer \| hmac`, `webhook_secret` in the package configuration | none | n/a: HTTP headers checked by the Hub; `configuration` is an open object (`hub/rows.rs` `PackageRow::configuration`, `hub/actions.rs` `PackagePatch::configuration`) |
| A2-41 | webhook URL `http://<net_listen>/plugins/<plugin_key>/<path>`; `net_listen` recorded and stable | `plugin/limits.rs` `net_listen` (`net`, restart) | present (the URL and the port record: Hub runtime) |
| A2-41, A2-42 | listener `errored{port_unavailable}` | none | n/a: HP-internal; no HC field in v1 (steward ruling, HC A7-2) |
| A2-42 | static origin `http://127.0.0.1:<port>`, stable per package key; `package` row `static_origin?` | `hub/rows.rs` `PackageRow::static_origin` | **fixed** with HC A7-2 (the HC A7 tip commit) |
| A2-43 | artifact path `botster-plugin-contract/schemas/v<api>/<artifact>`; `$id: urn:botster:schema:plugin:v<api>:<artifact>` | `plugin/artifacts.rs` `artifact_id`; `plugin/manifest.rs` `BOTSTER_PLUGIN_API` | present (the publish path: n/a, release layout) |
| A2-43 | optional top-level `$schema` accepted and ignored | `plugin/manifest.rs` `Manifest::schema_ref` (`$schema`); `plugin/files.rs` `AgentDefinition::schema_ref`, `Accessory::schema_ref` | present |
| A2-44 | catalog `catalog_version` = 1; entry `id` matches `[a-z0-9-]+`; `catalog.schema.json`; `failed{catalog_invalid}`; `catalog_bytes_max` | `plugin/files.rs` `Catalog::catalog_version` (range 1..1), `CatalogEntry::id` (pattern); `schemas/plugin/catalog.schema.json`; `CODES` `catalog_invalid`; `plugin/limits.rs` `catalog_bytes_max` | present (media type and `https`: n/a, Hub fetch) |
| A2-45 | `session_type_dir` is the package root | none | n/a: a member of the client context file (see HC A2 MC-1c) |
| A2-46 | static route `root` may be `package:<relative dir>` | `plugin/manifest.rs` `StaticRoute::root` | present (containment: Hub runtime) |
| A2-47 | `entrypoints.service.lua` relative to the package root; `require` rule | `plugin/globals.rs` `OTHER_GLOBALS` (`require`) | present (the file rule: n/a, Lua host, LB-17b) |
| A2-48 | `local` source of a discovered instance is its package root | `plugin/files.rs` `PackageSource::Local{path}` | present (the value: Hub runtime) |
| A2-49 | interface document at `interfaces/<name>@<MAJOR.MINOR.PATCH>.json` | `plugin/manifest.rs` `interface_document_path` | present |
| A2-50 | no exported lock file | none | n/a: `plugin/files.rs` `LockRecord` is Hub state |
| A2-51 (a) | `enable{fs_roots?}`; an unbound root keeps the package `staged` | `hub/actions.rs` `EnableArgs::fs_roots` | present |
| A2-51 (b) | `webhook_secret`, `webhook_auth` are package configuration fields | none | n/a: see A2-41 |
| A2-51 (c) | `[network] trusted_hosts` entries `{path, sha256}` | `plugin/limits.rs` `TRUSTED_HOSTS_KEY` (`network`, `trusted_hosts`, restart) | present |
| A2-51 (d) | `invoke(spawn_target, id, rescan_packages)` → `succeeded{found, removed}` | `hub/actions.rs` `RescanResult`; `hub/registry.rs` `SPAWN_TARGET` action `rescan_packages` | present |
| A2-52 | every parameter of the section 3 table has its `hub.toml` table and mode: `[plugins]` `target_plugin_dir`, `process_path`; `[network]` `trusted_hosts`; `[plugin_limits]` `frame_bytes`, `lanes`, `startup`, `shutdown`, `orphan_grace`, `restart_window`, `cpu_seconds`, `open_files`, `processes`, `address_space_bytes`, `stderr_bytes`, `instruction_budget` (restart), `invoke_queue`, `dispatch_deadline`, `long_deadline`, `page_items`, `max_result_bytes`, `max_watches`, `max_timers`, `operations_get_rate`, `operation_receipt_window`, `operation_receipts` (live); `[logging]` `log_record_bytes`, `log_ring_records`, `log_ring_bytes`, `log_rate`, `log_burst`; `[events]` `event_key_bytes`, `event_queue`, `event_queue_high`, `event_queue_normal`, `event_queue_low`, `event_starvation_bound`, `ingress_queue`, `max_event_bytes`, `loss_ledger_entries`, `terminal_event_bytes`, `output_tap_chunk_bytes`, `session_family_inflight`; `[sessions]` `session_port_range`, `probe_text` (restart), `max_write_bytes`, `max_raw_write_bytes`, `write_pending_bytes`, `echo_deadline_ms`, `cleanup_window_ms`, `write_client_quiet_ms`, `write_max_wait_ms`, `max_notification_patterns`, `max_argv_bytes`, `max_env_bytes` (live); `[store]` `max_document_bytes`, `max_plugin_bytes`, `max_plugin_documents`, `batch_ops`; `[net]` `net_listen`, `assets_listen` (restart), `net_timeout_max_ms`, `webhook_timeout_ms`, `net_body_bytes`, `ws_send_queue_bytes`, `net_concurrent_requests`, `net_open_sockets`, `net_bytes_in_flight`, `net_rate` (live); `[web]` `max_relay_envelope_bytes`, `relay_queue`, `relay_attempts_max`, `relay_attempt_ttl_ms`, `ice_servers_max`; `[assets]` `asset_bytes_max`, `asset_store_bytes`, `asset_count_max`, `asset_token_ttl_ms`; `[notices]` `notice_ttl_max_ms`, `notice_queue`; `[process]` `process_timeout_max_ms`, `process_tail_bytes`, `process_concurrent`; `[fs]` `fs_read_bytes`, `max_fs_watches`; `[plugin_calls]` `plugin_call_depth`, `plugin_call_idempotency_window`, `plugin_call_timeout_max_ms`; `[packages]` `package_bytes_max`, `lock_history_max`, `catalog_bytes_max`; `[mcp_proxy]` `mcp_proxy_call_deadline_ms`, `mcp_proxy_concurrent`, `mcp_servers_max`, `mcp_proxy_init_deadline_ms`, `mcp_proxy_list_pages`, `mcp_proxy_tools_max`, `mcp_proxy_message_bytes` (all live unless marked) | `plugin/limits.rs` `LIMIT_SPECS`, each `LimitSpec::toml` = `(table, KeyMode)`; `TRUSTED_HOSTS_KEY` | present: all 97 keys checked one by one against the table and mode |
| A2-52 | live change applies to new admissions only; `Hello.limits` is the load-time value | `plugin/frames.rs` `Hello::limits: PluginLimits` | present (the rule: Hub runtime) |
| A2-53 | `timers.arm{timer_id, delay_ms, handler_id?}`, `store.watch{collection, index?, equals?, handler_id?}`; `fs.watch` keeps `handler_id` mandatory | `plugin/call_types.rs` `TimersArmArgs::handler_id: Option`, `StoreWatchArgs::handler_id: Option`, `FsWatchArgs::handler_id: String` | present |
| A2-53 | `invalid_request{unknown_handler}`, `invalid_request{handler_id_required}` | `CALLS` `timers.arm`, `store.watch` failures; `CODES` `unknown_handler`, `handler_id_required` | present |

### HP A3 (`hub-plugin-contract-v1.1.amendment-3-candidate1.md`)

| Clause | Item | Crate symbol | Status |
|---|---|---|---|
| A3-1 | the scripted services of NT-10, PM-14, MP-10 are the real Hub's injected edges; the plugin-side FakeHub scripts `HostResult`s only | none | n/a: test architecture |
| A3-2 | `conf::wp_3_served_code_is_trusted_not_claimed_otherwise` withdrawn | none | n/a: a test id, no crate symbol |

### HP A4 (`hub-plugin-contract-v1.1.amendment-4-candidate2.md`) — fixed in 6b91b63

| Clause | Item | Crate symbol | Status |
|---|---|---|---|
| A4-1 | `write_pending_bytes` bounds the Hub-wide total | `plugin/limits.rs` `write_pending_bytes` (`sessions`, live) | present (the count: Hub runtime) |
| A4-1 | `backpressured{detail.code: write_pending_full}` on `sessions.safe_write`, effect `none` | `CODES` `write_pending_full` (kind `backpressured`, "A4-1, SW-5"); `CALLS` `sessions.safe_write` failures `backpressured{no_receipt_capacity, write_pending_full}` | **fixed** |
| A4-1 | on the client surface, `busy` | `hub/registry.rs` `SESSION` action `safe_write` reasons (`busy`) | present |
| A4-1 | `max_write_bytes` per-write bound, `invalid_request{text_too_long}` first | `plugin/limits.rs` `max_write_bytes`; `CODES` `text_too_long` | present |

### HP A5 (`hub-plugin-contract-v1.1.amendment-5-candidate3.md`) — fixed in 6b91b63

| Clause | Item | Crate symbol | Status |
|---|---|---|---|
| A5-1 | one entity record is `{id, entity}` | `plugin/entities.rs` `EntityRecord::{id, entity: OpenObject}` | **fixed** |
| A5-1 | `entity_provider` result `{records: [{id, entity}], cursor?}` | `plugin/entities.rs` `EntityProviderResult::records: Vec<EntityRecord>` | **fixed** |
| A5-1 | `entity.publish{entity_type, type, id, entity \| patch}`; `entity` is an object | `plugin/call_types.rs` `EntityPublishArgs::{entity_type, change, id, entity: Option<OpenObject>, patch}` | **fixed** (`entity` was an untyped value) |
| A5-1 | a top-level `id` in the declared schema fails the load; a top-level `id` in `entity` is `invalid_request{entity_schema}` | `CODES` `entity_schema` | present (the checks: Hub runtime) |
| A5-1 | client row = `id` + the members of `entity`; `id` is a core field; `schema` is the declaration unchanged | `hub/describe.rs` `KindDescriptor::{core_fields, schema}` | present (the projection: Hub runtime) |

### HP A6 (`hub-plugin-contract-v1.1.amendment-6-candidate2.md`)

| Clause | Item | Crate symbol | Status |
|---|---|---|---|
| A6-1 | `accepted`, `pending_gap`, `resync_scheduled` decided at one observation point | `plugin/entities.rs` `PublishStatus::{Accepted, PendingGap, ResyncScheduled}` (unchanged) | n/a: meaning only; the type is unchanged |

### HP A7 (`hub-plugin-contract-v1.1.amendment-7-candidate3.md`)

| Clause | Item | Crate symbol | Status |
|---|---|---|---|
| A7-1 | the history holds previous locks only; each entry keeps its admitted limit; rollback drops the replaced lock | none | n/a: Hub state (A2-50: no exported lock file) |
| A7-1 | `lock_history_max` | `plugin/limits.rs` `lock_history_max` (`packages`, live) | present |
| A7-1 | empty history: `failed{wrong_state, reason: no_history}`, effect `none` | `hub/registry.rs` `PACKAGE` action `rollback` reasons (`wrong_state:no_history`) | present |

### HP A8 (`hub-plugin-contract-v1.1.amendment-8-candidate2.md`) — fixed in 6b91b63 and the HC A7 tip commit

| Clause | Item | Crate symbol | Status |
|---|---|---|---|
| A8-1 | the Hub checks a `plugins.call` result against the bound document; `invalid_request{interface_unknown_required, interface, tool, path}`, effect `unknown` | `CODES` `interface_unknown_required` | present (the walk: Hub runtime; `interface`, `tool`, `path` are `detail` members) |
| A8-2 | a MINOR or PATCH only adds; `invalid_request{interface_not_additive, interface, version, against}` | `CODES` `interface_not_additive` (kind `invalid_request`, "A8-2, PD-9") | **fixed** (6b91b63) |
| A8-3 | `preview_upgrade` `breaking: [{consumer, interface, range, interface_version}]` | `hub/actions.rs` `UpgradePreview::breaking`, `BreakingEntry` | **fixed** with HC A7-1 (the HC A7 tip commit) |
| A8-3 | `preview_package_upgrades` `breaking` next to `ambiguous` | `hub/actions.rs` `PreviewPackageUpgradesResult::{ambiguous, breaking}` | **fixed** with HC A7-1 (the HC A7 tip commit) |
| A8-3 | `accept_breaking` on `upgrade`, `upgrade_packages` | `hub/actions.rs` `PackageUpgradeArgs::accept_breaking`, `UpgradePackagesArgs::accept_breaking`, `ConsumerInterface::{consumer, interface}` | present |
| A8-3 | `conflict{detail.code: breaking_not_accepted, consumers}`, effect `none` | `CODES` `breaking_not_accepted` (kind `conflict`, "A8-3, PD-6"); `hub/registry.rs` `PACKAGE` action `upgrade`, `HUB` action `upgrade_packages` reasons | **fixed** in HP (6b91b63); present in HC |

## Lua binding (LB)

The Lua binding states the Lua tables of the host. No crate models a Lua table. The crate holds the names (`plugin/globals.rs`, `plugin/calls.rs` `LUA_LOCAL_NAMES`, `LUA_REGISTRATION_NAMES`, `LUA_LOG_SUGAR`; generated as `schemas/plugin/lua-host-globals.json`) and the wire shapes that a Lua table maps to.

### LB r9 (`hub-plugin-lua-binding-v1.1-r9.md`; differences from final6's r6)

| Clause | Item | Crate symbol | Status |
|---|---|---|---|
| LB-6 | `hub/session_family` frames `snapshot_begin{}`, `snapshot_chunk{sessions}`, `snapshot_end{}`, `delta{op, session?, session_id}`, no `generation` | `plugin/entities.rs` `SessionFamilyFrame`, `DeltaOp` (HP A2-28) | present |
| LB-9 | `botster.json.null`; host-local names = LN-1 + `json.array`, `json.empty_array`, `json.null` | `plugin/calls.rs` `LUA_LOCAL_NAMES` | present (the sentinel behavior: n/a, Lua host) |
| LB-10 | the 13 handler-kind strings; `command`, `hook`, `session_action` removed; `tool`/`prompt` are `mcp_tool`/`mcp_prompt` | `plugin/frames.rs` `HandlerKind` (HP A2-5) | present |
| LB-10 | descriptors and payloads per kind | `plugin/handlers.rs` `HANDLERS` (HP A2-5) | present |
| LB-10 | `timers.arm`, `store.watch` `handler_id?`; `fs.watch` `handler_id` mandatory; `Ready.timers`, `Ready.watches` `handler_id?` | `plugin/call_types.rs` `TimersArmArgs`, `StoreWatchArgs`, `FsWatchArgs`; `plugin/frames.rs` `TimerDecl`, `WatchDecl` (HP A2-53) | present |
| LB-10 | one-timer / one-store_watch handler cap withdrawn | none | absent (withdrawn; HP A2-26) |
| LB-11 | `http.request` `method` set, `headers` as `[name, value]` pairs; `net_userinfo`, `net_header_forbidden`, `net_scheme` | `plugin/call_types.rs` `HttpMethod`, `HttpRequestArgs::headers`; `CODES` (HP A2-17) | present |
| LB-12 | state refusal `failed{wrong_state, reason}` with the closed reasons per call | `plugin/errors.rs` `WRONG_STATE_REASONS` (HP A2-2) | present |
| LB-12 | observation grants (`events_hub` + `session_observe`; `events_terminal`) | `plugin/events.rs` `EVENTS` `grant` (HP A2-23) | present |
| LB-13 | operation record fields; kind and state tables non-exhaustive; `reason` on `hub_restarted` | `plugin/operations.rs` `OperationFields`, `STATE_TABLES` (HP A2-4) | present |
| LB-15 | `web.set_origin{origin = botster.json.null}` clears; `{}` or `nil` is `invalid_request` | `plugin/call_types.rs` `SetOriginArgs::origin: WebOrigin` (required), `WebOrigin::{Clear, Url, Served}` | present (the Lua `nil` mapping: n/a, Lua host) |
| LB-15, LB-16 | `secrets.put{name, value = botster.json.null}` deletes | `plugin/call_types.rs` `SecretsPutArgs::value: SecretValue`, `SecretValue::{Delete, Set}` | present |
| LB-16 | `sessions.write` result `ok = true` with `state`, counts, `reason`; error mapping | `plugin/call_types.rs` `WriteResult`, `WriteState`; `plugin/errors.rs` `WRITE_RESULT_REASONS`, `NOT_WRITTEN_REASONS` (HP A2-3) | present |
| LB-16b | `sessions.update` `metadata` replaces the namespace; `null` removes | `plugin/call_types.rs` `SessionsUpdateArgs::metadata` (HP A2-21) | present |
| LB-17 | optional top-level `$schema` accepted | `plugin/manifest.rs` `Manifest::schema_ref` (HP A2-43) | present |
| LB-17b | entry relative to the package root; `require` rule | `plugin/globals.rs` `OTHER_GLOBALS` (`require`) | present (the file rule: n/a, Lua host; HP A2-47) |
| LB-18 | an unmet optional binding has `provider = botster.json.null` | `plugin/frames.rs` `HelloBinding::provider: Option<String>` (schema `string_or_null`; `None` is written as `null`) | present |
| LB-19 | `botster-agent.json`, `botster-accessory.json` fields | `plugin/files.rs` `AgentDefinition`, `Accessory` (HP A2-40) | present |
| LB-20 | `store.batch` ops; `revision_conflict`, `expected_revision_required` with `op_index` | `plugin/call_types.rs` `StoreBatchArgs`, `StoreBatchOp`; `CODES` (HP A2-9) | present |
| LB-21 | `hub/route.ended` is client-only; no grant admits it | `plugin/events.rs` `EVENTS` row `hub` `route.ended` (`Aud::Clients`, grant `-`) | present |
| LB-21 | `hub/config.changed{keys, restart_required}`, `hub/package.configuration_changed{plugin_key, names}` need `events_hub` | `plugin/events.rs` `ConfigChangedEvent`, `PackageConfigurationChangedEvent`; `EVENTS` grant `events_hub` | present |
| LB-21 | `operations_get_rate`; no `max_timer_delay_ms`; `rescan_packages` has no Lua call | `plugin/limits.rs` `operations_get_rate`; `CALLS` has no `rescan_packages` | present |

### LB Amendment 1 (`hub-plugin-lua-binding-v1.1-r9-amendment-1-candidate1.md`)

| Clause | Item | Crate symbol | Status |
|---|---|---|---|
| LBA1-1 | `json.array{ items }` returns the array; `json.empty_array` is a read-only value; `json.null` the sentinel | `plugin/calls.rs` `LUA_LOCAL_NAMES` (`json.array`, `json.empty_array`, `json.null`) | present (constructor vs envelope: n/a, Lua host) |
| LBA1-2 | `base64.*`, `json.encode`, `json.decode` return bare values in the envelope | `LUA_LOCAL_NAMES` (`base64.encode`, `base64.decode`, `json.encode`, `json.decode`) | present (the values: n/a, Lua host) |
| LBA1-3 | handler descriptor fields are flat in the Lua table; `entity_provider` `kind` is written `entity_kind` in Lua and sent as `kind` | `plugin/frames.rs` `HandlerDecl::{id, kind, descriptor}`; `plugin/handlers.rs` `EntityProviderDescriptor::kind` | present (the Lua spelling: n/a, Lua host) |

### LB Amendment 2 (`hub-plugin-lua-binding-v1.1-r9-amendment-2-candidate1.md`)

| Clause | Item | Crate symbol | Status |
|---|---|---|---|
| LBA2-1 | Lua `tools` entry `{name, description, input_schema, call, id?, callable_by?}` → wire `{name, description, input_schema, handler, callable_by?}` | `plugin/frames.rs` `ToolDecl::{name, description, input_schema, handler, callable_by}` | present (the Lua table: n/a, Lua host) |
| LBA2-1 | Lua `prompts` entry `{name, description, arguments, call, id?}` → wire `{name, description, arguments, handler}` | `plugin/frames.rs` `PromptDecl::{name, description, arguments, handler}` | present |
| LBA2-1 | Lua `resources` entry `{uri_template, name, mime_type?, call, id?}` → wire `{uri_template, name, mime_type?, handler}` | `plugin/frames.rs` `ResourceDecl::{uri_template, name, mime_type, handler}` | present |
| LBA2-1 | a Lua `handler` field or an unknown field fails the load | none | n/a: Lua host check |

## UI contract (UI)

`ui/` is `crates/botster-ui-contract/src/`. Admission codes are `ui/admission.rs` `AdmissionCode`; the feature names that the Hub knows are `ui/admission.rs` `default_features` (`AdmissionContext::features`).

### UI 1.1 (`ui-contract-v1.1-amendment-frozen-20260930.md`; frozen at final6, audited at the reviewer's request)

| Clause | Item | Crate symbol | Status |
|---|---|---|---|
| version | `ui_contract_view` `"1.1"`; feature `ui_1_1`; a 1.1 view needs `requires: ["ui_1_1"]` | `ui/doc.rs` `ViewDocument::{ui_contract_view, requires}`; `ui/admission.rs` `default_features` (`ui_1_1`), the `MissingRequires` check | present |
| A1, UI-SL-3 | `Activate` = `{select}` \| `{invoke}` \| `{open}` \| `{navigate}` \| `{url}` | `ui/doc.rs` `Activate::{Select, Invoke, Open, Navigate, Url}` | present |
| UI-SL-5 | `navigate{view, select?: {slot: Operand}}`; unknown view fails admission `unknown_view` | `ui/doc.rs` `Navigate::{view, select}`; `AdmissionCode::UnknownView` | present |
| UI-SL-5 | pending preset until the baseline; unnamed slots keep state | none | n/a: client behavior |
| UI-SL-6 | navigation target `select?: {slot: row key}` | `ui/doc.rs` `NavigationTarget::{view, select}`; `hub/rows.rs` `NavTarget::select` | present |
| UI-SL-6 | Web URL form `<base>/<package>/<view>?<slot>=<row key>` (UI-SL-9 gives the full form); not rewritten | none | n/a: client URL behavior (HC A2 G-17 states the path) |
| UI-FD-1 | `Field.display: "link"` | `ui/doc.rs` `Display::Link`, `Field::display` | present |
| UI-FD-1 | http/https only, no userinfo; ASCII host; open with no opener and no referrer | none | n/a: renderer behavior; `realize` emits `href` only when valid (UI-RZ-2e row) |
| UI-SL-7 | `{url: Operand}` activation; not on an action button | `ui/doc.rs` `Activate::Url(Operand)` | present (the button rule: admission/realize behavior) |
| UI-RZ-2e | realized props `activate` (navigate, url) and `href` on `text` | `ui/node.rs` `CLAUSE_PROPS` (`Text`: `activate`, `href`; `ListItem`: `activate`) | present |
| UI-CF-5 | capability `external_open`; badge suffix `(n)` / `●`; no home area | `ui/realize.rs` `FEATURE_EXTERNAL_OPEN`; `ui/input.rs` `Capabilities::features`; `ui/navigation.rs` `RealizedBadge::suffix`, `HomeArea` | present |
| UI-BD-6 | `owner` of the Hub `session` kind, only when `projection_allowlist` lists it | `hub/rows.rs` `SessionRow::owner`; `hub/describe.rs` `KindDescriptor::projection_allowlist`; `AdmissionCode::PathNotAllowed` | present |
| UI-PL-1 | `ViewDocument.placement? = {area: home, size: small \| medium \| wide \| full, order}` | `ui/doc.rs` `ViewDocument::placement`, `Placement::{area, size, order}`; `AdmissionCode::BadPlacement` | present |
| UI-PL-1 | size fallback, home order | `ui/navigation.rs` `HomeArea::sizes`, `HomeEntry::{view, size}` | present |
| UI-NB-1 | `Badge = {source: {kind, id, field}, display: dot \| count, tone?}` on a navigation entry | `ui/doc.rs` `Badge::{source, display, tone}`, `BadgeSource::{kind, id, field}`; `hub/rows.rs` `NavigationRow::badge` | present |
| UI-NB-1 | count / dot / hidden rules | `ui/navigation.rs` `RealizedBadge::{display, value, tone, suffix}` | present |
| UI-AD-3 | admission of `placement`, `navigate.view`, `badge.source`, `display: link` only on text | `AdmissionCode::{BadPlacement, UnknownView, KindNotAdmitted, PathNotAllowed, LinkNotText}` | present |
| UI-SA-1 to UI-SA-4 | `label_pointer`, `status_pointer`, `url_pointer`, `applies_to.has_port` on a contributed session action | `plugin/manifest.rs` `SessionActionUi::{label_pointer, status_pointer, url_pointer, applies_to}`, `AppliesTo::has_port` | present (the rendering: client behavior; `ui/realize.rs` places the status beside the button) |
| A5 | no `dialog` part; `dialog` is a realized node | `ui/doc.rs` `Part` (no dialog variant); `ui/vocabulary.rs` `UiNodeKind::Dialog` | present |
| A7 | `"1.1"` is the current MINOR | see the version row | present |

### UI 1.2 (`ui-contract-v1.2-embed-successor-candidate2.md`; frozen at final6, audited at the reviewer's request)

| Clause | Item | Crate symbol | Status |
|---|---|---|---|
| version | feature `ui_embed`; a view with `embed` needs `requires: ["ui_embed"]` | `ui/admission.rs` `default_features` (`ui_embed`), the `MissingRequires` check | present |
| E1 | part `embed{type, id, title, page{asset}, channel?, empty, error}` | `ui/doc.rs` `Part::Embed`, `EmbedPart::{id, title, page, channel, empty, error}`, `EmbedPage::asset` | present |
| UI-EM-1 | asset reference `{asset_id}` or `{name}`; no `src`, `srcdoc` or inline HTML | `ui/doc.rs` `AssetRef::{ById{asset_id}, ByName{name}}`; `EmbedPart` has no `src` field (`deny_unknown_fields`: `AdmissionCode::UnknownProperty`) | present |
| UI-EM-2 | any number of embeds; one at a time in a narrow layout | none | n/a: renderer behavior |
| UI-RZ-2f | UiNode `embed` with `title`, `asset_id`, `channel`, `state` | `ui/vocabulary.rs` `UiNodeKind::Embed`; `ui/node.rs` `CLAUSE_PROPS` (`Embed`: `title`, `asset_id`, `name`, `channel`, `state`) | present |
| UI-AD-4 | embed admission; `embed_source_not_own` | `AdmissionCode::EmbedSourceNotOwn` | present |
| E2 | `assets.publish{name, content_type, bytes_b64}` → `{asset_id}`; `assets_listen`; `asset_token_ttl_ms` | `plugin/call_types.rs` `AssetsPublishArgs`; `CALLS` `assets.publish`; `plugin/limits.rs` `assets_listen`, `asset_token_ttl_ms` | present |
| UI-EM-3, UI-EM-4, UI-EM-11 | sandbox `allow-scripts` only; no app credentials; origin checks | none | n/a: renderer behavior |
| UI-EM-7 | `invoke(package, key, open_asset, {asset_id \| name})` → `{url, expires_at}`, effect `immediate` | `hub/actions.rs` `OpenAssetArgs::{ById, ByName}`, `OpenAssetResult::{url, expires_at}`; `hub/registry.rs` `PACKAGE` action `open_asset` | present |
| UI-EM-8 | a token URL is hidden until activation and never logged | none | n/a: renderer behavior |
| E3, UI-EM-5 | channel messages `{botster: 1, type, payload}`; `from_page[{type, invoke? \| select?, args?}]`, `to_page[{type, source, fields, on: change}]` | `ui/doc.rs` `Channel::{from_page, to_page}`, `FromPage::{type_, invoke, select, args}`, `ToPage::{type_, source, fields, on}` | present (the `postMessage` envelope: n/a, renderer) |
| UI-EM-9 | label outside the frame | none | n/a: renderer behavior |
| UI-EM-10 | `realize` never emits `iframe`; `iframe` deprecated | `ui/vocabulary.rs` `UiNodeKind::Iframe` (kept for the vocabulary, never emitted by `ui/realize.rs`) | present |
| UI-EM-12 | `to_page.source` only package-owned kinds | `AdmissionCode::EmbedSourceNotOwn` | present |
| UI-EM-13 | channel retirement | none | n/a: renderer behavior |
| UI-EM-6 | fallback link; capability `embed` | `ui/realize.rs` `FEATURE_EMBED` | present |
| UI-SA-5 | action `warning?`, realized as a `text` node, tone `warning`, role segment `warning` | `ui/doc.rs` `Action::warning`; `plugin/manifest.rs` `SessionActionUi::warning`; `ui/ids.rs` `ROLE_WORDS` (`warning`) | present |
| UI-AC-6 | action result `navigate{view, select?}` | `ui/doc.rs` `ActionOk::navigate: Option<NavigationTarget>` | present |

### UI 1.3 (`ui-contract-v1.3-completeness-candidate9.md`)

| Clause | Item | Crate symbol | Status |
|---|---|---|---|
| version | feature `ui_1_3` | `ui/admission.rs` `default_features` (`ui_1_3`) | present |
| UI-AC-1b | success `ok` value: `message?`, `navigate?`; other members ignored | `ui/doc.rs` `ActionOk::{message, navigate}` | present |
| UI-AC-1b | failure fields from `Error.field_errors`, `Error.message` | `ui/doc.rs` `ActionError::{field_errors, message}`, `FieldError`; `hub/error.rs` `Error::{field_errors, message}` | present |
| UI-AC-2b | `accepted` effect: pending until `terminal`; "outcome unknown" | none | n/a: renderer behavior (the inputs are `hub/rows.rs` `OperationRow::{state, terminal}`) |
| UI-AC-7 | failure display; `stale_descriptor` re-read keeps the draft; no auto-retry | none | n/a: renderer behavior (the realized node is UI-NV-3) |
| UI-SL-8 | row `invoke` follows the button rules; absent `default_from` gives no default | none | n/a: renderer behavior |
| UI-RZ-4b | `order` compare function (absent first, type order, doubles, UTF-8 bytes, `desc`, row-key tie-break) | `ui/value.rs` `compare` | present |
| UI-FD-2 | `Field.map` key = RFC 8785 text; key domain; raw display text | `ui/value.rs` `map_key`, `jcs`, `es_number`, `raw_text`, `display_text`; `ui/doc.rs` `Field::map`, `MapEntry::{label, tone}` | present |
| UI-FD-2 | admission `number_out_of_range`, `non_canonical_map_key` | `AdmissionCode::{NumberOutOfRange, NonCanonicalMapKey}` | present |
| UI-PT-5 | `form.submit`, `form.cancel` = `{label?}` | `ui/doc.rs` `FormPart::{submit, cancel}`, `ButtonSpec::label` | present |
| UI-PT-5 | `actions.overflow` ∈ `auto`, `never` | `ui/doc.rs` `ActionsPart::overflow`, `ToolbarPart::overflow`, `Overflow::{Auto, Never}` | present |
| UI-PT-5 | `list.item = {fields, actions?}` | `ui/doc.rs` `ListPart::item`, `Item::{fields, actions}` | present |
| UI-PT-5 | `detail.source = {kind, id: Operand}` or `{sel}` | `ui/doc.rs` `DetailPart::source`, `DetailSource::{ByKind{kind, id}, BySel{sel}}` | present |
| UI-PT-5 | `alternatives` bool, default `false` | `ui/doc.rs` `StackPart::alternatives`, `InlinePart::alternatives`, `PanelPart::alternatives`, `SectionPart::alternatives` | present |
| UI-PT-5 | any other type is `bad_property_type` | `AdmissionCode::BadPropertyType` | present |
| UI-AC-8 | `Action.disabled_reason?` | `ui/doc.rs` `Action::disabled_reason` | present |
| UI-EM-14 | page message payloads (`rows`, `row_key`, argument filtering, `args` override, no result) | none | n/a: renderer channel behavior; the declarations are `ui/doc.rs` `FromPage::args`, `ToPage::fields` |
| UI-EM-15 | any `open_asset` error shows `error` with reload | none | n/a: renderer behavior |
| UI-SL-9 | Web URL `<web origin>/hubs/<hub_id>/v/<package key>/<view>?<slot>=<row key>`; `route_path`; `/signal`, `/ice` reserved | `hub/actions.rs` `ResolveRouteArgs::route_path` (HC A2 G-17) | present (the URL syntax: client behavior) |
| UI-RZ-3b | the vectors live in `vectors/` of `botster-ui-contract` | `crates/botster-ui-contract/vectors/` | present |

### UI 1.4 (`ui-contract-v1.4-realize-completeness-candidate3.md`)

| Clause | Item | Crate symbol | Status |
|---|---|---|---|
| version | feature `ui_1_4` | `ui/admission.rs` `default_features` (`ui_1_4`) | present |
| UI-NV-1 | `state` (`loading`, `empty`, `error`) and `detail` on `list`, `table`, `panel` | `ui/node.rs` `CLAUSE_PROPS` (`List`, `Table`, `Panel`: `state`, `detail`) | present |
| UI-NV-1 | `collapsed` on `form_section` | `CLAUSE_PROPS` (`FormSection`: `collapsed`) | present |
| UI-NV-1 | `read_only` on `text_input`, `textarea`, `checkbox`, `select` | `CLAUSE_PROPS` (`TextInput`, `Textarea`, `Checkbox`, `Select`: `read_only`) | present |
| UI-NV-1 | `masked`, `input: text \| number`, `secret_set` on `text_input` | `CLAUSE_PROPS` (`TextInput`: `masked`, `input`, `secret_set`) | present |
| UI-NV-1 | `indent` on `stack` | `CLAUSE_PROPS` (`Stack`: `indent`) | present |
| UI-NV-1 | `name` on `embed` | `CLAUSE_PROPS` (`Embed`: `name`) | present |
| UI-NV-1 | `selected`, `activate` on each `table.rows` entry | `ui/realize.rs` table row entries (`selected`, `activate`) | present |
| UI-CF-6 | `ui_capabilities{ui_contract_view, parts?, features?, widgets?, screens?}` | `ui/input.rs` `Capabilities::{ui_contract_view, parts, features, widgets, screens}` | present |
| UI-CF-6 | view features `ui_1_1`, `ui_embed`, `ui_1_3`, `ui_1_4`; renderer abilities `external_open`, `embed`, `display_metric`, `display_badge` | `ui/admission.rs` `default_features`; `ui/realize.rs` `FEATURE_EXTERNAL_OPEN`, `FEATURE_EMBED`, `FEATURE_DISPLAY_METRIC`, `FEATURE_DISPLAY_BADGE` | present |
| UI-CF-6 | `screens` entries `"<kind>@<MAJOR>"` | `ui/input.rs` `Capabilities::screens: Option<Vec<String>>` | present |
| UI-NV-2 | `confirm: {message, tone?}` | `ui/doc.rs` `Confirm::{message, tone}` | present |
| UI-NV-2 | `activate` shapes (`select`, `invoke` + `row_key?`, `open{kind, key}`, `navigate`, `url`) | `ui/realize.rs` (the realized `activate` prop) | present |
| UI-NV-2 | `options: [{value, label}]` | `ui/realize.rs` (the realized `options` prop of `select`) | present |
| UI-NV-2 | `error: {code, message?}`; local codes `option_unavailable`, `required_missing` | `ui/input.rs` `ErrorValue::{code, message}`; `ui/realize.rs` stamps `option_unavailable` | present (`required_missing` is stamped by the renderer at submit; the code is an open string; the UI crate keeps no code registry) |
| UI-NV-2 | `absent_reason` ∈ `not_found`, `no_baseline`, `remote_hub_unsupported` | `CLAUSE_PROPS` (`absent_reason` on `Text`, `Badge`, `Metric`, `StatusBadge`); `ui/realize.rs` | present |
| UI-NV-2 | button `action: {id, payload: {target, args}}` | `CLAUSE_PROPS` (`Button`, `MenuItem`: `action_ref`); `ui/realize.rs` | present |
| UI-NV-3 | role segments for dialogs, forms, sections, buttons, menus, rows, messages | `ui/ids.rs` `child_id`, `ROLE_WORDS` | present |
| UI-NV-4 | `RealizedTree{root, overlays, diagnostics, local_changes}` | `ui/node.rs` `RealizedTree`, `Diagnostic::{node_id, code, detail}` | present |
| UI-NV-4 | `local_changes{cleared_selections, unset_drafts, stamped_errors, dropped_view_state}` | `ui/node.rs` `LocalChanges`, `StampedError::{draft_key, code}` | present |
| UI-NV-5 | `local_state{selections, drafts, overlays, errors, focus, scroll}` | `ui/input.rs` `LocalState` | present |
| UI-NV-5 | drafts `{value}` \| `{unset: true}` \| `{clear: true}` | `ui/input.rs` `Draft::{Value, Unset, Clear}` | present |
| UI-NV-5 | overlays `[{form{button}} \| {confirm{button}}]` | `ui/input.rs` `Overlay::{Form, Confirm}`, `OverlayButton::button` | present |
| UI-NV-5 | errors `{fields?, forms?, actions?}` | `ui/input.rs` `LocalErrors::{fields, forms, actions}` | present |
| UI-AD-5 | admission codes `unsupported_part`, `unbound_context`, `unknown_view`, `unknown_property`, `bad_property_type`, `embed_source_not_own`, `number_out_of_range`, `non_canonical_map_key`, `duplicate_id`, `reserved_id`, `invalid_id`, `kind_not_admitted`, `path_not_allowed`, `unknown_slot`, `unknown_ref`, `unknown_action`, `unknown_handler`, `schema_mismatch`, `schema_unsupported`, `unknown_bound_property`, `arg_type_mismatch`, `visible_when_required`, `bad_default`, `when_not_exclusive`, `bad_alternatives`, `bad_placement`, `link_not_text`, `unsupported_version`, `missing_requires` | `ui/admission.rs` `AdmissionCode::{UnsupportedPart, UnboundContext, UnknownView, UnknownProperty, BadPropertyType, EmbedSourceNotOwn, NumberOutOfRange, NonCanonicalMapKey, DuplicateId, ReservedId, InvalidId, KindNotAdmitted, PathNotAllowed, UnknownSlot, UnknownRef, UnknownAction, UnknownHandler, SchemaMismatch, SchemaUnsupported, UnknownBoundProperty, ArgTypeMismatch, VisibleWhenRequired, BadDefault, WhenNotExclusive, BadAlternatives, BadPlacement, LinkNotText, UnsupportedVersion, MissingRequires}` (29 codes, 29 variants) | present |
| UI-NV-6 | `Text = string \| {copy, args?}` | `ui/realize.rs` `copy_key`, `copy_key_args` | present |
| UI-NV-6 | copy keys `generic_empty`, `generic_error`, `nothing_selected`, `submit`, `cancel`, `confirm`, `optional_section`, `form_unavailable`, `view_unavailable{view_id, reason}` | `ui/realize.rs` `DEFAULT_EMPTY_TITLE`, `DEFAULT_ERROR_TITLE`, `DEFAULT_NOTHING_SELECTED`, and the `copy_key` calls for `submit`, `cancel`, `confirm`, `optional_section`, `form_unavailable`, `view_unavailable` | present |
| UI-NV-7 | `overflow` defaults to `never` | `ui/doc.rs` `Overflow` (`#[default] Never`) | present |
| UI-NV-7 | advanced fields go to the `optional_section` default section | `ui/realize.rs` (`optional_section`) | present |
| (kept) | UiNode `iframe` stays, deprecated, never emitted | `ui/vocabulary.rs` `UiNodeKind::Iframe` | present |

### UI 1.4 erratum 1 (`ui-contract-v1.4-erratum-1-candidate1.md`)

| Clause | Item | Crate symbol | Status |
|---|---|---|---|
| UI-NV-8 | the error tone is `danger`; no tone `error` | `ui/doc.rs` `Tone::{Neutral, Info, Success, Warning, Danger}`; `ui/realize.rs` emits `tone: "danger"` | present |

## CLI (`cli-spec-v1-candidate34.md`)

| Clause | Item | Crate symbol | Status |
|---|---|---|---|
| all | the CLI spec | none | n/a: informative; it cites HC and HP and adds no contract type, field, code, limit or default. The values it prints come from the HC rows above (for example CLI-PL3 reads `PackageInstallResult`, HC A6-1; CLI-WT1 reads `WorktreeRow::branch`, HC A6-2) |
