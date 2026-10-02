#!/usr/bin/env bash
# The fetch step of a gate job on the shared test host (ci/remote/job.sh). It runs in its own container with the host's
# GitHub token, and downloads every dependency into the cargo volume, so the gate container runs with no network:
#   - the workspace's dependencies, botster-contracts (private) included;
#   - the dependencies of the conformance probe, which `cargo xtask prebuild-worker` installs from the same botster-contracts
#     tag with `cargo install --git … --locked`.
#   - the git submodules (libghostty's source, trybotster/ghostty), through a mirror per submodule in the cargo volume;
#   - libghostty's Zig packages, into the store that build.rs reads ($BOTSTER_ZIG_PACKAGES, in the zig volume). The
#     crate's prefetch-zig.sh fills it; it runs only when a package that build_data.rs lists is missing.
# It runs no code of this repo but cargo's resolver and prefetch-zig.sh (a Zig build of the pinned Ghostty source).
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

cargo fetch --locked -q

# The botster-contracts commit of Cargo.lock, and cargo's checkout of it.
rev=$(sed -n 's|^source = "git+https://github.com/trybotster/botster-contracts?tag=[^#]*#\([0-9a-f]*\)"$|\1|p' Cargo.lock | sort -u)
[ "$(printf '%s\n' "$rev" | wc -l)" = 1 ] && [ -n "$rev" ] || { echo "fetch.sh: Cargo.lock names no single botster-contracts commit" >&2; exit 1; }
checkout=
for candidate in "$CARGO_HOME"/git/checkouts/botster-contracts-*/"${rev:0:7}"; do
  [ -f "$candidate/Cargo.lock" ] && checkout=$candidate
done
[ -n "$checkout" ] || { echo "fetch.sh: no cargo checkout of botster-contracts $rev" >&2; exit 1; }
cargo fetch --locked -q --manifest-path "$checkout/Cargo.toml"
# `cargo install --git … --tag <tag>` looks the tag up in cargo's git database, under refs/remotes/origin/tags/<tag>; the
# locked workspace fetch stored only the commit. Fetch the tag ref the way cargo would.
tag=$(sed -n 's|^source = "git+https://github.com/trybotster/botster-contracts?tag=\([^#]*\)#.*|\1|p' Cargo.lock | sort -u)
for db in "$CARGO_HOME"/git/db/botster-contracts-*; do
  git -C "$db" fetch -q --no-tags https://github.com/trybotster/botster-contracts "+refs/tags/$tag:refs/remotes/origin/tags/$tag"
done
echo "fetch: dependencies of the workspace and of botster-contracts ${rev:0:8} are in the cargo volume"

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
