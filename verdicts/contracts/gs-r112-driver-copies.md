# contracts #103 — gs-r112-driver-copies (R-112 (1)-(3): the Core driver, the shared matcher and the route codec stop copying), reviewer 017f

HIGH (R-112; rule 1: the shared driver and matcher decide pass or fail for every Core transcript, and the codec). Implementer: GS (sess-…017b). Package reviewer: 016c. PR base: main `5ce2cff1`. Gate base: main `192d70bc` (`5ce2cff1..192d70bc` is `docs/BUILD.md` only, #101). One commit, `62a3398c`. Files: `botster-conformance` `matcher.rs`, `order.rs`; `botster-core-conformance` `driver.rs`; `botster-route-codec` `bytes.rs`, `props.rs`.

## Round 1: CLEAN on 62a3398c04e8de25682bd8257651a4861f99057b

### The gate and the evidence

- Gate: `~/botster-sessions/gates/botster-contracts-gs-r112-driver-copies-62a3398c-pool-20261012-134554-73907.log`, sha256 `1209374351a91112f6a4d2af6fd8dbc73405a811a0cd9b198ac1986be801d085` (the body's value). All ten steps PASS. Mutants: 53 tested, 52 caught, 1 unviable. Exit 0 after 747 s on msa1.
- Core evidence: `~/botster-sessions/gates/botster-core-gs-r112-evidence-7747f377-pool-aa22.log`, sha256 `707a5f77f4589f8d33c4e53598cfd3bb291c186366db8a5525e218d83997e87f` (the body's value). Reversal log: `botster-contracts-gs-r112-reversal-62a3398c.log`.

### (2) Bindings: the exactness argument, checked

`bind_if` saves the key names, runs the attempt, and on a rejected result removes every name that was not there before. That restores the bindings exactly only when no write inside the attempt changes or removes an existing name. I checked this on the head:
- `Bindings` is `BTreeMap<String, Value>` (`matcher.rs:20`). So `before` is sorted, and `binary_search` is valid.
- The only write to the bindings in `matcher.rs` is `binds.insert` at `:299`. It runs only in the `None` arm, when the name is unbound. A bound name is compared ("differs from the bound variable") and never rewritten.
- Every `bind_if` in `order.rs` wraps `try_match` or `assign`, and both call only `matches`. The `ack` steps and the driver's binding writers (`bind_result`, `let_step`, the `bind` of `begin` and `attach`, `driver.rs:1104`, `:1359`, `:2051`, `:2306-2318`, `:2464`) run outside every attempt.
- So each replaced clone keeps its meaning:
  - commit on a match (`matches`, `$one_of`, `$set`/`assign_set`, `order`'s `expect_set`/`assign`, and the driver's `poll_find`, `pump_until`, `pump_collect`, `control_matches`, `get_matches`);
  - keep nothing (`$not`, `forbid`, `ignore`, the duplicate check), through `matches_without_binding` or `try_match_only`.
- The backtracking in `assign_set` and `assign` resets `used[j]` on a failed branch inside the attempt, as the old code did after it.
- `bind_tests` (4) cover a failed match, a match that adds only, `$one_of` and `$set` after a backtrack, and `matches_without_binding`. The reversal ("`bind_if` never removes") fails all four.

### (1) and (3)

- `pty_has` compares `min_bytes` with the length of the borrowed hex text (`hex_text`). `hex_of` keeps its copying form for its other callers. The meaning is unchanged.
- `hex_decode` decodes in one pass. An odd count is still the error over the whole text, and it takes precedence over a bad digit, as when the digits were collected first. The messages are unchanged. `props.rs` compares it with the old function (`hex_decode_by_digit_vec`), with 64 property cases and all 7381 texts of up to four characters over a mixed alphabet. The reversal ("bad digit first") fails all three tests.

### R-112's rules for (1)-(3)

- **The deterministic tier is identical** (Core v1 `2d8e8acc` at .46, before and after, seeds 0-31, strict): the sorted outcome diff is empty for the testkit tier and the held-full run. The real-tier differences are classified, with the #88 precedent. `am_2_one` failed only in the "before" tree. `ev_1` step 13 and `in_3` step 18 are recorded races at other seeds.
- **`dp_5`'s time, seeds 0-31:** the sum goes from 8.292 s to 6.321 s, and the max trial from 0.300 s to 0.227 s.
- **HIGH**, reviewed by 016c and 017f.
- **Backlog B-1 records the PR** (`shared/steward/contracts-backlog.md:64`).

### Checklist (R-115)

The PR changes no transcript, so the checklist does not apply.

### Landing notes (not findings)

- **Main moved.** Main is now `3ad0ce7e` (final43 publish and R-116 rulings). `git merge-tree --write-tree 62a3398c origin/main` is clean (`8831eca2`). That is not a rulings-only move, so the landing head needs a main merge, my Q7 receipt and a gate.
- **#102 conflicts with this PR.** `git merge-tree --write-tree 62a3398c a038119f` (#102's CLEAN head) reports `CONFLICT (content)` in `crates/botster-core-conformance/src/driver.rs`. Both change `control_matches`: #102 adds the dispatch to the route endpoint, and this PR removes the bindings clone. Whichever lands second needs a resolution by hand that keeps both: the `routed` dispatch of #102, and `matches(expected, Some(&got), bindings, &NoSchemas).is_ok()` without the clone. Then it needs my review of the resolution and a gate.

CLEAN on `62a3398c04e8de25682bd8257651a4861f99057b`.

## Resolution review and Q7 receipt: main merge 9bb30d06200e8f4599547c48649d3cfafa55dfbf

- **The parents** are the CLEAN head `62a3398c` and main `9f67150a`. Main contains #102 (`a038119f` is an ancestor).
- **The conflict.** The commit tree `438793a9` differs from `git merge-tree --write-tree` of the parents (`2f4db052`) only in the conflict hunk of `driver.rs` `control_matches`. The resolution keeps both sides:
  - #102's `routed` dispatch to the route endpoint, with the harness for every other control;
  - #103's call `matches(expected, Some(&got), bindings, &NoSchemas).is_ok()` with the R-112 (2) comment, and no clone of the bindings.
- **The patch-id.** The diff from main has the same 5 paths as #103. Without `driver.rs`, its stable patch-id `52984aed` is equal to the patch-id of `62a3398c` against its base `5ce2cff1`. In `driver.rs`, the added and removed lines (61) are identical to #103's own. Only the context lines differ, because #102's dispatch is now around the hunk.
- **016c** checked the same resolution and found no finding.
- **The gate on the exact head is green:** `~/botster-sessions/gates/botster-contracts-gs-r112-driver-copies-9bb30d06-pool-20261012-143912-332.log`, sha256 `faa2a5bb992a359a0a56f3a9dc98d60bedd9f02b24b6f36104414b0c7a85ce65`. All ten steps PASS. Mutants: 53 tested, 52 caught, 1 unviable. Ledger 2552. Exit 0 after 548 s on msa1, base main `9f67150a`.
- **Main** is still `9f67150a`, so main fast-forwards to `9bb30d06`.

Q7 receipt given for `9bb30d06200e8f4599547c48649d3cfafa55dfbf`.
