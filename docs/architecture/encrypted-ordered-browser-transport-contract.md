# Encrypted Ordered Browser Transport Contract

`botster-core` owns the transport-neutral contract for encrypted ordered client
stream frames. Production browser transports should use one ordered reliable
WebRTC DataChannel as the canonical browser data plane, carrying Botster E2E
encrypted stream frames whose plaintext is the existing `TransportIngress` or
`TransportEgress` contract.

The concrete transport owns ordered delivery. Core does not implement a reorder
buffer and does not duplicate DataChannel delivery mechanics. Core defines an
ordered-stream validator: the next sequence is accepted, duplicate or replayed
sequences are rejected, gaps or out-of-order sequences are rejected, and closed
streams reject every later frame. These sequence and transcript fields exist for
replay, duplicate, drop, reconnect diagnostic, and cryptographic transcript
validation, not for repairing delivery.

## Protocol Lanes

The encrypted stream is one physical stream and one cryptographic transcript.
Lanes are Botster protocol lanes inside that stream, not separate transports or
separate ratchets. Each lane has its own sequence validator so pressure in one
class of payload cannot create a false integrity failure in another.

Protected lanes are lossless, ordered, and fail-closed:

- `CriticalControl` carries close, integrity, and pressure control frames.
- `TerminalLive` carries live transport ingress and egress such as terminal
  input, output, attach, resize, focus, snapshot requests, and process exits.
- `BulkReplay` carries replay or history payloads. Replay chunks are sequential
  content, so gaps or replays are integrity failures rather than latest-wins
  state.

Lossy lanes may coalesce, rate-limit, or drop older frames without advancing or
breaking protected lanes:

- `TerminalMetadata` carries coalescible terminal metadata such as titles,
  prompt marks, readiness hints, and mode hints.
- `Diagnostics` carries observability payloads that must not block or break
  live terminal flow.

The core validator records typed outcomes: accepted, coalesced, dropped,
rate-limited, or rejected. Coalescing a lossy lane advances only that lane.
Dropping an older lossy frame does not advance anything. Rejection stays reserved
for protected-lane integrity failures or closed protected lanes. The validator
also exposes per-lane counters for accepted, coalesced, rate-limited, dropped,
and rejected outcomes so anti-spam behavior remains observable at the transport
contract boundary.

Terminal metadata must not be encoded as terminal-live egress bytes. Metadata
such as prompt marks, bells, notifications, mode flags, and screen-readiness
hints belongs on `TerminalMetadata`; terminal output bytes belong on
`TerminalLive`; replay/history chunks belong on `BulkReplay`. Producers may
choose shaping thresholds, but they must preserve this lane classification.

These lanes align with the existing SessionIo coalescing and bounded-priority
shape: SessionIo still owns session-worker fanout and queue mechanics, while the
encrypted stream contract preserves the same pressure vocabulary at the client
transport boundary. The contract defines the lanes and outcomes; concrete
shaping thresholds remain host/adapter policy.

Local signaling, cloud signaling, pairing UX, relay choice, TURN/STUN policy,
client admission, hub presence, package policy, and browser persistence are
hub/provider/client concerns. `botster-core` exposes only mechanism-level
identifiers for peers, key ids, transcript ids, storage key ids, pairing state,
sealed frame headers, encrypted payload envelopes, backpressure, close reasons,
and fail-closed validation outcomes.

The core frame format intentionally wraps the existing transport-neutral
payloads rather than defining browser-specific message variants:

- `EncryptedStreamFrame.header` is public routing metadata.
- The same header is duplicated inside the encrypted plaintext.
- Opening a frame authenticates and compares the plaintext header to the public
  header before returning the payload.
- `EncryptedStreamPayload::TransportIngress` carries a real
  `TransportIngress`.
- `EncryptedStreamPayload::TransportEgress` carries a real `TransportEgress`.
- `EncryptedStreamPayload::Control` carries typed pressure or close semantics.

This ticket is contract/mechanics only. Concrete WebRTC adapter wiring,
signaling providers, reconnect policy, and production ratchet selection remain
outside this core slice.
