# Session Process Wire Protocol Plan

Ticket: Extract session process wire protocol specification
Run: run_1780026164_128656

## Context Loaded

- Pipeline context: ticket `ticket_1780014864_617311`, run `run_1780026164_128656`, current step `botster_plan`, gate `botster_plan_gate`, prior Plan Review `review_1780026500_393556`, and open findings `finding_1780026500_999329`, `finding_1780026500_141400`, and `finding_1780026500_771963`.
- Required playbooks and vault notes loaded:
  - `planner-playbook`
  - `botster-planner-playbook`
  - `botster-architecture`
  - `cli-patterns`
  - `spa-patterns`
  - `project pipeline orchestration belongs in a device-level botster plugin`
  - `project pipelines needs an operator workbench not more primitives`
  - `project pipelines ui contract belongs in the plugin readme`
  - `botster orchestration should spawn agents with explicit target ids`
  - `botster orchestration prompts must bind agents to explicit worktrees`
  - `plan steps need reviewable plan artifacts`
  - `botster data plane bypasses the hub through session and client actors`
  - `identity`
  - `goals`
- Closed dependencies reflected:
  - `ticket_1780014899_889542`: core/hub/client/provider boundary README. Current README says `botster-core` owns reusable mechanisms and transport-neutral contracts; hub owns policy, orchestration, lifecycle, and extension supervision.
  - `ticket_1780014900_475830`: extraction compatibility policy. The plan artifact from commit `c01bd8c` defines `preserve`, `translate`, and `drop`, with no defer bucket. This ticket should preserve/translate the reusable protocol contract and drop hub recovery policy from core.
- Current repo context:
  - `src/session.rs` currently contains only `SessionId`, `SubscriptionId`, and `RequestId`.
  - `src/transport.rs` already has transport-neutral ingress/egress frames including opaque snapshots.
  - `src/lib.rs` exports current modules and identifiers.
  - `README.md` documents the `core = reusable mechanisms and contracts` boundary.
  - `tests/boundary_test.rs` is the only current test file.
- Reference evidence inspected from `/Users/jasonconigliari/Rails/trybotster`:
  - `cli/src/session/protocol.rs`
  - `cli/src/session/connection.rs`
  - `cli/src/session/tests.rs`
  - `docs/binary-terminal-snapshot.md`

Old TryBotster paths are reference evidence only. Implementation must re-express the stable core contract in this crate without wholesale porting hub/session worker implementation.

## Architecture Decision

Add a transport-neutral session-process protocol surface to `botster-core`.

The protocol belongs in core because frame constants, handshake shape, length-prefixed encoding, serializable metadata, terminal mode state, process-exit payloads, and opaque snapshot transport are reusable contracts shared by future hub, CLI/session process, TUI, browser, and provider code. The old concrete socket connection, SessionIoWorker routing, PTY parser mutation, reconnect policy, and hub recovery behavior do not belong in core.

The implementation should use existing crate dependencies only: `serde`, `serde_json`, and `thiserror`. Do not add `anyhow`, `log`, terminal parser crates, Ghostty/restty bindings, Tokio, or Unix socket dependencies for this slice.

Protocol version behavior should align with the extraction compatibility policy: expose and preserve peer protocol versions as contract data, reject malformed handshakes explicitly, and leave migration/negotiation policy to callers. Core should not introduce compatibility shims for old implementation details.

## Scope

In scope:

- Add a focused protocol module, preferably `src/session_protocol.rs`, and export it from `src/lib.rs`.
- Preserve existing `src/session.rs` identifier types unless the implementer finds a very small, local reason to add metadata there. Avoid overloading `transport.rs` with session-process protocol details.
- Define public constants:
  - `PROTOCOL_VERSION = 2`
  - `HELLO_MAGIC = b"SPH1"`
  - `WELCOME_MAGIC = b"SPA1"`
  - `MAX_METADATA_LEN = 64 * 1024`
  - `MAX_FRAME_LEN = 128 * 1024 * 1024`
  - frame type bytes `0x01` through `0x16`
- Define serializable contract structs:
  - `SessionMetadata`
  - `ModeFlags`
  - `ModeChanged`
  - `NotificationPayload`
  - `PromptMarkPayload`
  - `TerminalColorProfile`
  - `Rgb`
  - `ProcessExitedPayload`
  - small request payload structs where they clarify tests: resize, tee, timeout
- Define frame helpers:
  - `Frame`
  - `FrameDecoder`
  - `ProtocolError`
  - `encode_frame`
  - `encode_empty`
  - `encode_string`
  - `encode_json`
  - `Frame::json`
- Define handshake helpers that can be tested without Unix sockets:
  - either read/write helpers over `impl Read + Write`, or pure encode/decode helpers plus IO wrappers
  - helpers must expose the peer protocol version and enforce metadata bounds
- Add focused Rust tests under `tests/session_protocol_test.rs`.
- Update `README.md` only if needed to make the public crate boundary discoverable.

## Non-Scope

- No hub recovery policy, retry policy, reconnect admission, process discovery, manifest reconciliation, target scoping, or lifecycle cleanup.
- No concrete `SessionConnection`, `SessionIoWorker`, Unix socket, Tokio, or client worker implementation.
- No PTY handling, parser resize mutation, terminal import/export, Ghostty/restty snapshot parsing, or terminal state comparison.
- No Rails, ActionCable, WebRTC, TUI, React SPA, Project Pipelines plugin, provider, or hosted preview changes.
- No legacy compatibility adapter. Translate evidence into the new core contract and drop old hub policy from this crate.

## Assumptions and Unknowns

Assumptions:

- The active worktree is the pipeline-assigned implementation target for `trybotster/botster-core`.
- The current branch is behind `origin/main`, but the dependency plan documents were inspected from their commits. This plan should not merge unrelated branch history just to create the plan artifact.
- `PROTOCOL_VERSION` remains `2` because the old evidence and current ticket both point at the same session-process protocol generation.
- The frame length prefix is little-endian `u32` and includes one byte of frame type plus payload bytes.
- Snapshot payloads are arbitrary opaque bytes. Core frame tests should prove byte-identical round trips only.
- The frame cap remains 128 MiB to allow large binary terminal snapshots; metadata remains capped at 64 KiB.
- `TerminalColorProfile` should use a small core `Rgb` struct rather than depending on the old terminal crate type.

Unknowns for implementation to resolve narrowly:

- Whether `FrameDecoder::feed` returns `Result<Vec<Frame>, ProtocolError>` or stores an error/desync state. The acceptance requires bad headers to fail explicitly, so silent discard-only behavior is not acceptable.
- Whether handshake helpers should be IO-first or pure-buffer-first. Prefer the smallest API that proves the wire bytes and metadata bound in tests.
- Whether `README.md` needs a new bullet naming session-process protocol. If rustdoc and exports are sufficient, README can stay unchanged.

## Affected Surfaces and Files

Expected changes:

- `src/session_protocol.rs`: new core protocol constants, payload structs, encoder/decoder, errors, and handshake helpers.
- `src/lib.rs`: export the new module and selected public protocol types/functions.
- `tests/session_protocol_test.rs`: acceptance tests mapped below.
- `README.md`: optional one-line boundary update if needed for discoverability.

Existing surfaces to preserve:

- `src/session.rs`: current identifier newtypes remain valid.
- `src/transport.rs`: existing transport-neutral `Snapshot` egress stays a client transport contract. Do not duplicate concrete client routing here.
- `tests/boundary_test.rs`: current boundary guardrails remain unchanged unless README boundary wording changes.

Reference-only sources:

- `/Users/jasonconigliari/Rails/trybotster/cli/src/session/protocol.rs`
- `/Users/jasonconigliari/Rails/trybotster/cli/src/session/connection.rs`
- `/Users/jasonconigliari/Rails/trybotster/cli/src/session/tests.rs`
- `/Users/jasonconigliari/Rails/trybotster/docs/binary-terminal-snapshot.md`

## Acceptance Mapping

| Ticket criterion | Planned test name | Command |
| --- | --- | --- |
| Handshake | `handshake_round_trips_magic_version_and_metadata` | `cargo test handshake_round_trips_magic_version_and_metadata` |
| Frame constants | `frame_constants_match_session_process_wire_spec` | `cargo test frame_constants_match_session_process_wire_spec` |
| Length-prefixed encoding/decoding | `frame_round_trips_binary_string_empty_and_json_payloads` | `cargo test frame_round_trips_binary_string_empty_and_json_payloads` |
| Partial frame decoding | `decoder_buffers_split_header_and_payload_until_complete` | `cargo test decoder_buffers_split_header_and_payload_until_complete` |
| Multiple frame decoding | `decoder_drains_multiple_frames_from_one_feed` | `cargo test decoder_drains_multiple_frames_from_one_feed` |
| Bad headers fail explicitly | `decoder_rejects_zero_length_header` and `decoder_rejects_oversized_header` | `cargo test decoder_rejects_` |
| Desync handling | `decoder_reports_desync_after_repeated_bad_headers` | `cargo test decoder_reports_desync_after_repeated_bad_headers` |
| Metadata bounds enforced | `handshake_rejects_metadata_over_64k` | `cargo test handshake_rejects_metadata_over_64k` |
| Metadata | `session_metadata_round_trips_optional_recovery_identity_and_mode_flags` | `cargo test session_metadata_round_trips_optional_recovery_identity_and_mode_flags` |
| Mode flags | `mode_flags_round_trip_all_fields` | `cargo test mode_flags_round_trip_all_fields` |
| Mode changes | `sparse_mode_changed_omits_unchanged_fields` | `cargo test sparse_mode_changed_omits_unchanged_fields` |
| Mode replay representable | `mode_changed_from_flags_populates_every_replay_field` | `cargo test mode_changed_from_flags_populates_every_replay_field` |
| Color profile | `terminal_color_profile_serializes_core_rgb_map` | `cargo test terminal_color_profile_serializes_core_rgb_map` |
| Process exit | `process_exit_payload_supports_code_and_signal_absence` | `cargo test process_exit_payload_supports_code_and_signal_absence` |
| Protocol-version behavior | `handshake_exposes_peer_protocol_version_without_policy_negotiation` | `cargo test handshake_exposes_peer_protocol_version_without_policy_negotiation` |
| Snapshot payloads round-trip byte-identically as opaque blobs | `snapshot_frame_round_trips_opaque_bytes_without_parsing` | `cargo test snapshot_frame_round_trips_opaque_bytes_without_parsing` |
| Hub recovery policy remains out of scope | `README`/rustdoc review plus absence of hub recovery modules in diff | `cargo test` and review diff |

Full verification command:

- `cargo test`

Targeted verification command:

- `cargo test session_protocol`

## Verification

Implementation should run:

- `cargo test`
- `cargo test session_protocol`

Review should additionally confirm:

- No new runtime dependency beyond existing crate dependencies.
- No Unix socket, Tokio, PTY, hub recovery, client worker, Ghostty/restty, Rails, ActionCable, WebRTC, TUI, or SPA code entered `botster-core`.
- Bad header handling is explicit and test-covered.
- Snapshot tests assert opaque byte equality rather than terminal semantics.
- Metadata-bound tests exercise the parser/handshake boundary, not only struct construction.

Baseline before this plan artifact:

- `cargo test` passed with the existing 2 boundary tests.

## Runtime or User Path Evidence

This ticket is intentionally a core extraction/specification slice.

The production runtime path is not rewired in this step. The proof path is:

- public `botster_core` exports for the session-process protocol;
- tests exercising those exported functions/types directly;
- crate docs or README text stating that hub/CLI/session-process code should consume these contracts later.

That scaffold-only boundary is intentional because the ticket target is `botster-core`, while the concrete session process runtime still lives in the full TryBotster application. Wiring the full app to the extracted crate would be a separate integration ticket.

## Risks

- Copying old TryBotster code wholesale would import hub/session worker policy, `anyhow`, `log`, terminal-specific types, and socket implementation into core. Mitigation: re-express only constants, payload contracts, frame codec, errors, and handshake helpers.
- Bad-header handling in old evidence discarded headers until a desync threshold. The ticket requires explicit failure. Mitigation: make malformed lengths return typed errors and add negative tests.
- Protocol-version behavior can accidentally become hidden policy. Mitigation: expose peer version and leave compatibility decisions to callers, consistent with the extraction compatibility policy.
- Snapshot wording can overreach into Ghostty internals. Mitigation: core treats snapshots as opaque frame payloads only.
- Metadata bounds can be tested too shallowly. Mitigation: test handshake/parser rejection of oversized metadata bytes.
- Placing protocol code in `transport.rs` could blur client transport frames with session-process wire protocol. Mitigation: prefer a focused module.

## Vault Gaps Worth Capturing

No new durable vault note is required before implementation.

Capture after implementation if the final API establishes a reusable convention worth keeping:

- `botster-core` owns session-process wire protocol constants, payload contracts, length-prefixed framing, and handshake helpers.
- Hub/session actors own socket lifecycle, recovery policy, routing, backpressure, and cleanup.
- Bad frame headers in extracted core protocol fail explicitly instead of being silently discarded as an implementation detail.
