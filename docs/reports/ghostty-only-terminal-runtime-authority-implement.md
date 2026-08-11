# Implementation report: Ghostty-only terminal runtime authority

## Target repository and target_id
- Target repository: `botster-core` (`trybotster/botster-core`)
- Target id: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
- Run: `run_1786471511_632427`
- Ticket: `ticket_1786471489_484901`
- Worktree: Botster-managed ticket worktree for this run
- PR: https://github.com/trybotster/botster-core/pull/119
- Revision: addresses Review `review_1786475031_127288` (plus prior Review findings)

## Repository playbook and other playbooks/notes applied
- [[implementer-playbook]]
- [[botster-implementer-playbook]]
- [[botster-core-playbook]] (primary ownership charter)
- [[botster-terminal-ghostty-playbook]]
- Targeted notes: ghostty shadow terminal integration belongs outside botster core; session-process-owns-vt-parser-hub-rpc-snapshots; coredaemon must expose terminal truth used by the production hub path; pinned libghostty exposes synchronous exact mouse mode state; libghostty-vt-embedder-callback-architecture-and-constraints; split terminal runtimes drop color probe responses before client attachment; botster core contract surface needs consumer proof
- Not loaded: [[project-pipelines-playbook]]; [[botster runtime teardown lenses]] (`teardown_class_applies=false`)

## Review findings addressed (this revision)
| Finding | Fix |
| --- | --- |
| `[ownership] Move the production color policy outside the botster-core repository` | Removed all in-repo RGB defaults. Added policy-free `CoreDaemonConfig::terminal_color_profile` / `with_terminal_color_profile`. Hosts outside this repository supply presentation policy; Core only applies a supplied profile. |
| `[test-gap] Bind each OSC identifier to its expected RGB value` | `assert_osc_color_reply_sequence` requires the full bound sequence `]Ps;rgb:RRRR/GGGG/BBBB` and returns indices for ordered OSC 10/11/12 proof. Test supplies host profile via config seam. |

## Prior findings (still resolved)
- Ghostty-only hard cutover; hub-shaped exact assertions; plain-lane docs removed; plan path-neutral + whitespace clean

## Files changed (latest rework)
- `crates/botster-core-daemon/src/daemon.rs` — optional host color profile config seam; no production RGB defaults
- `crates/botster-core-daemon/tests/daemon_integration_test.rs` — host-supplied profile for OSC proof; bound OSC identifier+value asserts
- Report update

## Ownership boundaries preserved
- Adapter: Ghostty mechanism only (modes, palette apply, write_pty)
- CoreDaemon: policy-free optional config seam for host-supplied `TerminalColorProfile`
- Presentation policy: **outside this repository** (tests act as host; Hub will supply production policy later)
- `botster-core` remains neutral; PlainTerminalScreenRuntime is library/test harness only

## Cross-repo dependencies or separately routed work
- Hub ticket `ticket_1786471489_718500` depends on this Core ticket via `dependency_1786471500_696870`
- Hub/host composition must call `with_terminal_color_profile` when OSC color defaults are required

## Deviations from plan
- Color defaults are not embedded in CoreDaemon; hosts supply them via config seam (review ownership correction, aligns with “policy outside Core”)

## Tests and downstream proof run
- `cargo fmt --all` / `cargo clippy --workspace --all-targets -- -D warnings` — pass
- `cargo test --workspace` — pass
- `worker_backed_osc_color_queries_receive_session_side_write_pty_replies` — host-supplied profile + bound OSC 10/11/12 sequences
- Kitty/mouse input, mode flags, hub conformance proofs remain green

## Unverified behavior or residual risk
- Real Hub host composition supplying production colors is deferred to Hub ticket
- Without a host-supplied profile, OSC 10/11/12 may not emit RGB replies (mechanism-only, intentional)

## Missing vault guidance discovered
- CoreDaemon color profile is a policy-free config seam; presentation defaults belong outside botster-core
- Session-side OSC ownership: adapter answers queries; host supplies defaults

## Runtime-teardown class
- `teardown_class_applies=false`
