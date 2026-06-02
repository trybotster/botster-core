# Non-Blocking Plugin Capability Runtime

Botster core defines plugin capability I/O as a bounded runtime mailbox contract, not as inline HTTP, WebSocket, file, store, watch, or timer execution. Plugin code and first-party host profiles submit typed requests, receive typed handles, and drain typed events through host-provided runtimes. PTY, session, client, and plugin-worker hot paths must not perform blocking capability I/O inline.

This document describes the scaffold in `botster_core::runtime::capability`. It is intentionally contract-only. There is no concrete async runtime, HTTP client, WebSocket backend, filesystem resolver, plugin-store adapter, watcher, timer scheduler, or Lua integration in this ticket.

## Boundary

The existing `PluginWorkerEngine` remains the plugin execution boundary. Plugin handlers run through stable `PluginHandlerRef` values, per-plugin capacity, timeout attribution, cooperative cancellation, and unload/reload cleanup. Capability I/O composes with that boundary:

- Plugin code requests capability I/O by submitting a `CapabilityRuntimeRequest`.
- The host-owned `PluginCapabilityRuntime` enqueues the request and returns a `CapabilityRuntimeHandle` or a typed `CapabilityRuntimeError`.
- Long-running I/O continues outside the worker invocation path.
- Completion, inbound WebSocket messages, file-watch events, timer firings, failures, cancellations, and pressure observations return as `CapabilityRuntimeEvent` values.
- Runtime resources are tracked as `PluginResourceRef` values so plugin unload/reload can release only the owning plugin's resources.

The queue source for pressure reports is `QueueSource::PluginWorker`. The capability runtime is part of the plugin-worker boundary rather than a separate public actor family in this scaffold.

## Operation Families

The scaffold names these operation families:

- `HttpCapabilityRequest`: outbound HTTP request metadata.
- `WebSocketCapabilityRequest`: connect, send, and close operations.
- `WatchCapabilityRequest`: scoped file-watch register and unregister operations.
- `FilesystemCapabilityRequest`: scoped read, write, list, stat, and remove operations.
- `PluginStoreCapabilityRequest`: plugin-scoped JSON get, set, delete, and list operations.
- `TimerCapabilityRequest`: one-shot, interval, and cancel operations.

Each request carries:

- `PluginKey` ownership.
- A stable `CapabilityOperationId`.
- A timeout budget in milliseconds.
- An optional `PluginHandlerRef` for callbacks.
- An explicit required `Capability`.

HTTP maps to `CapabilitySurface::Network` with scope `http`. WebSocket maps to `CapabilitySurface::Network` with scope `websocket`. File watch and scoped filesystem operations map to `CapabilitySurface::Filesystem` with the host-owned scope id. Plugin-store operations map to `CapabilitySurface::PluginDb` with the store namespace. Timers map to `CapabilitySurface::Timers` with scope `callbacks`.

Core names the capability requirement. Hub/profile policy decides which packages receive which grants, allowed origins, filesystem scopes, namespaces, credentials, quotas, and backend implementations.

## Scoped Filesystem

Core does not trust or resolve absolute filesystem paths. Filesystem and watch requests carry:

- An opaque host-owned `scope_id`.
- A `ScopedRelativePath` below that scope.

`ScopedRelativePath::is_scoped_relative()` verifies only the contract-level shape: non-empty, not Unix absolute, not Windows drive-prefixed, not UNC-style, and no `..` traversal segments. Host profiles still own the real containment guard: canonicalize the granted root and candidate target, then prove the canonical target remains under the canonical root according to the platform's filesystem semantics. Core deliberately does not implement root resolution or symlink policy.

`FilesystemCapabilityGrant`, `FilesystemCapabilityPermissions`, and `FilesystemCapabilityLimits` name the scoped grant and size-limit contract that a host profile enforces. Core does not decide which plugin receives a grant, which directory backs a scope, how symlinks are treated, or which quota values apply.

Successful filesystem completion uses `CapabilityOperationResult::Filesystem(FilesystemCapabilityResult)`. Filesystem read, write, list, stat, and remove results must not be tunneled through `HttpCapabilityResponse` or untyped JSON.

## Handles And Resources

The runtime returns `CapabilityRuntimeHandle` after accepting a request into the bounded mailbox. Persistent or in-flight resources use `PluginResourceRef` with plugin ownership and a stable resource id.

Existing resource kinds are reused for already-named families:

- `PluginResourceKind::HttpRequest`
- `PluginResourceKind::Watch`
- `PluginResourceKind::Timer`

The scaffold adds only the missing cleanup families:

- `PluginResourceKind::NetworkConnection`
- `PluginResourceKind::FilesystemOperation`
- `PluginResourceKind::PluginStoreOperation`

These resource refs fit the existing `PluginCleanupResult` unload/reload cleanup path.

## Queue And Backpressure

The capability runtime must use bounded submit and callback/event queues. When it cannot accept more work, it reports `CapabilityRuntimeErrorKind::Backpressured` or emits `CapabilityRuntimeEvent::Backpressure(BackpressureSummary)`.

Pressure reports must include:

- `QueueSource::PluginWorker`
- The configured capacity.
- Current depth.
- `BackpressureRoute.plugin_key`

The runtime must reject or report pressure before unbounded memory growth. It must not block PTY/session/client hot paths waiting for queue space.

## Timeout And Cancellation

Every request carries `timeout_ms`. Runtime implementations should treat timeout as the host-side budget for accepting or completing the operation according to the operation family.

Cancellation is explicit:

- `PluginCapabilityRuntime::cancel(plugin_key, operation_id)` requests cancellation for an operation.
- `PluginCapabilityRuntime::release_resource(resource)` releases one runtime resource.
- `PluginCapabilityRuntime::cleanup_plugin(plugin_key)` releases all resources owned by one plugin.

Timeouts, cancellations, invalid requests, runtime stops, unknown operations, and unknown resources are represented with `CapabilityRuntimeErrorKind` and mirrored in failure events.

## Events

`CapabilityRuntimeEvent` is the typed event surface for future hub/profile/plugin-runtime integration:

- `Completed`
- `ResourceOpened`
- `ResourceReleased`
- `WebSocketMessage`
- `Watch`
- `TimerFired`
- `TimedOut`
- `Cancelled`
- `Failed`
- `Backpressure`
- `CleanupCompleted`

Events carry plugin identity, operation ids, and resource refs where applicable. Plugin-owned JSON store values and WebSocket messages are payload data, while stable Botster control fields remain typed.

`CapabilityOperationCompleted` carries `CapabilityOperationResult`, with family-specific variants such as `Http` and `Filesystem`. This is an additive shared-contract shape; future operation families should add typed result variants instead of reusing another family's envelope.

## Hub/Profile-Owned Policy

The host profile owns policy and backend selection:

- Admission rules.
- Capability grants and default grants.
- Network origin allowlists.
- Filesystem root resolution and symlink behavior.
- Plugin-store namespace backing.
- Credentials and secret lookup.
- Timer quotas.
- Queue capacities.
- Retry policy.
- Concrete async runtime, threads, tasks, and watchers.

Botster core owns reusable contract mechanics only: typed requests, capabilities, handles, events, errors, cleanup refs, and pressure metadata.

## Runtime Path Evidence

This ticket is scaffold-only by design for production core. There is no live HTTP, WebSocket, watch, filesystem, plugin-store, or timer entry point in `botster-core`.

The verifiable changed runtime path is the public `botster_core` contract surface that future host profiles and plugin runtimes import. The contract is exercised by `plugin_capability_runtime_test`, by the `PluginWorkerEngine` cleanup regression that records and removes capability runtime resources through the existing plugin unload path, and by the `botster-core-test-support` fake capability runtime that proves `submit()` accepts and tracks work before any completion event is available.

Real temporary-file I/O proofs for symlink containment, normalization, size-limit enforcement, atomic write durability, and timeout/cancel behavior remain host-profile responsibilities. Review and Verify should treat those as intentionally deferred until a concrete host profile such as Botster Hub wires a filesystem backend over this contract.
