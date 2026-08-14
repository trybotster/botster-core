# Implementation report: publish @trybotster/terminal-protocol 0.1.0

Ticket: `ticket_1786723347_177328`
Run: `run_1786724335_910699`
Step: `botster_stack_implement`
Plan: `docs/archive/plans/publish-trybotster-terminal-protocol-0.1.0.md`
Plan artifact: `artifact_1786724754_135772`
Implement checklist: `checklist_1786729666_160645`

## Target repository and target_id

- Target repository: `botster-core`
- Target id: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
- Spawn-target name: `botster-core`
- Repository path from `list_spawn_targets`: admitted `botster-core` spawn target (`trybotster/botster-core`)
- Pipeline worktree: Botster-managed ticket worktree for this run
- Merge policy: `direct` (no PR)

Independent `list_spawn_targets` maps `tgt_1f7bce66eb304881980f9b4a2a5ae3fe` to `trybotster/botster-core`. The approved plan used the same routing.

## Repository playbook and other playbooks/notes applied

Repository charter: [[botster-core-playbook]]

Role and overlay:

- [[implementer-playbook]]
- [[botster-implementer-playbook]]
- [[botster-architecture]]
- [[cli-patterns]]
- [[spa-patterns]] — Node/TypeScript consumer smoke only; no React or entity-store work

Not loaded:

- [[project-pipelines-playbook]] — this ticket does not change Project Pipelines package or plugin paths
- [[botster runtime teardown lenses]] — `teardown_class_applies` is no
- [[botster-hub-playbook]], [[botster-web-playbook]], [[botster-tui-playbook]] — consumers stay on their own targets

Targeted notes:

- [[public protocol versions host control and Core terminal planes independently]]
- [[Core terminal protocol separates Hub-safe envelopes from client semantic bodies]]
- [[botster terminal v1 starts at protocol 1 and conformance revision 1]]
- [[terminal wire enums and TypeScript unions share one variant inventory]]
- [[botster core contract surface needs consumer proof]]
- [[botster-core uses CI-owned Cargo commands because it has no test script]]
- [[hub test support npm releases need external consumer smoke]]
- [[Hub test support capability cutovers use a new unpublished package version]]
- [[closed dependency tickets signal merged source not a consumable release]]
- [[generated typescript dtos must encode serde field optionality]]
- [[ready then history is advertised as optional daemon support]]
- [[additive daemon capabilities do not raise the default client requirement]]
- [[implement gate must verify committed work and pr link before review]]
- [[implementation artifacts must match actual git state]]
- [[implementation steps must persist report artifacts for review]]
- [[pipeline vault checklists must cite exact resolvable note titles]]
- [[pipeline artifacts should use path neutral worktree references]]
- [[project pipelines checklist worker timeouts require artifact evidence fallback]]

## Botster layers changed

- Core types-only npm package `@trybotster/terminal-protocol@0.1.0` is now on the public registry.
- Repository Node smoke now type-checks `TerminalCompatibilityRequirement` and `TerminalEvent` in addition to `PROTOCOL`, `PROTOCOL_VERSION`, and `FEATURE_*` tokens.

Not changed: Hub runtime, Restty, adapters, Web pin, TUI crate pin, protocol version, conformance revision, feature tokens.

## Files changed

- `script/terminal-protocol-node-smoke.sh` — import and construct `TerminalCompatibilityRequirement` and `TerminalEvent[]` in the existing consumer.
- `docs/archive/plans/publish-trybotster-terminal-protocol-0.1.0.md` — approved plan (Plan commit).
- `docs/reports/publish-trybotster-terminal-protocol-0.1.0-implement.md` — this report.

No crate versions, generated TypeScript, `package.json`, or `metadata.json` edits. Registry `0.1.0` was unused.

## Ownership boundaries preserved

- Core still owns the types-only terminal protocol and the `@trybotster/terminal-protocol` npm coordinate.
- The package remains types, metadata, and the ready-then-history event-order fixture.
- Hub-safe opacity is unchanged. This run did not edit Hub, Web, TUI, or adapter crates.
- Web pin remains on `ticket_1786661008_897067` / `tgt_40abcf71ccf049f4ac0c99953a799869`.

## Cross-repo dependencies or separately routed work

- Downstream Web ticket `ticket_1786661008_897067` needs this published coordinate. A closed Core source ticket is not enough; this report records `npm view` and a clean-dir registry install.
- Sibling Hub publish `ticket_1786723348_522242` publishes `@trybotster/hub-test-support` on the Hub target. Not in this run.
- No child ticket was registered from this run.

## Deviations from plan

None.

- Published committed `0.1.0`. Did not bump versions.
- Did not open a pull request. Merge stays on the pipeline Merge step because `merge_policy` is `direct` and the next step is Review.
- Human publish was the planned 401/404 unblock (`question_1786729749_948235`).
- Duplicate Implement vault checklist `checklist_1786729706_961555` was a create-timeout sibling. All of its items are skipped. Evidence lives on `checklist_1786729666_160645`.

## Tests and downstream proof run

Pre-publish, ticket worktree:

```sh
git checkout HEAD -- .gitignore
# tracked .gitignore remains 9 lines and matches HEAD

npm view @trybotster/terminal-protocol version
# 404 Not Found — 0.1.0 unused

cd packages/terminal-protocol
npm pack --dry-run
# 8 files: LICENSE, README.md, fixtures/ready-then-history-event-order.json,
# index.d.ts, index.js, metadata.json, package.json, terminal-protocol.ts

script/terminal-protocol-node-smoke.sh
# terminal-protocol node smoke passed
```

Agent publish attempt:

```sh
cd packages/terminal-protocol
npm whoami
# 401 Unauthorized

npm publish --access public
# 404 on PUT https://registry.npmjs.org/@trybotster%2fterminal-protocol
```

Human publish (`question_1786729749_948235`):

- Published `@trybotster/terminal-protocol@0.1.0`.
- Integrity: `sha512-Gpd0qFd/tFkauCD6vBvJtH2HoR3wUzrc6iLwHZN5n3ErWRjTQxLeqWfEJt3TTkyHFpcc1tWAzhjK514qg0f64g==`
- Matches the local pack dry-run integrity.

Post-publish registry proof:

```sh
npm view @trybotster/terminal-protocol version
# 0.1.0

npm view @trybotster/terminal-protocol
# @trybotster/terminal-protocol@0.1.0
# dist-tags.latest: 0.1.0
# dist.integrity matches the pack dry-run
```

Clean-dir registry consumer, required. New temp directory. Not a path or workspace install:

```sh
cd "$CONSUMER_DIR"
npm init -y
npm install @trybotster/terminal-protocol@0.1.0 typescript
npx tsc --strict --module nodenext --moduleResolution nodenext --noEmit consumer.ts
# tsc ok
```

`npm ls` resolved `https://registry.npmjs.org/@trybotster/terminal-protocol/-/terminal-protocol-0.1.0.tgz`.

Runtime assertions on the installed package:

- `metadata.protocol` equals `botster-terminal-v1`
- `metadata.protocol_version` equals `1`
- `metadata.features` includes `terminal_streaming`, `resize`, and `snapshot_delivery=ready_then_history`
- `metadata.package_version` equals `0.1.0`
- ready-then-history fixture export still loads

The production entry point is a Node consumer installing `@trybotster/terminal-protocol@0.1.0` from the public registry. Local pack smoke is pre-publish only.

Cargo workspace fmt/test/clippy/doc were not run. This change is Node smoke plus docs. The plan names the Node smoke as the required repo gate for that scope. No wrapper was invented.

## Unverified behavior or residual risk

- This ticket did not run botster-web or pin the package there. Web `ticket_1786661008_897067` still owns the pin.
- Agent session still has no npm login. Later Core publishes will need the same human `--access public` path unless a later ticket adds an authorized operator flow.
- Published `0.1.0` is immutable. A bad content change later requires a new unused version on every version surface.
- The published tarball was the committed package before the smoke-script type import change. Those types already existed in `index.d.ts` / `terminal-protocol.ts`. The smoke change is repository proof, not a package-content change.

## Missing vault guidance discovered

- Core had no checked-in npm publish runbook. Captured to vault inbox: `core-types-only-npm-publishes-use-human-access-public-after-agent-401.md`.
- No new protocol decision. Did not recapture [[botster terminal v1 starts at protocol 1 and conformance revision 1]] or the two-crate opacity rule.

## Runtime-teardown class

`teardown_class_applies`: no.

This ticket did not change peer lifecycle, SessionIo, ClientWorker teardown, multi-peer ownership, or live-runtime bounds.

## Assumptions

- The committed `packages/terminal-protocol@0.1.0` assets were the intended first publish. Registry integrity matches the local pack.
- First publish used `--access public`. The package.json file has no `publishConfig`.
- Merge remains the later direct-merge pipeline step. Implement does not open a PR.
