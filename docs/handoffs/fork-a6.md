# Handoff: Ghostty fork A6 (R-32) implementer — PAUSED 2026-10-04

**Nothing unpushed.** Fork local head = origin head; Core local head = origin head. The only dirt in the Core worktree
is the spawner's `.gitignore` (never commit it; `git restore .gitignore` before a gate). No botsterq or testq job runs.

## People

- Lead `sess-1790903471-008f-8b9f78eef51d48aba5a45748495fd673`.
- Reviewer `sess-1791171305-0113-e9a8c18db6f6331ae5c2274348370f8d` (sol-high, branch `stage1/review-fork-a6`, file `verdicts/fork-a6.md`).
- Integration reviewer (binding PR) `sess-1791168757-0109-73d2ca212653045545e7480ab60be9a9`. P3 implementer `sess-1791168825-010a-e873347e8fdb315566b5545e5c2d418b` (told about set_clipboard_limit / option 39).
- Brief `brief-fork-a6.md`. USER RULE: push only to origin = trybotster/ghostty (`git remote get-url origin` = `git@github.com:trybotster/ghostty.git` before every push, explicit `git push origin <branch>`); never contact ghostty-org (only `git fetch ghostty-org`).

## Fork (trybotster/ghostty)

- Branch `botster/upstream-sync-20261004` (new; no other `botster/*` branch changed), head **`0bfddc16fdf1e9b71f7662fbfa8314cd497fd92a`**.
- Upstream base: ghostty-org `main` **`5dc28bb8eebaf57a6c793a406bfea8c632d4fa94`** (#14509), fetched 2026-10-04. Old pin `3f8eb6810` on `f523504ea`.
- Synced stack head `c370ef4d9` (22 rebased commits). Decisions: KEEP 21; REWORK 1 = patch 1 first commit (`13f66d661`): options renumbered 44/45 -> 46/47 because upstream #14483 (DECRQCRA/XTCHECKSUM) took 44/45; DROP 0. Patch 8 first commit changed only by a context line. DECRQCRA is not reported as a query (off by default; binding never enables it).
- Patch 14 (NEW): `779907e0e` the R-32 patch (C API: over the OSC 5522 limit the callback still fires with `too_large`, no contents, `total_len` = decoded size of the whole transaction incl. replaced regions, aliases add nothing; no EFBIG from the model; app path keeps EFBIG) + `0bfddc16f` R-33 tests (ignored MIME types past 64 are not decoded, not counted, not against the limit).
- Watched upstream PRs: merged #14483, #14508, #14515; not merged #14439, #13332, #14362, #14200.
- Upstream changed Zig package `iterm2_themes`: `N-V-__8AAM94...` -> `N-V-__8AAAfDBACe1jGqjr9jIG3UAK8KbzJKQ7xrzYoau-_a` (in `build_data.rs`).

## Contract state

- R-32 (contracts 9a00db8): implemented by patch 14.
- R-33 (contracts 14c86ab): ignored MIME types are outside the decoded size (reading (a)); pinned by 0bfddc16f tests.
- Core A14 FINAL (final33, contracts 69327d5): two steps (decoded size, then contents size with aliases at full length); model decode limit = `clipboard_bytes`. **Option 39 is set in production** by `Terminal::set_clipboard_limit` (called by `Terminal::new` and the snapshot restore). A6 status: implemented; verification pending. The a14_* conformance ids stay pending (P3 owns the worker side in M2b).

## botster-core binding PR (no PR opened yet)

- Branch `stage1/p2-fork-a6` (from v1 `144b023`), head **`65b2064de09a7a8c72f4edf42d6e1c2f467708ed`**:
  f9eb114 submodule + `.gitmodules` -> fork branch, sys.rs 46/47 + ClipboardWrite fields, build_data iterm2 hash;
  cfd24e7 binding over-limit report; 59abb1f sync record + evidence dir + audit revision 11;
  054a1b5 option 39 = clipboard limit + submodule 0bfddc16f + A14 test; e676a2d docs (A14, R-33); 65b2064 docs (F-A6-03).
- Sync record: `docs/stage1/ghostty-upstream-sync-20261004.md`; evidence `docs/stage1/ghostty-upstream-sync-20261004/` (README, mac-zig.sh, first Mac log of c370ef4d9). Audit: `docs/stage1/libghostty-audit.md` revision 11 (G19 row). GHOSTSNP.md unchanged (snapshot sources identical).
- Review: round 3 **NOT CLEAN (1 open)**, verdict `debe4bb` at fork 0bfddc16f / Core 65b2064. F-A6-01 closed (R-33), F-A6-03 closed. **Only F-A6-02 HIGH open: required test evidence.** No code or doc finding open.
- Nothing has been compiled or tested since the sync head's first Mac run (c370ef4d9: test-lib-vt exit 0, lib build exit 0, no Build Summary).

## Exact next step (when the lead lifts the pause/HOLD)

1. Mac: `botsterq run --exclusive --label "fork-a6 zig" -- /bin/bash -c "E=~/botster-sessions/shared/core-stage1/evidence-fork-a6; $E/mac-zig.sh ~/botster-sessions/ghostty-fork-a6 $E/mac-0bfddc16f; $E/mac-zig.sh ~/botster-sessions/ghostty-fork-a6-synchead $E/mac-c370ef4d9"` (Zig 0.16.0 from mise; retry `zig build --fetch=all` on TlsInitializationFailed).
2. Empty-cache Zig package list at 0bfddc16f with `GHOSTTY_BUILD_ARGS` (compare with `build_data.rs` ZIG_PACKAGES, 7 hashes).
3. Core: `git submodule update --init crates/botster-terminal-ghostty/vendor/ghostty`, then focused binding tests `env -u RUSTUP_TOOLCHAIN CARGO_BUILD_JOBS=4 NEXTEST_TEST_THREADS=4 cargo nextest run -p botster-terminal-ghostty` (via botsterq).
4. Tell the lead before any Linux gate: the fetch step needs fork commit 0bfddc16f and the new iterm2_themes package.
5. Fill "Test results" in the sync record, copy logs into the evidence dir, push, send the reviewer the evidence.
6. Reviewer CLEAN -> open the PR (Prior art note) -> integration reviewer -> one gate -> READY to the lead.

## Worktrees and leftovers

- Fork `~/botster-sessions/ghostty-fork-a6` (branch above, no upstream tracking); detached `~/botster-sessions/ghostty-fork-a6-synchead` at c370ef4d9 (for step 1).
- Core `~/botster-sessions/trybotster-botster-core-stage1-p2-fork-a6`.
- Local-only branch `wip/fork-a6-r32` in ~/Projects/ghostty (stale draft, never pushed; `branch -d` refuses, leave it).
- Scripts/raw logs `~/botster-sessions/shared/core-stage1/evidence-fork-a6/`.
