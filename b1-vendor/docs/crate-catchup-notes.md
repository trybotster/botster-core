# crate-catchup notes (Stage 0, Part 1)

Brief: `~/botster-sessions/shared/foundation/brief-crate-catchup.md`. Base: main `d18b6de` (+ `docs/handoffs/steward.md`).
Goal: every type, field, variant, error code, limit and default that a FINAL amendment or erratum (since manifest final6) adds or changes is present in its crate (and in the generated schemas and TS).

## Prior art

- **`foundation/core-a13` @ `8bfc5b7`** (not merged) and `docs/handoffs/core-a13.md`, "Left" item 1. REUSED: the lead's decision (a) and the shape it states (`ClipboardContent{mime, bytes}`, `contents: Option<Vec<_>>`, `None` = TooLarge, `Some(vec![])` = clear). The code is written here from A13-1, not cherry-picked: the branch has no crate change.
  REJECTED for this feature (brief default): the 11 A13 transcripts, the `oracle_clipboard_ack` control and the four R-28 transcript fixes. They add a new real-only control and new transcripts that need their own clause review; they stay on `foundation/core-a13`, and the a13 ids stay pending.
- The EV-7 transcripts: the handoff names only the two EV-3 transcripts, but three EV-7 transcripts also matched the old `bytes` field. They move to `contents` too (the vocabulary test fails otherwise). Their stimulus is unchanged.
- Vault: `botster key text limits bound exhaustive encoder state probes` (A15's reason); no note covers crate catch-up.
- Ecosystem: none needed. The changes use the crates' existing serde/schemars types and codegen.

## Decisions

- **Core A16:** `OpOutput::ServiceExit(ServiceExit)` becomes `OpOutput::ServiceEnd(ServiceEnd)` (wire `service_end`), as `Stop` completes with `End(SessionEnd)`. Four transcripts move from `service_exit` to `service_end: {Exited: ...}` and add `Core A16-1` to `because`. FakeCore keeps its behavior (frozen fakes get no new fidelity, BUILD.md) and wraps its exit in `Exited`; the a16_1 ids stay pending (real-only, Stage 1 P7).
- **HC A.9b verb reasons:** the registry recorded A.9b failures for actions only. `VerbSpec` gains `reasons`, and `x-verbs` carries them. This is a gap of HC A2, found by the audit.
- **HC A.9b `package open_asset`:** the registry listed only `wrong_state:assets_origin_unconfigured`. A.9b also lists `not_found` and `invalid_arguments` (both or neither of `asset_id` and `name`). Other actions list a bare common code when A.9b names it for the action (`attach` `busy`, `resolve_route` `not_found`), so `open_asset` does the same. Found by the per-item audit.
- **HC A7 (pending):** the steward's answers to the preview `breaking` question (HP A8-3 vs HC A.9) and to `static_origin` (HP A2-42), candidate 2. Its crate change is its own commit at the tip of the branch; its ids enter the ledger only when the steward publishes A7 on a final branch.

## Audit table

The per-item audit is `docs/crate-catchup-audit.md`. It lists each type, field, variant, error code, limit, default, config key and feature name that an amendment or erratum adds or changes, with its exact crate symbol, or `n/a` with the reason. It covers Core A2 to A16 and errata 1 to 4, the route codec revision 36, HC A1 to A6 and A7 (pending), HP A1 to A8, the Lua binding r9 and its Amendments 1 and 2, UI 1.1 to 1.4 and 1.4 erratum 1, and the CLI spec.

Summary of the **fixed** items (every other item is present, or n/a with a reason in the audit):

| Amendment | Item | Crate symbol | Commit |
|---|---|---|---|
| Core A13 | A13-1 `ClipboardWrite.contents`, `ClipboardContent{mime, bytes}` | `Event::ClipboardWrite::contents`, `ClipboardContent` | 5bc6ded |
| Core A15 | A15-1 `max_key_text_bytes`, default 256, range 1 to 4,096 | `CoreLimits::max_key_text_bytes`, `CoreLimits::check_ranges` | 5bc6ded |
| Core A16 | A16-1, A16-2 `ServiceEnd`; `StopService` Ok value | `ServiceEnd`, `OpOutput::ServiceEnd` | 5bc6ded |
| HC A2 | A.9b verb failures | `VerbSpec::reasons`, `x-verbs.*.reasons` | f4557da |
| HC A2 | A.9b `package open_asset` failures `not_found`, `invalid_arguments` | `PACKAGE` action `open_asset` reasons | 753fdaa |
| HC A6 | A6-1 install result; A6-2 `branch?`, `head` | `PackageInstallResult`; `WorktreeRow::{branch, head}` | d3158fd |
| HC A7 (pending) | A7-1 preview `breaking`; A7-2 `static_origin?` | `BreakingEntry`, `UpgradePreview::breaking`, `PreviewPackageUpgradesResult::breaking`, `PackageRow::static_origin` | the tip commit |
| Core A7 | A7-1 `InvalidInput{field}` (also A9-1, A15-1) | `ErrorCode::InvalidInput::field` | stage0/invalid-input-field |
| HP A4 | A4-1 `backpressured{write_pending_full}` | `CODES`, `CALLS` `sessions.safe_write` | 6b91b63 |
| HP A5 | A5-1 record `{id, entity}` | `EntityRecord`, `EntityProviderResult::records`, `EntityPublishArgs::entity` | 6b91b63 |
| HP A8 | A8-2 `interface_not_additive`; A8-3 `breaking_not_accepted` | `CODES` | 6b91b63 |
