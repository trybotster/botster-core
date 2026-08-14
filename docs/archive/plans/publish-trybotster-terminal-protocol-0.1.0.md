# Publish @trybotster/terminal-protocol 0.1.0

Ticket: `ticket_1786723347_177328`
Run: `run_1786724335_910699`
Target: `botster-core` / `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
Pipeline: `botster_stack_delivery` / `botster_stack_plan`
Base: ticket worktree at `a047574` (`origin/main`)

## Target repository and target_id

- Target repository: `botster-core`
- Target id: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
- Spawn-target name: `botster-core`
- Repository path from `list_spawn_targets`: admitted Core checkout, not the ambient session directory
- Repository playbook: [[botster-core-playbook]]

## Repository playbook loaded

[[botster-core-playbook]]

## Other role and surface playbooks and atomic notes loaded

Role and overlay:

- [[planner-playbook]]
- [[botster-planner-playbook]]
- [[botster-architecture]]
- [[cli-patterns]]
- [[spa-patterns]]
- [[project pipeline orchestration belongs in a device-level botster plugin]]
- [[project pipelines needs an operator workbench not more primitives]]
- [[project pipelines ui contract belongs in the plugin readme]]
- [[botster orchestration should spawn agents with explicit target ids]]
- [[botster orchestration prompts must bind agents to explicit worktrees]]

Not loaded:

- [[project-pipelines-playbook]] — this ticket does not change Project Pipelines package or plugin paths.
- [[botster runtime teardown lenses]] — this ticket is a types-only npm publish. It is not a WebRTC, SessionIo, ClientWorker teardown, multi-peer, CPU/battery/FD, or live-runtime ticket.
- [[botster-hub-playbook]], [[botster-web-playbook]], [[botster-tui-playbook]] — consumers stay on their own targets.

Targeted atomic notes:

- [[public protocol versions host control and Core terminal planes independently]]
- [[Core terminal protocol separates Hub-safe envelopes from client semantic bodies]]
- [[botster terminal v1 starts at protocol 1 and conformance revision 1]]
- [[terminal wire enums and TypeScript unions share one variant inventory]]
- [[botster core contract surface needs consumer proof]]
- [[botster-core uses CI-owned Cargo commands because it has no test script]]
- [[hub test support npm releases need external consumer smoke]]
- [[Hub test support capability cutovers use a new unpublished package version]]
- [[closed dependency tickets signal merged source not a consumable release]]
- [[cross repo dependency registration must use dependency repo target]]
- [[generated typescript dtos must encode serde field optionality]]
- [[ready then history is advertised as optional daemon support]]
- [[additive daemon capabilities do not raise the default client requirement]]
- [[vault example paths are not repository placement conventions]]
- [[botster core historical plan docs should not sit beside living architecture]]
- [[plan steps need reviewable plan artifacts]]
- [[plan agents must author vault context as wikilinks not home paths]]
- [[pipeline vault checklists must cite exact resolvable note titles]]
- [[pipeline artifacts should cite vault notes by wikilink not home path]]
- [[pipeline artifacts should use path neutral worktree references]]
- [[plan review must verify a plan artifact exists before trusting gate summaries]]
- [[colon worktree paths break cargo dyld library paths]]
- [[hearth gate runs require restoring a pipeline wiped gitignore before attribution]]
- [[project pipeline step activation must preserve tracked gitignore]]

Session context from the orient hook: [[identity]], [[goals]]. The hook defaulted the session directory to rails/general. This run uses the resolved Core target and Botster conventions instead.

## Context loaded

- Pipeline ticket, run, gates, artifacts, checklists, sibling project tickets.
- Project `project_1786660949_205223` (`Botster Terminal Transport North Star`). Direct merge. No pull request.
- Consumer ticket `ticket_1786661008_897067` on botster-web needs this published npm coordinate. Web cannot pin a Hub Git revision for terminal compatibility.
- Sibling Hub publish ticket `ticket_1786723348_522242` publishes `@trybotster/hub-test-support`. That ticket is a different package and a different target.
- Target README, `docs/README.md`, `docs/plans/README.md`, `docs/architecture/terminal-protocol.md`, `.github/workflows/ci.yml`.
- Committed package `packages/terminal-protocol` at version `0.1.0`.
- Local `npm pack --dry-run` ships eight files: `LICENSE`, `README.md`, `index.js`, `index.d.ts`, `metadata.json`, `package.json`, `terminal-protocol.ts`, `fixtures/ready-then-history-event-order.json`.
- Local `script/terminal-protocol-node-smoke.sh` passed against the packed tarball.
- `npm view @trybotster/terminal-protocol version` returned registry 404.
- Agent-local `npm whoami` returned 401. Hub publish reports used the same 401 path and asked a human to run `npm publish --access public`.
- Tracked `.gitignore` has nine lines and matches HEAD. The ticket worktree path has no colon.

## Botster layers touched

- Core types-only npm package and its Node consumer smoke.
- Registry publish of `@trybotster/terminal-protocol`.
- Not Hub runtime, not Restty, not adapter policy, not Web pin, not TUI crate pin.

## Scope

Publish the committed Core package so Web can pin a protocol version.

1. Keep the package types-only: generated TypeScript, `metadata.json`, and the ready-then-history event-order fixture.
2. Publish `@trybotster/terminal-protocol@0.1.0` if that version is still unused.
3. If `0.1.0` is taken or a bad publish occurs, select the next unused version and keep crate, generated TypeScript, `index.js`, `metadata.json`, and `package.json` on the same version. `PACKAGE_VERSION` comes from `botster-terminal-protocol-client` `CARGO_PKG_VERSION`.
4. Extend the existing smoke consumer so it imports `TerminalCompatibilityRequirement` and `TerminalEvent` in addition to `PROTOCOL`, `PROTOCOL_VERSION`, and `FEATURE_*` tokens. Those types already exist in `packages/terminal-protocol/index.d.ts`.
5. Publish with `npm publish --access public` from `packages/terminal-protocol`.
6. Prove the published coordinate from a clean temp directory with `npm install @trybotster/terminal-protocol@<published>` and the smoke consumer shape.
7. Merge the ticket branch directly into `main`. Do not create a pull request.

## Non-scope

- Hub runtime, host DTOs, Unix or WebRTC adapters, Restty, or Ghostty.
- Inspection of READY, PAGE, FINISH, Snapshot, or GHOSTSNP bodies in Hub.
- Web pin, vendoring, or browser proof. Those stay on `ticket_1786661008_897067` and target `tgt_40abcf71ccf049f4ac0c99953a799869`.
- TUI or `botster-hub-client` Cargo pins.
- Publishing `@trybotster/hub-test-support`. That work is `ticket_1786723348_522242` on the Hub target.
- New protocol fields, new feature tokens, protocol version bumps, or conformance revision bumps.
- A GitHub Actions npm publish workflow or a stored token in this repository.
- Dual production paths or a fallback to a Hub Git revision.

## Product decision ledger

- Default: publish the committed `0.1.0` assets. Change version only when the registry already owns that version or the publish is bad.
- Non-goals: Web consumption, Hub policy, adapter work, CI publish automation.
- Follow-up-ok: the Web ticket pins the published coordinate after this ticket closes.
- Ask-human threshold: npm 401, 2FA, or missing `@trybotster` publish rights. Do not invent a token or a CI secret.

## Repository ownership boundaries and cross-repo dependencies

Core owns the types-only terminal protocol and the `@trybotster/terminal-protocol` npm coordinate. Web pins that package. Hub must not import it to inspect terminal bodies.

This run stays on `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`. Do not register a Web or Hub child ticket from this run. Do not edit those repositories.

Web `ticket_1786661008_897067` is a downstream consumer. A closed Core source ticket is not enough. The Web ticket needs `npm view` and a clean registry install. That rule is [[closed dependency tickets signal merged source not a consumable release]].

## Assumptions and unknowns

- Assumption: the committed `packages/terminal-protocol@0.1.0` assets are the intended first publish. Plan-stage pack and local smoke support that assumption.
- Assumption: first publish uses `--access public`. The package.json file has no `publishConfig`.
- Assumption: operator publish rights for `@trybotster` exist outside this agent session. Agent-local `npm whoami` is 401.
- Unknown at Implement start: whether `0.1.0` is still unused. Re-check `npm view` immediately before publish.
- Unknown: whether the operator uses 2FA. If publish fails with 401 or OTP, ask a human. Give the exact command. Do not store credentials in the repo.
- Not unknown: ticket meaning. The ticket asks for a registry publish of the existing types-only package, not a protocol redesign.

## Affected surfaces and files

Likely:

- `script/terminal-protocol-node-smoke.sh` — add the required type imports to the existing consumer.
- `docs/archive/plans/publish-trybotster-terminal-protocol-0.1.0.md` — this plan.
- `docs/reports/publish-trybotster-terminal-protocol-0.1.0-implement.md` — Implement report after publish.

Only if the registry rejects `0.1.0` or a bad publish occurs:

- `crates/botster-terminal-protocol/Cargo.toml`
- `crates/botster-terminal-protocol-client/Cargo.toml`
- `crates/botster-terminal-protocol-client/generated/terminal-protocol.ts`
- `packages/terminal-protocol/package.json`
- `packages/terminal-protocol/metadata.json`
- `packages/terminal-protocol/index.js`
- `packages/terminal-protocol/terminal-protocol.ts`
- any lock or fixture that asserts `PACKAGE_VERSION`

Do not change adapter crates, daemon code, or Hub-safe opacity rules.

## Worktree and target assumptions

- Work on the pipeline-provided ticket worktree for `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`.
- Restore tracked `.gitignore` from HEAD if it is empty or missing. Do not truncate it.
- The current ticket worktree path has no colon. If a later path contains `:`, set `CARGO_TARGET_DIR` to a colon-free directory before Cargo commands.

## Implementation sequence

1. Confirm `.gitignore` is the HEAD file.
2. Re-run `npm view @trybotster/terminal-protocol version`. Expect 404, or select the next unused version.
3. Re-run local `npm pack --dry-run` and `script/terminal-protocol-node-smoke.sh`.
4. Add `TerminalCompatibilityRequirement` and `TerminalEvent` imports to the existing smoke consumer. Keep the current metadata and feature assertions.
5. From `packages/terminal-protocol`, run `npm publish --access public`.
6. If `npm whoami` or publish returns 401, call `project_pipelines_ask_human` with this command and wait:

   ```sh
   cd packages/terminal-protocol
   npm publish --access public
   ```

7. After publish, prove the registry coordinate. Do not use a path or workspace install for the acceptance proof.
8. Merge directly into `main`. Do not open a pull request.

## Risks

- Agent session has no npm login. Hub publish history shows a human OTP or credentialed shell is the usual unblock.
- A scoped first publish without `--access public` can create a restricted package that Web cannot install.
- Local pack smoke is not registry proof. CI already runs the pack smoke. Acceptance requires `npm install` of the published version.
- A bad `0.1.0` publish cannot be overwritten. Select a new unused version and keep every version surface aligned.
- Web may close this ticket as a source merge and still fail if the registry install is missing. Require `npm view` and clean-dir install before close.

## Acceptance checks and tests

Production entry point: Web and other Node consumers install `@trybotster/terminal-protocol` from the public npm registry. Local pack smoke is pre-publish proof only.

Pre-publish:

```sh
npm view @trybotster/terminal-protocol version
# expect 404 for an unused 0.1.0

cd packages/terminal-protocol
npm pack --dry-run

script/terminal-protocol-node-smoke.sh
```

If Implement changes Rust crate versions or generated TypeScript, also run:

```sh
cargo fmt --all -- --check
BOTSTER_ENV=test cargo test -p botster-terminal-protocol-client typescript
BOTSTER_ENV=test cargo test --workspace
BOTSTER_ENV=test cargo clippy --workspace --all-targets -- -D warnings
BOTSTER_ENV=test cargo test --doc --workspace
```

If Implement changes only the Node smoke and docs, the Node smoke is the required repo gate. Do not invent a test wrapper.

Post-publish, required:

```sh
npm view @trybotster/terminal-protocol version
# must print the published version, expected 0.1.0

npm view @trybotster/terminal-protocol
```

Clean-dir registry consumer, required. Use a new temp directory. Do not install from the worktree path:

```sh
cd "$CONSUMER_DIR"
npm init -y
npm install @trybotster/terminal-protocol@<published> typescript
# compile the existing smoke consumer shape, including
# PROTOCOL, PROTOCOL_VERSION, FEATURE_* tokens,
# TerminalCompatibilityRequirement, and TerminalEvent
npx tsc --strict --module nodenext --moduleResolution nodenext --noEmit consumer.ts
```

Runtime assertions on the installed package:

- `metadata.protocol` equals `botster-terminal-v1`
- `metadata.protocol_version` equals `1`
- `metadata.features` includes `terminal_streaming`, `resize`, and `snapshot_delivery=ready_then_history`
- `metadata.package_version` equals the published version
- ready-then-history fixture export still loads

Merge:

- Ticket branch merges to `main`.
- No pull request exists.

Downstream proof:

- This ticket does not run botster-web.
- Closing this ticket is valid only after the registry install proof.
- Web `ticket_1786661008_897067` then pins the published coordinate on the Web target.

## Pipeline gates and artifacts

- Plan artifact: this file, plus `project_pipelines_add_artifact` `artifact_id`.
- One vault checklist for this Plan visit.
- Implement must attach publish command output, `npm view` output, and clean-dir registry install output.
- Gate evidence must include `plan_uri`, `artifact_id`, `checklist_id`, `target_id`, and `target_repository`.

## Required docs

- Keep living design in `docs/architecture/terminal-protocol.md`. That file already names `@trybotster/terminal-protocol` 0.1.0.
- After a successful `0.1.0` publish, do not rewrite the architecture page.
- If Implement selects a later version, update the architecture table and package README to the published version.
- Write the Implement report under `docs/reports/`.
- Do not add files under `docs/plans/`. That directory is a retired stub.

## Vault gaps worth capturing

- Core has no checked-in npm publish runbook. Hub already uses human `npm publish --access public` after agent 401. A short Core note would prevent a later agent from adding a token workflow.
- No new protocol decision appeared. Do not recapture [[botster terminal v1 starts at protocol 1 and conformance revision 1]] or the two-crate opacity rule.

## Runtime-teardown class

`teardown_class_applies`: no.

This ticket does not change peer lifecycle, SessionIo, ClientWorker teardown, multi-peer ownership, or live-runtime bounds.
