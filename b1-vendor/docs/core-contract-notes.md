# core-contract: feature notes

Feature `core-contract` (design section 12, lane A, order 2). Crate: `crates/botster-core-contract`. Schemas: `schemas/core/`.

## Prior art (BUILD.md, "Starting fresh" rule 0)

- `~/botster-sessions/shared/core-rebuild-plan-v1.md` marks `botster-core-contract` and Core sections 1 to 3 and 8 to 9B as NEW. Nothing is reused there.
- botster-core at 72b2e3354ffc: `crates/botster-core/src/contract/*` has the legacy actor and daemon vocabulary. It does not follow the begin, pump and one-queue model, so nothing was stolen. There is no `Stolen-From` commit.
- `frozen/final6/audits` (completeness sweep B2, client parity) is the source of Amendments 2 to 4 and gave no type to reuse. `botster-architecture.md` has no Core type notes.
- The section 5.1A encoders (`botster-terminal-ghostty`) belong to the real Core. A types crate has no encoding.
- Vault: "botster core public enums are breaking until non exhaustive is decided" means every enum is `#[non_exhaustive]`. "botster core contract surface needs consumer proof" means every public type traces to a clause.

## Judgment calls

1. `ErrorCode` carries typed data where the contract writes `Code{..}`. This covers `StartFailed`, `RegistryFailed{uncertain}`, `InvalidOffer`, `BoundUnavailable`, `InvalidConfig{field}` and `Unsupported{what?}`.
2. `SpawnRequest.size_policy` is added (DP-6 says `SetSizePolicy` is also accepted in `Create`; the A2-1 field list omits it).
3. `EventsLost.tap_dropped_bytes` is added (TP-1).
4. `attach` returns `AttachResult{route, limits}` (OU-1).
5. `InputRecord.at` is a `u64` (ST-7 names no unit). `InputRecord` keeps the 5.1A fields as sent: `shifted_key`, `base_layout_key`, `text`, `repeat` and `require_bracketed`. `pty_bytes` is null while the record is pending.
6. `CoreLimits.tap_inflight_bytes` and `max_route_frame_bytes` are optional, because 9B derives their defaults. Durations travel as integer milliseconds.
7. `RouteCloseReason::BadFrame` uses the codec's `ProtocolErrorCode`. A2-3 limits it to four values, and a Rust type cannot say that.

## Rulings applied

- R-9 C1, now Core A4-1 (accepted, frozen/final9): `service_report` is a `CoreApi` method. `Started` and `Reconnected` carry no lists.
- R-9 C2: `CoreLimits::validate` checks only the 1 KiB lower bound of `default_route_chunk_bytes`.

## Not derivable from the clauses

- `RouteAdopted` has no queue class in 6.2.
- TH-1 "not `Sync`" cannot be stated on a trait. `Send` is.
- Invalid UTF-8 in `Text` cannot occur in the Rust facade (C embedders only).
- The SV-2 `ServiceLimits` refusals are left to the implementations. `validate` covers `CoreLimits` only.
- OU-1 names no default for `AttachOptions.history`.
