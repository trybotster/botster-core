# Core Daemon

`botster-core-daemon` is the production core daemon layer over `botster-core`.
It owns daemon mechanics: persistent registry metadata, session supervision
state, attach/input/resize/output routing through the existing core engine,
readiness-gated guarded writes, adoption scans, health, and shutdown.

Session workers still own PTYs and terminal/session evidence. The daemon
records only non-sensitive metadata needed for discovery and adoption: session
id, process identity, lifecycle state, terminal size, protocol version,
handshake/liveness booleans, optional recovery identity, timestamps, and a
scrubbed command label. It does not persist PTY bytes, scrollback, credentials,
auth state, workflow records, or product routing policy. A registry record is
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

Screen reads, mode reads, and snapshot captures are also part of the typed production daemon
API. `CoreDaemon::read_screen`, `CoreDaemon::read_mode_flags`, and
`CoreDaemon::capture_snapshot` internally
drain the target session before reading terminal state because worker-backed
terminal truth advances on the drain path. Any client egress or observations
produced by that internal drain are retained and prepended to the next explicit
`CoreDaemon::drain` result for the session, matching the attach retention
contract. Natural-exit readback retention does not consume the host's pending
`CoreDaemon::drain` obligation: final egress remains available exactly once.

After final PTY output is drained, natural exit and per-session shutdown freeze
one immutable paired screen/snapshot/mode-read record. `read_screen`,
`read_mode_flags`, and `capture_snapshot` serve that same record symmetrically, repeatedly, and
idempotently without polling the dead runtime. The exited session stays
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
source instead of repeatedly polling `list()`. `lifecycle_baseline()` returns
registry rows in deterministic `SessionId` order plus a watermark cursor. Each
row carries durable `DaemonSession` facts and, when this daemon currently owns
or adopted the runtime, its in-memory `SessionLifecycleState`. A fresh daemon
can therefore expose durable registry truth before adoption without fabricating
live runtime evidence; successful adoption later publishes a `Running` upsert
for the same stable session id.

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
returns an explicit `source_changed`, `cursor_expired`, or `cursor_ahead`
resync reason with an empty change list when continuity cannot be proven. It
never returns a silently truncated suffix. Recovery is always a new baseline;
replay is not durable across daemon restart.

Lifecycle consumption does not advance runtimes. The host still drives normal
`drain` calls (or other existing progress paths), then consumes lifecycle
changes. `SessionLifecycleChange` contains session projection facts only;
`TransportEgress`, PTY bytes, snapshots, and attach ordering remain exclusively
on the existing drain/data plane.

Snapshot payloads are backend-neutral opaque terminal state. The current plain
fallback runtime returns raw retained PTY bytes with format
`plain-opaque-v1`, terminal dimensions, and a 1 MiB retained-tail bound; older
bytes are truncated from the head. The plain fallback runtime does not parse VT
sequences. It is retained for `botster-core-daemon --no-default-features`
contract-only embeds.

The default `botster-core-daemon` feature set enables `ghostty-terminal`, which
uses the sibling `botster-terminal-ghostty` crate and its `libghostty-vt`
feature as the production terminal backend. On that path,
`CoreDaemon::read_screen` returns Ghostty-formatted plain text and
`CoreDaemon::capture_snapshot` returns opaque bytes labeled
`ghostty-terminal-snapshot-v1`. The daemon configures Ghostty with a 10 MB
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
and workspace builds therefore require Zig `0.15.2` and an initialized
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
