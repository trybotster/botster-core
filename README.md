# botster-core

`botster-core` is the embeddable, programmable, tmux-like local engine for
Botster hosts.

It is intentionally not the Botster application, not the hub, and not the CLI.
It contains the reusable local execution mechanics, typed contracts, and
policy-free engine facades that every Botster host, client, provider, and
plugin runtime must agree on.

## Workspace Layout

- `crates/botster-core`: production contract and engine crate.
- `crates/botster-core-test-support`: dev-dependency fixtures, fakes, and
  conformance helpers for consumers pinned to the same core version.
- `crates/botster-core-dev`: dev-only engine smoke harnesses that run an
  explicit local command through the public default command API for core
  development; not the product CLI, install UX, auth flow, hub daemon,
  marketplace, or persistent config surface.
- `crates/botster-terminal-ghostty`: feature-gated crate for the blessed
  Ghostty shadow-terminal adapter path. It depends on the backend-neutral core
  terminal seam, owns the trybotster Ghostty fork pin under `vendor/ghostty`,
  and builds static `libghostty-vt` only when the `libghostty-vt` feature is
  explicitly enabled.

## What Core Proves Today

The current crate proves the reusable engine pieces that make Botster embeddable:

- typed session, client, subscription, request, transport, entity, UI, package,
  crypto, identity, notification, terminal snapshot, and plugin worker contracts
- default local session mechanics through `SessionWorkerEngine`
- subscription fanout through `SubscriptionMultiplexer`
- session registry, activity/lifecycle observations, typed notification inbox,
  and plugin worker invocation through `MultiplexerEngine`
- the synchronous `BotsterEngine` facade for consumers that want tmux-like
  operations instead of raw transport-frame plumbing
- consumer test harnesses and fakes in `botster-core-test-support`

This documentation reframe does not move product policy into core. It clarifies
that hosts embed the local engine while the hub or other products still own auth,
persistence policy, config locations, cloud federation, marketplace and
install/update policy, WebRTC/signaling/API adapters, UI, and workflow-specific
behavior.

## Embedding Path

Use `BotsterEngine` when a host wants method-level operations for the common
tmux-like path: spawn a session, attach clients, write and resize terminal
streams, receive output, drain notifications, invoke plugin handlers, classify
activity, and shut sessions down. The rustdoc example on `BotsterEngine` is
compile-checked against the public facade, and `crates/botster-core-dev` mirrors
that path with a deterministic smoke harness.

The policy-free command surface is specified in
`docs/architecture/engine-command-surface.md`. It names `BotsterEngine` as the
canonical command facade, keeps `MultiplexerEngine` as the lower-level assembled
primitive, and maps supported commands to existing typed request, result, event,
and error shapes.

`DefaultBotsterEngine` is available through the default `local-runtime` feature
for embedders that want the policy-free local PTY/process path without writing
their own `SessionRuntime`. The feature is default-on to preserve the current
crate behavior for local hosts:

```toml
botster-core = "0.1.0"
```

Contract-only embedders can opt out of the local process dependency and keep the
public contracts, `BotsterEngine`, runtime traits, package, identity, UI, entity,
transport, and plugin-worker types:

```toml
botster-core = { version = "0.1.0", default-features = false }
```

With default features disabled, `LocalProcessRuntime`,
`LocalProcessWorkerRuntime`, `LocalProcessRuntimeOptions`,
`DefaultBotsterEngine`, and `DefaultBotsterEngineError` are intentionally not
exported, and `portable-pty` is absent from the production dependency tree.

## Local Verification

Run the same workspace checks used by pull request and main-branch CI before
opening a pull request:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test --doc --workspace
cargo doc --workspace --no-deps
```

On Unix hosts, also run the local PTY acceptance test:

```sh
cargo test -p botster-core --test local_process_runtime_test
```

The PTY test file is Unix-gated so unsupported platforms skip it at compile
time. CI runs that targeted check only on the Linux runner and does not depend
on hub, Rails, WebRTC, TUI, browser, marketplace, cloud, or Project Pipelines
runtime setup.

Release verification also builds the whole workspace in release mode on pushes
to `main` and on manual workflow dispatch:

```sh
cargo build --workspace --release
```

## Consumer Test Support

`botster-core-test-support` is the version-coupled test surface for downstream
crates that depend on `botster-core`. Add it only under `dev-dependencies`, at
the same release version as `botster-core`, when consumer tests need shared
fixtures, in-memory fakes, or conformance assertions for the public core
contracts.

The support crate is intentionally publishable so downstream crates can depend
on the matching released version. It must not be required by production builds,
and it must not carry hub policy, CLI startup behavior, renderer assumptions,
auth, provider marketplace behavior, Project Pipelines product behavior, or
other product-specific flows.

For embedders that need to prove the released local engine path, the support
crate also exposes a managed local conformance harness. It spawns explicit
test commands through `ManagedSessionRuntime<LocalProcessRuntime>`, attaches
fake clients with public transport frames, drains real PTY output through
`ManagedSessionRuntime::drain_runtime_once`, and provides assertions for
fanout, activity, and shutdown semantics. PTY-dependent tests should be
Unix-gated or use the harness skip reason on unsupported hosts.

## Terminal Screen And Snapshot Boundary

`botster-core` exposes a narrow terminal screen boundary for hosts that need
to normalize terminal output, capture or replay opaque snapshots, and read
synchronous screen state through a small runtime adapter. The boundary is
defined by `TerminalScreenEngine`, `TerminalScreenRuntime`,
`TerminalOutputChunk`, `TerminalSnapshotPayload`, and `TerminalScreenState`.

The Ghostty adapter audit lives in
`docs/architecture/ghostty-shadow-terminal-adapter.md`; it keeps Ghostty and
Zig build policy in a future optional adapter crate rather than in core.

Snapshot payload bytes are opaque. Core records dimensions and an optional
host-owned format label, but it does not parse terminal cells or decide which
terminal backend a host should use. Botster's blessed authoritative
shadow-terminal backend path is Ghostty, housed in the sibling
`botster-terminal-ghostty` crate so concrete native parser policy stays out of
`botster-core`.

restty is a web/client renderer path. It may consume terminal state and streams
through client data-plane contracts, but it is not core shadow-terminal
infrastructure and must not own authoritative terminal truth.

`botster-terminal-ghostty` pins the trybotster Ghostty fork at
`76853b34274208fe7c051cfe13eb1c7ee63c469b`. Default workspace builds do not
require the submodule or Zig. To exercise the native path, initialize the
submodule and enable the feature:

```sh
git submodule update --init crates/botster-terminal-ghostty/vendor/ghostty
cargo test -p botster-terminal-ghostty --features libghostty-vt
```

Feature-enabled builds require Zig `0.15.2` and run
`zig build -Demit-lib-vt -Doptimize=ReleaseFast -Dsimd=false -Dcpu=baseline -Dversion-string=1.3.2-dev`.
The vendored Ghostty fork is MIT licensed; preserve
`crates/botster-terminal-ghostty/vendor/ghostty/LICENSE` in source or binary
distributions that include it.

Correlated delivery still uses the existing session-worker carriers such as
`SnapshotReady`, `PreparedSnapshotRequest`, `PreparedSnapshotReady`, and
`ScreenReady`. `TerminalSnapshotPayload` is only the reusable, correlation-free
value a runtime adapter can convert into those carriers.

## Ownership Boundary

This crate documents contracts the current code proves. It is not a parking
lot for future hub, client, cloud, or plugin behavior.

`BotsterEngine` is the ergonomic public facade for hosts that want to embed
Botster like a tmux-like library. It provides method-level operations for
spawning sessions, attaching and detaching clients, writing terminal bytes,
resizing, receiving output frames, posting and draining notifications, invoking
plugin handlers, classifying activity, and shutting sessions down. It composes
the lower-level `MultiplexerEngine`, which remains available for embedders that
need direct transport-frame control. Hosts may provide their own
`SessionRuntime`, or use `DefaultBotsterEngine` as the default local PTY-backed
facade for explicit spawn requests.

The facade is intentionally synchronous and policy-free. Hosts still choose
executables, working directories, environment inheritance, auth, persistence,
retention, reconnect rules, concrete transports, plugin installation policy,
notification presentation, and any async supervision. `DefaultBotsterEngine`
wires `LocalProcessRuntime` through the managed session worker and subscription
fanout path, but it does not discover product config, select default commands,
or mutate requests; it only runs the executable, arguments, directory,
environment, and PTY size the host already supplied. Shutdown terminates the
direct child process owned by the runtime; process-tree or process-group
escalation is left to a later supervision layer. Core returns typed
outcomes such as client egress frames, session worker requests/events,
notification items, plugin invocation results, and activity/lifecycle
observations for the host to deliver or persist.

`LocalProcessRuntime` remains only the process adapter. Hosts that use
`MultiplexerEngine` directly still supply a `SessionWorkerRuntime` for terminal
snapshots, screen state, and worker event delivery. `DefaultBotsterEngine`
provides the default managed bridge for local PTY output, input, resize,
activity, and shutdown without exposing the internal worker adapter.

| Layer | Owns | Does not own | Current proof |
| --- | --- | --- | --- |
| Core | Reusable mechanisms and transport-neutral contracts: session, client, subscription, and request identifiers; session-process protocol constants, handshake bytes, frame payload contracts, and length-prefixed framing; terminal ingress/egress frames; a default local PTY-backed `SessionRuntime` adapter and `DefaultBotsterEngine` managed facade for explicit spawn requests; plugin worker handler refs, descriptors, invocation, lifecycle, cleanup, and pressure events; entity frames; UI node shapes; package, capability, extension, crypto, identity contracts, the synchronous `MultiplexerEngine` assembled primitive, and the ergonomic `BotsterEngine` facade that coordinates those mechanisms for embedders. | Runtime policy, executable startup or selection, product workflows, product-specific concrete adapters, device persistence policy, executable plugin callbacks, async supervision, notification presentation, or raw private key material. | `crates/botster-core/src/contract/boundary.rs`, `crates/botster-core/src/contract/actor.rs`, `crates/botster-core/src/contract/session.rs`, `crates/botster-core/src/contract/session_protocol.rs`, `crates/botster-core/src/contract/client.rs`, `crates/botster-core/src/contract/transport.rs`, `crates/botster-core/src/contract/entity.rs`, `crates/botster-core/src/contract/ui.rs`, `crates/botster-core/src/runtime/local_process.rs`, `crates/botster-core/src/engine/botster.rs`, `crates/botster-core/src/engine/multiplexer.rs`, `crates/botster-core/src/package/manifest.rs`, `crates/botster-core/src/package/capability.rs`, `crates/botster-core/src/package/extension.rs`, `crates/botster-core/src/identity/crypto.rs`, `crates/botster-core/src/identity/device.rs`, `crates/botster-core/src/identity/keyring.rs` |
| Hub | Runtime policy, lifecycle, routing, recovery, and extension supervision. | Raw terminal byte delivery, CLI argument parsing, React/TUI rendering, Rails/cloud/Auth policy, Project Pipelines/GitHub/Cloudflare product logic, or legacy compatibility paths. Terminal bytes are represented by core frames and should flow through session/client data-plane actors, not hub policy loops. | `Layer::Hub` responsibility text in `crates/botster-core/src/contract/boundary.rs`; terminal byte exclusions are reinforced by `TransportIngress::TerminalInput` and `TransportEgress::TerminalOutput` in `crates/botster-core/src/contract/transport.rs` |
| CLI | Operator commands and process startup. `crates/botster-core/src/contract/boundary.rs` also names CLI argument parsing as something the hub does not own. | Reusable protocol contracts, hub runtime policy, provider policy, or UI/product behavior. | `Layer::Cli` and `Layer::Hub` responsibility text in `crates/botster-core/src/contract/boundary.rs` |
| Client | Presentation, local input, concrete transport adaptation, liveness reporting, and rendering of core UI/entity contracts. | Session lifecycle policy, hub supervision, provider authority, concrete WebRTC negotiation policy in core, or product-specific workflow state. | `crates/botster-core/src/contract/client.rs`, `crates/botster-core/src/contract/transport.rs`, `crates/botster-core/src/contract/entity.rs`, `crates/botster-core/src/contract/ui.rs` |
| Provider/plugin | `Layer::Extension` behavior described by package manifests, `ExtensionKind::Plugin` or `ExtensionKind::Provider`, entrypoints, and granted capabilities such as client admission, signaling relay, hub presence, or browser shell. | Implicit hub internals, private key material, marketplace/update policy, Rails/cloud/Auth implementation in core, or bypassing capability declarations. Providers are privileged extension packages, not a separate `Layer::Provider` variant. | `Layer::Extension` responsibility text in `crates/botster-core/src/contract/boundary.rs`, plus `crates/botster-core/src/package/manifest.rs`, `crates/botster-core/src/package/extension.rs`, `crates/botster-core/src/package/capability.rs`, and `crates/botster-core/tests/boundary_test.rs` |

## Explicit Ban List

The following behavior does not belong in `botster-core`:

- hub policy
- CLI startup
- Rails/cloud/Auth implementation
- concrete WebRTC negotiation policy
- React/TUI rendering
- Project Pipelines/GitHub/Cloudflare product logic
- legacy compatibility paths
- device config files, OS keychain or file-fallback persistence, operator prompts, or signing-key storage policy

## BoundaryJson Escape Hatches

`BoundaryJson` is reserved for payloads whose schema is owned outside
`botster-core`. Current public actor and transport contracts allow it only for
relay-owned signaling/envelope data, plugin-owned descriptors and metadata, and
plugin-owned handler request/response payloads.

Stable Botster-owned controls must stay typed: terminal attach state, ping/pong,
focus, terminal input, resize, snapshot requests, scrollback, process exit,
client health/state, session lifecycle, and backpressure use typed variants or
fields. Terminal mode and kitty keyboard state are represented by
`ModeFlags`/session-protocol probing; the legacy pushed mode-change frame is not
a raw actor-control payload in this crate.

Every public actor/transport `BoundaryJson` use is classified with owner and
reason metadata in `tests/actor_contract_test.rs`. That test suite also asserts
the exact allowed inventory so a new stable Botster control cannot silently
adopt raw JSON.

## Crypto And Identity Surface

Core owns the reusable AES-GCM envelope utility surface: encryption,
decryption, and the shared serialized `nonce`/`ciphertext`/`version` shape.
Rails and relay code treat encrypted envelopes as opaque transport payloads.

Core also owns public device metadata and fingerprint helpers. Fingerprints are
derived from public verifying key bytes only; deserialized metadata should use
the verification helper before treating a fingerprint as an identity anchor.

Non-exportable signing and credential-store types are boundary contracts here.
CLI and provider packages own runtime credential policy, keychain or file
persistence, operator prompts, and signing-key storage.

The AES-GCM implementation intentionally uses the latest stable `aes-gcm`
release line (`0.10.x`) instead of the newer release-candidate line because no
RC-only feature is required for this core surface.

## Migration Guidance

Every extraction decision must be classified as preserve, translate, or drop.
There is no defer category.

### Preserve

Preserve contracts already represented in this crate: layer responsibility
names, session/client/subscription/request identifiers, session-process
protocol constants, handshake bytes, frame payload contracts, length-prefixed
framing, transport-neutral ingress and egress frames, client liveness and scope
shapes, entity frames, minimal UI node kinds, package manifests, extension
metadata, capability surfaces, and narrow crypto/identity operation requests.

### Translate

Translate concrete runtime implementations into core contracts only after the
current code proves a stable cross-layer shape. Examples include converting a
client adapter behavior into a transport-neutral frame or converting a
provider need into an explicit capability declaration.

### Drop

Drop behavior that is application policy, product integration, historical
compatibility, or executable wiring. Do not preserve it in core behind a new
compatibility branch. Hub policy belongs in the hub, CLI startup belongs in the
CLI, client rendering belongs in clients, and product workflows belong in
plugins or providers.

## Extraction Compatibility Policy

Extraction compatibility for `botster-core` has only three verdicts:
`preserve`, `translate`, and `drop`. There is no defer bucket. If old
behavior is outside this crate's boundary, future extraction tickets may delete
the old expectation or exclude it from core instead of fossilizing accidental
coupling.

Preserve means the contract is reusable core surface and must remain stable.
Translate means the old name or call path maps to a current core contract, but
the legacy surface is not carried forward. Drop means the behavior belongs to
the hub, CLI, a client, a plugin, or a product layer rather than this crate.

| Path | Verdict | Policy |
| --- | --- | --- |
| Transport-neutral identifiers, ingress and egress frames, entity frames, UI contract shapes, package manifests, capabilities, extension metadata, and narrow crypto or identity operation contracts | preserve | These are the reusable cross-client contracts `botster-core` exists to carry. |
| `context.json` migration | drop | Legacy migration belongs to hub or CLI migration policy if it is still needed, not to reusable core contracts. |
| Legacy repo-cwd hub identity | drop | Hub identity must not be derived from ambient repository cwd. Core may keep narrow identity operation contracts, but cwd-derived hub policy stays out. |
| Old forwarder terminology | translate | Terminal client data-plane lifetimes translate to terminal subscriptions. Public core vocabulary should not preserve `PtyForwarder`, `StopForwarder`, or `create_pty_forwarder` names. |
| Browser-only plugin stores | drop | Browser-only persistence is client or product behavior. Plugin-owned dynamic state should use plugin/runtime storage contracts and namespaced entity frames, not a browser-only core store. |
| Direct snapshot helpers | translate | Transport-neutral snapshot frames and payload shapes may be preserved, but helper calls that bypass session/client-worker ownership are legacy mechanics. Do not add direct helper APIs such as `snapshot_and_subscribe`. |
| Hub-owned PTY relays | drop | The hub owns attach policy and cleanup, not terminal byte delivery. PTY egress belongs to the session/client-worker data plane. |
| Product-specific UI refresh behavior | drop | Product refresh behavior belongs in clients, plugins, or hub policy. Core preserves UI and entity contract shapes only. |

## License

[O'Saasy License](LICENSE) - free to use, modify, and distribute. Cannot be
repackaged as a competing hosted/SaaS product.
