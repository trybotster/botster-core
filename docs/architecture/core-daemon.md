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
routes subscription setup through the core engine, and that engine outcome can
already contain initial history replay for the newly attached subscription.
`CoreDaemon` retains that output and prepends it to the next `drain` result for
the session so late subscribers see replay before later live terminal output.
It must not re-route the returned session requests; those requests are an
already-routed record from the engine, not daemon follow-up work.

Screen reads and snapshot captures are also part of the typed production daemon
API. `CoreDaemon::read_screen` and `CoreDaemon::capture_snapshot` internally
drain the target session before reading terminal state because worker-backed
terminal truth advances on the drain path. Any client egress or observations
produced by that internal drain are retained and prepended to the next explicit
`CoreDaemon::drain` result for the session, matching the attach retention
contract. Lifecycle observations from the internal readback drain are
retained for the next explicit `CoreDaemon::drain`. The internal readback
drain advances the engine's session lifecycle, so a process that has already
exited is observed on that same call and the readback returns
`SessionNotReadable` rather than stale terminal state.
Readback requires a live readable session: `read_screen` and
`capture_snapshot` return `SessionNotReadable` once the engine lifecycle is
stopping, exited, or failed, or when the registry record has been marked
stopping, exited, or stale. `CoreDaemon::drain` intentionally remains
available for those sessions so hosts can flush retained egress and lifecycle
observations after readback has stopped, even when the runtime handle has
already been reaped.

Snapshot payloads are backend-neutral opaque terminal state. The current plain
fallback runtime returns raw retained PTY bytes with format
`plain-opaque-v1`, terminal dimensions, and a 1 MiB retained-tail bound; older
bytes are truncated from the head. The plain fallback runtime does not parse VT
sequences. `ScreenReady.text` is a lossy UTF-8 view of the same retained byte
tail the snapshot returns: raw PTY bytes including escape sequences, not a
rows-by-columns rendered screen. A rendered screen requires a parsing backend.

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
is taken.

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
workflow terms, message semantics, auth policy, retention policy, or UI
presentation.

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
