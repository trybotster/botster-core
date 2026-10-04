# Shared status file scope review

Exact reviewed head: `d716a250245ee90129744527b01cdfc50e16f672`.
Branch: `stage1/lists-withdrawn-scope`.
Base: `origin/v1` at `f96a9dfde243cd9a125c0d1d553ede284a89d596`.

VERDICT: CLEAN (0 open) for this tool correction on the exact head above.

## Scope and rule

The lead assigned this separate correction after the P2 Linux gate failed on a shared withdrawn ID from another contract.
The lead requires every withdrawn or deferred ID to exist in the pinned `conformance/ledger.json`.
Core applies only entries whose ledger `contract` is `core`.
This review includes whole-ID deferrals and `not-applicable` cases because their checks use the same shared files.

The delta changes only `xtask/src/lists.rs`.
The reviewer read the complete delta and the surrounding check, command, and ledger generation paths.
The reviewer ran no tests, builds, or gates.

## Accepted behavior

- `all_ids_of_ledger` and `core_ids_of_ledger` read the same pinned ledger document.
- `command` passes every shared status ID to `check`. It no longer discards unknown deferral IDs before validation.
- Withdrawn IDs outside the full pinned ledger produce an error.
  Withdrawn IDs from another contract do not enter Core conflict checks.
  Core withdrawals still cannot be pending or deferred.
- Whole-ID deferrals outside the full pinned ledger produce an error.
  `core-deferred.toml` must equal the Core subset of the shared deferrals.
  Other contracts' deferrals do not require Core entries.
- Unknown `not-applicable` IDs produce an error.
  Other contracts' cases are skipped. Core cases must remain active.
- The summary counts only Core withdrawals. The case output includes only Core cases.
- The exact-copy checks for the pinned shared files remain in place.
  The Core pending, ledger, authority, start-condition, and base comparisons remain in place.

## Verification evidence and limits

The new tests accept another contract's ID in all three shared status categories without applying it to Core.
They also check that a neighboring Core withdrawal still rejects a pending entry.
Unknown withdrawn, deferred, and `not-applicable` IDs retain error coverage.
The ledger extraction test distinguishes the full ledger from its Core subset.

The implementer reports 95 xtask tests passed, clippy passed, fmt passed, and `cargo xtask lists` passed on the base pin.
The implementer states that this proof used the same code before the commit.
This is source acceptance of the exact committed correction; it is not a gate result.

P2 finding P37 remains open until the merged correction and regenerated lists receive a P2 delta review.
P2 finding P32 still requires a green Linux gate on the exact reviewed P2 head.
