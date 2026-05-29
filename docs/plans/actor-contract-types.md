# Define botster-core Actor Contract Types

## Context Loaded

- Pipeline context: `ticket_1780014863_508751`, `run_1780026163_694058`, current step `botster_plan`, gate `botster_plan_gate`.
- Review context: `review_1780026518_776572` returned changes required because the first plan lacked a durable artifact, surface inventory, mandatory behavior classification, and named acceptance assertions.
- Worktree: `/Users/jasonconigliari/botster-sessions/trybotster-botster-core-project-pipelines-ticket_1780014863_508751`.
- Target: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`.
- Required playbooks loaded:
  - `/Users/jasonconigliari/knowledge/notes/planner-playbook.md`
  - `/Users/jasonconigliari/knowledge/notes/botster-planner-playbook.md`
- Additional vault constraints loaded:
  - `/Users/jasonconigliari/knowledge/notes/botster-architecture.md`
  - `/Users/jasonconigliari/knowledge/notes/cli-patterns.md`
  - `/Users/jasonconigliari/knowledge/notes/spa-patterns.md`
  - `/Users/jasonconigliari/knowledge/notes/project pipeline orchestration belongs in a device-level botster plugin.md`
  - `/Users/jasonconigliari/knowledge/notes/project pipelines needs an operator workbench not more primitives.md`
  - `/Users/jasonconigliari/knowledge/notes/project pipelines ui contract belongs in the plugin readme.md`
  - `/Users/jasonconigliari/knowledge/notes/botster orchestration should spawn agents with explicit target ids.md`
  - `/Users/jasonconigliari/knowledge/notes/botster orchestration prompts must bind agents to explicit worktrees.md`
  - `/Users/jasonconigliari/knowledge/notes/plan steps need reviewable plan artifacts.md`
- Repo context loaded:
  - `README.md`: core owns reusable mechanisms and transport-neutral contracts; hub owns policy, orchestration, lifecycle, and extension supervision.
  - `src/lib.rs`: exports current contract modules.
  - `src/session.rs`: currently only `SessionId`, `SubscriptionId`, `RequestId`.
  - `src/client.rs`: currently `ClientId`, `ClientScope`, `ClientState`.
  - `src/transport.rs`: currently a small transport ingress/egress subset.
  - `src/boundary.rs`: currently `Layer` and `LayerResponsibility`; no `BoundaryJson`.
  - `tests/boundary_test.rs`: current crate boundary tests.
- Old trybotster evidence loaded:
  - `/Users/jasonconigliari/Rails/trybotster/docs/worker-actor-contracts.md`
  - `/Users/jasonconigliari/Rails/trybotster/docs/architecture-drift-orchestration.md`
  - `/Users/jasonconigliari/Rails/trybotster/cli/src/worker/mod.rs`
  - `/Users/jasonconigliari/Rails/trybotster/cli/src/worker/hub_control.rs`
  - `/Users/jasonconigliari/Rails/trybotster/cli/src/worker/client.rs`
  - `/Users/jasonconigliari/Rails/trybotster/cli/src/worker/session_io.rs`
  - `/Users/jasonconigliari/Rails/trybotster/cli/src/worker/transport.rs`
  - `/Users/jasonconigliari/Rails/trybotster/cli/src/worker/plugin.rs`

## Scope

Build the first runnable actor-contract slice in `botster-core`. The slice defines reusable, serializable contract shapes. It does not define worker runtimes or hub policy.

In scope:

- Hub-control request and event shapes.
- Client-worker message and control-frame shapes.
- Session-I/O request and event shapes.
- Transport ingress and egress frames.
- Plugin-worker message and event shapes.
- Bounded queue metadata.
- Typed backpressure summaries with routing context.
- A `BoundaryJson` newtype explicitly reserved for Lua/plugin/relay payloads.
- Exports from `src/lib.rs` for the new public contract types.
- Tests that prove the ticket acceptance criteria.

Non-scope:

- Tokio worker loops, mailbox senders, runtime handles, or thread/task orchestration.
- WebRTC registry internals, socket frame adapters, ActionCable/Rails relay logic, TUI IPC adapters, or concrete transport implementations.
- Session process protocol implementation, paste temp path resolution, snapshot gzip preparation, coalescing windows, or terminal parser details.
- Hub policy, plugin loading policy, marketplace policy, CLI argument parsing, or executable startup flow.
- Compatibility shims, dual code paths, broad refactors, or copied old trybotster implementation.

## Surface Inventory

| Required surface | Target module | New or changed types | Reuse versus new |
| --- | --- | --- | --- |
| Bounded queue metadata | New `src/actor.rs` or `src/actor/queue.rs` | `BoundedQueueConfig`, queue constants for hub control, client worker, session I/O, transport adapter, plugin worker | New core type; preserve finite-capacity concept from old `worker::BoundedQueueConfig` without Tokio/runtime coupling |
| Typed backpressure summaries | New `src/actor.rs` or `src/actor/backpressure.rs` | `BackpressureSummary`, `BackpressureRoute`, maybe `QueueSource` | New core types; reuse `ClientId`, `SessionId`, `SubscriptionId`; route context must be typed, not a free-form string only |
| Hub-control requests | New `src/actor.rs` or `src/actor/hub_control.rs` | `HubControlMessage`, `HubControlOrigin`, `SessionLifecycleState`, `TransportPeerState`, `TransportConnectionMode`, `TransportDisconnectReason`, `TransportSignal` | New message shapes; reuse `ClientId`, `SessionId`, `SubscriptionId`, `RequestId`, `BoundaryJson` only for relay envelopes |
| Client-worker messages | New `src/actor.rs` or `src/actor/client_worker.rs`; may extend `src/client.rs` only for client-owned enums | `ClientWorkerMessage`, `ClientControlFrame`, `ClientConnectionHealth`, `TerminalAttachState` | New contract shapes; reuse `ClientId`, `SessionId`, `SubscriptionId`, `RequestId`; do not import or mention concrete transport types |
| Session-I/O requests/events | New `src/actor.rs` or `src/actor/session_io.rs`; may add terminal summary structs to `src/session.rs` if they are pure session concepts | `SessionIoRequest`, `SessionIoEvent`, `PasteFileErrorReason`, `PreparedSnapshot` fields | New core summaries translated from old worker contract; keep terminal bytes/snapshots opaque but type routing/control fields. Human clarification for PR #4: terminal mode is Session/Ghostty state-sync/probe-owned, not a server-pushed `ModeChanged` contract. |
| Transport ingress/egress | Existing `src/transport.rs` | Extend `TransportIngress` and `TransportEgress` to cover subscribe, unsubscribe, input, resize, snapshot, focus, heartbeat, terminal bytes, scrollback, process exit, attach state, focus changes, binary, and `BoundaryJson` | Extend existing scaffold; transport may depend inward on session/client/boundary types |
| Plugin-worker messages/events | New `src/actor.rs` or `src/actor/plugin_worker.rs`; possibly `src/plugin.rs` if the implementer prefers a public plugin module | `PluginKey`, `PluginHandlerKind`, `PluginHandlerRef`, `PluginLoadSpec`, `PluginWorkerMessage`, `PluginWorkerEvent` | New contract shapes; preserve handler-ref model, translate paths/runtime execution data into pure metadata, drop Lua closure/function handles |
| Boundary JSON reservation | Existing `src/boundary.rs` | `BoundaryJson(serde_json::Value)` newtype with docs limiting use to Lua/plugin/relay payloads | New type; transport/plugin/relay contracts may use it, session/client core controls must not use raw `serde_json::Value` for stable controls |

Module shape is intentionally flexible within that inventory. A single `actor` module with focused submodules is preferred if it keeps `src/lib.rs` clear and avoids spreading actor terms through unrelated primitives. Any choice must keep core as a contract crate, not a runtime crate.

## Preserve, Translate, Drop Classification

For old trybotster behavior used as evidence, there is no defer category.

| Evidence area | Preserve | Translate | Drop |
| --- | --- | --- | --- |
| `worker/mod.rs` queue metadata | Preserve bounded mailboxes with finite named capacities. | Translate `usize` capacity and `is_bounded` into a core metadata type and constants that do not require Tokio. | Drop tests that `include_str!` old runtime files and any sender/runtime handle behavior. |
| `worker/hub_control.rs` | Preserve hub-owned mutation boundary: attach, detach, snapshot, lifecycle, reconnect, shutdown, transport peer state/signal, transport backpressure. | Translate old `SessionUuid = String` aliases to `SessionId`; old browser identity strings become generic relay/peer routing strings only where needed. | Drop actual hub mutation handling and WebRTC-specific implementation details. |
| `worker/client.rs` | Preserve client worker as transport-neutral stream state and typed terminal/control delivery. Preserve attach states and connection health. | Translate session I/O sender registration into contract-level routing intent only, not a Tokio sender. Translate old control frames into serializable core summaries. | Drop `ClientWorkerHandle`, runtime config structs, HashMaps, `tokio::sync::mpsc`, and message-processing behavior. |
| `worker/session_io.rs` | Preserve PTY input, resize, snapshot, initial snapshot intent, terminal subscription intent, paste file, prepared snapshot, mode/screen/color/shutdown, terminal output/event vocabulary. | Translate terminal protocol structs into minimal core-owned summaries or opaque bytes where parser detail is not stable. Use `RequestId` and `SessionId` for correlation/routing. | Drop paste path resolution, gzip preparation, OSC filtering, timers, coalescing, actual socket read/write behavior, and worker delivery handles. |
| `worker/transport.rs` | Preserve transport adapter as the only concrete framing boundary and stable typed ingress before JSON fallback. | Translate existing `src/transport.rs` variants into the fuller actor contract while keeping it transport-neutral. Use `BoundaryJson` for Lua/plugin/relay-owned payloads. | Drop socket/TUI frame adapters, concrete `Frame`, `TuiRequest`, `TuiOutput`, and WebRTC adapter commands. |
| `worker/plugin.rs` | Preserve per-plugin worker boundary, stable plugin keys, stable handler refs, load/invoke/shutdown, loaded/failed/completed/backpressure/stopped events. | Translate `PathBuf` and loader source data into pure serializable metadata if included; keep handler refs typed by capability family. | Drop Lua VM execution, `mlua::Function`, supervisor behavior, timers/watchers/runtime callback plumbing. |
| Architecture guardrail tests | Preserve intent: no transport leak into session/client, BoundaryJson limited, stable controls typed. | Translate into `botster-core` tests over public types, serde JSON, and source-level dependency direction where useful. | Drop static old-repo include checks tied to trybotster paths. |

## Boundary Rules

- Core defines message shapes only. Hub, session workers, client workers, plugin workers, and adapters own handling.
- Dependency direction is one-way: `transport` may import `session`, `client`, and `boundary`; `session` and `client` must not import `transport`.
- Session/client contracts must not contain concrete transport names or types such as WebRTC, browser, socket, TUI, ActionCable, Rails, or DataChannel.
- `BoundaryJson` must be introduced in `src/boundary.rs` as a documented wrapper around `serde_json::Value`.
- `BoundaryJson` is allowed only for Lua/plugin/relay-owned payload exceptions. It must not be used to represent stable hub/client/session controls that this ticket is supposed to type.
- Human clarification implemented in commit `0217733`: terminal mode/color state is owned by Session/Ghostty synced state and client probing, not by a server-pushed `ModeChanged` actor or transport event.
- `FocusChanged` remains a pushed event because focus reporting is an interactive PTY-emitted stream event, while terminal mode/color is durable terminal state that clients probe from the synced Session/Ghostty model.

## Assumptions and Unknowns

Assumptions:

- This is a scaffold/contract slice with no production runtime entry point inside `botster-core` yet.
- Public wire contracts should derive `Debug`, `Clone`, `PartialEq`, `Eq` where possible, plus `Serialize` and `Deserialize`.
- Existing dependencies are sufficient; do not add crates.
- Old trybotster paths are evidence only and must not be imported, included, copied, or made runtime dependencies.

Unknowns for implementation:

- Whether the cleanest shape is `src/actor.rs` or `src/actor/*.rs`. The implementer should choose the smallest readable layout that fits this inventory.
- Whether relay `TransportSignal` should carry `BoundaryJson` or a narrower envelope type. Prefer the narrowest type that still represents the old relay exception without leaking Rails/WebRTC policy.

## Affected Files

Expected:

- `src/lib.rs`
- `src/boundary.rs`
- `src/transport.rs`
- `src/session.rs` if adding pure terminal/session summaries
- `src/client.rs` if adding pure client state summaries
- New `src/actor.rs` or new `src/actor/*.rs`
- New `tests/actor_contract_test.rs` or extensions to `tests/boundary_test.rs`
- This plan file

Not expected:

- `Cargo.toml` unless existing dependencies prove insufficient, which is not expected.
- Runtime crates, old trybotster source, or generated files.

## Risks

- Over-porting old runtime internals would put hub/session/plugin execution policy into core.
- Under-typing stable controls by using `serde_json::Value` would fail the ticket intent.
- Concrete WebRTC/browser/socket/TUI terms in session/client contracts would make core transport-specific.
- A `BoundaryJson` wrapper without tests/docs could become a broad escape hatch.
- Type additions without exports or serde-level tests would be hard for downstream crates to consume.
- Opaque terminal payloads are valid for bytes/snapshots, but routing and control metadata must remain typed.

## Acceptance Checks and Tests

Run `cargo test` after implementation. Add named assertions that map one-to-one with the ticket acceptance criteria:

1. Queue configs are bounded.
   - Test name: `actor_queue_configs_are_bounded`.
   - Assertions: every public queue constant has `capacity > 0`; `BoundedQueueConfig::new("test.unbounded", 0).is_bounded()` is false or rejected; expected queue names include hub-control, client-worker, session-I/O, transport-adapter, and plugin-worker.

2. Stable controls are typed.
   - Test name: `stable_hub_and_client_controls_are_typed`.
   - Assertions: instantiate representative `HubControlMessage`, `ClientWorkerMessage`, `SessionIoRequest`, and `TransportIngress` variants for attach/subscribe/input/resize/snapshot/focus/heartbeat without stringly control names or raw JSON.
   - Include serde round trips for representative serializable values.

3. Backpressure has routing context.
   - Test name: `backpressure_summary_round_trips_with_route_context`.
   - Assertions: serialize and deserialize a backpressure summary carrying queue source, capacity, `SessionId`, `ClientId`, and `SubscriptionId`; assert all route fields survive.

4. `BoundaryJson` is reserved for Lua/plugin/relay payloads.
   - Test name: `boundary_json_is_reserved_for_lua_plugin_or_relay_payloads`.
   - Assertions: `BoundaryJson` type exists in `boundary`; transport/plugin/relay exception variants use `BoundaryJson`; stable control variants for hub/client/session use typed fields instead of raw `serde_json::Value`.

5. No transport-specific types leak into session/client contracts.
   - Test name: `session_and_client_contracts_do_not_depend_on_transport`.
   - Assertions: source-level guard checks `src/session.rs`, `src/client.rs`, and actor session/client modules do not import `crate::transport`; rendered type/debug names for session/client contract values do not contain `WebRtc`, `Browser`, `Socket`, `Tui`, `ActionCable`, `Rails`, or `DataChannel`.

6. Plugin workers use stable handler refs, not Lua functions.
   - Test name: `plugin_worker_messages_use_handler_refs`.
   - Assertions: instantiate load/invoke/shutdown and loaded/completed/backpressure events; rendered/debug output contains `PluginHandlerRef` and does not contain `mlua` or `Function`.

Runtime path evidence:

- This ticket is intentionally scaffold-only in `botster-core`. There is no production runtime entry point in this crate yet.
- The changed user path is downstream compile-time consumption: hub/client/session/plugin runtime crates will import these exported contract shapes rather than duplicating or inventing local message shapes.
- The proof for this slice is exported public types plus serde/runtime tests that instantiate and round-trip the contracts without concrete transport leakage.

## Vault Gaps Worth Capturing

No new durable vault knowledge is required before implementation. Existing notes already cover:

- core versus hub policy boundaries
- session/client actor data-plane ownership
- plugin worker handler refs
- `BoundaryJson` as a limited exception
- pipeline artifact discipline

Capture later only if implementation discovers a reusable rule for terminal mode/color representation in `botster-core` that is not already covered by the vault.
