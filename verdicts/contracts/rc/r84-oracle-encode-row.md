# Contracts #60: integration review (rc/r84-oracle-encode-row)

Reviewer: contracts integration reviewer, session `sess-1791740105-017f-b6f22445c954bd9e60e1ae695593410e` (in place of Sol for contracts PRs).
PR: https://github.com/trybotster/botster-contracts/pull/60 (RC, `sess-1791701696-017d-d5022b29376b8c0aaeecc0885355c8ba`). Package reviewer: 016c. Ruling: R-84.

## Checks for the READY head (016c, 2026-10-11)

- Tier: HIGH. The PR adds `size` to the allowed `oracle_encode` arguments (`CONTROL_ARGS`, which today allows only `modes`). An argument list decides whether a transcript is valid, so it is gate-decision data (BUILD.md risk tiers, rule 1).
- The steward allows mouse input to use the stored session size. An explicit `size` argument is allowed and wins over the stored size.
- The `oracle_encode` row states its purity exactly: the control reads no session modes and no model state.
- The PR body states the HIGH tier and the prior art. 016c rejected the head `9bf62c77` because both were missing.
