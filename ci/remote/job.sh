#!/usr/bin/env bash
# One queued Botster gate on the shared test host, started by `testq submit` (botster-gate; ~/testq/README.md on the host).
# Runs a command in the snapshot inside the gate image, limited to the job's CPUs. The snapshot is a git checkout of the
# exact head, with the base branch at refs/remotes/origin/<base>, so the diff checks have their base.
#
#   ci/remote/job.sh <snapshot-dir> <cpus> [--run=<id>] [--branch=<name>] [--base=<sha>] [--lease=<file>]
#                    -- <command and args...>
#
# --run names the job for its caller: botster-gate finds and cancels the job by it. A failed job keeps its mutation reports
# (mutants.out, mutants-fakes, target/mutants.out) in ~/testq/projects/<project>/artifacts/<run> for 7 days.
# --branch picks the target volume: one per project and branch, so a branch's gates build incrementally. botster-gate
# holds a host lock per branch for the whole gate, so two gates never share a target volume at once.
# --base sets BOTSTER_CI_BASE_REF to the base commit that the client recorded, so the diff checks use it.
# --lease is the client's lease file (~/testq/projects/botster-locks/leases/<run>). The client's holder process keeps it
# locked while the client lives and sends heartbeats. When the lock is free, the client is gone: the job does not start,
# or it stops and cleans up.
#
# Volumes: <project>-cargo (cargo's registry and git checkouts, shared by the project's jobs), <project>-target-<branch>,
# and <project>-npm (the npm cache). A target volume that no gate used for 7 days is removed.
#
# This file is the same in botster-contracts and botster-core except for the settings block below; keep them in step.
set -euo pipefail

# --- settings -----------------------------------------------------------------------------------------------------------
project=botster-core
# 1: Cargo.lock has private git dependencies (trybotster). A separate fetch container gets the host's `gh auth token`;
# the gate container then runs with no network and no token.
private_git_deps=1
# Extra build arguments for the image, one per line, read from the snapshot. They are part of the image tag.
# The pinned nightly of the public-api and fuzz steps (xtask/src/tools.rs).
image_args() { echo "RUST_NIGHTLY=$(sed -n 's/^pub const NIGHTLY: &str = "\(.*\)";/\1/p' "$dir/xtask/src/tools.rs")"; }
# Files whose content picks the image tag, besides the Dockerfile.
image_inputs=(rust-toolchain.toml)
# Volumes besides cargo, target and npm: "<suffix>:<mount>". zig: Zig's global cache (libghostty's packages).
extra_volumes=(zig:/zig)
# --------------------------------------------------------------------------------------------------------------------------

dir=$(realpath -- "${1:?job.sh: no snapshot directory}")
name= secrets= watcher=

# Deletes <child> only when its real path is one directory directly inside <parent>.
remove_child() {
  local parent child
  [ -e "$2" ] || return 0
  parent=$(realpath -- "$1") child=$(realpath -- "$2") || return 0
  [[ $child == "$parent"/* && ${child#"$parent"/} != */* ]] || return 0
  rm -rf -- "$child"
}

cleanup() {
  local status=$?
  # testq cancel TERMs the job's process group and testq-launch forwards a second TERM; ignore both so cleanup finishes.
  trap - EXIT
  trap '' INT TERM HUP
  [ -n "$watcher" ] && kill "$watcher" 2>/dev/null
  if [ -n "$name" ]; then
    docker ps --all --quiet --filter "label=testq.job=$name" | xargs --no-run-if-empty docker rm --force >/dev/null 2>&1 || true
  fi
  [ -n "$secrets" ] && remove_child "${TMPDIR:-/tmp}" "$secrets"
  [[ $dir =~ /testq/projects/[^/]+/jobs/[^/]+$ ]] && remove_child "${dir%/*}" "$dir"
  exit "$status"
}
trap cleanup EXIT
# Long docker runs are waited on in the background so a signal interrupts them. Each handler ignores further signals
# before it exits, so a second signal cannot cut cleanup short (HyperFlex test/docker/job.sh has the full reasoning).
on_signal() { trap '' INT TERM HUP; exit "$1"; }
trap 'on_signal 129' HUP
trap 'on_signal 130' INT
trap 'on_signal 143' TERM

cpus=${2:?job.sh: no CPU count}
shift 2
run=$(basename "$dir") branch= base= lease=
while (( $# )); do
  case $1 in
    --run=*) run=${1#--run=} ;;
    --branch=*) branch=${1#--branch=} ;;
    --base=*) base=${1#--base=} ;;
    --lease=*) lease=${1#--lease=} ;;
    --) shift; break ;;
    *) echo "job.sh: unknown option '$1' (the command follows --)" >&2; exit 64 ;;
  esac
  shift
done
reject() { echo "job.sh: $1" >&2; exit 64; }
segment='^[A-Za-z0-9][A-Za-z0-9_.-]*$'
[[ $run =~ $segment ]] || reject "invalid --run name"
[[ $cpus =~ ^[1-9][0-9]*$ ]] || reject "invalid CPU count"
(( $# )) || reject "no command given"
[ -z "$base" ] || [[ $base =~ ^[0-9a-f]{40}$ ]] || reject "--base must be a full commit sha"
[ -z "$lease" ] || [[ $lease =~ ^$HOME/testq/projects/botster-locks/leases/[A-Za-z0-9][A-Za-z0-9_.-]*$ ]] \
  || reject "invalid --lease file"

# The client's lease: a free lock means the client is gone (Ctrl-C it could not report, a lost connection, a killed shell).
# The holder deletes the file when it exits; flock would create it again, so a missing file also means "gone".
client_gone() { [ ! -e "$lease" ] || flock -n "$lease" true; }
if [ -n "$lease" ]; then
  if client_gone; then
    echo "job.sh: the client is gone; the job does not start" >&2
    exit 125
  fi
  # timer: deadline — polls the lease every 5 s; flock has no wait-for-release-by-another-process primitive here.
  ( while sleep 5; do
      if client_gone; then
        echo "job.sh: the client is gone (no heartbeat); stopping the job" >&2
        kill -TERM $$
        exit 0
      fi
    done ) &
  watcher=$!
fi

# A branch name becomes a volume name: lower case, [a-z0-9_.-], with a hash of the full name against collisions.
if [ -z "$branch" ]; then branch=$(git -C "$dir" rev-parse --abbrev-ref HEAD); fi
slug=$(printf %s "$branch" | tr 'A-Z' 'a-z' | tr -c 'a-z0-9_.\n-' '-' | cut -c1-40)
slug=$slug-$(printf %s "$branch" | sha1sum | cut -c1-8)

projdir=$(dirname "$(dirname "$dir")")
artifacts=$projdir/artifacts
stamps=$projdir/targets
name=testq-$project-$(basename "$dir" | sha1sum | cut -c1-10)
hash=$(cd "$dir" && { cat ci/remote/Dockerfile "${image_inputs[@]}"; image_args; } | sha256sum | cut -c1-12)
image=$project-gate:$hash
toolchain=$(sed -n 's/^channel *= *"\(.*\)"/\1/p' "$dir/rust-toolchain.toml")
cargo_volume=$project-cargo
target_volume=$project-target-$slug
npm_volume=$project-npm

# Priority jobs (CI) get more CPU weight when the host is busy. Memory: rustc and the linker need about 2 GB a CPU.
shares=$(( ${TESTQ_PRIORITY:-0} ? 2048 : 1024 ))
limits=(--cpus "$cpus" --cpu-shares "$shares" --memory "$(( cpus * 2 + 4 ))g")

# --init: tini is PID 1 and reaps orphans, as launchd does on the Mac. Without it, a test's reparented child stays a zombie
# and the leftover-process check fails.
container() {
  docker run --rm --init --label "testq.job=$name" "${limits[@]}" --user "$(id -u):$(id -g)" -e HOME=/tmp "$@" &
  wait $!
}

echo "gate job: $project $(git -C "$dir" rev-parse HEAD) on $(hostname), $cpus CPUs, image $image, target $target_volume"

mkdir -p "$artifacts" "$stamps"
find "$artifacts" -mindepth 1 -maxdepth 1 -type d -mtime +7 -exec rm -rf {} +
# Target volumes that no gate used for 7 days. Docker refuses to remove a volume that a running job mounts.
while IFS= read -r stamp; do
  docker volume rm "$(basename -- "$stamp")" >/dev/null 2>&1 && rm -f -- "$stamp"
done < <(find "$stamps" -mindepth 1 -maxdepth 1 -type f -name "$project-target-*" -mtime +7)
touch "$stamps/$target_volume"

exec {image_lock}>"$projdir/image.lock"
flock "$image_lock"
if ! docker image inspect "$image" >/dev/null 2>&1; then
  echo "Building $image (rust $toolchain)..."
  build_args=(--build-arg "RUST_TOOLCHAIN=$toolchain" --build-arg "BUILD_JOBS=$cpus")
  while IFS= read -r arg; do [ -n "$arg" ] && build_args+=(--build-arg "$arg"); done < <(image_args)
  build_log=$projdir/image-build.log
  docker build --progress=plain "${build_args[@]}" -t "$image" "$dir/ci/remote" >"$build_log" 2>&1 &
  wait $! || { tail -n 40 "$build_log"; echo "job.sh: the image build failed; the full log is $build_log on the host" >&2; exit 1; }
  # Drop superseded images; Docker refuses to remove one that a running job still uses.
  docker image ls "$project-gate" --format '{{.Repository}}:{{.Tag}}' | grep -vxF "$image" \
    | xargs --no-run-if-empty docker image rm >/dev/null 2>&1 || true
fi
for volume in "$cargo_volume" "$target_volume" "$npm_volume" "${extra_volumes[@]/#/$project-}"; do
  volume=${volume%%:*}
  if ! docker volume inspect "$volume" >/dev/null 2>&1; then
    docker volume create "$volume" >/dev/null
    docker run --rm --label "testq.job=$name" -v "$volume:/v" "$image" chown "$(id -u):$(id -g)" /v &
    wait $!
  fi
done
exec {image_lock}>&-

mounts=(-v "$dir:/work" -v "$target_volume:/work/target" -v "$cargo_volume:/cargo" -v "$npm_volume:/npm")
for volume in "${extra_volumes[@]}"; do mounts+=(-v "$project-${volume%%:*}:${volume#*:}"); done
network=()
gate_env=(-e TZ=UTC)
[ -n "$base" ] && gate_env+=(-e "BOTSTER_CI_BASE_REF=$base")

if (( private_git_deps )); then
  # Fetch every dependency with the host's GitHub CLI token, in a container that runs no repo code but cargo's resolver.
  # The token is a 0600 file mounted read-only and read by a git credential helper for github.com only: never in a process
  # argument, a container's environment, the image, or a volume. The gate container then runs offline.
  secrets=$(mktemp -d)
  chmod 700 "$secrets"
  (umask 077; printf '%s' "$(gh auth token)" > "$secrets/github-token")
  container --name "$name-fetch" "${mounts[@]}" -v "$secrets:/run/testq-secrets:ro" -w /work \
    -e GIT_TERMINAL_PROMPT=0 -e GIT_CONFIG_COUNT=1 \
    -e GIT_CONFIG_KEY_0=credential.https://github.com.helper \
    -e 'GIT_CONFIG_VALUE_0=!f() { cat >/dev/null; [ "$1" = get ] || exit 0; echo username=x-access-token; printf "password=%s\n" "$(cat /run/testq-secrets/github-token)"; }; f' \
    "$image" bash ci/remote/fetch.sh
  remove_child "${TMPDIR:-/tmp}" "$secrets"
  secrets=
  network=(--network none -e CARGO_NET_OFFLINE=true)
fi

status=0
container --name "$name" "${network[@]}" "${gate_env[@]}" "${mounts[@]}" -w /work "$image" "$@" || status=$?

# The mutation reports: botster-contracts writes them in the tree, botster-core in the target volume, which only a
# container sees.
if (( status )); then
  remove_child "$artifacts" "$artifacts/$run"
  mkdir -p "$artifacts/$run"
  container --name "$name-artifacts" "${mounts[@]}" -v "$artifacts/$run:/out" "$image" \
    sh -c 'for p in mutants.out mutants-fakes target/mutants.out; do
             [ -e "/work/$p" ] && cp -a "/work/$p" "/out/$(echo "$p" | tr / -)"; done; true' || true
  if [ -n "$(ls -A "$artifacts/$run")" ]; then
    echo "Failure artifacts: $artifacts/$run (kept 7 days)"
  else
    rmdir "$artifacts/$run"
  fi
fi
exit "$status"
