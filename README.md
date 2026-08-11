# botster-core

`botster-core` is the embeddable, programmable, tmux-like local engine for
Botster hosts.

It is intentionally not the Botster application, not the hub, and not the CLI.
It contains the reusable local execution mechanics, typed contracts, and
policy-free engine facades that every Botster host, client, provider, and
plugin runtime must agree on.

## Workspace crates

| Crate | Role |
| --- | --- |
| `botster-core` | Production contracts, engine facades, local PTY/process runtime, and the `botster-session-worker` binary |
| `botster-core-daemon` | Production supervisor: registry, adoption, guarded writes, typed daemon API over core; default features use the sibling Ghostty terminal backend |
| `botster-core-test-support` | Dev-dependency fixtures, fakes, and conformance helpers for consumers pinned to the same core version |
| `botster-core-dev` | Dev-only real-embedder smoke harnesses over `DefaultBotsterEngine` / `DefaultEngineCommand` |
| `botster-terminal-ghostty` | Sibling Ghostty shadow-terminal adapter (feature-gated `libghostty-vt`); stays outside the core crate |

Hubs and product hosts still own auth, persistence policy, config locations,
cloud federation, marketplace and install/update policy, WebRTC/signaling/API
adapters, UI presentation, and workflow-specific behavior.

## Choose one path

Outside faces of this workspace should start from **one** of these, not from
raw multiplexers or frame codecs.

| Path | Use when | Entry point |
| --- | --- | --- |
| **Library** | Embed sessions in-process (tests, toys, custom hosts that own process lifetime) | `DefaultBotsterEngine` — prefer `DefaultBotsterEngine::worker_backed(path)` when sessions should outlive parent process restarts |
| **Production** | Durable local supervision (hubs, long-lived hosts) | `botster_core_daemon::CoreDaemon` configured with the `botster-session-worker` executable (`CoreDaemonConfig::with_worker_path`) |

**Do not** treat registry APIs on `CoreDaemon` as durable if you omit the worker
path. Without `with_worker_path`, the daemon falls back to in-process PTYs that
die with the daemon process even though registry files still exist.

### Library path: `DefaultBotsterEngine`

Default features enable `local-runtime` and export the local PTY facade:

```toml
botster-core = "0.1.0"
```

- `DefaultBotsterEngine::new()` — embedding process owns `LocalProcessRuntime`
  directly (good for tests and short-lived embeds).
- `DefaultBotsterEngine::worker_backed(path)` — one `botster-session-worker` OS
  process per session owns the live PTY; parent keeps IPC handles only.

Contract-only embeds can opt out of the local process dependency:

```toml
botster-core = { version = "0.1.0", default-features = false }
```

With default features disabled, `LocalProcessRuntime`, `DefaultBotsterEngine`,
and related local-runtime items are not exported.

### Production path: `CoreDaemon` + session worker

`botster-core-daemon` is the production supervisor over core. Configure it with
the session-worker binary so each session is restart-adoptable:

```rust
use botster_core_daemon::{CoreDaemon, CoreDaemonConfig};

let mut daemon = CoreDaemon::new(
    CoreDaemonConfig::new(data_dir).with_worker_path(session_worker_path),
);
// spawn → attach → drain → input → shutdown via CoreDaemon methods
```

Workers own PTYs and control sockets. Intentional daemon restart can call
`release_for_restart` so workers keep running; a fresh daemon over the same
`data_dir` can adopt them. See [`docs/architecture/core-daemon.md`](docs/architecture/core-daemon.md).

Terminal history and authoritative screen/snapshot intent go through core’s
opaque terminal seams. Botster’s blessed shadow-terminal backend is Ghostty in
the sibling `botster-terminal-ghostty` crate. `botster-core-daemon` enables it
on every production path. The daemon always constructs Ghostty; there is no plain terminal production feature lane.
Default daemon and workspace builds therefore require Zig `0.16.0` plus the
initialized `crates/botster-terminal-ghostty/vendor/ghostty` submodule.
Contract-only daemon embedders can opt out with:

```toml
botster-core-daemon = { path = "crates/botster-core-daemon", default-features = false }
```

That opt-out uses the plain fallback terminal state and avoids the Ghostty/Zig
dependency. The default daemon profile configures Ghostty with a 10 MB retained
scrollback byte budget. Hosts can tune it without rebuilding:

```rust
let config = CoreDaemonConfig::new(data_dir)
    .with_worker_path(session_worker_path)
    .with_ghostty_max_scrollback_bytes(2_000_000);
```

The effective retained line count is determined by Ghostty's page allocator and
terminal width. At the 10 MB default, a saturated 24x80 warm session currently
produces about 9.0 MiB of opaque Snapshot payload per attaching client. Lower
budgets reduce that per-client snapshot egress at the cost of less history for
late attaches and reattaches; they do not change attach or snapshot semantics.
`botster-core` itself still has no Ghostty dependency.

Full command vocabulary (including typed `execute_command`):
[`docs/architecture/engine-command-surface.md`](docs/architecture/engine-command-surface.md).

## Start here: spawn → attach → drain → input → shutdown

Core is **synchronous and policy-free**. The host supplies executable, cwd, env,
ids, and clocks (`now_seconds`). The host also runs the event loop (see below).

**Rust import path:** use `botster_core::prelude` for the library lifecycle types
(engine facade, ids, spawn request, transport outcomes). Full rustdoc for that
module is the compile-checked start-here surface. Prefer module paths
`contract::*`, `engine`, `package`, `runtime`, and `identity` over treating every
crate-root re-export as equally primary.

### Library sketch

```rust
use botster_core::prelude::*;

let mut engine = DefaultBotsterEngine::new();
// Or: DefaultBotsterEngine::worker_backed("/path/to/botster-session-worker");

let session_id = SessionId("session-1".into());
let client_id = ClientId("client-1".into());
let subscription_id = SubscriptionId("sub-1".into());
let now = 1_700_000_000;

let _spawn = engine.spawn_session(
    SessionSpawnRequest {
        request_id: RequestId("spawn-1".into()),
        session_id: session_id.clone(),
        executable: "printf".into(),
        arguments: vec!["hello".into()],
        working_directory: SpawnWorkingDirectory {
            path: "/workspace".into(),
        },
        environment: SpawnEnvironment::default(),
        initial_pty_size: None,
    },
    CoreSessionMetadata::new(),
)?;

let _attach = engine.attach_client(
    client_id.clone(),
    session_id.clone(),
    subscription_id,
    now,
)?;

// Host loop: drain often enough that queues do not stall.
let drained = engine.drain_runtime_once(&session_id, now)?;
// deliver drained.client_egress (and other outcomes) to clients

let _input = engine.write_bytes(client_id, session_id.clone(), b"next\n", now)?;
let _more = engine.drain_runtime_once(&session_id, now)?;

let _shutdown = engine.shutdown_session(session_id, "done", now)?;
```

The same lifecycle verbs on the production path are `CoreDaemon::spawn`,
`attach`, `drain`, `input`, and `shutdown` (plus `resize`, detach, adoption, and
guarded writes). Prefer those methods over assembling lower-level engines.

Production hosts that maintain a session projection start with
`CoreDaemon::lifecycle_baseline`, retain its source-generation cursor, and call
`CoreDaemon::lifecycle_changes` after their normal daemon progress/drain work.
Each lifecycle row includes the opaque, host-owned `CoreSessionMetadata` map
supplied at spawn. The daemon persists that map in its registry and restores it
when adopting a worker, so reconnecting hosts can rebuild their own session
classification without a parallel metadata store. Core serializes the map but
does not define its keys; hosts remain responsible for the existing size and
no-PII constraints. The standalone Core daemon CLI supplies an empty map by
design because Core owns no host taxonomy. Registry records created before
metadata persistence was introduced project an empty map permanently; hosts
should treat that as unknown classification rather than infer a meaning or
maintain a parallel backfill store.
Changes are ordered and replayed from a bounded in-memory journal. A cursor
from another daemon generation, ahead of the source, or older than retained
history returns an explicit resync reason and no partial suffix; fetch a fresh
baseline before continuing. Terminal bytes and snapshots remain on
`CoreDaemon::drain`. Hosts may call `CoreDaemon::remove_session` only after a
session is terminal; that policy-free mechanism clears core, subscription,
retained-terminal, pending-drain, and registry state before publishing
`Removed`.

`crates/botster-core-dev` mirrors the library path with a deterministic smoke
harness. The rustdoc example on `BotsterEngine` is compile-checked against the
generic facade for custom runtimes.

## Host event-loop expectations

`botster-core` does not run an async runtime for you. Embedders own the loop:

1. **Clock** — pass host `now_seconds` into attach, drain, input, inspect, and
   shutdown. Core does not call wall clocks for policy.
2. **Drain** — call `drain_runtime_once` / `drain_runtime_all_once` (library) or
   `CoreDaemon::drain` (production) regularly while sessions are live. Output,
   lifecycle observations, and attach-time history replay surface on drain.
3. **Deliver** — route returned client egress and session requests to transports;
   do not re-inject already-routed session requests as new work.
4. **Backpressure** — honor bounded queue / backpressure summaries from drain and
   report slow clients with `report_backpressure` / `report_delivery_lag` when
   the host observes delivery lag. Local PTY reader pressure appears as typed
   session-I/O backpressure on the public default-engine path.
5. **Other drains** — if you use notifications or routed envelopes on
   `CoreDaemon`, drain those APIs separately; they are daemon process memory,
   not registry-durable across process restart.
6. **Lifecycle projection** — establish one lifecycle baseline, then consume
   ordered changes after normal progress calls instead of polling `list()`.
   Resync from a fresh baseline whenever the change result says the cursor is
   no longer valid.

Depth lives in architecture docs:

- [`docs/architecture/engine-command-surface.md`](docs/architecture/engine-command-surface.md)
  — command facade, sync model, exclusions
- [`docs/architecture/core-daemon.md`](docs/architecture/core-daemon.md)
  — registry, adoption, attach drain ordering, guarded writes
- [`docs/architecture/durable-session-worker-protocol.md`](docs/architecture/durable-session-worker-protocol.md)
  — protocol vocabulary for daemon ↔ worker durability

## Architecture docs

| Doc | Topic |
| --- | --- |
| [engine-command-surface.md](docs/architecture/engine-command-surface.md) | Canonical `BotsterEngine` / `DefaultBotsterEngine` commands |
| [core-daemon.md](docs/architecture/core-daemon.md) | Production `CoreDaemon` supervisor and adoption |
| [durable-session-worker-protocol.md](docs/architecture/durable-session-worker-protocol.md) | Durable session-worker protocol shapes |
| [ghostty-shadow-terminal-adapter.md](docs/architecture/ghostty-shadow-terminal-adapter.md) | Ghostty adapter boundary (sibling crate) |

Living design notes live under `docs/architecture/`. Historical ticket plans under
`docs/plans/` are archival context, not the start-here guide.

## Durable session vocabulary

`botster_core::durable_session` exports **protocol vocabulary** (spawn/adopt,
heartbeat, guarded writes, queue limits, daemon control operation names) spoken
by the daemon and session-worker path. Types alone do not schedule processes.

**Runtime ownership:**

| Layer | What exists today |
| --- | --- |
| `botster-core` | Contracts, `DefaultBotsterEngine` / worker-backed engine, `botster-session-worker` binary, session-process framing |
| `botster-core-daemon` | Durable supervisor: registry metadata, adoption scan, guarded-write delivery states, typed API and thin CLI |

Do **not** read “durable_session types are contracts” as “no durable daemon
exists.” The workspace implements the production daemon in
`botster-core-daemon`; the module remains the shared typed vocabulary.

## Advanced / lower-level surfaces

Most embedders should not start here:

| Surface | Role |
| --- | --- |
| `BotsterEngine<R, W>` | Generic facade when you supply a custom `SessionRuntime` / worker (still prefer `prelude` for imports) |
| `MultiplexerEngine` | Assembled lower-level primitive (session worker + subscription fanout + plugins + notifications). Used internally by facades; direct use is advanced |
| `session_protocol` | Byte-frame constants and length-prefixed framing for process wire protocol |
| Capability runtime modules | Plugin HTTP/filesystem/store/timer/watch mechanics — separate plane from session I/O |
| Package contracts | Policy-free manifest, capability, dependency, configuration, and runnable-entrypoint shapes via `package` |
| Flat crate-root re-exports | Compatibility imports for every public type; not the discovery path — use `prelude` or modules |

## Public surface migration notes

Introducing `botster_core::prelude` does **not** remove existing crate-root
re-exports. Existing `use botster_core::{Type, ...}` code keeps compiling.

For new code and gradual cleanups:

1. Lifecycle embeds: `use botster_core::prelude::*;`
2. Contracts: `botster_core::contract::<module>` (or short aliases that already
   re-export those modules)
3. Engine / runtime / package / identity: module paths on the crate root
4. If a future minor release narrows flat re-exports, migrate by the same
   steps — types remain reachable through modules and the prelude

## What core proves today

- typed session, client, subscription, request, transport, entity, package,
  crypto, identity, notification, terminal snapshot, and plugin worker contracts
- default local session path through `DefaultBotsterEngine` (and worker-backed
  variant) plus production supervision through `CoreDaemon`
- subscription fanout, activity/lifecycle observations, notification inbox, and
  plugin worker invocation composed under the engine facades
- consumer test harnesses and fakes in `botster-core-test-support`

Host-profile admission is a policy-free core helper:
`admit_host_profile(manifest, enabled, host_botster_version)`. A trusted host or
package manager calls it before loading provider or plugin runtime paths. Ordinary
plugins that declare host-profile metadata are rejected by the admission helper;
plugin-worker capability checks continue to use only
`PackageManifest.capabilities`.

`PluginWorkerEngineConfig` separates each plugin's bounded waiting queue from
its executor width. `per_plugin_queue_capacity` defaults to 256 and
`per_plugin_executor_concurrency` defaults to 2; both must be greater than
zero. Queue capacity controls waiting jobs, while executor concurrency controls
simultaneously running handlers, so loading a plugin does not allocate one OS
thread per queue slot.

Hosts can observe this mechanism through
`BotsterEngine::plugin_workers().debug_snapshot()`. The snapshot reports the
configured queue/executor values plus aggregate and sorted per-plugin counts for
live executors/workers, queued jobs, and in-flight jobs. Aggregate counts include
retiring generations until their executor workers join; per-plugin rows cover
currently registered generations. `BackpressureSummary.depth` for a plugin
worker counts waiting jobs only, with executing jobs reported separately as
`in_flight_jobs`. Replacement, reload, unload, and final engine drop reject new
work, cancel queued and running jobs, call `PluginRuntime::stop`, and join the
executor workers before teardown returns. Runtime implementations must make
`stop` promptly unblock cancelled work. A panic escaping `PluginRuntime::invoke`
retires that executor thread and narrows the plugin's concurrency until its next
reload.

## Package contracts (summary)

Core owns policy-free package **shapes**, not product or presentation policy.

- **Configuration** — optional `configuration` schema (fields, types, secret
  markers). Hubs own validation, persistence, and secret redaction. See
  `docs/examples/package-configuration-schema.json`.
- **Runnable entrypoints** — `web_app` / `terminal_app` launch vocabulary; hub
  owns launch policy. Required launch context is `hub_connection` plus
  `data_dir`; the portable Hub descriptor currently carries an exhaustive
  `unix_socket` transport with an absolute POSIX path. Canonical schema and
  valid/invalid consumer fixtures ship in
  `botster-core-test-support/fixtures/runnable-entrypoint-hub-connection/`.
  See `docs/examples/package-runnable-entrypoints.json`.
- **Dependencies / features** —
  `resolve_package_dependencies(manifest, input)` builds an availability matrix
  from caller-supplied state. See `docs/examples/package-dependencies.json`.

## Terminal screen and snapshot boundary

`botster-core` exposes a narrow terminal screen boundary:
`TerminalScreenEngine`, `TerminalScreenRuntime`, `TerminalOutputChunk`,
`TerminalSnapshotPayload`, and `TerminalScreenState`.

Snapshot payload bytes are opaque. Core records dimensions and an optional
host-owned format label; it does not parse cells or choose a backend. Botster’s
blessed authoritative shadow-terminal path is Ghostty in
`botster-terminal-ghostty` so native parser policy stays out of `botster-core`.

restty is a web/client renderer path. It may consume terminal state and streams
through client data-plane contracts, but it is not core shadow-terminal
infrastructure.

`botster-terminal-ghostty` pins upstream Ghostty
(`https://github.com/ghostty-org/ghostty`) at
`22d13172cde98a0a4dda05d3d6a3fcb0dd8ed018`. Default workspace builds do not
require the submodule or Zig. To exercise the native path:

```sh
git submodule update --init crates/botster-terminal-ghostty/vendor/ghostty
cargo test -p botster-terminal-ghostty --features libghostty-vt
```

Feature-enabled builds require Zig `0.16.0` and run
`zig build -Demit-lib-vt -Doptimize=ReleaseFast -Dsimd=false -Dcpu=baseline -Dversion-string=1.3.2-dev`.
The vendored Ghostty fork is MIT licensed; preserve
`crates/botster-terminal-ghostty/vendor/ghostty/LICENSE` in distributions that
include it.

Subscribe-time initial terminal snapshots use the core data-plane path. When a
host requests an initial snapshot for a new subscription to a running session,
the client stream emits subscription-scoped `Attaching`, then `SessionIo`
delivers `InitialSnapshotReady` for that exact client and subscription. The
client stream projects a non-empty payload as a renderable `Snapshot`, then
emits `Attached`; live `TerminalOutput` follows. For empty initial history it
emits `Attaching`, then `Attached`, without fabricating `Snapshot` or
`Scrollback`. Stale or replaced subscription deliveries emit neither history
nor `Attached`. Core currently produces `Attaching` and `Attached`; it does not
yet produce `Detached`. Hub/host policy decides when to subscribe; core does not
keep a duplicate product cache.

## Ownership boundary

This crate documents contracts the current code proves. It is not a parking lot
for future hub, client, cloud, or plugin behavior.

| Layer | Owns | Does not own |
| --- | --- | --- |
| Core | Reusable mechanisms and transport-neutral contracts; local PTY/worker path; `DefaultBotsterEngine` / `BotsterEngine`; session-process framing; plugin-worker mechanics; policy-free package/capability/entity/crypto shapes; sibling `CoreDaemon` supervisor crate | Product auth, config paths, marketplace, concrete WebRTC policy, UI payloads and package presentation, workflow policy, async product supervision |
| Hub | Runtime policy, lifecycle, routing, recovery, extension supervision as a first-party host profile | Raw terminal byte ownership (bytes flow through session/client data plane) |
| CLI | Operator commands and process startup | Reusable protocol contracts or hub policy |
| Client | Presentation, local input, transport adaptation, and rendering of Hub-owned UI contracts | Session lifecycle policy or hub supervision |
| Provider/plugin | Extension packages via manifests, entrypoints, granted capabilities | Implicit hub internals or marketplace policy |

Proof anchors include `crates/botster-core/src/contract/boundary.rs`,
`engine/botster.rs`, `runtime/local_process.rs`, and
`crates/botster-core-daemon/src/daemon.rs`.

### Explicit ban list

The following behavior does not belong in `botster-core`:

- hub policy
- CLI startup
- Rails/cloud/Auth implementation
- concrete WebRTC negotiation policy
- React/TUI rendering
- UI payload, package-surface/navigation, and presentation contracts
- Project Pipelines/GitHub/Cloudflare product logic
- legacy compatibility paths
- device config files, OS keychain or file-fallback persistence, operator
  prompts, or signing-key storage policy

### BoundaryJson escape hatches

`BoundaryJson` is reserved for payloads whose schema is owned outside
`botster-core`. Current public actor and transport contracts allow it only for
relay-owned signaling/envelope data, plugin-owned descriptors and metadata, and
plugin-owned handler request/response payloads.

Stable Botster-owned controls stay typed: terminal attach state, ping/pong,
focus, terminal input, resize, snapshot requests, scrollback, process exit,
client health/state, session lifecycle, and backpressure. Every public
actor/transport `BoundaryJson` use is classified with owner and reason metadata
in `tests/actor_contract_test.rs`.

## Crypto and identity surface

Core owns the reusable AES-GCM envelope utility surface and public device
metadata/fingerprint helpers. Non-exportable signing and credential-store types
are boundary contracts; CLI and provider packages own runtime credential policy.
The AES-GCM implementation uses the stable `aes-gcm` `0.10.x` line.

## Local verification

Run the same workspace checks used by pull request and main-branch CI:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test --doc --workspace
cargo doc --workspace --no-deps
```

For command-surface docs, also verify the contract-only feature set:

```sh
cargo doc -p botster-core --no-default-features --no-deps
cargo test -p botster-core --no-default-features --lib
```

On Unix hosts, also run the local PTY acceptance test:

```sh
cargo test -p botster-core --test local_process_runtime_test
```

Release verification also builds the whole workspace in release mode on pushes
to `main` and on manual workflow dispatch:

```sh
cargo build --workspace --release
```

## Consumer test support

`botster-core-test-support` is the version-coupled test surface for downstream
crates. Add it only under `dev-dependencies`, at the same release version as
`botster-core`.

It exposes managed local conformance harnesses and a many-PTY load harness over
the public `DefaultBotsterEngine` facade. CI-safe adversarial proof:

```sh
BOTSTER_ENV=test cargo test -p botster-core-test-support adversarial_hot_path -- --nocapture
```

```sh
BOTSTER_ENV=test cargo test -p botster-core-test-support many_pty_load_default -- --nocapture
```

Opt-in larger load:

```sh
BOTSTER_ENV=test cargo test -p botster-core-test-support many_pty_load_100 -- --ignored --nocapture
```

Focused isolation:

- slow client:
  `BOTSTER_ENV=test cargo test -p botster-core --test subscription_multiplexer_engine_test`
- slow plugin:
  `BOTSTER_ENV=test cargo test -p botster-core --test plugin_worker_engine_test`

## Migration guidance

Every extraction decision must be classified as **preserve**, **translate**, or
**drop**. There is no defer category.

### Preserve

Reusable contracts already in this crate: layer responsibility names,
session/client/subscription/request identifiers, session-process protocol,
transport-neutral frames, client liveness shapes, entity frames, policy-free
package manifests, capabilities, extension metadata, crypto/identity operation
requests, engine facades, and daemon/worker durable vocabulary.

### Translate

Concrete runtime implementations become core contracts only after code proves a
stable cross-layer shape (for example, adapter behavior → transport-neutral
frame).

### Drop

Application policy, product integration, historical compatibility, or executable
wiring. Hub policy stays in the hub; CLI startup in the CLI; rendering in
clients; workflows in plugins or providers.

## Extraction compatibility policy

| Path | Verdict | Policy |
| --- | --- | --- |
| Transport-neutral identifiers, frames, entity/package/capability/crypto contracts, engine and daemon mechanisms | preserve | Reusable core surface |
| `context.json` migration | drop | Hub/CLI migration policy if needed |
| Legacy repo-cwd hub identity | drop | Ambient cwd is not hub identity |
| Old forwarder terminology | translate | Terminal subscriptions, not `PtyForwarder` names |
| Browser-only plugin stores | drop | Client/product persistence |
| Direct snapshot helpers | translate | Session/client-worker owned snapshot frames only |
| Hub-owned PTY relays | drop | Hub owns attach policy, not terminal byte delivery |
| Product-specific UI refresh behavior | drop | Clients/plugins/hub policy |
| UI and package presentation contracts | translate | Hub-owned `botster-ui-contract`; no Core alias or forwarding path |

## License

[O'Saasy License](LICENSE) - free to use, modify, and distribute. Cannot be
repackaged as a competing hosted/SaaS product.
