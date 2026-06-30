# Encrypted Ordered Browser Transport Contract Plan

## Context Loaded

- Pipeline context: `ticket_1782857255_885895`, run `run_1782857310_170255`, active step `botster_plan`, gate `botster_plan_gate`.
- Role and repo playbooks: [[planner-playbook]], [[botster-planner-playbook]].
- Botster architecture/conventions: [[botster-architecture]], [[cli-patterns]], [[spa-patterns]], [[project pipeline orchestration belongs in a device-level botster plugin]], [[project pipelines needs an operator workbench not more primitives]], [[project pipelines ui contract belongs in the plugin readme]], [[botster orchestration should spawn agents with explicit target ids]], [[botster orchestration prompts must bind agents to explicit worktrees]], [[plan agents must author vault context as wikilinks not home paths]].
- Transport/crypto context: [[botster is a local rust runtime that can optionally federate with cloud]], [[stable device fingerprints derive from public verifying key bytes]], [[botster webrtc request consumers should use operation gates not connection checks]], [[terminal session switches must cancel in-flight webrtc pty connects]], [[subscription ID namespacing separates TUI and browser clients]], [[clients subscribe to entities not ptys]].
- Olm history loaded as learned context only: [[e2e encryption uses vodozemac olm directly]], [[olm envelope is the single wire format]], [[ratchet restart recovers from session desync]], [[webrtc e2e encryption now mandatory no plaintext paths]], [[connection code qr pairing for e2e trust]].
- Repo surfaces inspected: `README.md`; `crates/botster-core/src/contract/transport.rs`; `crates/botster-core/src/contract/client_stream.rs`; `crates/botster-core/src/contract/session_protocol.rs`; `crates/botster-core/src/identity/crypto.rs`; `crates/botster-core/src/identity/device.rs`; `crates/botster-core/src/lib.rs`; `crates/botster-core/src/contract/mod.rs`; `crates/botster-core/src/identity/mod.rs`; `crates/botster-core/tests/crypto_test.rs`; `crates/botster-core/tests/client_stream_contract_test.rs`; `crates/botster-core/tests/boundary_test.rs`; `Cargo.toml`; `crates/botster-core/Cargo.toml`.

## Scope

- Add a transport-neutral core contract for an encrypted ordered client stream. Prefer a new focused core module, likely `crates/botster-core/src/contract/encrypted_stream.rs` or `crates/botster-core/src/identity/envelope.rs`, plus public re-exports.
- Define mechanism-level public types for:
  - peer/client identity and pairing/handshake phase state;
  - key, transcript, and storage-key identifiers without raw private key material;
  - sealed frame envelope metadata;
  - stream sequence numbers, replay/drop validation outcomes, and close/backpressure semantics;
  - encrypted frame payload kind that can carry serialized `TransportIngress` and `TransportEgress` bytes without naming the concrete transport.
- Add deterministic test helpers or fake sealing keys in tests only. Reuse the existing `AesGcmKey` and `AesGcmEnvelope` helpers if sufficient; if deterministic nonces are needed for test-only repeatability, keep that helper private to tests or behind an explicitly named test seam so production nonce generation stays internal.
- Add a new architecture doc, likely `docs/architecture/encrypted-ordered-browser-transport-contract.md`, stating the canonical browser data plane: ordered WebRTC DataChannel carries Botster E2E encrypted client stream frames, while local/cloud signaling and pairing UX are hub/provider concerns.
- Update README boundary docs only where needed to name the new core-owned encrypted stream contract and preserve the ban on concrete negotiation, browser, Rails, cloud, and provider policy.
- Add focused tests that prove:
  - ordered sequence validation accepts the next frame;
  - duplicate/replayed sequence numbers are rejected or dropped as defined;
  - out-of-order sequence numbers are rejected or held/dropped as defined by the contract;
  - representative `TransportIngress` and `TransportEgress` values serialize into the encrypted envelope and round trip back through deterministic test keys/fakes;
  - source/doc boundary guards keep concrete terms out of core session/client contracts.

## Non-Scope

- No `RTCPeerConnection`, ICE, DataChannel API, browser worker, Rails, cloud signaling, localhost signaling, hub admission, package policy, or UI implementation in `botster-core`.
- No ActionCable, WebSocket relay, QR rendering, IndexedDB, OS keychain, or provider persistence implementation.
- No concrete WebRTC adapter, hub/client worker wiring, or production browser implementation.
- No raw private key material exposed in core public types.
- No broad refactor of existing `TransportIngress`, `TransportEgress`, `ClientStreamHarness`, `SessionIo*`, or crypto modules beyond the minimal exports and tests needed for this contract.
- No speculative support matrix, multi-protocol negotiation, or dual old/new compatibility branch.

## Assumptions And Unknowns

- Assumption: the new contract should be scaffold-level but executable through tests: production entry points do not change yet because the ticket is explicitly not a concrete WebRTC implementation.
- Assumption: current `TransportIngress` and `TransportEgress` are already the correct plaintext payload contract; the new work wraps their serialized bytes rather than adding browser-specific frame variants.
- Assumption: ordered delivery should be enforced above the concrete transport as a sequence/replay validator even though the canonical browser transport is ordered. This guards reconnect, duplicate delivery, and misuse by alternate adapters.
- Assumption: AES-GCM is acceptable for deterministic fake tests of sealing mechanics, but the public contract should not claim that AES-GCM is the final browser E2E ratchet.
- Olm decision: old Botster context favors direct vodozemac/Olm over Matrix SDK, but this ticket should not add a concrete `vodozemac` dependency unless implementation proves core needs ratchet semantics that cannot be expressed through key ids, sealed payloads, and transcript abstractions. If proposed, the dependency must sit behind the core crypto contract and the plan/review evidence must explain why the mechanism belongs in core.
- Unknown: exact replay policy for future production reconnects. The implementer must choose one explicit contract behavior for duplicate and out-of-order frames, document it, and test it. Prefer fail-closed/drop with typed observation over buffering unless buffering is required by existing core semantics.
- Unknown: whether to place the module under `contract` or `identity`. Prefer `contract` if it primarily describes stream frames and ordering; prefer `identity` only for key/transcript identifiers. Avoid splitting unless the code shape clearly demands it.

## Affected Surfaces And Files

- `crates/botster-core/src/contract/encrypted_stream.rs` or similarly named focused module: new public contract types and validator helpers.
- `crates/botster-core/src/contract/mod.rs` and `crates/botster-core/src/lib.rs`: public re-exports.
- `crates/botster-core/src/identity/crypto.rs`: only if existing `AesGcmEnvelope` needs small metadata/key-id support or a mechanism-level sealed-payload wrapper. Avoid changing production nonce APIs casually.
- `crates/botster-core/tests/encrypted_stream_contract_test.rs`: new contract tests for ordering, replay/drop, close/backpressure semantics, and transport frame round trips.
- `crates/botster-core/tests/boundary_test.rs`: guard that core session/client contracts still exclude concrete browser/WebRTC/Rails/cloud terms while allowing the architecture doc to name the canonical browser data plane.
- `README.md`: boundary ownership table and explicit ban/preserve language if new public contract family must be discoverable.
- `docs/architecture/encrypted-ordered-browser-transport-contract.md`: canonical data-plane architecture doc.

## Risks

- Concrete transport leakage: naming WebRTC/DataChannel/browser/Rails/cloud in core public session/client types would violate the ticket and existing boundary tests.
- Crypto overcommitment: adding vodozemac or Olm-shaped public types too early could fossilize old product implementation details in reusable core.
- False proof: tests that only instantiate new structs would miss the actual runtime path. The critical proof is serializing real `TransportIngress`/`TransportEgress` through the encrypted ordered frame abstraction and validating sequence behavior.
- Exhaustive public enum churn: adding variants to existing public enums can break downstream consumers. Prefer additive new structs/enums in a new contract module unless an existing enum is the natural public surface.
- Non-deterministic crypto tests: production encryption uses random nonces, so tests must compare decrypted payloads and metadata, not ciphertext equality, unless using a narrowly scoped fake.
- Backpressure ambiguity: if close/backpressure semantics are documented but not represented in testable types, future adapters will reinterpret them. Add explicit typed outcomes or frame flags.

## Acceptance Checks And Tests

- Focused tests:
  - `cargo test -p botster-core --test encrypted_stream_contract_test`
  - `cargo test -p botster-core --test crypto_test`
  - `cargo test -p botster-core --test client_stream_contract_test`
  - `cargo test -p botster-core --test boundary_test`
- Contract-only build:
  - `cargo test -p botster-core --no-default-features --lib`
- Repo gates from README:
  - `cargo fmt --all -- --check`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --workspace`
  - `cargo test --doc --workspace`
  - `cargo doc --workspace --no-deps`
- Runtime/user-path proof for this scaffold-only ticket: tests must drive representative `TransportIngress` and `TransportEgress` values through the exported encrypted ordered frame API, decrypt them, validate ordering, and deserialize the original frames. The architecture doc must state that production WebRTC wiring is intentionally out of scope.
- Boundary proof: source/doc guard should allow the architecture doc to describe the canonical WebRTC data plane, but should keep `RTCPeerConnection`, `ICE`, `DataChannel`, `browser`, `Rails`, `cloud`, and `localhost signaling` out of core session/client contract code and tests where the ticket forbids concrete transport names.

## Pipeline Gates And Artifacts

- Plan gate artifact: this file plus `botster_plan_gate` evidence.
- Implement gate should attach exact changed files, the focused test outputs, README cargo gate outputs, and the explicit runtime-path proof described above.
- Review should check correctness, boundary leakage, crypto overcommitment, missing sequence/replay tests, unwired exports, public API docs under `missing_docs = "warn"`, and PII/path leaks.

## Vault Gaps Worth Capturing

- Capture if confirmed during implementation: the extracted `botster-core` repo has no `test.sh` wrapper, so this project uses README cargo gates even though broader Botster notes often reference `BOTSTER_ENV=test`/`cli/test.sh`.
- Capture if the implementation settles it: whether Botster core should expose an Olm-shaped encrypted envelope abstraction or stay algorithm-neutral with key ids, sealed payloads, transcript ids, and test fakes.
- Capture if the sequence policy is chosen: the exact replay/out-of-order behavior for encrypted ordered client streams, because browser reconnect and provider adapters will need the same vocabulary later.

## Convention Check

- No convention conflicts found. The plan keeps product workflow policy in plugins/providers, keeps core transport-neutral, preserves existing typed transport frames, avoids speculative abstractions beyond the requested contract, and writes vault context as note titles/wiki links rather than local home paths.
