# Core Crypto and Identity Utility Surface Plan

## Context Loaded

- Pipeline context: ticket `ticket_1780014899_920149`, run `run_1780018174_373351`, step `botster_plan`, gate `botster_plan_gate`.
- Human answer `question_1780022428_780337`: this run is correctly targeted at `botster-core`; old TryBotster paths are reference evidence only.
- Playbooks and vault notes loaded:
  - `planner-playbook`
  - `botster-planner-playbook`
  - `botster-architecture`
  - `cli-patterns`
  - `spa-patterns`
  - `project pipeline orchestration belongs in a device-level botster plugin`
  - `project pipelines needs an operator workbench not more primitives`
  - `project pipelines ui contract belongs in the plugin readme`
  - `botster orchestration should spawn agents with explicit target ids`
  - `botster orchestration prompts must bind agents to explicit worktrees`
  - `plan steps need reviewable plan artifacts`
  - `botster hub registration must reconcile stable device fingerprints when local identifiers change`
  - `identity`
  - `goals`
- Reference evidence inspected from the existing TryBotster application checkout:
  - `cli/src/crypto.rs`: AES-256-GCM envelope with JSON fields `nonce`, `ciphertext`, and `version`; nonce/ciphertext are base64 strings.
  - `cli/src/device.rs`: public stored metadata is verifying key, fingerprint, and name; signing key stays in credentials; fingerprint is first 8 bytes of SHA-256 over public verifying key bytes, formatted as colon-separated hex.
  - `cli/src/keyring.rs`: concrete keyring/file fallback behavior is CLI policy; core should expose only boundary contracts.
  - Relay docs/tests: Rails relays opaque encrypted envelopes and must not inspect ciphertext; relay policy stays outside core.

## Architecture Decision

`botster-core` should ship a real, narrow AES-GCM reference implementation for the envelope utilities, not only traits plus fake test implementations.

Reason: the ticket acceptance explicitly requires encrypt/decrypt round trip and wrong-key failure. Those are cryptographic behavior checks, not only serialization checks. A reference implementation belongs in core because the README already names "narrow crypto/identity operation contracts" as a first extraction slice, and AES-GCM envelope encoding is a reusable mechanism/data shape shared by future CLI, hub, provider, and plugin callers. This is not a concrete transport adapter, cloud policy, OS keychain backend, or Rails auth flow.

The dependency increase is intentional and bounded to crypto primitives required by the ticket. Latest-version lookup was performed on May 29, 2026:

- `aes-gcm = "0.11.0-rc.4"` from crates.io via `cargo info aes-gcm`
- `base64 = "0.22.1"` from crates.io via `cargo info base64`
- `rand = "0.10.1"` from crates.io via `cargo info rand`
- `sha2 = "0.11.0"` from crates.io via `cargo info sha2`
- `ed25519-dalek = "3.0.0-rc.0"` was checked, but should not be added for this slice unless implementation proves it is required. Core can express signing contracts over opaque public key/signature bytes and avoid committing to an Ed25519 implementation in this ticket.

`rustc --version` reports `rustc 1.92.0`, so the current workspace toolchain can satisfy the Rust 1.85 minimum reported by the new crypto crates.

## Scope

In scope:

- Extend `src/crypto.rs` with a concrete AES-GCM envelope utility surface:
  - `AesGcmEnvelope`
  - `AesGcmKey`
  - `CryptoError`
  - `encrypt_aes_gcm(key: &AesGcmKey, plaintext: &[u8], version: u8) -> Result<AesGcmEnvelope, CryptoError>`
  - `decrypt_aes_gcm(key: &AesGcmKey, envelope: &AesGcmEnvelope) -> Result<Vec<u8>, CryptoError>`
- Keep the envelope JSON shape stable:
  - `nonce: String`
  - `ciphertext: String`
  - `version: u8`
  - nonce and ciphertext encoded with standard base64
- Add `src/device.rs` for public identity data and fingerprint helpers:
  - `DevicePublicMetadata`
  - `DeviceFingerprint`
  - `PublicSigningKeyBytes`
  - `device_fingerprint(public_key: &[u8]) -> DeviceFingerprint`
  - `verify_device_fingerprint(public_key: &[u8], expected: &DeviceFingerprint) -> bool`
- Add `src/keyring.rs` for credential-store and non-exportable signing boundaries:
  - `CredentialStore`
  - `CredentialStoreError`
  - `CredentialRecord`
  - `NonExportableSigner`
  - `SigningKeyHandle`
  - `SignatureBytes`
  - `SigningError`
- Update `src/lib.rs` to export the new modules and the narrow public types/functions.
- Update `README.md` to document:
  - core owns the envelope implementation, public metadata shape, fingerprint helpers, signing/keyring contracts
  - CLI owns device config files, OS keychain/file fallback, prompts, and signing-key persistence
  - Rails/relay owns ActionCable/auth/registration policy and treats encrypted envelopes as opaque
- Add focused tests for every acceptance item.
- Add only the necessary dependencies to `Cargo.toml`.

Out of scope:

- Rails auth, device-code flow, cloud provider registration, billing, provider credential policy.
- ActionCable, relay routing, hub registration, or browser signaling implementation.
- OS keyring integration, file fallback storage, config directory resolution, hostname defaults, WSL/Linux/macOS prompts, retry policy, or reset/re-auth guidance.
- Ed25519 signing-key generation/persistence unless needed only as a test helper; signing operations should be modeled as non-exportable contracts.
- Plugin, hub, TUI, SPA, Project Pipelines UI, or workflow changes.
- Broad package-boundary refactors unrelated to the crypto/identity slice.

## Assumptions and Unknowns

Assumptions:

- The current package is the intended target; old TryBotster paths are behavior evidence only.
- AES-GCM envelope behavior is a reusable core mechanism, not CLI policy.
- The first core envelope version can be serialized as the same `u8` version field from the old evidence. Core should preserve unknown versions during serialization/deserialization; callers own migration/negotiation policy.
- Fingerprints are stable identity anchors and should be derived from public key material only. This supports the hub-registration reconciliation rule where local identifiers may change but the fingerprint remains authoritative.
- Public metadata should contain no private key material and should be safe to serialize.

Unknowns for implementation to resolve narrowly:

- Whether `AesGcmKey` should be a `[u8; 32]` newtype or a type alias. Prefer a newtype if it improves error messages and avoids passing arbitrary byte slices.
- Whether `DevicePublicMetadata.public_key` should be named `verifying_key` to match current CLI evidence, or use a more algorithm-neutral name. Prefer `verifying_key` if the code keeps Ed25519 identity semantics visible; otherwise document the algorithm-neutral shape.
- Whether `CredentialStore` needs associated types or can stay simple with string keys and `CredentialRecord` values. Prefer simple unless tests reveal type noise.

## Affected Surfaces and Files

Expected changes:

- `Cargo.toml`
- `README.md`
- `src/crypto.rs`
- `src/device.rs`
- `src/keyring.rs`
- `src/lib.rs`
- `tests/crypto_test.rs`
- `tests/device_test.rs`
- `tests/keyring_test.rs`

Reference-only files outside this worktree:

- TryBotster CLI crypto, device, and keyring modules
- TryBotster worker actor contract docs
- old Rails channel/request tests around opaque signaling envelopes

## Acceptance Mapping

- Encrypt/decrypt round trip:
  - `tests/crypto_test.rs` creates an `AesGcmKey`, encrypts plaintext, then decrypts to identical bytes.
- Wrong-key failure:
  - `tests/crypto_test.rs` decrypts with a different `AesGcmKey` and asserts `Err(CryptoError::DecryptFailed)` or equivalent authenticated failure.
- Serialized envelope round trip:
  - `tests/crypto_test.rs` serializes `AesGcmEnvelope` through `serde_json`, deserializes, verifies `version`, `nonce`, and `ciphertext` survived, then decrypts successfully.
- Nonce/ciphertext/version serialization:
  - `tests/crypto_test.rs` asserts JSON contains only string `nonce`, string `ciphertext`, and numeric `version`; malformed base64 and wrong nonce length return errors.
- Public metadata excludes private key material:
  - `tests/device_test.rs` serializes `DevicePublicMetadata` and asserts it includes only public fields and does not contain `signing_key`, `private_key`, `secret`, `token`, `credential`, or raw secret field names.
- Fingerprint verification helpers:
  - `tests/device_test.rs` asserts deterministic fingerprint format from public key bytes, matching verification success, and mismatch failure.
- Non-exportable signing operation contracts:
  - `tests/keyring_test.rs` uses a fake `NonExportableSigner` that signs through a handle without exposing key bytes; public serializable structs must not include private key bytes.
- Credential-store boundary traits:
  - `tests/keyring_test.rs` implements an in-memory fake `CredentialStore` and verifies get/set/delete error behavior without OS keyring or filesystem policy.
- Source/docs contain no personal/company PII:
  - Review `docs/archive/plans/core-crypto-identity-utility-surface.md`, `README.md`, `src/**/*.rs`, and `tests/**/*.rs` for real names, personal emails, personal hostnames, internal absolute paths copied from the old checkout, real tokens, and company/customer identifiers.

## Verification

Implementation should run:

- `cargo fmt --check`
- `cargo test`
- `cargo clippy --all-targets --all-features -- -D warnings`
- A PII/source scan for personal names, personal host paths, real token
  prefixes, and private-key field names across README, docs, source, and tests.

The PII scan must be interpreted carefully: test assertions may intentionally mention forbidden field names such as `private_key` or `signing_key` to prove absence. Those are acceptable when they are assertion strings, not serialized data, example secrets, or copied real identifiers.

## Runtime or User Path Evidence

This ticket is an extraction/scaffold slice for `botster-core`. The production CLI, hub, and Rails relay paths in the full TryBotster application are intentionally not rewired in this run.

The runtime-facing evidence for this slice is:

- `botster-core` public exports in `src/lib.rs`
- focused tests proving the reusable core behavior
- README boundary documentation explaining that future CLI/hub consumers should import the core envelope, metadata, fingerprint, signing, and credential-store surfaces rather than duplicating them

If implementation chooses to wire a consumer despite this plan, it must show the concrete production entry point and keep the change inside this ticket's boundaries.

## Risks

- Adding crypto crates broadens the dependency surface. Keep dependencies limited to AES-GCM, base64, randomness, and SHA-256; avoid Ed25519 implementation dependency unless strictly necessary.
- `aes-gcm` and `ed25519-dalek` latest reported versions are release candidates. For this plan, only `aes-gcm` is required. If the implementer elects not to use an RC, they must document the stability tradeoff and verify the selected stable version before implementation.
- Ambiguous envelope version policy could create future compatibility bugs. This slice should preserve the `version` value and leave migration policy to callers.
- Public metadata can accidentally leak private material if runtime and serializable types are mixed. Keep runtime signing traits separate from `DevicePublicMetadata`.
- Fingerprint helpers must derive from public key bytes only; tying fingerprints to local identifiers would recreate the registration bug described in the vault.
- Contract tests can accidentally become too fake. AES-GCM tests must exercise real encryption/decryption, while keyring tests should remain fake because OS keyring behavior is out of scope.

## Vault Gaps Worth Capturing

Capture after implementation once final names are known:

- `botster-core` owns reusable crypto/identity envelope and public metadata contracts, while concrete CLI device config/keyring persistence and Rails relay/auth policy remain outside core.
- Stable device fingerprints are the identity anchor across local identifier changes; core fingerprint helpers should derive from public key material only.
- The chosen crypto dependency posture for `botster-core`, especially whether the implementation accepted latest release candidates or pinned stable alternatives.
