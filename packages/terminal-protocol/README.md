# @trybotster/terminal-protocol

Core-owned types-only terminal protocol for Web and other Node consumers.

This package is generated from `botster-terminal-protocol-client`. It is not a
Hub Git revision. Pin this package, not a Hub commit, for terminal frame types.

It ships generated TypeScript, package metadata, encode helpers for compact
binary input frames, and the ready-then-history event-order fixture. GHOSTSNP
goldens stay in the Rust protocol crate.

The advertised feature token is `transport=duplex_binary`. The explicit
requirement constructor `for_duplex_binary_transport()` requires it. Conformance
fixture revision is 2. Protocol name and version stay `botster-terminal-v1` / `1`.

Hub adapters must not import this package to inspect Snapshot bodies or attach
phases. Hub depends only on the Rust crate `botster-terminal-protocol`.
