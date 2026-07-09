# Ghostty Shadow-Terminal Architecture

Botster's authoritative terminal screen and snapshot state belongs on the
core-side runtime path. Ghostty is the blessed backend path for that
shadow-terminal role because tmux-like attach, detach, recovery, and replay need
one server-side source of terminal truth.

This does not make the `botster-core` crate a Ghostty crate. `botster-core`
continues to own the backend-neutral `TerminalScreenRuntime` seam,
`TerminalScreenEngine`, `TerminalSnapshotPayload`, and related terminal screen
contracts. Those contracts are the reusable boundary that hosts implement.

Concrete Ghostty integration belongs in `botster-terminal-ghostty`. That crate
depends on `botster-core`, names the Ghostty adapter home, and keeps future
libghostty build, FFI, snapshot format, and parser policy out of the reusable
core contract crate.

restty is a client/web rendering path. It may consume Botster terminal state,
streams, and snapshots through the client data plane, but it must not be used as
core shadow-terminal infrastructure and must not own authoritative terminal
truth.

This note reconciles the Ghostty direction with
`docs/archive/plans/terminal-screen-snapshot-boundary.md`: that earlier boundary remains
correct for `botster-core`. The later architecture decision is placement of the
blessed concrete backend path in the sibling Ghostty crate, not a rewrite of the
core crate's neutrality.

Runtime wiring is intentionally deferred. The production path remains the
existing session runtime and terminal screen seams until a later implementation
adds a real Ghostty-backed adapter and proves attach, snapshot, and recovery
behavior through the SessionIo/ClientWorker data path.
