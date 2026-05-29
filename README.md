# botster-core

`botster-core` is the reusable Botster runtime contract crate.

It is intentionally not the Botster application, not the hub, and not the CLI.
It contains the transport-neutral mechanisms and data shapes that every Botster
host, client, provider, and plugin runtime must agree on.

## Boundary

```text
core = reusable mechanisms and contracts
hub  = Botster policy, orchestration, lifecycle, and extension supervision
cli  = executable entrypoint and operator commands
```

Core owns stable primitives such as:

- session, client, subscription, and request identifiers
- session-process protocol constants, handshake bytes, and frame payload contracts
- transport-neutral ingress and egress frames
- terminal attach and liveness state
- entity and UI contract shapes
- package manifest, capability, and extension metadata
- narrow crypto and identity operation contracts

Core does not own:

- Rails, TryBotster Cloud, ActionCable, or hosted web policy
- concrete WebRTC, TUI, or socket adapter implementations
- plugin loading policy or marketplace update policy
- auth product flows
- CLI argument parsing or terminal raw-mode setup

## Migration Goal

Botster should become a local-first PTY/workspace multiplexer runtime that can
optionally federate with TryBotster Cloud. `botster-core` is the first stable
package boundary toward that shape.

The first migration slices should move already-proven contracts here before
moving behavior:

1. transport-neutral identifiers, session-process protocol contracts, and frames
2. entity/UI contract types
3. package manifest and capability declarations
4. narrow crypto/identity operation contracts
5. worker message contracts once the current repo is ready

Do not move hub policy here just because it is written in Rust. Hub policy
belongs in `botster-hub`.

## License

[O'Saasy License](LICENSE) - free to use, modify, and distribute. Cannot be
repackaged as a competing hosted/SaaS product.
