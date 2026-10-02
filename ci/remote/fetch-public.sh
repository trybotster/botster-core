#!/usr/bin/env bash
# The public fetch step of a gate (botster-gate). It brings the public sources that the gate needs and that come from
# third-party code, with network and without any GitHub token:
#   - the git submodules (libghostty's source, trybotster/ghostty, public), through a mirror per submodule in
#     $CARGO_HOME/submodule-mirrors, each submodule cleaned to its pinned commit;
#   - libghostty's Zig packages, into the store that build.rs reads ($BOTSTER_ZIG_PACKAGES, default
#     ~/.cache/botster/zig-packages, as build.rs). The crate's prefetch-zig.sh (a Zig build of the pinned Ghostty source)
#     fills a staging directory inside the store; each archive is published with an atomic rename and never overwritten,
#     so a gate never reads a partial archive. An archive that fails `gzip -t` counts as missing.
# Linux: ci/remote/job.sh runs it after fetch.sh, in its own container; the gate container then runs with no network.
# Mac: botster-gate runs it in the gate tree, inside the exclusive botsterq job, with the gate's cleared environment.
set -euo pipefail

: "${CARGO_HOME:=$HOME/.cargo}"
# A lock that works on Linux (flock) and on macOS (no flock: perl's flock).
with_lock() {
  local file=$1; shift
  if command -v flock >/dev/null 2>&1; then
    flock "$file" "$@"
  else
    perl -MFcntl=:flock -e 'open(my $f, ">", shift) or die "$!"; flock($f, LOCK_EX) or die "$!"; exec @ARGV or die "$!"' \
      "$file" "$@"
  fi
}
sha1() { if command -v sha1sum >/dev/null 2>&1; then sha1sum; else shasum; fi; }

# Submodules. The mirror keeps every gate from cloning the whole repository again; --dissociate copies the objects into
# the tree, so the gate needs nothing outside it. Each submodule is then cleaned: no file of an earlier build stays.
if [ -f .gitmodules ]; then
  git config -f .gitmodules --get-regexp '^submodule\..*\.path$' | while read -r key path; do
    name=${key#submodule.}; name=${name%.path}
    url=$(git config -f .gitmodules "submodule.$name.url")
    mirror=$CARGO_HOME/submodule-mirrors/$(printf %s "$url" | sha1 | cut -c1-16).git
    mkdir -p "${mirror%/*}"
    # Gates share the mirror: one updates it at a time.
    with_lock "$mirror.lock" bash -c '
      set -euo pipefail
      mirror=$1 url=$2 path=$3
      if [ -d "$mirror" ]; then git -C "$mirror" fetch -q --prune; else git clone -q --mirror "$url" "$mirror"; fi
      git submodule update -q --init --force --reference "$mirror" --dissociate -- "$path"
      git -C "$path" clean -q -ffdx' _ "$mirror" "$url" "$path"
  done
  echo "fetch: submodules at the pinned commits, clean"
fi

# libghostty's Zig packages: build.rs fetches nothing, so the store must hold every package that build_data.rs lists.
ghostty=crates/botster-terminal-ghostty
if [ -f "$ghostty/prefetch-zig.sh" ]; then
  store=${BOTSTER_ZIG_PACKAGES:-$HOME/.cache/botster/zig-packages}
  mkdir -p "$store/p"
  # The same extraction as prefetch-zig.sh (it also matches ZIG_PACKAGES_IN_ZON); each hash once.
  hashes=$(sed -n '/pub const ZIG_PACKAGES/,/^];/p' "$ghostty/build_data.rs" | sed -n 's/^ *"\(.*\)",$/\1/p' | sort -u)
  # Prints the hashes whose archive is absent or broken.
  missing() {
    local hash
    for hash in $hashes; do
      gzip -t "$store/p/$hash.tar.gz" 2>/dev/null || echo "$hash"
    done
  }
  if [ -z "$(missing)" ]; then
    echo "fetch: every Zig package of build_data.rs is in $store"
  else
    # Under the store lock: check again (another gate may have published them meanwhile), prefetch into a staging
    # directory on the same file system, and publish each missing archive with a rename.
    export store hashes ghostty
    export -f missing
    with_lock "$store/.prefetch.lock" bash -c '
      set -euo pipefail
      todo=$(missing)
      [ -n "$todo" ] || { echo "fetch: another gate published the Zig packages"; exit 0; }
      staging=$(mktemp -d "$store/.staging.XXXXXX")
      trap "rm -rf \"$staging\"" EXIT
      echo "fetch: $(echo $todo | wc -w | tr -d " ") Zig packages are missing from $store; running prefetch-zig.sh"
      BOTSTER_ZIG_PACKAGES=$staging bash "$ghostty/prefetch-zig.sh"
      for hash in $todo; do
        gzip -t "$staging/p/$hash.tar.gz"
        mv -f "$staging/p/$hash.tar.gz" "$store/p/$hash.tar.gz"
      done
      echo "fetch: published $(echo $todo | wc -w | tr -d " ") Zig packages in $store"'
  fi
fi
