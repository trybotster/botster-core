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

`CoreDaemon::new` also reuses a process-local live engine supervisor for the
same `data_dir`. Replacing the daemon facade therefore does not intentionally
shut down live local sessions; the restarted facade can list the persisted
record, attach a client, drain output, send input, and shut the session down
through the same typed daemon API. This is a core daemon behavior and does not
require `botster-hub`.

Adoption scan is read-only. It reports deterministic follow-up state and leaves
registry files untouched until a caller performs an explicit operation such as
`mark_stale`. Terminal registry states (`stopping`, `exited`, `stale`) report
`terminal`. Running records with complete restart-contract evidence report
`adoptable` when a single candidate exists. Records with a protocol mismatch
report `stale_worker` with `incompatible_protocol`. Records whose protocol
evidence exists but whose worker route is gone report `stale_worker` with
`worker_died` when prior process identity is known, or `process_missing` when
it is not. Records with handshake and recovery identity but missing ping/pong
support report `unhealthy_worker` with `missed_heartbeat`. If more than one
candidate claims the same session identity, the scan reports `duplicate_worker`
with the candidate count; operators should treat that as non-adoptable until
one candidate is chosen or the extras are cleaned up explicitly.

The current restart guarantee is deliberately local to the core daemon
supervisor process. It proves daemon-facade replacement over a shared
supervisor and durable registry. It does not claim that dropping the entire OS
process can reattach to anonymous inherited pipes; runtimes that need that
stronger crash-recovery boundary must expose a durable worker endpoint that can
be reopened after process exit.

Guarded writes use explicit daemon delivery states: accepted, deferred,
rejected, written, delivered, and acknowledged. Acceptance means only that the
daemon received a valid typed request. Written means bytes were injected through
the existing PTY input path. Delivered and acknowledged require downstream
proof; plain PTY input does not currently provide that proof, so the daemon does
not fabricate those states.

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
