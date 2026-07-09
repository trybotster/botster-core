# Durable Session Worker Protocol

This document defines the public **protocol vocabulary** for Botster's durable
local session-worker model. Types live in `botster_core::durable_session` and are
spoken by the production daemon and session worker; they do not by themselves
start processes or open sockets.

## Runtime ownership (workspace truth)

| Layer | Crate / binary | Role today |
| --- | --- | --- |
| Protocol vocabulary | `botster-core` (`durable_session`) | Shared typed shapes for spawn/adopt, health, guarded writes, queues, daemon control ops |
| Session worker process | `botster-session-worker` (binary in `botster-core`) | Owns one PTY and child process; reconnectable control socket for adoption |
| Production daemon | `botster-core-daemon` (`CoreDaemon`) | Registry metadata, adoption scan, guarded-write delivery states, typed host API |
| Library engine | `DefaultBotsterEngine` / `worker_backed` | In-process or worker-backed embed path without the full supervisor |

Do **not** treat this module as “unimplemented durable daemon.” The durable
supervisor exists in `botster-core-daemon`. Embedding `CoreDaemon` without
`with_worker_path` still uses in-process PTYs that are not restart-adoptable.

See also [`core-daemon.md`](core-daemon.md) and the workspace README production
path.

## Topology

The durable model has three layers:

| Layer | Responsibility | Explicitly excluded |
| --- | --- | --- |
| Hub or host | Product policy, user workflows, auth, config locations, routing decisions, plugin policy, persistence choices | Owning terminal bytes, parsing CLI output as an API |
| Core daemon | Local policy-free multiplexer, supervisor, router, typed daemon control facade, bounded queue reporting, worker health/adoption coordination | Marketplace, Rails, WebRTC/cloud policy, product CLI UX |
| Session worker | Owns one PTY and child process, emits output/snapshots/health, accepts PTY input, resize, shutdown, and guarded session-visible write commands | Host policy for when writes are allowed |

The data plane remains session/client-worker owned. The durable worker contract
sits above `contract::session_protocol` (advanced byte-frame constants) for PTY
input/output, resize, snapshot, ping/pong, shutdown, mode flags, screen reads,
prompt marks, and notifications.

## Public Contracts

The durable contract is exported from `botster_core::durable_session` and the
crate root. The main shapes are:

- `DurableSessionProtocolVersion` and `SessionWorkerCapability` for
  compatibility metadata.
- `SessionWorkerIdentity`, `SessionWorkerSpawnRequest`,
  `SessionWorkerSpawned`, `SessionWorkerAdoptRequest`, and
  `SessionWorkerAdoptionVerdict` for spawn/adopt identity.
- `SessionWorkerAttachRequest` and `SessionWorkerDetached` for attach/detach.
- `SessionWorkerHeartbeat`, `SessionWorkerHealth`, `SessionWorkerFailure`, and
  `SessionWorkerShutdownRequest` for health, stale detection, shutdown, and
  failure.
- `SessionWorkerOutputFrame` for terminal bytes, snapshot handoff, and plain
  screen handoff.
- `SessionReadinessEvidence`, `GuardedSessionWriteRequest`, and
  `GuardedSessionWriteState` for readiness-gated session writes.
- `SessionWorkerQueueLimits` for bounded output, pressure, lag, and
  slow-consumer semantics.
- `DaemonControlOperation`, `DaemonControlOutcome`, and `DaemonCliOperation` for
  typed daemon control and the thin CLI wrapper vocabulary.

These are serializable public contracts. They do not execute scheduling,
restart adoption, process supervision, or socket I/O by themselves.

## Daemon Control API And CLI Role

Embedders and the hub should use typed local IPC/library contracts. The daemon
CLI is a thin operator/dev/debug wrapper over the same typed operations:
`start`, `status`, `session list`, `attach` or `stream`, `shutdown`, and
`health`.

The CLI output is never the API. Programmatic callers should consume typed
daemon control outcomes or the existing `BotsterEngine` facade documented in
`docs/architecture/engine-command-surface.md`. The durable daemon operation
types name the outer daemon lifecycle and health commands without introducing a
second core router.

## Write Primitives

The contract separates two session write primitives:

- `GuardedSessionWritePrimitive::PtyInput` is raw terminal input bytes and maps
  to the existing `SessionIoRequest::PtyInput` path when routed.
- `SessionAnnotation` and `SessionNotification` are host/plugin-authorized
  writes intended to appear in a session stream or screen.

Notification inbox delivery remains a separate core primitive. Guarded
session-visible notification writes may reference `NotificationId` and can
report `NotificationDeliveryStatus` when a later implementation can prove that
relationship.

## Readiness Evidence

Core owns terminal/session observation and deterministic scheduling mechanics.
Hosts and plugins own the semantic policy decision.

`SessionReadinessEvidence` can carry:

- terminal mode flags, including `ModeFlags::cursor_visible`, when a runtime
  adapter reports them;
- plain screen text summaries when available;
- prompt waiting markers such as `waiting_for_answer`;
- `unsafe_to_interrupt` for prompt or screen state that would make injection
  unsafe;
- snapshot-pending state;
- worker health and session activity;
- host semantic hints that core serializes but does not interpret.

Cursor visibility is intentionally represented through `ModeFlags` only when
that observation exists. Runtime adapters that expose only plain text must leave
`mode_flags` empty rather than pretending cursor visibility was observed.

## Delivery States

Guarded writes must not overclaim delivery. The public state transitions are:

| State | Meaning |
| --- | --- |
| `Accepted` | Request was accepted for evaluation only. Nothing has been written. |
| `Queued` | Request is waiting in the existing bounded session-I/O pressure path. |
| `Deferred` | Request is held until readiness or host policy changes. |
| `Rejected` | Request will not be written. |
| `Written` | Bytes or annotation content were injected into the worker path. |
| `Acknowledged` | Delivery was acknowledged where the implementation can prove it. |

The contract intentionally reuses `BackpressureSummary`, `DeliveryLag`,
`QueueSource`, and `NotificationDeliveryStatus` instead of creating a parallel
pressure or notification-delivery vocabulary.

## Restart Semantics

`DurableRestartSemantics::durable_worker_contract()` defines the north-star
survival matrix:

| Boundary | Contract |
| --- | --- |
| Hub restart | Session survives because the worker owns the PTY/child process. |
| Core daemon restart with successful adoption | Session survives after worker identity and protocol are verified. |
| Core daemon restart with failed adoption | Session may survive degraded only if host recovery has enough evidence; otherwise stale-worker failure is reported. |
| Session worker death | Session dies. The PTY and child-process owner is gone. |

Stale workers are detected through identity mismatch, incompatible protocol,
expired heartbeat, missing process, or worker death. Hosts may add a classified
`Other` reason, but core does not define product policy from that detail.

## Queue And Backpressure

Durable worker output is bounded. `SessionWorkerQueueLimits` names output frame
and byte capacity, reuses existing `BackpressureSummary` and `DeliveryLag`, and
describes slow-consumer behavior as preserve-order/backpressure, drop live
output after snapshot, or detach the slow consumer.

The default durable worker path should use the existing `QueueSource::SessionIo`
unless a future implementation proves a separate queue source is necessary.

## PII And Examples

Docs, fixtures, and tests for this contract use synthetic ids and generic
labels only. Durable worker contracts must not log or fixture local usernames,
paths, prompt text, terminal transcripts, customer data, or product workflow
titles.

## Current Runtime Proof

`botster_core::durable_session` remains a serializable vocabulary layer (plus
serialization/conformance tests). Production runtime proof for supervision and
adoption lives in `botster-core-daemon` and the `botster-session-worker` path:

- worker-backed local sessions with control sockets recorded as
  `SessionMetadata.recovery_identity`
- intentional daemon restart via `release_for_restart` and re-adoption over the
  same `data_dir`
- guarded-write delivery states and readiness fail-closed behavior

Hosts that need durable local sessions should use `CoreDaemon` with
`with_worker_path`, not only import these contract types. Hub product restart
policy remains outside core.
