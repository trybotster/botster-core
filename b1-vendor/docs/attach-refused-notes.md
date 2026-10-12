# attach-refused notes (steward ruling R-19)

`CoreApi::attach` took the `RouteTransport` by value and returned `Result<AttachResult, CoreError>`, so a synchronous refusal dropped the caller's stream endpoint. DP-2 gives the transport to Core only "from a successful attach return", and AM-4 says a sync refusal takes nothing. The ruling calls the signature a contract-crate defect, not a contract change.

## The change

- `botster-core-contract`: new `AttachRefused { error: CoreError, transport: RouteTransport }` (`#[non_exhaustive]`, with `AttachRefused::new`), in the prelude. `CoreApi::attach` returns `Result<AttachResult, AttachRefused>`.
- `botster-fake-core` (frozen): a signature adaptation only. The inner `attach` borrows the transport (it only reads its kind) and gives the same refusals. The trait method wraps every `Err`, so every early return and every injected refusal returns the transport.
- `botster-core-conformance`: the two test wrappers follow the new signature. `FakeHarness::attach_stream` builds its own endpoint, so on a refusal it only reports the `CoreError` (harness-owned cleanup: nothing of an embedder is returned or dropped).
- Test: `crates/botster-fake-core/tests/attach_refused.rs`. The transport is a non-`Clone` stream with an identity and a flag that its `Drop` sets. For each sync refusal a test can trigger (`UnknownSession`, the A2-1 state refusal, `InvalidInput` twice, `RouteLimit`, `Unsupported`), the refusal returns the same stream, not yet dropped, and dropping it afterward is the caller's act.
- No transcript, schema or TS file changes: `AttachRefused` is a Rust-only shape (the C ABI keeps a descriptor number on an error return).

## Entry-point audit

- `attach` (stream or WebRTC transport): fixed above.
- `Op::AttachWebRtc { client, session, offer, expected_fingerprint, options }` through `begin`: the offer and the fingerprint are strings. The caller owns no descriptor that Core takes, so a sync refusal drops nothing the caller needs. Not changed.
- `RouteTransport::WebRtc` passed to `attach`: carried back too (it is the same enum), checked in the `Unsupported` test.
- Other `Op`s and `open`: take no transport or descriptor.
