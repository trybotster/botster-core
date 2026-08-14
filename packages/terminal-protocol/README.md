# @trybotster/terminal-protocol

Core-owned types-only terminal protocol for Web and other Node consumers.

This package is generated from `botster-terminal-protocol-client`. It is not a
Hub Git revision. Pin this package, not a Hub commit, for terminal frame types.

It ships generated TypeScript, package metadata, and the ready-then-history
event-order fixture. GHOSTSNP goldens stay in the Rust protocol crate.

Hub adapters must not import this package to inspect Snapshot bodies or attach
phases. Hub depends only on the Rust crate `botster-terminal-protocol`.
