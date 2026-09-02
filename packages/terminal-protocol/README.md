# @trybotster/terminal-protocol

Core-owned types-only terminal protocol for Web and other Node consumers.

This package is generated from `botster-terminal-protocol-client`. It is not a
Hub Git revision. Pin this package, not a Hub commit, for terminal frame types.

It ships generated TypeScript, package metadata, encode helpers for compact
binary input frames, and the ready-then-history event-order fixture. GHOSTSNP
goldens stay in the Rust protocol crate.

The required feature token is `transport=duplex_binary`. Conformance fixture
revision is 2. Protocol name and version stay `botster-terminal-v1` / `1`.

Hub adapters must not import this package to inspect Snapshot bodies or attach
phases. Hub depends only on the Rust crate `botster-terminal-protocol`.

Use `encodePaste(operationId, modeGeneration, modeRevision, data)` for a paste.
The helper returns Begin, ordered Chunk frames, and Commit. Use
`encodePasteAbort(operationId)` before worker submission to abort an operation.
The Rust equivalents are `encode_paste` and `encode_paste_abort`.

Operation ids must strictly increase for each terminal subscription generation.
Paste content must contain 1 through 1,048,576 bytes. Clients must not split a
paste with a separate chunk policy. Core owns segmentation, mode fencing,
timeouts, admission, and the single `input_result`.
