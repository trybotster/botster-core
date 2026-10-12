# Ghostty upstream sync, 2026-10-02

The Botster patch stack of the trybotster/ghostty fork, rebased onto the current upstream `main`, as the BUILD.md
"Ghostty fork policy" requires.
This file records the sync, a decision for each patch, the test results and the new pin candidate.
Stage 1 is paused. This sync moves no pin.

## Summary

| Item | Value |
|---|---|
| Old base | upstream `83edd491e3024ae5e50393d62877b8897da1cccd` ("Update VOUCHED list (#14510)") |
| Old stack head | `botster/vt-core-stage1-c` = `ada251c5e99cdb753de8bf72d5d1f307d474518f` |
| Binding submodule head | `ada251c5e99cdb753de8bf72d5d1f307d474518f` in botster-core `stage1/p2-libghostty` `f711184` (`.gitmodules` branch `botster/vt-core-stage1-c`). `v1` has no Ghostty submodule. |
| Ghostty pin recorded in the plan | `85a8d8eb197c5752887c017c9a3faa6f1dc1969b` (branch `botster/vt-core-stage1-b`). The P2 handoff (botster-core `v1` `docs/handoffs/p2-libghostty.md`, last changed in `0b6bee3e3855637fe7cd805d5b8787bd23aa2ac7`) states that `ada251c5e` is not recorded. |
| New base | upstream `main` = `f523504ea5c9f41d150d1eb93cc7a748b90f9361` (#14506), fetched 2026-10-02 |
| New stack head (pin candidate) | `botster/upstream-sync-20261002` = `3f8eb6810bb673aa782b047de21783ac81fb1121` |
| Patches | 22 commits in 14 patch groups (0 to 13). KEEP 21, REWORK 1 (patch 12, test only), DROP 0 |
| Conflicts | None. `git range-diff` shows all 22 commits as `=` (identical patches) |
| Zig | 0.16.0, unchanged (upstream `build.zig.zon` `minimum_zig_version` is unchanged) |
| Zig packages | Unchanged: no `build.zig.zon` changed, so `build_data.rs` `ZIG_PACKAGES` stays valid |
| `xterm-ghostty` terminfo | Unchanged (no change under `src/terminfo/`) |

## What upstream changed since our base

Upstream `main` has 3 commits plus 1 merge after `83edd491e`:

| Commit | Subject |
|---|---|
| `136029788` | macos: honor window-save-state=never for restorable windows |
| `934ef8140` | macos: update window restoration on config reload |
| `0e75d015f` | macos: simplify window restoration setup |
| `f523504ea` | merge of #14506 |

All three change one file: `macos/Sources/Features/Terminal/TerminalController.swift` (+11, −6).

### Benefits for Botster

None.
The changes are in the macOS application.
libghostty-vt does not compile that file.
`git diff ada251c5e 3f8eb6810` lists only that Swift file, so every other file of the tree, including every source file
that goes into `libghostty-vt.a`, is byte-identical between the old stack head and the new stack head.

The value of this sync is that it obeys the fork policy: the stack is now on the current upstream `main`, and it rebased
with no conflict.
The next patch to the fork can start from this branch.

### Risks and behavior changes

There is no behavior change in these four areas:

- **Core EV-8 queries** (query detection, request bytes, `vt_write_until_query`, typed replies): no change.
- **OSC 52** (parser, clipboard read and write callbacks, selection and terminator): no change.
- **Snapshots** (GHOSTSNP encoder and decoder, continuation, the decoder options): no change.
- **Input encoding** (key, mouse, focus, paste encoders): no change.

Scope of this result: it is a source comparison. The only changed file is a macOS Swift file that libghostty-vt does
not build, and the tests in "Test results" pass at the new head.
The open upstream pull requests in the next section touch the same files as our patches.

### Open upstream pull requests to watch (read-only check, 2026-10-02)

I searched upstream pull requests and issues with `gh search` (read-only).
The search scope is the keyword queries and result limits listed in `evidence-20261002/README.md` ("Upstream search
scope"). No result of those searches implements one of our patches. The search is not an exhaustive review of upstream.
These open pull requests change the same files or areas as our patches. If they merge, they can cause conflicts or
behavior changes at the next sync:

| Upstream PR | Subject | Effect on Botster if it merges |
|---|---|---|
| #14439 | terminal: group mouse mode state | Changes `src/terminal/c/mouse_encode.zig`, `stream_terminal.zig`, `modes.zig`, and the snapshot header (new `terminal-header-grouped-mouse-v1` test data). Possible conflict with patch 6 (mouse getters). Changes GHOSTSNP content (ST-6, ST-6b) and DECRQM mouse-mode replies (EV-8). Patch 6's getters could then read the grouped state; re-audit patch 6. |
| #14483 | terminal: add support for DECRQCRA and XTCHECKSUM | New queries in `stream_terminal.zig`, `terminal.h` and `stream.zig`. Off by default (`vt-checksum-report`). Possible conflict with patch 1 (query effect). If libghostty enables them, the binding needs new query kinds; `QueryKind::from_c` refuses an unknown kind today. |
| #13332 | terminal: report modifyOtherKeys state via XTQMODKEYS | New query (`CSI ? 4 m` and DECRQSS `> 4 m`) in `stream_terminal.zig`, `dcs.zig`, `stream.zig`. Possible conflict with patch 1. A new EV-8 query kind. Complements patch 9 (the modifyOtherKeys state 2 getter) and does not replace it. |
| #14362 (draft), #14200 (draft) | modes 47/1047/1048/1049 reports | DECRQM replies for the alternate screen modes change (EV-8 shadow replies). #14200 changes the snapshot test data `complete-v1.hex`. |
| #14508 | input: encode ctrl+alt+shift+backspace in legacy mode | `function_keys.zig` and `key_encode.zig`, the same files as patches 4 and 12. Changes legacy Backspace bytes (IN-9, 5.1A); Botster takes the change through the encoder. |
| #14515 | libghostty: document allocator alignment as log2 | Documentation only. The binding passes a null allocator everywhere (`lib.rs`, `encode.rs`), so it is not affected. |

## Patch decisions

Each patch keeps one concern per commit.
The patch numbers are those of `docs/handoffs/p2-libghostty.md` in botster-core `v1`.
No patch has upstream cover: upstream `main` changed only the macOS file above, and no result of the upstream search
(scope in `evidence-20261002/README.md`) implements a patch.

| # | New commit | Old commit | Subject | Decision | Reason |
|---|---|---|---|---|---|
| 0 | `ee1875dd3` | `ea5a1e297` | terminal: own startHyperlink uri and id across capacity retries | KEEP | Use-after-free fix in `Screen.zig` with a regression test. Upstream code is unchanged. |
| 1 | `bcdcad95b` | `6495721bb` | report every query with its exact bytes and stop after it | KEEP | EV-8 (c), (g). No upstream equivalent. |
| 1 | `79ae024c3` | `38599d320` | track query request boundaries; add CSI 14;2 t and 13;2 t | KEEP | EV-8 (c), request bytes are exact. |
| 1 | `303013782` | `b60d00542` | apply R-17 to every executed C0 control; restart at a C1 CSI introducer | KEEP | EV-8 (c), ruling R-17. |
| 1 | `99ceb84cc` | `56b54e923` | test that a C1 byte inside a string sequence is payload | KEEP | Test that pins parser behavior that patch 1 relies on. |
| 1 | `50d8494c5` | `72902b1b7` | a C1 introducer ends an APC string in the query request tracking | KEEP | EV-8 (c). |
| 2 | `d9531aca1` | `7afa387dd` | report the notification source (OSC 9 or OSC 777) | KEEP | A2-4, A3-1. |
| 3 | `51c80ac71` | `970a1c9df` | paste marker frame without payload rewrite | KEEP | IN-8, DP-5. Upstream `ghostty_paste_encode` still rewrites the payload. |
| 4 | `fddc30c7c` | `50569efc8` | key events carry hyper, meta, shifted and base layout keys and F26 to F35 | KEEP | IN-9, 5.1A. |
| 4 | `1bef3ee4a` | `22035f7c2` | legacy Shift with no text; key tests use the structured sequence encoder | KEEP | IN-9, 5.1A legacy rule iii. |
| 4 | `ec9a95861` | `3f28780d2` | the legacy Alt test compares with the real base-character result | KEEP | Test correction for patch 4. |
| 5 | `9f476b148` | `da42a8ac0` | export the xterm-ghostty terminfo name and source | KEEP | TI-1, A2-8. |
| 6 | `f8aa86979` | `b59b1f47b` | mouse cells as given, and getters for the active mouse enums | KEEP | IN-9, 5.1A, ST-4, R-13. Re-audit when upstream #14439 merges. |
| 7 | `72d54d39e` | `05540bd69` | encode the typed replies to terminal queries | KEEP | EV-8. |
| 7 | `49e68f944` | `96b4f3f4f` | query reply encoders check their enums, use unsigned positions and parse in tests | KEEP | EV-8; review correction of the previous commit. |
| 8 | `41024b252` | `d27593e55` | report the OSC 52 selection and terminator on clipboard requests | KEEP | EV-3, EV-8 (G10). |
| 8 | `cfcd2c21c` | `85a8d8eb1` | the OSC 52 parser reads the whole selection | KEEP | Parser bug: every OSC 52 with a selection longer than one character was invalid. Upstream parser is unchanged. |
| 9 | `eafd0967b` | `468268e5c` | terminal data getters for modifyOtherKeys state 2 and XTSHIFTESCAPE | KEEP | E2-2 (G15). Upstream #13332 adds a query, not a getter. |
| 10 | `f0d70e3f1` | `92d13482a` | link libc on Linux so the static archive defines no allocator symbol | KEEP | P32 (G16). Confirmed on Linux in this sync (see "Test results"). |
| 11 | `1eb68104a` | `170d6faf8` | the OSC 5522 write acknowledgement is written with the host's reply | KEEP | A13-1b (G14). |
| 12 | `d5bebc7e2` | `f71c7651f` | the keypad equals key has an application keypad sequence | REWORK (test only) | The source change is correct and stays. The test compares with the literal `"\x1bOX"`, which is the open P2 review checkpoint P35 (BUILD.md architecture rule 2). See the note below. Owner: P2 on resume. |
| 13 | `3f8eb6810` | `ada251c5e` | the snapshot decoder takes the host's Kitty image storage limit | KEEP | ST-6b, P34 (G17). The P2 reviewer has not issued a verdict on this patch; P34's native image-resume proof is still incomplete (P2 work, not a sync item). |

**Note on patch 12 (REWORK).**
P35 asks the fork test to avoid a hand-written expected sequence.
A tradeoff applies.
Inside libghostty, the Zig unit tests define the encoder's expected behavior, and every neighbor upstream test
(`legacy: keypad 1 with application keypad` expects `"\x1bOq"`) uses literal bytes.
If Botster offers patch 12 upstream, upstream will expect a literal test.
My recommendation for the P2 lead: rule that BUILD.md rule 2 binds Botster repositories, where libghostty is the oracle, and
not the fork's own Zig tests, which are the oracle.
If the lead keeps P35, P2 rewrites the test to compare the application result with the `kpKeys` table entry of
`numpad_equal`, and the numeric result with the key's character, without a literal.
This sync does not change patch 12, because the P2 reviewer has not issued a verdict on P35.

**Commit history note.**
Some groups contain review corrections as separate commits (patch 4: `1bef3ee4a`, `ec9a95861`; patch 7: `49e68f944`).
The rebase already changed all 22 SHAs. I kept the commits separate so that each new commit corresponds one to one
with a reviewed old commit (the table above), and `git range-diff` shows each pair as identical.
A squash would remove that correspondence. Squash them only if a patch is offered upstream (that needs the user).

## The remaining patch list

### Generic bug fixes that upstream would likely take

These fix defects that any embedder or Ghostty user can hit.
Offering them upstream needs the user's approval (BUILD.md); this sync contacted nobody.

| # | Commit | Defect |
|---|---|---|
| 0 | `ee1875dd3` | `startHyperlink` reads freed memory when it retries after page growth and the caller's uri or id slices point into the current hyperlink. |
| 8 (second commit) | `cfcd2c21c` | The OSC 52 parser refuses every selection longer than one character (for example `s0` or `cp`), so no clipboard callback runs. |
| 10 | `f0d70e3f1` | With `-Dsimd=false` on Linux, the static archive defines weak `calloc` and `free` next to the host's `malloc` and `realloc`. Upstream's default `-Dsimd=true` links libc already, so the defect shows only without SIMD. |
| 12 (source part) | `d5bebc7e2` | The keypad equals key encodes to nothing; xterm sends `SS3 X` in application keypad mode and `=` otherwise. |

### Botster-specific APIs

These add C API surface or semantics for Botster Core clauses.
Upstream could take some of them as general features, but each one exists for a Botster clause.

| # | Commits | API |
|---|---|---|
| 1 | `bcdcad95b`, `79ae024c3`, `303013782`, `99ceb84cc`, `50d8494c5` | Query effect with kind and exact request bytes; `ghostty_terminal_vt_write_until_query`; CSI 14;2 t and 13;2 t. |
| 2 | `d9531aca1` | Notification `source` (OSC 9 or OSC 777). |
| 3 | `51c80ac71` | `ghostty_paste_frame` (bracketed paste markers without payload rewrite). |
| 4 | `fddc30c7c`, `1bef3ee4a`, `ec9a95861` | Key event hyper and meta, shifted and base-layout keys, F26 to F35, legacy Shift with no text. |
| 5 | `9f476b148` | `ghostty_terminfo_name`, `ghostty_terminfo_source`. |
| 6 | `f8aa86979` | Mouse cells as given; getters for the active tracking and format enums. |
| 7 | `72d54d39e`, `49e68f944` | `ghostty_query_reply_encode`. |
| 8 (first commit) | `41024b252` | OSC 52 selection and terminator on clipboard requests. |
| 9 | `eafd0967b` | Terminal data getters for modifyOtherKeys state 2 and XTSHIFTESCAPE. |
| 11 | `1eb68104a` | The OSC 5522 acknowledgement is written during the host's reply call. |
| 13 | `3f8eb6810` | `GHOSTTY_SNAPSHOT_DECODER_OPT_KITTY_IMAGE_STORAGE_LIMIT`. |

## Test results

Raw logs, scripts and the action audit are in `docs/ghostty/evidence-20261002/` (its `README.md` lists each file).

### Mac (Apple Silicon, Zig 0.16.0 from mise, through `botsterq run --exclusive`)

At `3f8eb6810`, with 0 tracked changes (`mac-recorded-run.log` prints the head, the Zig path and each exit status):

- `zig build test-lib-vt --summary all`: **pass** (exit 0) in two runs, the first run and the recorded run. Build Summary
  of both: 46/46 steps succeeded; 6632/6686 tests passed (54 skipped).
- The library build with the binding's arguments (`GHOSTTY_BUILD_ARGS` of `build_data.rs`):
  `zig build -Demit-lib-vt -Doptimize=ReleaseFast -Dsimd=false -Dcpu=baseline -Demit-xcframework=false`: **pass**.
  Exit 0. It installed `libghostty-vt.a` (10156232 bytes) and the dylib.
- The first test run failed before any test ran: `zig` package fetch failed with `TlsInitializationFailed`
  (the known Mac fetch flake of `docs/handoffs/infra.md`). `zig build --fetch=all` succeeded on the second try, and
  the rerun passed.

### Linux (gate host `structvr@192.168.1.185`, testq, image `botster-core-gate:bbc12a44852f`)

The ghostty repository has no `ci/remote/job.sh`, so `botster-gate` cannot run it.
I ran three testq jobs (8055, 8056 and the recorded run 8059; 4 CPUs, 16 GiB, Botster slot lock `slot.3`) in the core gate image, which
has Zig 0.16.0, Debian glibc 2.36 and gcc 12.2.
Each job built the archive with the binding's arguments twice: at `eafd0967b` (patch 9, before patch 10) and at
`3f8eb6810` (the candidate).

Job 8059 prints the sha256 of each input tarball before extraction; they equal `git archive` of `3f8eb6810` and
`eafd0967b`. Jobs 8055 and 8056 did not record input hashes (limitation in `evidence-20261002/README.md`); 8059
repeats their checks, except the C link test, with the same results.

Results (testq jobs 8055, 8056 and 8059, x86_64, Debian glibc 2.36, gcc 12.2, rustc 1.97.0):

- `zig build test-lib-vt --summary all` at `3f8eb6810`: **pass** (jobs 8056 and 8059). Build Summary: 45/45 steps succeeded;
  6616/6686 tests passed (70 skipped). The Mac run has 46 steps and 54 skips; I did not analyze the platform difference.
- The library build with the binding's arguments: **pass** at both commits.
- `nm --defined-only libghostty-vt.a`, allocator symbols (jobs 8055 and 8059):

  | Commit | Defined in the archive | Undefined (taken from the host libc) |
  |---|---|---|
  | `eafd0967b` (before patch 10) | `W calloc`, `W free` (in `libghostty-vt-static_zcu.o`) | `calloc` |
  | `3f8eb6810` (with patch 10) | none | `calloc`, `free`, `malloc`, `realloc` |

- A Rust program that links the archive statically, the way the binding does (`rustc -L native=… `, `#[link(name =
  "ghostty-vt", kind = "static")]`). It makes zeroed, plain and reallocated allocations from 1 byte to 64 MiB, then
  creates a terminal, writes 2000 lines, frees it, and makes one 128 MiB zeroed allocation (jobs 8056 and 8059):

  | Commit | `-C opt-level=0` | `-C opt-level=3` |
  |---|---|---|
  | `eafd0967b` (before patch 10) | **abort**: `memory allocation of 1 bytes failed`, exit 134 | pass |
  | `3f8eb6810` (with patch 10) | pass | pass |

  The opt-level 0 abort is the same failure as the infra report ("memory allocation failed" in every binding test).
  At opt-level 3 the program passes before patch 10 too; the optimizer probably removes some of the allocations, so
  the defect does not show in every build.
- A C program (job 8055) linked the same way (gcc, `-O1`, `malloc`, `calloc`, `free` and a terminal) passed at both commits, so
  only the Rust program reproduces the abort.

Conclusion: the Linux allocator defect (P32, G16) reproduces at native level before patch 10, and patch 10 removes it.
This is native evidence only.
P32 still needs the green Linux gate of the binding at its reviewed head, which needs the pin move (Stage 1 resume).

## Pin candidate

- Fork branch: `botster/upstream-sync-20261002` on trybotster/ghostty (a new branch; no existing `botster/*` branch
  was changed).
- Head: `3f8eb6810bb673aa782b047de21783ac81fb1121`.
- Upstream base: `f523504ea5c9f41d150d1eb93cc7a748b90f9361`.

## What Core P2 must change when Stage 1 resumes

1. Move the submodule `crates/botster-terminal-ghostty/vendor/ghostty` of `stage1/p2-libghostty` from `ada251c5e` to `3f8eb6810bb673aa782b047de21783ac81fb1121`, and change
   the `.gitmodules` branch from `botster/vt-core-stage1-c` to `botster/upstream-sync-20261002`.
   The plan records `85a8d8e`. The lead records the pin move from `85a8d8e` to the new head after the P2 reviewer is
   CLEAN on it; this sync does not do it.
2. Nothing in the binding source changes: libghostty-vt sources are identical, and `build_data.rs` `ZIG_PACKAGES`
   stays valid.
3. Update the pin record in the plan, the audit ("Ghostty pin" line) and the state log with the upstream base
   `f523504ea` and this patch list (BUILD.md fork policy).
4. The infra `fetch-public.sh` mirror must fetch the new commit (it fetches the pinned commit, so a new pin needs no
   script change, only a mirror fetch).
5. Close the open P2 review items that this sync does not close:
   - P35 for patch 12 (the lead's ruling on the test, see "Note on patch 12").
   - P34, the native image-resume proof for patch 13.
   - P32 needs a green Linux gate of the binding at the reviewed head. This sync gives native evidence for patch 10
     (see "Test results"), not the binding gate.
6. Before any new fork patch, sync again with upstream `main`. Check the open pull requests in "Open upstream pull
   requests to watch" first: if #14439, #14483 or #13332 merge, they can conflict with patches 1 and 6.

## Appendix: the Rust link program

Built with `rustc -C opt-level=<0|3> -L native=<prefix>/lib repro.rs` in `botster-core-gate:bbc12a44852f`.

```rust
// Links libghostty-vt.a like the binding does and exercises the host allocator.
use std::ffi::c_void;
#[link(name = "ghostty-vt", kind = "static")]
extern "C" {
    fn ghostty_terminal_new(alloc: *const c_void, out: *mut *mut c_void, cols: u16, rows: u16) -> i32;
    fn ghostty_terminal_vt_write(t: *mut c_void, data: *const u8, len: usize);
    fn ghostty_terminal_free(t: *mut c_void);
}
fn main() {
    let mut total = 0usize;
    for shift in 0..27 {
        let v = vec![0u8; 1usize << shift]; // alloc_zeroed -> calloc
        total += v.len();
        let mut w: Vec<u8> = Vec::with_capacity(1 << shift); // malloc
        w.resize(1 << shift, 1);
        w.shrink_to(1); // realloc
        drop(w);
        drop(v); // free
    }
    let mut t: *mut c_void = std::ptr::null_mut();
    let rc = unsafe { ghostty_terminal_new(std::ptr::null(), &mut t, 80, 24) };
    assert_eq!(rc, 0, "terminal_new");
    let s = b"hello \x1b[31mred\x1b[0m\r\n";
    for _ in 0..2000 { unsafe { ghostty_terminal_vt_write(t, s.as_ptr(), s.len()) }; }
    unsafe { ghostty_terminal_free(t) };
    let big = vec![0u64; 1 << 24];
    println!("rust repro ok total={} big={}", total, big.len());
}
```
