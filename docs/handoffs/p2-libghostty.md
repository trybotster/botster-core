# Handoff: P2, the libghostty binding (Botster v1 Stage 1 Core)

Written at the lead's PAUSE. Nothing is running. A new agent can resume from this file.

## Roles and ids

- Lead: `sess-1790903471-008f-8b9f78eef51d48aba5a45748495fd673`.
- P2 reviewer: `sess-1790906405-009f-10e64919c37b4c1dedc79a777b690161` (verdict file `verdicts/p2-libghostty.md` on branch `stage1/review-p2` of botster-core).
- P3 implementer (worker): `sess-1790940814-00b7-10ebbedc6ec6f9fe6ea13a273ee6925d`. P3 consumes the binding.
- Infra engineer: `sess-1790920314-00ac-0c0ba2e7e13aded5cfe3af28a328c550` (CI fetch path).
- Message tool: `mcp__botster__post_message` with `session_uuid` and `payload` (a string). Read with `mcp__botster__receive_messages`.
- Report to the lead, not to the orchestrator. Never force-push. Never kill processes by name or pattern.

## The binding branch

- Repository: botster-core, worktree `~/botster-sessions/trybotster-botster-core-stage1-p2-libghostty`, branch `stage1/p2-libghostty`.
- **Pushed head: `f711184885f933723c64124a1eaaa3bbc547b51f`** (contracts pin move and the R-28 audit line). The head that the reviewer last saw is `52d1f730309a7361870871895f78e03f3e5df9ca`; the only change since is `f711184` (Cargo.toml and Cargo.lock to `contracts-v0.1.13`, one audit line). 93 crate tests pass at `f711184`, `cargo clippy -p botster-terminal-ghostty --all-targets -- -D warnings` has 0 errors, `cargo fmt --check` is clean.
- The branch has NOT been rebased onto `origin/v1` (41ebcc0 or later). Do that before the gate. The branch is pushed, so a rebase needs a force-push, which is forbidden: use `git merge origin/v1` instead, and tell the reviewer that the head changed.
- The crate: `crates/botster-terminal-ghostty`. Submodule `crates/botster-terminal-ghostty/vendor/ghostty` points at fork commit `ada251c5e99cdb753de8bf72d5d1f307d474518f`; `.gitmodules` names branch `botster/vt-core-stage1-c`.
- Audit: `docs/stage1/libghostty-audit.md`, revision 9 (section "Revision 9" maps every review finding). Unique gap ids G1 to G18 and H1.
- The PR description draft is `scratchpad/pr-body.md` of the old session; it is not on disk in a stable place. Rewrite it. Required content: a "Prior art" note (final reuse: `build.rs` and `sys.rs` stolen from old botster-core `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c` with `Stolen-From:` and `For-Clause:` trailers, already in the history; custom pieces and reasons as in the audit's "Prior art" section; the `base64` crate for the clipboard reply), and a line that `terminal_identity()` is backed by the new `ghostty_terminfo_*` exports and that P1 wires it after merge.

## The fork

- Repository `trybotster/ghostty`, worktree `~/botster-sessions/ghostty-p2-vt-core-stage1`.
- Base: upstream ghostty-org main `83edd491e3024ae5e50393d62877b8897da1cccd`.
- **Branch `botster/vt-core-stage1-c`, head `ada251c5e99cdb753de8bf72d5d1f307d474518f`, pushed.** Branch `-b` (head `85a8d8eb197c5752887c017c9a3faa6f1dc1969b`) is the previous pin and is pushed.
- Also pushed as leftovers: `p2-old-stack` (`1fd093bd3`) and `p2-patch4-wip` (`ee89698a4`); they hold superseded work and no one needs them.
- Zig tests run with: `zig build test-lib-vt -Dtest-filter="<filter>" --summary all --global-cache-dir $HOME/.cache/botster/zig-global`. Zig is 0.16.0 (mise). Run heavy commands through `botsterq run --label "<text>" --exclusive --deadline 60m -- <command>`.

### Patch list (0 to 13; clause each answers)

| # | Commit | What | Clause |
|---|---|---|---|
| 0 | `ea5a1e297` | `startHyperlink` owns the uri and id across capacity retries (use-after-free fix) | ST-6b, EV-7 |
| 1 | `6495721bb` and the R-17 commits (`38599d320`, `b60d00542`, `56b54e923`, `72902b1b7`) | query effect with kind and exact request bytes; `ghostty_terminal_vt_write_until_query`; CSI 14;2 t and 13;2 t | EV-8 |
| 2 | `7afa387dd` | notification `source` (OSC 9 or OSC 777) | A2-4, A3-1 |
| 3 | `970a1c9df` | paste marker frame without payload rewrite | IN-8, DP-5 |
| 4 | `50569efc8`, `22035f7c2`, `3f28780d2` | key events: hyper and meta, shifted and base-layout keys, F26 to F35, associated text, legacy Shift with no text | IN-9, 5.1A |
| 5 | `da42a8ac0` | export the `xterm-ghostty` terminfo name and source | TI-1, A2-8 |
| 6 | `b59b1f47b` | mouse cells as given; getters for the active mouse tracking and format enums | 5.1A, ST-4, EV-7, R-13 |
| 7 | `05540bd69`, `96b4f3f4f` | `ghostty_query_reply_encode` for typed replies | EV-8 |
| 8 | `d27593e55`, `85a8d8eb1` | OSC 52 parser reads the whole selection; selection and terminator on clipboard requests | EV-3, EV-8 (G10) |
| 9 | `468268e5c` | terminal data getters for modifyOtherKeys state 2 (45) and XTSHIFTESCAPE (46) | E2-2 (G15) |
| 10 | `92d13482a` | link libc on Linux so the static archive defines no allocator symbol | P32 (G16) |
| 11 | `170d6faf8` | the OSC 5522 acknowledgement is written when the host replies | A13-1b (G14) |
| 12 | `f71c7651f` | keypad equals key: SS3 X in application keypad mode, `=` otherwise | IN-9 (G18) |
| 13 | `ada251c5e` | snapshot decoder option `GHOSTTY_SNAPSHOT_DECODER_OPT_KITTY_IMAGE_STORAGE_LIMIT` | ST-6b, P34 (G17) |

Reviewer status: patches 0 to 8 CLEAN. Patches 9 to 11 CLEAN for source logic through `170d6fa` (verdict `e26e146`). Patches 12 and 13 are NOT yet reviewed.

## Pin status

- Plan revision pin: `~/botster-sessions/pins/stage1-plan.e4862c71.md` (older: `a16bee0c`, `62f664de`).
- Contracts: `contracts-v0.1.13` (`a8db5c9`, manifest final30, Core Amendment 13 final). The binding's `Cargo.toml` is on it as of `f711184`.
- Ghostty pin recorded in the plan: `85a8d8e` (branch `-b`). **The `-c` head `ada251c` is NOT recorded.** It waits for the reviewer's CLEAN on the fork head and the binding; then the lead records the pin move.

## Order the lead set (do not skip steps)

1. Reviewer reviews fork `ada251c` (patches 9 to 13) and the binding.
2. On CLEAN, the lead records the pin move.
3. The infra engineer's `ci/remote/fetch-public.sh` fetches the submodule at the commit the tree pins and publishes the Zig packages into the gate volume (merged in botster-core `41ebcc0`, PR #131).
4. Merge `origin/v1` into the branch, then run `botster-gate` **once** on the exact reviewed head. The Linux gate sets `BOTSTER_ZIG_NETWORK_DENIED=1` (unshare cannot run in the container) and `/tmp` is a tmpfs.
5. Send the lead DONE with the head SHA, the verdict commit and the gate log path. Open the PR with the description above. Never gate before step 3.

## The Linux defect (P32) and patch 10

- Symptom (infra engineer, head `977d986`): every `botster-terminal-ghostty` test aborts on Linux with "memory allocation of 9680 bytes failed" (SIGABRT, 146 tests).
- Cause: the vendored wuffs C code calls `calloc`. A module that does not link libc makes Zig compile its own `malloc.zig` into the static archive. The archive then defines `calloc` and `free` (weak) while `malloc` and `realloc` stay glibc's, so Rust frees glibc memory with the wrong `free`. macOS hides it.
- Fix: patch 10 links libc on Linux (`src/build/GhosttyZig.zig`). I checked with an `x86_64-linux-gnu` cross build and `nm`: before, `calloc` and `free` are defined; after, none is defined and all four are referenced.
- Test: `crates/botster-terminal-ghostty/src/tests_archive.rs` runs `nm -g --defined-only` on the archive (path from `build.rs` env `BOTSTER_GHOSTTY_VT_ARCHIVE`) and fails on any libc allocator symbol. The build host needs `nm`.
- **Not yet proven by a Linux gate.** P32 stays open until a green Linux run. The infra engineer's box: `ssh structvr@192.168.1.185`, image `botster-core-gate`; old failing log `~/botster-sessions/gates/botster-core-infra-ghostty-test-ed46fdd8-linux-20261002-081955-16421.log`.

## Build and fetch

- `build.rs` fetches nothing. It stages Zig packages from a store (`~/.cache/botster/zig-packages`, env `BOTSTER_ZIG_PACKAGES`; `p/<hash>.tar.gz`) into `OUT_DIR/zig-global`, requires Zig exactly 0.16.0 (`BOTSTER_ZIG` or PATH), and runs `zig build -Demit-lib-vt -Doptimize=ReleaseFast -Dsimd=false -Dcpu=baseline -Demit-xcframework=false` under network denial (`sandbox-exec` on macOS, `unshare --net --map-root-user` on Linux; `BOTSTER_ZIG_NETWORK_DENIED=1` opts out).
- `prefetch-zig.sh` (with network) fills the store; `build_data.rs` holds `ZIG_PACKAGES` and `GHOSTTY_BUILD_ARGS`.
- The Zig packages (store file = the archive as downloaded). The lead says the gate needs **9** hashes at the current pin; `build_data.rs` `ZIG_PACKAGES` lists 7 at `85a8d8e`. **Check this first**: run `prefetch-zig.sh` at `ada251c` and compare its list with `build_data.rs`; if Zig fetched two more packages, add them to `ZIG_PACKAGES` and re-run. The 7 known:
  - `aro-0.0.0-JSD1Qk6lNgDdcDV4Vh7Sfy-34m2TluIVOdPzMmj_0BjX` https://github.com/vancluever/arocc/archive/f97cdfc3779aec4b242299e2fc9a1c828c3547c6.tar.gz
  - `N-V-__8AAB0eQwD-0MdOEBmz7intriBReIsIDNlukNVoNu6o` (zlib) https://deps.files.ghostty.org/zlib-1220fed0c74e1019b3ee29edae2051788b080cd96e90d56836eea857b0b966742efb.tar.gz
  - `N-V-__8AADYiAAB_80AWnH1AxXC0tql9thT-R-DYO1gBqTLc` (pixels) https://deps.files.ghostty.org/pixels-12207ff340169c7d40c570b4b6a97db614fe47e0d83b5801a932dcd44917424c8806.tar.gz
  - `N-V-__8AAM94BAAFk_hn4UW0x_OBD2g0vOwexeAAyWNNo4eB` (themes) https://deps.files.ghostty.org/ghostty-themes-release-20260921-150923-0b55a9e.tgz
  - `N-V-__8AAP5JWgCGP_AD0teWpa4krRvE9VPZzvviGdbmN4jI` (wuffs) https://deps.files.ghostty.org/wuffs-7411f488fe2e2c205c3d3b3d28638b7356522930.tar.gz
  - `translate_c-0.0.0-Q_BUWhVNBwDOEcIqub4VFPJPB6D9dgwzUMHTX5KWr8Xr` https://codeberg.org/vancluever/translate-c/archive/4e879eb8aba615de112eabd1231ea6e01920cead.tar.gz
  - `uucode-0.2.0-ZZjBPuuFVgC8YZ8eld4fOKsZANLIhTFMzULQxhkLi1C7` https://github.com/jacobsandlund/uucode/archive/9d55524551411b493cca41ca06363625d90aff1e.tar.gz
- Test commands (always through the queue): `botsterq run --label "core p2 binding tests" --exclusive --deadline 60m -- env CARGO_BUILD_JOBS=4 sh <script>` where the script runs `env -u RUSTUP_TOOLCHAIN CARGO_BUILD_JOBS=4 cargo test -p botster-terminal-ghostty`; clippy: `cargo clippy -p botster-terminal-ghostty --all-targets -- -D warnings`. Run `eval script/prebuild-worker` is for the full Core suite, not this crate.

## The API P1 and P3 consume (`botster_terminal_ghostty`)

- `Terminal::new(&Size, History)`, `resize`, `vt_write` (never for PTY output: it counts queries in `Drained::unrouted_queries` and the library's shadow reply lands in `Drained::pty_writes`), `vt_write_until_query(&[u8]) -> Result<QueryStep{consumed, query}, Error>` (returns `Error::AckBacklog` when undrained ack bytes pass `set_ack_backlog_limit`, default 1 MiB), `drain_events() -> Drained{events, dropped, dropped_kinds, pty_writes, clipboard_acks, unrouted_queries}`.
- Reads: `modes()` (other_modes include `xterm_modify_other_keys_2`, `xterm_mouse_shift_capture`), `title`, `cwd`, `screen_text`, `cursor`, `row_cells`.
- Input: `encode_key`, `encode_mouse`, `encode_focus`, `paste_frame`, and `*_with_modes` free functions. Results: `EncodeError::{Unsupported(UnsupportedWhat), NotReported}`.
- Queries: `Query{kind, request (R-17 bytes), request_truncated, shadow_reply, shadow_reply_overflow, selection, terminator}`, `Query::label()`, `Query::reply_bytes(&QueryReply)`, `set_query_request_limit`, `set_cell_px` (internal), `set_color_profile`, `shadow_answerable_kinds`.
- Clipboard: `TerminalEvent::ClipboardWrite(ClipboardWrite{location, selection: Option, terminator, contents: Option<Vec<ClipboardEntry>>, total_bytes, too_large})`; `set_clipboard_limit`; OSC 5522 acknowledgements are `Drained::clipboard_acks` (the worker writes each as one contiguous AM-2 transaction). The worker maps `selection`: a non-empty string as written, else location (standard `c`, primary `p`, selection `s`).
- Snapshot: `snapshot() -> Result<Vec<u8>, SnapshotError::{ContinuationUnavailable, Library}>` (map `ContinuationUnavailable` to `SnapshotTooLarge`), `CONTINUATION_LIMIT` (1 MiB), `snapshot_format()` (name and version from the library's envelope; the snapshot holds no graphics, so `snapshot_graphics` is absent). A restore must set the decoder's image storage limit to zero.
- `terminal_identity() -> {term, terminfo_source}` from `ghostty_terminfo_*`; P1 wires it after merge.

## Open review findings (verdict at `stage1/review-p2`, last head `ab8577a`)

Open at the last verdict: P25, P27, P32, P33, P34. All but P32 have been addressed in `52d1f73` and are awaiting review: P25 (acks as `Drained::clipboard_acks` with backlog limit), P27 (exact `f32` and `i32` check), P33 (audit ids and wording), P34 (patch 13 plus image resume tests). P32 awaits the Linux gate. Closed by the reviewer: P24, P26, P28, P29, P30, P31.

## Known gaps and limits

- `ClipboardWrite.selection` is `None` for an OSC 52 write with no selection and for OSC 1337 and 5522; the worker maps it from `location`.
- Legacy release gives `NotReported` (R-28; `release_event` stays a listed value that no v1 stimulus produces).
- No library getter for XTSHIFTESCAPE "never set" versus "off": both are `false`.
- Pixel screen: with no cell size, the SGR-pixels screen is unbounded.
- On macOS the Alt key needs `MACOS_OPTION_AS_ALT=1`; the binding sets it.
- `vt_write` on PTY output would answer queries before a client could (EV-8(d)); the worker must use `vt_write_until_query`.

## Rulings applied

- **R-13:** SGR-pixels positions are zero-based, no +1.
- **R-17:** query request bytes are parser-assembled; executed C0 controls are excluded.
- **R-28:** a legacy release is `NotWritten(NotReported)`.
- **R-14.1:** the eight named modifier keys alone give `Unsupported{named_key}` unless kitty flag 8 is on.
- **E2:** E2-1 (OSC 1 gives no `TitleChanged`), E2-2 (`other_modes` are tracked modes with no normative field), E2-3 (`ModesChanged` after each `vt_write`).
- **A8-2:** no ground state means `SnapshotTooLarge`; a snapshot is never partial.
- **A13 (final, candidate 5, contracts `627d507`, manifest final30, file `frozen/current/core-contract-v1.17-amendment-13-candidate5.md`):** A13-1 clipboard write shape and size admission (SUCCESS or IO_ERROR only); A13-1b the acknowledgement goes through the worker's single admission point as one contiguous transaction; a native clipboard read other than typed OSC 52 reaches the client untyped with no shadow answer.
- **EV-8:** the shadow never answers a clipboard read; the held reply is bounded (64 KiB); the request limit defaults to 4096 bytes.
- **Test oracle rule (BUILD.md rule 2):** no handwritten expected terminal bytes in tests; compare with native encoder results or native parsed state. Literal request stimuli and host payload bytes are allowed.

## What to do next

1. Read the reviewer's newest verdict in `verdicts/p2-libghostty.md`.
2. Check the 9 Zig hashes (see "Build and fetch").
3. Wait for the lead's go. Then follow the order above. Do not gate before the reviewer's CLEAN, the pin record and the infra fetch path.


## Reviewer state

The lead relayed the user's PAUSE request on 2026-10-02. The reviewer stopped at a clean point.
The lead then instructed the reviewer to issue no new verdict while paused.

### Heads and verdict

- Reviewer branch: `stage1/review-p2` in `~/botster-sessions/trybotster-botster-core-stage1-review-p2`.
- Last published verdict commit: `e26e1469cb01c04bcc16de36c2e625fbd9a4141b`.
- Verdict file: `verdicts/p2-libghostty.md`.
- Last formally reviewed binding head: `ab8577a0e0f7017271be2daa27d63ee324e1ac9e`.
- Last binding head examined before pause: `52d1f730309a7361870871895f78e03f3e5df9ca`.
- Last native head examined before pause: `ada251c5e99cdb753de8bf72d5d1f307d474518f`, branch `botster/vt-core-stage1-c`.
- Last native head with a published scoped verdict: `170d6faf82fb1a90f4776707421702f4ff4a66fc`.
- The published verdict remains NOT CLEAN, with five open findings: P25, P27, P32, P33 and P34.
- No verdict was issued for `52d1f73` or native patches 12–13. The notes below are a review checkpoint.

The reviewer read all production deltas and the changed tests for `ab8577a..52d1f73`.
The reviewer read native patches 12 and 13 with `git show`.
The reviewer read audit revision 9, including its complete status table.
Some combined tool output was truncated; the reviewer read the relevant audit and native sections again separately.
The reviewer ran no tests throughout this assignment.

### Binding review inputs

- Contracts pin: `contracts-v0.1.9`, commit `7f72acf8427ad7bf414db42d2360d5dccc7b3e13`, manifest final22.
- The lead also requires frozen Core A13 candidate 5 at contracts commit `627d507`, manifest final30.
- Plan revision 19: `~/botster-sessions/pins/stage1-plan.62f664de.md`.
  SHA-256: `62f664de2476bc172336a9ddac7b111517a9fc49f34eb59b69a8d11a24c0bea8`.
- Recorded Ghostty pin: `85a8d8eb197c5752887c017c9a3faa6f1dc1969b`.
  The lead will record the reviewed final `-c` head. The reviewer approves no pin move.
- Zig: 0.16.0. Rust: 1.97.0. Gate nightly: nightly-2026-09-30.

Read `pair-common.md`, `brief-p2-libghostty.md`, the pinned plan and BUILD.md before resuming.
All terminal semantics belong to libghostty. No handwritten expected terminal bytes are permitted in new tests.
Ghostty reads must use `git show` only. Do not run tests except to prove a finding.
Do not spawn agents. Do not poll. Never kill processes by name or pattern.

### Native patches 9–13

- Patch 9, `468268e5c9f9073d1d5856da9a088aacc52aab02`: native getters for modifyOtherKeys state 2 and mouse Shift capture.
  Published source-logic verdict: CLEAN in `e26e146`. P26 closes.
- Patch 10, `92d13482af18eaa39b3abb898759dd38874109e4`: link libc on Linux.
  Published source-logic verdict: CLEAN in `e26e146`. P32 still needs Linux gate evidence.
- Patch 11, `170d6faf82fb1a90f4776707421702f4ff4a66fc`: deliver the OSC 5522 acknowledgement during the synchronous reply call.
  Published source-logic verdict: CLEAN in `e26e146`. The binding must preserve each acknowledgement independently of class D events.
- Patch 12, `f71c7651fbe4522ad8138255ddbf83a36dcedd29`: add the keypad equals entry through the existing `kpKeys` helper.
  Source delta examined. No verdict issued. Its new Zig test contains handwritten expected terminal bytes; see the P35 checkpoint below.
- Patch 13, `ada251c5e99cdb753de8bf72d5d1f307d474518f`: add the snapshot decoder's image storage limit option.
  Source delta examined. No verdict issued. The option reaches each restored screen before continuation replay.
  The native test checks storage limits but does not feed image input. P34's required native image-resume proof remains incomplete.

### OPEN findings from the published verdict

**P25 — HIGH — Acknowledgements must survive event loss.**
Required change: preserve each acknowledgement as an ordered input transaction outside the class D event buffer.
Bound that path with admission backpressure or a native model-step boundary. Never drop or truncate an acknowledgement.
Checkpoint at `52d1f73`: `Drained.clipboard_acks` is now a separate queue. Each reply is captured without truncation.
`vt_write_until_query` refuses before another call when undrained acknowledgement bytes exceed `set_ack_backlog_limit`.
One supplied chunk can exceed the backlog limit; the caller must use bounded chunks and drain before another step.
Native metadata limits the echoed id to 512 bytes (`kitty/clipboard_command.zig::max_id_len`).
The regression fills the event buffer and still receives the acknowledgement. The source correction appears sufficient for closure.
Do not close it until the paused review resumes and issues a verdict.

**P27 — HIGH — Pixel coordinates must preserve exact values.**
Required change: check exact `f32` representation and the native `i32` range, rather than refusing every value above 2^24.
Checkpoint at `52d1f73`: `pixel_is_representable` compares `f64::from(pixel as f32)` with `f64::from(pixel)` and checks `i32::try_from`.
The tests include 2^24+1, 2^24+2, 2^24+3, 2^24+4, 2^31-128 and 2^31, including releases.
The source correction appears sufficient for closure. No verdict was issued while paused.
The delta also uses `u32::MAX` screen dimensions for SGR-pixels without a supplied cell size.
This preserves supplied pixels through the native encoder and avoids a fabricated small viewport refusal.
The native metadata, padding and grid conversion remain responsible for encoding.

**P32 — HIGH — Linux allocator defect; evidence pending.**
The old native archive defined its own `calloc` and `free`, while `malloc` and `realloc` came from glibc.
The infra engineer reported heap aborts in every Linux binding test. The reviewer did not reproduce them.
Patch 10 corrects the source. `tests_archive.rs` runs `nm --defined-only` and rejects defined libc allocator symbols.
Required evidence: a green Linux gate on the exact reviewed binding head.
The lead's pause decision resolves the gate-order conflict:
Once every other code and test finding closes, issue CLEAN for source logic with P32 explicitly marked `pending Linux evidence`.
The infra engineer then runs the gate on that exact head. That gate supplies the binding tests on Linux.
P32 closes only on a green Linux gate. No separate focused run is needed. The merge rule still requires the green gate.

**P33 — LOW — Audit identifiers and read scope must be consistent.**
Required change: use unique GAP identifiers, state typed OSC 52 and untyped clipboard read rules, and report finding status accurately.
Checkpoint at `52d1f73`: audit revision 9 uses G13–G18 and states both read rules. It marks P32 pending Linux evidence.
The source correction appears sufficient for closure. No verdict was issued while paused.
The audit still uses its original PIN/UP names for historical evidence; revision 9 describes the current corrections separately.

**P34 — HIGH — Disabled graphics policy must survive restore.**
Required change: apply the disabled image storage policy before native continuation replay and before later output.
The lead chose policy preservation. It authorized one minimal native patch on top of `170d6fa`.
The lead permits either a decoder configuration option or a snapshot record change, whichever is smaller.
Keep `snapshot_graphics` absent. State the image exclusion in the snapshot format description.
Closure requires a native Zig image-resume test and a binding every-offset image-resume test.
Checkpoint at `52d1f73` and `ada251c`: the decoder option applies the configured limit to every restored screen before replay.
The binding restore test sets the option to zero and includes direct and chunked image input at every byte offset.
It checks a zero storage limit before and after the suffix, including the alternate screen. The format doc states the exclusion.
The native Zig test only checks zero/default limits and the alternate screen. It never feeds image input or compares resumed behavior.
That native test must add the lead's required image-resume case before P34 closes.

### New test finding to record when review resumes

**P35 — MEDIUM — New tests construct expected terminal protocol bytes.**
This finding was identified before pause but has no published verdict yet.
Patch 12's new test in `src/input/key_encode.zig` compares the encoded keypad equals result with literal `\x1bOX`.
The binding's new acknowledgement-order test in `tests.rs` constructs the expected protocol field with `format!("id={id}")`.
BUILD.md architecture rule 2 forbids handwritten expected terminal bytes when libghostty is the oracle.
Required change: use independent native encoder results, native parsed state, or structured properties that do not construct protocol framing.
For acknowledgement order, compare each queued response with the response from an isolated native write using the same id.
For keypad encoding, use native data or properties of the numeric and application results rather than a literal application sequence.
The original P30 remains closed at its reviewed head; this is a regression in newly added tests.

### Closed findings and communications

F1–F12 and P13–P23 are closed in the earlier verdicts.
P24, P26, P28, P29, P30 and P31 close in published verdict `e26e146` at binding head `ab8577a`.
The original P24 live-model configuration correction is valid. P34 is the separate restore policy defect.

Lead session: `sess-1790903471-008f-8b9f78eef51d48aba5a45748495fd673`.
Implementer session: `sess-1790913443-00a6-5ccbd06a737415a19f55ad6663316aac`.
Reviewer session: `sess-1790906405-009f-10e64919c37b4c1dedc79a777b690161`.
Use Botster `post_message` with `{session_uuid, payload}` and a string payload.
The lead's last instruction is PAUSE: issue no new verdict, save this handoff, push the reviewer branch, and stay idle.
The initial user restricted reports to the lead to QUESTION or BLOCKED. Verdicts go to the implementer.
The workspace has a pre-existing untracked `.gitignore`; do not edit it.
Git metadata requires sandbox escalation for add, commit and push. Never force-push.
