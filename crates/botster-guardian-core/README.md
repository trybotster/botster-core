# botster-guardian-core

This crate contains the sans-IO `Guardian` machine. A sans-IO machine performs no I/O.
It has no compatibility promise. Consumers must use `botster-core`.

The real driver and the testkit driver use the same machine.
The driver supplies the clock, process results, control bytes, logs, and cause bytes.
The driver performs the machine's actions.
The guardian holds no service lane connection.
