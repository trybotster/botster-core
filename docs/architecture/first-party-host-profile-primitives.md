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
  owns per-plugin class-aware admission, reserved RequestResponse executors,
  capability checks, Core-owned async deadlines, cancellation, load, reload,
  unload, descriptor cleanup, resource cleanup, completion draining, and
  backpressure reporting. `PluginWorkerDebugSnapshot` and
  `PluginWorkerPluginDebugSnapshot` expose per-class queued count/bytes,
  in-flight jobs, reserved completions, pressure, and reserved executor
  capacity. Concrete Lua, process, or future WASM execution stays behind the
  host-supplied `PluginRuntime` trait in `crates/botster-core/src/runtime/mod.rs`.
  Which operations are Background vs RequestResponse remains host policy.
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
  state through entity families instead of smuggling records through
  presentation payloads.
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
- Notification, entity, and plugin-action policies are not host-profile policies
  in core. Core defines typed frames, inbox mechanics, and generic worker-routing
  kinds; the profile must still decide UI payloads, presentation, routing,
  acknowledgement, filtering, retention, and workflow-specific meanings outside
  core.

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

## Implementation status

The first minimal core contract now exists in
`crates/botster-core/src/package/host_profile.rs` and
`PackageManifest.host_profile`. It covers recommendation 1 and the narrow
admission portion of recommendation 2: typed profile identity, compatibility,
precedence, required provider names, required capabilities, typed policy
section names, provider-only admission, host enablement, source provenance,
bootstrap entrypoints, required capability presence, nonblank host-profile
metadata, and explicit compatibility checks against a caller-supplied host
Botster point version.

The blessed admission helper is
`admit_host_profile(manifest, enabled, host_botster_version)`. It validates both
`PackageManifest.botster` and `HostProfileMetadata.compatibility` with the
intentionally narrow core syntax: exact `MAJOR.MINOR.PATCH` and lower-bound
`>=MAJOR.MINOR.PATCH` requirements only. It does not pull in a semver solver,
and unsupported or malformed requirements fail admission with typed errors.
This keeps compatibility validation explicit without turning core into a
package manager.

This remains intentionally scaffold-only at the core layer. There is no
in-crate production caller because admission, install state, registry
resolution, profile enablement, startup ordering, concrete policy storage, and
hub wiring remain host/hub responsibilities. Wiring profile admission into
`PluginWorkerEngine` or `BotsterEngine` would collapse the core-vs-host
boundary this audit is preserving. The production caller is expected to be a
host/hub package manager or startup path in a separate ticket.

The ordinary-plugin boundary is enforced at the public admission API and at the
plugin-worker capability gate. A manifest with `kind = Plugin` and
`host_profile` metadata is rejected by `HostProfileAdmissionError::NotProvider`,
and `PluginWorkerEngine` grants handler access only from
`PackageManifest.capabilities`. Host-profile `required_capabilities` describe
requirements for admission; they are not worker capability grants.

The metadata fields carry no personal or user data. They are mechanism-level
contract fields only: profile identity, compatibility, precedence, required
provider names, required capability declarations, and typed policy-section
names.

`crates/botster-core-dev` now includes a no-hub host-profile smoke proof. The
shared binary/test harness constructs and admits a minimal provider manifest,
then uses the admitted profile's exact `required_capabilities` entry as the
ordinary plugin handler requirement. One plugin manifest carries that capability
and completes through `BotsterEngine<LocalProcessRuntime, LocalProcessWorkerRuntime>`;
a second ordinary plugin omits the same capability and is rejected by the typed
plugin-worker capability gate before its runtime is called.

The proof also spawns a real local PTY session through the same generic
`BotsterEngine` instance, attaches a client subscription, routes terminal input,
drains runtime bytes back through `receive_output`, resizes the session,
classifies activity, reads screen/snapshot command events, and shuts down. This
proves a custom host can compose real local session management and worker-based
plugin mechanics without `botster-hub`. The generic local worker reports
screen/snapshot command events and terminal dimensions, but it does not own the
managed shadow-terminal parser used by `DefaultBotsterEngine`; callers should
not read this proof as a claim that the generic local worker provides the same
snapshot payload fidelity as the managed default facade.

The custom host inputs proven by the dev harness are: caller-supplied host
Botster version, enablement decision, source provenance, bootstrap entrypoint,
required provider names, required capabilities, explicit spawn request fields,
client and subscription ids, logical clocks, and plugin worker registration plus
runtime.

Open follow-ups remain recommendations 3-5: capability grant/consume ledger,
typed startup/config lifecycle if core needs to bless ordering, durable
persistence contracts, and concrete host/hub registry or package-manager
wiring.

## Verification anchors

This audit began as scaffold-only. The current core production-facing path is
now a public contract helper that embedders can call before loading provider or
plugin runtime paths; host/hub wiring still remains outside this crate. The
proof is grounded in existing public entry points and contract tests:

- `crates/botster-core/tests/host_profile_contract_test.rs` exercises
  host-profile manifest serde compatibility, admission success, exact and
  lower-bound compatibility acceptance, typed compatibility and blank-field
  rejection cases, and typed policy sections.
- `crates/botster-core/tests/plugin_worker_engine_test.rs` verifies
  host-profile metadata is not consulted by the plugin-worker capability gate.
- `crates/botster-core/tests/botster_engine_api_test.rs` exercises
  `BotsterEngine`, plugin invocation, notifications, entity handling through
  the engine path, and `DefaultBotsterEngine` when `local-runtime` is enabled.
- `crates/botster-core-dev/tests/engine_smoke_test.rs` exercises the shared
  dev harness used by `cargo run -p botster-core-dev`, proving the no-hub
  host-profile admission, one generic local-runtime engine for real session and
  plugin work, subscribed terminal egress, and load-bearing plugin capability
  allow/deny behavior.
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
