# First-party host profile primitives audit

This note audits whether current `botster-core` exposes enough blessed
primitives for a first-party host profile to configure product policy without
bypassing core safety. It is intentionally an architecture note, not an API
change.

Working definition: a first-party host profile is a privileged host or package
profile that configures Botster product policy through stable core contracts
while leaving reusable mechanisms in `botster-core` and runtime/product policy
in the host, hub, provider, or plugin.

## Current sufficient primitives

- Layer ownership is explicit. `crates/botster-core/src/lib.rs` frames core as
  reusable contracts while the hub owns Botster policy and orchestration.
  `crates/botster-core/src/contract/boundary.rs` reinforces the same boundary
  through `Layer::Core`, `Layer::Hub`, `Layer::Cli`, `Layer::Extension`, and
  `Layer::Client`. That is enough to keep a host profile from treating core as
  the product-policy home.
- The command API is policy-free and typed. `crates/botster-core/src/engine/command.rs`
  defines `EngineCommand`, `DefaultEngineCommand`, `EngineCommandKind`, and
  `ENGINE_COMMAND_KINDS`; `crates/botster-core/src/engine/botster.rs` exposes
  `BotsterEngine::execute_command` and, behind `local-runtime`,
  `DefaultBotsterEngine::execute_command`. `docs/architecture/engine-command-surface.md`
  documents the same public path and names the host-owned policy inputs:
  executables, cwd, env, ids, timestamps, transport delivery, persistence,
  auth, provider policy, and workflow policy.
- Runtime startup is explicit enough for a host profile to resolve policy before
  entering core. `crates/botster-core/src/runtime/mod.rs` defines
  `SessionSpawnRequest`, `SpawnWorkingDirectory`, `SpawnEnvironment`, and
  `SessionRuntime`. Core can run a host-supplied request, but it does not infer
  default shell, target admission, cwd, environment inheritance, marketplace
  state, or lifecycle policy.
- Plugin worker isolation has a reusable core mechanism. `crates/botster-core/src/engine/plugin_worker.rs`
  owns per-plugin capacity, capability checks, deadlines, cancellation, load,
  reload, unload, descriptor cleanup, resource cleanup, and backpressure
  reporting. Concrete Lua, process, or future WASM execution stays behind the
  host-supplied `PluginRuntime` trait in `crates/botster-core/src/runtime/mod.rs`.
- Capabilities and extension metadata are already manifest-level contracts.
  `crates/botster-core/src/package/capability.rs` defines broad capability
  surfaces such as `Mcp`, `PluginDb`, `Filesystem`, `Network`, `Secrets`,
  `ClientAdmission`, `SignalingRelay`, `HubPresence`, and `BrowserShell`.
  `crates/botster-core/src/package/extension.rs` distinguishes
  `ExtensionKind::Plugin` from privileged `ExtensionKind::Provider` and marks
  bootstrap entrypoints. `crates/botster-core/src/package/manifest.rs` carries
  package kind, source, capabilities, and entrypoints.
- The event and subscription model gives host profiles typed pressure and
  routing contracts. `crates/botster-core/src/contract/actor.rs` names bounded
  queue sources, backpressure routes, hub-control messages, client-worker
  frames, session I/O requests/events, plugin worker messages/events, plugin
  descriptors, resources, and handler refs. Stable hub/client/session controls
  are typed rather than raw JSON.
- Plugin-owned dynamic state has a blessed entity-frame path.
  `crates/botster-core/src/contract/entity.rs` defines `EntityFrame`
  snapshot, scoped snapshot, upsert, patch, and remove operations, with
  validation for reserved built-ins and plugin-namespaced entity families. That
  is enough for a host profile to require product plugins to publish dynamic
  state through entity families instead of smuggling records through UI
  snapshots.
- Notifications are transport-neutral and can be policy-shaped by the host.
  `crates/botster-core/src/contract/notification.rs` defines notification
  targets, severity, source metadata, actions, content, expiry, delivery
  status, and an in-memory `NotificationInbox`. `BotsterEngine` exposes
  `post_notification` and `drain_notifications` through the command surface;
  presentation, routing policy, and acknowledgement behavior remain host-owned.

## Availability boundary

The default `botster-core` feature set includes `local-runtime`
(`crates/botster-core/Cargo.toml`). With default features, embedders can use
`DefaultBotsterEngine`, `DefaultEngineCommand`, `LocalProcessRuntime`, and the
policy-free local PTY/process adapter.

With `default-features = false`, a contract-only embedder still has
`BotsterEngine`, runtime traits, package/capability/extension metadata,
BoundaryJson classification, actor contracts, entity frames, notification
contracts, and plugin-worker contracts. It does not get `DefaultBotsterEngine`
or `LocalProcessRuntime`. A host-profile design should treat those local
runtime pieces as convenient defaults, not as universal host-profile
requirements.

## Missing or not yet blessed primitives

- No first-class host-profile manifest exists. Current `PackageManifest` can
  represent `ExtensionKind::Provider` with bootstrap entrypoints and requested
  capabilities, but it does not define a dedicated host-profile kind, profile
  precedence, profile compatibility rules, or profile-scoped policy document.
- No core admission registry exists for installed packages, providers, or host
  profiles. Core has manifest contracts and `PluginWorkerEngine::load_plugin`,
  but install state, provenance enforcement, lockfiles, enable/disable state,
  updates, and provider selection remain outside `botster-core`.
- Capabilities are declarative and checked only where a core mechanism consumes
  them. `PluginWorkerEngine` checks handler-required capabilities against
  `PackageManifest.capabilities`, but core does not provide a global capability
  admission engine for filesystem, network, secrets, browser shell, hub
  presence, signaling relay, or client admission policy.
- Startup and config hooks are not a core lifecycle primitive. `ExtensionEntrypoint.bootstrap`
  marks bootstrap participation, and `SessionSpawnRequest` lets the host enter
  core with resolved process policy, but config loading, profile layering,
  startup ordering, and hook execution are host/hub/plugin responsibilities.
- Persistence is a capability, not a storage API. `CapabilitySurface::PluginDb`
  lets a package request plugin-owned durable storage, and entity/notification
  contracts provide in-memory state shapes, but core does not define durable
  profile-policy storage, plugin-db schemas, migrations, retention, or recovery.
- Provider/plugin registry concepts are metadata-only in core. `ExtensionKind`,
  `ExtensionRuntime`, `ExtensionEntrypoint`, `PackageSource`, and capabilities
  are sufficient to describe providers and plugins, but not to resolve a live
  registry, choose the active provider, or arbitrate conflicting providers.
- Notification, entity, and UI action policies are not host-profile policies in
  core. Core defines typed frames and inbox mechanics; the profile must still
  decide presentation, routing, acknowledgement, filtering, retention, and
  workflow-specific meanings outside core.

## Unsafe escape hatches to keep constrained

`BoundaryJson` is the main intentional escape hatch. Its definition in
`crates/botster-core/src/contract/boundary.rs` says stable core controls should
use typed Rust fields and raw JSON should be reserved for Lua, plugin, or
relay-owned schemas.

The current classified opaque fields are:

- `TransportSignal.payload` in `crates/botster-core/src/contract/actor.rs`:
  relay-owned signaling envelope.
- `TransportIngress::BoundaryPayload.payload` and
  `TransportEgress::BoundaryPayload.payload` in
  `crates/botster-core/src/contract/transport.rs`: relay/plugin adapter
  payloads.
- `PluginOwnedDescriptor.body`, `PluginLoadSpec.metadata`,
  `PluginInvocationContext.metadata`, `PluginInvocationRequest.payload`, and
  `PluginInvocationSuccess.payload` in
  `crates/botster-core/src/contract/actor.rs`: plugin-owned descriptor,
  load, context, handler input, and handler response schemas.
- `NotificationAction.extension` and `NotificationContent.extension` in
  `crates/botster-core/src/contract/notification.rs`: plugin-owned
  notification action/content schemas.

Those fields are acceptable because their owners and reasons are classified in
`crates/botster-core/tests/actor_contract_test.rs`, and
`crates/botster-core/tests/boundary_test.rs` scans source markers so new public
`BoundaryJson` fields must be classified. A first-party host profile should not
use any of these opaque payloads to carry stable Botster-owned controls such as
admission rules, capability grants, config precedence, provider selection,
startup policy, persistence policy, or client trust decisions. Those controls
need typed core contracts if they become core primitives.

Other host-profile bypass risks:

- Calling `SessionRuntime` or `DefaultBotsterEngine` with already-expanded
  host policy can bypass profile admission if the host does not validate before
  building `SessionSpawnRequest`.
- Loading a plugin worker with a permissive `PackageManifest.capabilities` can
  grant handler access before the host has enforced package provenance or
  enablement state.
- Treating plugin entity records as trusted profile policy would bypass the
  entity contract's intended use as plugin-owned dynamic read model state.
- Treating notification extension payloads as host policy would bypass typed
  notification content and action contracts.

## Recommended core follow-up tickets

If hub-as-profile is accepted, core follow-up work should stay mechanism-level:

1. Define a typed host-profile policy contract or package manifest extension
   that names profile identity, compatibility, precedence, required providers,
   and policy sections without embedding product workflow defaults.
2. Add a provider/profile admission contract that evaluates
   `PackageManifest`, `ExtensionKind::Provider`, bootstrap entrypoints,
   capabilities, provenance metadata, and enabled state before any plugin worker
   or runtime path is invoked.
3. Add a capability enforcement ledger that records granted, denied, and
   consumed capability surfaces per package/provider/profile, while preserving
   the existing handler-level capability checks in `PluginWorkerEngine`.
4. Add a typed startup/config lifecycle contract if core needs to bless profile
   startup ordering. Keep config discovery and product-specific policy in the
   host/hub; core should only define the state machine and validation shapes.
5. Add durable persistence contracts for profile policy and plugin-owned
   storage metadata if persistence becomes part of the host-profile safety
   boundary. Do not turn `CapabilitySurface::PluginDb` into implicit storage
   access without schema, migration, and ownership checks.
6. Continue hardening `BoundaryJson` by keeping the source scan and
   owner/reason inventory mandatory for every public opaque payload. Any
   profile-owned control currently proposed as `BoundaryJson` should become a
   typed contract instead.

## Verification anchors

This audit is scaffold-only. The production/user path did not change; the proof
is that the note is grounded in existing public entry points and contract tests:

- `crates/botster-core/tests/botster_engine_api_test.rs` exercises
  `BotsterEngine`, plugin invocation, notifications, entity handling through
  the engine path, and `DefaultBotsterEngine` when `local-runtime` is enabled.
- `crates/botster-core/tests/plugin_worker_engine_test.rs` exercises worker
  load/invoke/reload/unload, backpressure, timeout/cancellation, resource
  cleanup, and package capability rejection/grant behavior.
- `crates/botster-core/tests/actor_contract_test.rs` exercises typed actor
  controls and the BoundaryJson owner/reason inventory.
- `crates/botster-core/tests/boundary_test.rs` verifies layer ownership,
  provider package metadata, extraction policy anchors, and BoundaryJson source
  classification.
- `crates/botster-core/tests/entity_test.rs` exercises entity-frame validation
  and plugin namespace ownership.
- `crates/botster-core/tests/notification_inbox_test.rs` exercises notification
  inbox posting, expiry, delivery, and acknowledgement mechanics.
