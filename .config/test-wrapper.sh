#!/bin/sh
# A nextest run-wrapper (see .config/nextest.toml). It records the pid of each test process
# (`exec` keeps the pid) in $BOTSTER_TEST_PIDFILE, so `cargo xtask test-budget` can prove which
# processes belong to the run. nextest puts each test in its own process group; the group id is the test pid.
[ -n "$BOTSTER_TEST_PIDFILE" ] && echo "$$" >> "$BOTSTER_TEST_PIDFILE"
exec "$@"
