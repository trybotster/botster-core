# Integration review: #161 (the contracts pin to contracts-v0.1.18; P5 since the lead's hand-over from P6)

Reviewer: integration reviewer (Opus), `sess-1791517267-0168-401cc27284f329b42ec84da0de9d91e0`. This reviewer runs no
build, test or gate. Until now #161 was a pin move only (Cargo files and lock), which the lead exempted from integration
review (`pause3-status.md`). The delta now changes a wire type in `botster-core-link` and its use in `botster-core-host`,
so it is cross-package.

## Round 1 — CLEAN on head f8157885 (delta from the P6 reviewer's CLEAN at dea90ed4)

Reviewed head: `f81578858b137528aca82496904e89d16d6262ea`, base v1 `c869dbeaf3a2922e4f4e7202f8e55490f7ce99fa` (the current
v1). Delta on `dea90ed4`: the merge `0fe5ba7a` of v1 `c869dbea`, and `f8157885` (7 files, +29 -25). The PR's gate log:
`shared/core-stage1/gate-logs/pin-v0.1.18-f8157885.log` (full Linux gate, green). This reviewer did not read it.

- **The merge.** `git merge-tree --write-tree dea90ed4 c869dbea` gives tree `3cd2dd78`, which is the tree of `0fe5ba7a`.
- **The pin.** Every contracts crate in `Cargo.toml` and `Cargo.lock` moves from `contracts-v0.1.17` (`1725abf`) to
  `contracts-v0.1.18` (`ae4abc9b`), and no other lock entry changes.
- **A13: the wire shape.** `Observation::ClipboardWrite` in `botster-core-link/src/msg.rs` now carries `contents:
  Option<Vec<ClipboardContent>>`. `ClipboardContent` comes from `botster_core_contract::prelude` (`msg.rs:12`), so the link
  has no second spelling of the event's type. `inbound.rs` passes `contents` through to `Event::ClipboardWrite` unchanged.
  - No production code builds `Observation::ClipboardWrite` at the head: the worker's clipboard path is P3's M2b. The
    binding's `ClipboardWrite` (#179) already has the same shape (`contents: Option<Vec<ClipboardEntry>>`, `None` when
    `too_large`).
  - Protocol 1 is not released, so no adoptable worker of another protocol sends the old `bytes` field (AD-4).
- **A15 and A16.** No Core code change: the new limits come in only through `..CoreLimits::default()`, and there is no
  `StopService` completion yet. `core-pending.txt` changes only the owner text of two A15 ids (P1 lifecycle per
  `owners.py`). The id count stays 675.

Carry (not counted), for P3's M2b: the worker must map the binding's `ClipboardWrite` to the observation one-to-one
(`contents: None` exactly when `too_large`, with `reason: Some(TooLarge)`). The host passes `contents` and `reason` through
and does not check that they agree. The `boundaries.rs` test sends `Some(contents)` with `TooLarge`, which tests the
pass-through only, not a valid pair.

VERDICT: CLEAN (0 open) at f81578858b137528aca82496904e89d16d6262ea
