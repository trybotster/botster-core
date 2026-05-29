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
| Core | Reusable mechanisms and transport-neutral contracts: session, client, subscription, and request identifiers; session-process protocol constants, handshake bytes, frame payload contracts, and length-prefixed framing; terminal ingress/egress frames; plugin worker handler refs, descriptors, invocation, lifecycle, cleanup, and pressure events; entity frames; UI node shapes; package, capability, extension, crypto, and identity contracts. | Runtime policy, executable startup, product workflows, concrete adapters, device persistence policy, executable plugin callbacks, or raw private key material. | `src/boundary.rs`, `src/actor.rs`, `src/session.rs`, `src/session_protocol.rs`, `src/client.rs`, `src/transport.rs`, `src/entity.rs`, `src/ui.rs`, `src/package.rs`, `src/capability.rs`, `src/extension.rs`, `src/crypto.rs`, `src/device.rs`, `src/keyring.rs` |
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
