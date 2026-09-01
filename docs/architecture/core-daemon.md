# Core Daemon

`botster-core-daemon` is the production core daemon layer over `botster-core`.
It owns daemon mechanics: persistent registry metadata, session supervision
state, attach/input/resize/output routing through the existing core engine,
readiness-gated guarded writes, adoption scans, health, and shutdown.

Session workers still own PTYs and terminal/session evidence. The daemon
records only non-sensitive metadata needed for discovery and adoption: session
id, process identity, lifecycle state, terminal size, protocol version,
handshake/liveness booleans, optional recovery identity, timestamps, and a
scrubbed command label. It also persists the opaque host-owned string map from
`CoreSessionMetadata`, subject to Core's existing encoded-size limit. Core does
not interpret or define that map's keys, and hosts remain responsible for
excluding PII. It does not persist PTY bytes, scrollback, credentials, auth
state, workflow records, or product routing policy. A registry record is
adoptable only after the daemon has observed the session-worker restart
contract: HELLO/WELCOME protocol negotiation, FRAME_PING/PONG liveness support,
and `SessionMetadata.recovery_identity`. Local in-process spawns that have not
produced those protocol facts remain `missing_protocol_evidence`, not
`adoptable`.

The typed Rust API is the embedder and hub path. The CLI is intentionally thin
operator/dev/debug tooling over that same API and requires `--data-dir` so
smoke runs do not depend on ambient home directories.

Attach output is part of the daemon output contract. `CoreDaemon::attach`
routes subscription setup through the core engine, which emits `Attaching` only
when a new or replacement subscription requests its initial snapshot.
`InitialSnapshotReady` for that exact client and subscription then produces an
optional non-empty `Snapshot` followed by `Attached`; empty history produces
`Attached` without a fabricated `Snapshot` or `Scrollback`. The session worker
holds live output behind that initial-snapshot barrier, so the production order
is `Attaching` -> `Snapshot` (when non-empty) -> `Attached` ->
`TerminalOutput`. Stale or replaced snapshot deliveries produce neither history
nor readiness. Core currently has no production `Detached` emitter.

`CoreDaemon` retains attach output and prepends it to the next `drain` result
for the session so late subscribers see readiness and replay before later live
terminal output. It must not re-route the returned session requests; those
requests are an already-routed record from the engine, not daemon follow-up
work.

Screen reads, mode reads, snapshot captures, and atomic color+snapshot captures
are also part of the typed production daemon API.
`CoreDaemon::read_screen`, `CoreDaemon::read_mode_flags`,
`CoreDaemon::capture_snapshot`, and `CoreDaemon::capture_color_and_snapshot`
internally drain the target session before reading terminal state because
worker-backed terminal truth advances on the drain path. That internal drain
does not pump bound adapters. Any client egress or observations produced by
that internal drain are retained and prepended to the next explicit
`CoreDaemon::drain` result for the session, matching the attach
retention contract. Natural-exit readback retention does not consume the host's
pending `CoreDaemon::drain` obligation: final egress remains available exactly
once.

Wake-driven pumping runs on the thread that owns `CoreDaemon`.
`CoreDaemon::wait_wakes(timeout)` blocks on a transport-neutral source.
`CoreDaemon::pump_woken` drains runtime
output only for named sessions and intakes/pumps only named waking-adapter
routes. It does not `try_read` an unnamed adapter. The method commits lifecycle
observations and retains unmatched output before it returns. Its public outcome
contains no terminal or lifecycle body, so a content-blind host can ignore it.

A host can call `CoreDaemon::wake_pump_control()` before it starts its pump
loop. This call returns `WakePumpControl`, which is `Clone + Send + Sync`.
The control has no daemon access. Another thread can call `interrupt()` when it
adds host work. The owner thread then receives `WakePumpWait::Interrupted` from
`wait_pump(timeout)`. The owner thread can process bounded host work against the
same `&mut CoreDaemon`.

Only one thread can own and mutate `CoreDaemon`. The host must construct the
daemon on that thread. The host must not add shared mutable daemon access. Only
one waiter can drain a daemon wake source at a time.

`WakePumpControl::request_stop()` is monotonic and interrupts a blocked wait.
The first wait that observes the stop can return one final real wake batch. The
drain reads at most `WAKE_QUEUE_CAPACITY` channel nodes. The next wait returns
`WakePumpWait::Stopped` without a channel read. Thus, a live producer cannot
extend the pump loop without a bound.

After `Stopped`, the owner thread finishes its bounded accepted host work. The
owner thread then calls `CoreDaemon::shutdown(None, ..)`. A pump-hosted daemon
rejects full shutdown if the pump loop did not observe its stop. A daemon that
never issued a pump control keeps the existing shutdown behavior. The owner
thread exits after Core shutdown. Another thread can then join it.

One live-session registry owns ingress coalescing, overflow recovery, and
retirement. A retained reader handle cannot resurrect a forgotten session.
Overflow sets a flag and leaves `queued` true. The next `wait_wakes` reconciles
without a correctness timer. Session shutdown keeps the ingress wake while the
lifecycle is `Stopping`. Converting or routing `ProcessExited` is not an
authoritative observation. `CoreDaemon` retires the session ingress wake only
after it persists the terminal lifecycle transition, appends the lifecycle
journal entry, and completes required final-state retention. A failed commit
keeps or re-arms that exact wake under a bounded retry rule. Shutdown acceptance
still retires nothing. Explicit runtime removal also retires the wake.

`CoreDaemon::shutdown` uses a capacity-capped wake drain plus `pump_woken`.
It uses the two-second bound as a hang watchdog. The capped drain does not
consume the host interrupt flag. The current `drain` poll path remains
for unbound adapters during the migration window. Core creates no extra OS
thread for either wake wait.

`CoreDaemon::capture_color_and_snapshot` is the Hub-facing ordering boundary for
current Ghostty colors and durable GHOSTSNP state. It returns
`TerminalColorProfile` (full 256 palette plus reserved special indexes
`0x1000` foreground, `0x1001` background, `0x1002` cursor) together with the
opaque snapshot payload from **one** session terminal borrow after drain. Hosts
must not reconstruct agreement by calling independent color and snapshot reads.
Host-supplied `CoreDaemonConfig::with_terminal_color_profile` remains
spawn/initial baseline only; after session start Ghostty owns current
palette/specials (including OSC 4/10/11/12 mutations). GHOSTSNP remains the
authoritative durable state for late attach/reconnect.

After final PTY output is drained, natural exit and per-session shutdown freeze
one immutable paired screen/snapshot/mode-read/color-profile record.
`read_screen`, `read_mode_flags`, `capture_snapshot`, and
`capture_color_and_snapshot` serve that same record symmetrically, repeatedly,
and idempotently without polling the dead runtime. The exited session stays
read-only and cannot be resized, written, attached for reactivation, or resumed
through this record. No valid paired final capture yields
`SessionNotReadable`; blank or opaque bytes are never invented as fallback
truth. Registry-stale and failed sessions remain unreadable.

Retention is daemon-memory-only and clears on explicit terminal-session
removal or daemon exit. There is no TTL or one-consumer budget, and durable
registry records still contain no terminal bytes. `CoreDaemon::remove_session`
is deliberately a policy-free mechanism rather than automatic retention
policy: it rejects live and stopping sessions, then clears the registry row,
engine session/handle/worker indexes, subscriptions, retained terminal state,
and pending drains before publishing the lifecycle `Removed` change.

## Authoritative lifecycle source

Hosts that maintain a session projection use the public `CoreDaemon` lifecycle
source instead of repeatedly polling `list()`. `lifecycle_baseline_page`
returns one frozen snapshot sequence, bounded rows, bounded encoded bytes,
a bounded elapsed yield, a next-row cursor, and an explicit `complete`
flag. `snapshot = None` mints the freeze at the current journal watermark
and walks the registry directory under the call budget. Later pages at
that sequence continue the same freeze and must not re-read a mutated
registry. Setup-only and index-in-progress yields keep the freeze
identity and are not complete. An incomplete page is not finished ended
evidence. `lifecycle_baseline()` remains the unbounded compatibility
reader that loads every row in one call. Hub Stage A must not call it.
Each
row carries durable `DaemonSession` facts, the exact opaque host metadata from
the authoritative registry record, and, when this daemon currently owns or
adopted the runtime, its in-memory `SessionLifecycleState`. A fresh daemon can
therefore expose durable registry truth and host metadata before adoption
without fabricating live runtime evidence; successful adoption restores that
metadata into `CoreSession` and later publishes a `Running` upsert for the same
stable session id. Older registry and lifecycle JSON without the field decodes
to an empty map.

Adoption revalidates persisted metadata against Core's current size cap. An
oversized or hand-edited map fails adoption with `MetadataTooLarge` before the
daemon touches the live worker. The daemon does not truncate it, replace it
with an empty map, mutate the registry record, or publish a successful adoption
upsert. The running worker remains untouched and can be adopted after the
record is repaired. A future cap reduction therefore requires an explicit
compatibility or migration decision for older records.

Each cursor contains an opaque daemon source-generation id and a strictly
monotonic sequence. Spawn, adoption, material registry-row updates, lifecycle
transitions, and explicit terminal removal append changes only after the
authoritative mutation succeeds. Repeated terminal observations do not append
duplicate changes. Shutdown normally publishes `Stopping` then `Exited`; when
the process completes synchronously, the truthful collapsed sequence is only
`Exited`.

The journal is process-memory-only and bounded to 1,024 changes by default;
tests and specialized embedders can set
`CoreDaemonConfig::with_lifecycle_journal_capacity`. `lifecycle_changes()`
remains the unbounded compatibility reader. `lifecycle_changes_page(after,
max_changes, max_bytes)` is the bounded reader: it validates the cursor first,
returns the exact `source_changed`, `cursor_expired`, or `cursor_ahead`
resync reason with empty changes when continuity cannot be proven, and never
returns a silently truncated suffix. A valid cursor whose empty successful
page encodes larger than `max_bytes` returns typed `BudgetTooSmall {
minimum_bytes }`. `SessionLifecyclePageError` is `#[non_exhaustive]` at first
publish; consumers must match `BudgetTooSmall` and a wildcard. Recovery is
always a new baseline; replay is not durable across daemon restart.

Control-plane progress is `CoreDaemon::observe_lifecycle_slice`. One call
visits live sessions in deterministic `SessionId` order under item, encoded-
result, and elapsed budgets. It drains and reconciles each visited session
independently, retains incidental terminal egress for a later `drain`, and
continues after a per-session error. A later call with the returned cursor
resumes that pass only when `pass_id` and `last_visited` both match the
open snapshot; otherwise the result is a resync with `complete = false`.
`resume = None` starts a new pass and snapshots the ordered live set.
Later slices walk that remaining snapshot and do not rescan. Sessions that
appear after mint wait for a new pass. Elapsed starts at API entry and
includes snapshot setup. It does not call `drain_runtime_all_once` and returns no terminal
bytes, phases, snapshots, attach state, or `ProcessExited` frames.
`observe_lifecycle` remains the unbounded compatibility wrapper that starts a
new pass and visits every remaining live session. Page, wake, and baseline
reads stay side-effect-free.

`append_lifecycle_change` sets one coalesced `journal_advanced` bit.
`take_journal_advanced_wake` clears that bit. Duplicate appends before take
stay one bit. Page and baseline never clear it. The safe consume order is
take, page until `next == source_watermark` or resync, take again, and
re-page if that second take is true. Never page-then-take-then-sleep.
An empty successful page with `next != source_watermark` is not catch-up:
the first remaining change does not fit the valid budget, or `max_changes`
is 0. Recovery is a fresh `lifecycle_baseline`, not sleep. Do not treat
`changes.is_empty()` alone as caught up.

`SessionLifecycleChange` contains session projection facts only;
`TransportEgress`, PTY bytes, snapshots, and attach ordering remain exclusively
on the existing drain/data plane. `CoreDaemon::drain` may still update the
journal; it is no longer required to learn exit.

Snapshot payloads are Ghostty-owned opaque terminal state. Production
`CoreDaemon::capture_snapshot` and the snapshot half of
`CoreDaemon::capture_color_and_snapshot` return `GHOSTSNP` bytes labeled
`ghostty-terminal-snapshot-v1`. There is no daemon plain/`plain-opaque-v1`
production path; `PlainTerminalScreenRuntime` is only a `botster-core` library
or unit-test harness.

`botster-core-daemon` always depends on Ghostty (`botster-terminal-ghostty` with
`libghostty-vt`). There is no optional plain production terminal feature. Ghostty
uses the sibling `botster-terminal-ghostty` crate and its `libghostty-vt`
feature as the production terminal backend. On that path,
`CoreDaemon::read_screen` returns Ghostty-formatted plain text and
`CoreDaemon::capture_snapshot` returns opaque Ghostty snapshot bytes. Atomic
color+snapshot capture proves GHOSTSNP content by replaying the payload into a
fresh Ghostty terminal and comparing the restored color profile. The
daemon configures Ghostty with a 10 MB
retained scrollback byte budget. Ghostty stores parsed terminal pages instead
of a raw byte tail, so the effective retained line count is page-quantized and
depends on terminal width. At 24x80 today the daemon test fixture with 12,000
generated lines retains more than 4,000 generated markers while dropping the
oldest marker. Ghostty snapshots serialize native terminal state: a fresh 24x80
terminal is roughly 578 KiB, while a warm session at the 10 MB default measured
roughly 9.0 MiB for a 24x80 terminal after scrollback saturation. That cost is
per attaching client because `CoreDaemon` emits one
`TransportEgress::Snapshot` frame to each subscriber. Snapshot size converges on
the retained scrollback byte budget plus the roughly 578 KiB base. Daemon tests
enforce a 16 MiB ceiling for the reviewed scrollback fixture. Attach/recovery
paths should treat those frames as large opaque state, not renderable text;
payload cost scales with retained scrollback. Follow-up
`ticket_1783631884_479370` owns host-configurable scrollback budget and any
chunking or backpressure changes for multi-MiB snapshot frames. Default daemon
and workspace builds therefore require Zig `0.16.0` and an initialized
`crates/botster-terminal-ghostty/vendor/ghostty` submodule. Disable daemon
default features for pure contract builds that must avoid Ghostty and Zig.

When configured with the `botster-session-worker` executable, `CoreDaemon`
spawns worker-backed local sessions. Each worker owns its PTY in a separate
process and exposes a reconnectable Unix control socket recorded in
`SessionMetadata.recovery_identity`. An intentional daemon restart calls
`release_for_restart` instead of session shutdown, allowing the worker process
to keep running. A fresh daemon over the same `data_dir` can scan the persisted
record, reconnect to that worker endpoint with `adopt_session`, then list,
attach, drain output, send input, and shut the session down through the same
typed daemon API. This is a core daemon behavior and does not require
`botster-hub`.

The control socket is private transport state, not session identity. Its
basename is a fixed-length URL-safe encoding of the first 128 bits of a SHA-256
digest over the complete `SessionId`; the full unmodified id remains in every
handle, protocol frame, health response, registry record, and client
projection. `CoreDaemon` chooses a short, data-directory-derived root beneath
`std::env::temp_dir()`. A library caller's explicit
`WorkerProcessRuntimeOptions::control_socket_dir` is used unchanged. Core
validates the complete pathname in bytes before spawning a worker (103 pathname
bytes on macOS/BSD-shaped targets, 107 on other Unix targets) and returns
`SpawnFailed` with a `worker control socket path ...` diagnostic when it cannot
fit.

Worker bind is non-destructive by default. A connectable existing socket is
treated as live. A connection-refused socket may be removed only after its
device/inode/change-time identity is rechecked; changed sockets and non-socket
entries are preserved and fail the spawn. The worker removes only its own
unchanged endpoint on normal exit, while the daemon removes its derived root
only when that directory is empty. Caller-provided roots remain caller-owned.
Parent-side startup waits for a child-PID readiness signal and verifies the
welcome's worker PID before accepting the control connection. Failures reap the
exact spawned worker before returning.

A successful worker-backed shutdown is a terminal completion boundary, not an
acknowledgement that shutdown merely started. The worker runtime publishes the
final process-exit observation only after earlier retained output is drainable,
the control reader has reached EOF, the owned worker child has exited cleanly
when present, and the control socket has been removed. `CoreDaemon` then freezes
the final terminal readback and marks the registry record `exited`. Any
subscription egress consumed while reconciling that successful per-session
shutdown is appended after existing pending egress and remains available
exactly once through the next explicit `CoreDaemon::drain` for the original
client and subscription. Recovery egress is retained before final terminal
capture, so a capture failure cannot erase data that the shutdown path already
drained. Repeating per-session shutdown neither clears nor duplicates the
retained batch.

Whole-daemon `shutdown(None, ..)` is terminal host teardown: it stops the
daemon, clears retained terminal readback, and intentionally has no
post-shutdown drain surface. A host that must deliver final subscription egress
before stopping the daemon must shut down and drain each session while the
daemon is still running.

If completion cannot be proven within the existing supervisor deadline,
shutdown returns the original typed delivery error when one started recovery,
or `ShutdownFailed` after accepted delivery. It leaves the registry non-exited,
retains pending drain evidence, and keeps cleanup ownership. This failure path
is deliberately distinct from `release_for_restart`, which preserves a worker
only when the host intentionally chooses restart adoption without first
completing session shutdown.

Shutdown remains idempotent when natural exit wins the race with an explicit
cleanup request. Direct `MultiplexerEngine` and `BotsterEngine` workers commit
`stopping` after their synchronous shutdown callback accepts delivery. The
managed worker path queues that control input, so its `stopping` transition is
provisional until the runtime accepts the queued input. If delivery fails, the
managed runtime restores the prior `starting` or `running` lifecycle and
returns the original typed error without consuming runtime output. The daemon
then uses its existing bounded final-output drain loop to wait for lagging
natural-exit evidence. A same-session process-exit observation makes cleanup
succeed; if the deadline expires first, the original typed delivery error is
returned so a later call can retry. A known `exited` session and a confirmed
`stopping` session accept repeated shutdown without another control frame.
Unknown sessions and failures without same-session terminal evidence remain
errors.

Adoption scan is read-only. It reports deterministic follow-up state and leaves
registry files untouched until a caller performs an explicit operation such as
`mark_stale`. Terminal registry states (`stopping`, `exited`, `stale`) report
`terminal`. Running records with complete restart-contract evidence report
`adoptable` when a single candidate exists and the daemon is configured with a
session-worker path. If the same restart evidence is scanned by a daemon
without `CoreDaemonConfig::with_worker_path(...)`, the record reports
`in_process_daemon_not_restart_durable` instead of claiming it can be adopted.
Records with a protocol mismatch report `stale_worker` with
`incompatible_protocol`. Records whose protocol evidence exists but whose
worker route is gone report `stale_worker` with `worker_died` when prior
process identity is known, or `process_missing` when it is not. Records with
handshake and recovery identity but missing ping/pong support report
`unhealthy_worker` with `missed_heartbeat`. If more than one candidate claims
the same session identity, the scan reports `duplicate_worker` with the
candidate count; operators should treat that as non-adoptable until one
candidate is chosen or the extras are cleaned up explicitly.

The restart guarantee depends on the worker-backed runtime and its durable
control socket. Plain local in-process runtimes remain registry-visible but are
not restart-adoptable because their PTY handles die with the daemon process.
`CoreDaemon::adopt_session` returns `MissingWorkerPath` on a daemon with no
configured worker executable rather than attempting registry-based adoption.
Unexpected daemon crashes can leave workers running; they remain adoptable only
while their control sockets and PTYs are alive. Operators should treat missing
control sockets, incompatible protocol versions, duplicate candidates, and
missed heartbeats as non-adoptable until explicit cleanup or mark-stale action
is taken. Adoption uses the exact persisted endpoint and never falls through to
a fresh bind. A missing or unreachable persisted socket therefore remains a
`SpawnFailed` whose message starts with
`connect worker control socket failed: `; hosts may use that stable seam to
classify the registry record as stale. This is also the behavior when macOS
reaps a socket pathname while its worker process identity still appears live.
This deliberately differs from the Hub listener's self-repair convention:
session-worker adoption has only persisted route and process evidence, not
authority to create a second listener for a possibly-live PTY owner. The live
worker does not currently republish a reaped pathname, so the host must report
the record stale rather than risk duplicate ownership. Revisit that deviation
only with a worker-owned repair handshake that proves the original process is
the listener being restored.

Guarded writes use explicit daemon delivery states: accepted, deferred,
rejected, written, delivered, and acknowledged. Acceptance means only that the
daemon received a valid typed request. Written means bytes were injected through
the existing PTY input path. Delivered and acknowledged require downstream
proof; plain PTY input does not currently provide that proof, so the daemon does
not fabricate those states.

The daemon also exposes policy-free notification inbox and routed-envelope
mechanics through the typed Rust API. Notification methods queue, drain, query,
and acknowledge `NotificationItem` values by `NotificationTarget`. Routed
envelope methods publish, drain with cursor and limit semantics, acknowledge one
target copy, and report delivery state for `RoutedEnvelope` values. These APIs
are generic multiplexer coordination primitives; they do not define product
workflow terms, message semantics, auth policy, retention policy, UI payloads,
package surface/navigation contracts, or presentation. Those contracts belong
to Hub and `botster-ui-contract`.

`CoreDaemon` owns this notification and routed-envelope state directly instead
of delegating it to the session engine. That keeps behavior identical for plain
local daemons and worker-backed daemons: worker-backed PTY sessions may be
restart-adoptable, but notification inbox and routed-envelope queues are
in-memory daemon process state. A fresh daemon over the same `data_dir` starts
with empty notification and envelope queues unless a future persistence layer is
added. Embedders that use `DefaultBotsterEngine` directly still own that
engine's separate notification inbox; hub-native coordination should use the
sanctioned `CoreDaemon` API rather than creating a parallel hub-local inbox.

Readiness evidence is currently a host-supplied composite over core-owned facts
such as terminal mode flags, cursor visibility, prompt evidence,
snapshot/screen availability, and safe-write indicators. The daemon validates
that composite fail-closed. Missing safe-write evidence defers the write instead
of guessing. Unsafe evidence rejects it. A future synchronous readiness API can
move more evidence sourcing inside the daemon without changing these delivery
states.

Hub, hosts, and plugins remain responsible for auth, marketplace/cloud/WebRTC
policy, product copy, spawn target admission, notification meaning, UI
presentation, and retention policy.
