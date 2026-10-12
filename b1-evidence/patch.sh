# Points every contracts crate at the vendored contracts .43 tree with the B-1 change (b1-vendor/), for the "after" runs.
perl -pi -e 's/^(members = \["crates\/\*", "xtask"\])$/$1\nexclude = ["b1-vendor"]/' Cargo.toml
printf '\n[patch."https://github.com/trybotster/botster-contracts"]\n' >> Cargo.toml
for c in botster-conformance botster-core-conformance botster-core-contract botster-hub-conformance botster-hub-contract \
         botster-probe-script botster-route-codec botster-test-support botster-ui-contract; do
  printf '%s = { path = "b1-vendor/crates/%s" }\n' "$c" "$c" >> Cargo.toml
done
tail -11 Cargo.toml
