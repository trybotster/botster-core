# The cursor ordinal (ST-7; P5; steward ruling R-101 Q6 and its follow-up)

- **What it is.** The high 24 bits of every input cursor of `ReadFacts` (`CURSOR_ORDINAL_BITS`, `botster-core-link`).
  Cursors must stay distinct across instances for the life of the data directory, also after a `Remove` and a `Create`
  of the same id and in a later Core process. So the ordinal is a durable counter of the registry, under the
  data-directory lock. It goes to the worker in `LaunchSpec.cursor_ordinal` only; the worker keeps it across an
  adoption.
- **The row.** `meta/cursor-ordinal` (`session::ORDINAL_KEY`): the highest ordinal that a `Start` reserved, in decimal.
  It is outside `session/`, as `meta/host-epoch` is: a row there is not a session, so the id scan of `HostDriver::open`,
  `AdoptAll` and the testkit's row tap never see it.
- **Two reads, one reason.** The host epoch is read and raised in `botster-core-sys` `DataDir::open`, before the driver
  exists: the real tier's edge owns it. The ordinal is read by `HostDriver::open` through the registry edge
  (`read_rows`), with the session rows. The testkit has no `DataDir`, and it must apply the same ordinal rule, so the
  rule lives in the host, over the edge that both tiers have.
- **Reserve per `Start`** (`StartPhase::RowOrdinal`, after the token and before the `Starting` row). The phase takes the
  next ordinal, moves memory past it at once, and writes the row. `Ok` goes on to the `Starting` row. A failed or
  uncertain write ends the `Start` as a failed `Starting` row write does (`start_not_run`): the session is `Created`,
  and the op completes `RegistryFailed` with the write's `uncertain`. An ordinal reaches a worker only after a write of
  a value at or above it returned `Ok`, and memory never goes back, so no ordinal is used twice. A `Start` after a
  failure reserves the next ordinal.
- **Exhaustion is permanent for the data directory.** Past `MAX_CURSOR_ORDINAL`, `Start` completes
  `RegistryFailed{uncertain: false}` with nothing written and no event but its completion. Every later `Start`, in this
  process or a later one, fails the same way. A counter row that is not a number fails `open` with `RegistryFailed`.
- **Effect on the transcripts.** `storage_fail_write` fails the next write. Before a `Start` it is now taken by the
  ordinal write, with the same outcome (`RegistryFailed` with the fault's `uncertain`, the session `Created`); for
  example `a5_3_async_failure_only_through_edges_and_real_completion` steps 5-10. The cost is one more registry write
  per `Start`.
