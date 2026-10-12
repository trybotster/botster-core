# A read's reply follows the observations of the output before it (R-114)

The worker sends a read's reply only after the output observations of every byte that it applied before it took the
read. A read is `ReadScreen`, `ReadCursor`, `ReadModeFlags` or `CaptureSnapshot`.

- The model applies PTY output in `feed_model`. In the same turn, `after_step` reports the step's observations (`Title`,
  `Cwd`, `Bell`, `Modes`, `Lost`, ...) through `report`, and `output_advanced` sends the `Output` report or marks it as
  waiting (`output_unsent`) while the host is slow.
- The worker takes a read in a later turn, and sends its `Done` through `report`. `report` sends a waiting `Output`
  report first.
- Every report is one `LinkSend` in the worker's one ordered action queue, so the host reads the reports in this order.

A host that has consumed a read's `Completed` has consumed those observations too: Core's model holds them. A test that
compares Core's model with the bytes (the conformance `oracle_resume`) uses that `Completed` as its fence. An output
fence (`Activity`, `await_title`) proves only that the worker applied the bytes. Bytes that the model keeps unapplied
(an ESC that can start the ST of an unfinished string) are not covered. The proof is the worker-core test
`a_reads_reply_follows_the_observations_of_the_output_before_it`.
