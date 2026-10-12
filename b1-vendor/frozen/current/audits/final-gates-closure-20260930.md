# Final gates closure (2026-09-30)

Owner: plan auditor. This record states the final result of gates 2 to 5 for the joint package in `contracts-v1-joint-manifest-final3.md`. It closes the items that `final-gates-recheck-20260930.md` left open.

## Results

| Gate | Result | Evidence |
|---|---|---|
| 2. Cross-contract consistency | **PASS** | `final-gates-recheck-20260930.md`: M1–M6 closed. The remaining lows are fixed in HC parity45 and HP J4e, or are accepted-open (below). |
| 3. Requirements coverage | **PASS** | 181 rows. The recheck closed 13 of 16 PARTIAL rows. **PAR-3** (and **UD-4**, which depended on it) is closed by HP J4e: the Lua LN-2 `session_types.spawn` row now matches App A and CA-14 (Sol Hub B, J4e CANDIDATE ACCEPT). #16 is a plan item, not a contract clause. No PARTIAL or GAP remains. |
| 4. Suite traceability (every clause names a test id; J1–J7 match) | **PASS** | The recheck closed all 18 earlier items. **LB-16b** is closed by Lua r6 (`conf::lb_16b_update_and_spawn_arguments_match_the_hp_rows`; Sol Hub B CANDIDATE ACCEPT). The final journey text is in `gate4-suite-traceability-20260930.md`, section "Final journey text" (J3 relay wording and J7 `permission_denied` are corrected). Running the suites is a BUILD acceptance gate. |
| 5. Stability | **PASS** | `final-gates-recheck-20260930.md`: every item is closed except C-33 (accepted-open). |

## Accepted-open items (low; deliberately deferred)

- **Codec** (the first codec revision after v1): the `attach_failed` reasons `snapshot_too_large` and `bad_peer` (the reason set is non-exhaustive, so a client shows "other"); L5 (a refusal at `file_begin`, compatible with TS-5f; Sol Core accepted this disposition); C-33 (Hub-only text in the extract). A codec change now would force a Core re-pin for behavior that is already fixed.
- **Core wording** (behavior is correct and tested): DP-4 still says "applies the route's paste policy (OU-1)"; the OU-8 sentence form.
- **ui 1.2 frozen file:** UI12:37 "deleted with its plugin on unload" is superseded by the accepted ui 1.2 successor2, which cites HP AS-4.
- **#16** (Hub-internal ownership): a build-plan item.

## Plugin parity

- Plugin parity PAR-1 to PAR-27 and client parity PAR-28 to PAR-45 are inside gate 3's 181 rows.
- The user rulings U-1 to U-8 and UC-1 are applied, or ruled out by the user (the six USER-RULED-OUT rows in `final-gate-3-20260930.md`).
- The zero-regression requirement (UD-4) is COVERED.
