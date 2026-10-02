#!/usr/bin/env bash
# The public fetch step of a gate job on the shared test host (ci/remote/job.sh). It runs after fetch.sh, in its own
# container WITH network and WITHOUT the GitHub token, because it builds third-party source:
#   - the git submodules (libghostty's source, trybotster/ghostty, public), through a mirror per submodule in the cargo
#     volume;
#   - libghostty's Zig packages, into the store that build.rs reads ($BOTSTER_ZIG_PACKAGES, in the zig volume). The
#     crate's prefetch-zig.sh fills it (a Zig build of the pinned Ghostty source); it runs only when a package that
#     build_data.rs lists is missing.
# The gate container then runs with no network.
set -euo pipefail

# Submodules. The mirror keeps every gate from cloning the whole repository again; --dissociate copies the objects into
# the snapshot, so the gate container needs nothing outside it.
if [ -f .gitmodules ]; then
  git config -f .gitmodules --get-regexp '^submodule\..*\.path$' | while read -r key path; do
    name=${key#submodule.}; name=${name%.path}
    url=$(git config -f .gitmodules "submodule.$name.url")
    mirror=$CARGO_HOME/submodule-mirrors/$(printf %s "$url" | sha1sum | cut -c1-16).git
    mkdir -p "${mirror%/*}"
    # Gates of this project share the mirror: one updates it at a time.
    (
      flock 9
      if [ -d "$mirror" ]; then git -C "$mirror" fetch -q --prune; else git clone -q --mirror "$url" "$mirror"; fi
      git submodule update -q --init --reference "$mirror" --dissociate -- "$path"
    ) 9>"$mirror.lock"
  done
  echo "fetch: submodules at the pinned commits"
fi

# libghostty's Zig packages: build.rs fetches nothing, so the store must hold every package that build_data.rs lists.
ghostty=crates/botster-terminal-ghostty
if [ -f "$ghostty/prefetch-zig.sh" ]; then
  store=${BOTSTER_ZIG_PACKAGES:?fetch.sh: BOTSTER_ZIG_PACKAGES is not set}
  missing=0
  for hash in $(sed -n '/pub const ZIG_PACKAGES/,/^];/p' "$ghostty/build_data.rs" | sed -n 's/^ *"\(.*\)",$/\1/p'); do
    [ -f "$store/p/$hash.tar.gz" ] || missing=$(( missing + 1 ))
  done
  if (( missing )); then
    echo "fetch: $missing Zig packages are missing from $store; running prefetch-zig.sh"
    mkdir -p "$store"
    # One prefetch at a time per store; a gate that waited finds the packages there and the copy is the same file.
    flock "$store/.prefetch.lock" bash "$ghostty/prefetch-zig.sh"
  else
    echo "fetch: every Zig package of build_data.rs is in $store"
  fi
fi
