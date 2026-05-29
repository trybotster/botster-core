# botster-core

`botster-core` is the reusable Botster runtime contract crate.

It is intentionally not the Botster application, not the hub, and not the CLI.
It contains the transport-neutral mechanisms and data shapes that every Botster
host, client, provider, and plugin runtime must agree on.

## Ownership Boundary

This crate documents contracts the current code proves. It is not a parking
lot for future hub, client, cloud, or plugin behavior.

| Layer | Owns | Does not own | Current proof |
| --- | --- | --- | --- |
| Core | Reusable mechanisms and transport-neutral contracts: session, client, subscription, and request identifiers; terminal ingress/egress frames; entity frames; UI node shapes; package, capability, extension, crypto, and identity contracts. | Runtime policy, executable startup, product workflows, concrete adapters, or raw private key material. | `src/boundary.rs`, `src/session.rs`, `src/client.rs`, `src/transport.rs`, `src/entity.rs`, `src/ui.rs`, `src/package.rs`, `src/capability.rs`, `src/extension.rs`, `src/crypto.rs` |
| Hub | Runtime policy, lifecycle, routing, recovery, and extension supervision. | Raw terminal byte delivery, CLI argument parsing, React/TUI rendering, Rails/cloud/Auth policy, Project Pipelines/GitHub/Cloudflare product logic, or legacy compatibility paths. Terminal bytes are represented by core frames and should flow through session/client data-plane actors, not hub policy loops. | `Layer::Hub` responsibility text in `src/boundary.rs`; terminal byte exclusions are reinforced by `TransportIngress::TerminalInput` and `TransportEgress::TerminalOutput` in `src/transport.rs` |
| CLI | Operator commands and process startup. `src/boundary.rs` also names CLI argument parsing as something the hub does not own. | Reusable protocol contracts, hub runtime policy, provider policy, or UI/product behavior. | `Layer::Cli` and `Layer::Hub` responsibility text in `src/boundary.rs` |
| Client | Presentation, local input, concrete transport adaptation, liveness reporting, and rendering of core UI/entity contracts. | Session lifecycle policy, hub supervision, provider authority, concrete WebRTC negotiation policy in core, or product-specific workflow state. | `src/client.rs`, `src/transport.rs`, `src/entity.rs`, `src/ui.rs` |
| Provider/plugin | `Layer::Extension` behavior described by package manifests, `ExtensionKind::Plugin` or `ExtensionKind::Provider`, entrypoints, and granted capabilities such as client admission, signaling relay, hub presence, or browser shell. | Implicit hub internals, private key material, marketplace/update policy, Rails/cloud/Auth implementation in core, or bypassing capability declarations. Providers are privileged extension packages, not a separate `Layer::Provider` variant. | `Layer::Extension` responsibility text in `src/boundary.rs`, plus `src/package.rs`, `src/extension.rs`, `src/capability.rs`, and `tests/boundary_test.rs` |

## Explicit Ban List

The following behavior does not belong in `botster-core`:

- hub policy
- CLI startup
- Rails/cloud/Auth implementation
- concrete WebRTC negotiation policy
- React/TUI rendering
- Project Pipelines/GitHub/Cloudflare product logic
- legacy compatibility paths

## Migration Guidance

Every extraction decision must be classified as preserve, translate, or drop.
There is no defer category.

### Preserve

Preserve contracts already represented in this crate: layer responsibility
names, session/client/subscription/request identifiers, transport-neutral
ingress and egress frames, client liveness and scope shapes, entity frames,
minimal UI node kinds, package manifests, extension metadata, capability
surfaces, and narrow crypto/identity operation requests.

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

## License

[O'Saasy License](LICENSE) - free to use, modify, and distribute. Cannot be
repackaged as a competing hosted/SaaS product.
