#!/usr/bin/env bash
# One queued Botster gate on the shared test host, started by `testq submit` (botster-gate; ~/testq/README.md on the host).
# Runs a command in the snapshot inside the gate image, limited to the job's CPUs. The snapshot is a git checkout of the
# exact head, with the base branch at refs/remotes/origin/<base>, so the diff checks have their base.
#
#   ci/remote/job.sh <snapshot-dir> <cpus> [--run=<id>] [--branch=<name>] [--base=<sha>] [--lease=<file>]
#                    -- <command and args...>
#
# --run names the job for its caller: botster-gate finds and cancels the job by it. A failed job keeps its mutation reports
# (mutants.out, mutants-fakes, target/mutants.out, target/mutants-stage2/mutants.out) in
# ~/testq/projects/<project>/artifacts/<run> for 7 days.
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
# Volumes besides cargo, target and npm: "<suffix>:<mount>". zig: Zig's global cache and the libghostty package store.
extra_volumes=(zig:/zig)
# Environment of the fetch and gate containers. libghostty's build.rs reads its Zig packages from the store that fetch.sh
# fills, and runs Zig under a network denial: unshare --net cannot work in the container (Docker's seccomp profile and
# the host's apparmor_restrict_unprivileged_userns refuse it), and the gate container has no network at all
# (--network none), so it declares that with BOTSTER_ZIG_NETWORK_DENIED=1.
extra_env=(BOTSTER_ZIG_PACKAGES=/zig/packages BOTSTER_ZIG_NETWORK_DENIED=1)
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
  # Every step runs, whatever the one before returned (an already-ended watcher, a container already gone).
  set +e
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

# Priority jobs (CI) get more CPU weight when the host is busy. Memory: rustc and the linker need about 2 GB a CPU, plus the
# gate container's tmpfs /tmp (tmp_gb).
tmp_gb=4
shares=$(( ${TESTQ_PRIORITY:-0} ? 2048 : 1024 ))
limits=(--cpus "$cpus" --cpu-shares "$shares" --memory "$(( cpus * 2 + 4 + tmp_gb ))g")

# --init: tini is PID 1 and reaps orphans, as launchd does on the Mac. Without it, a test's reparented child stays a zombie
# and the leftover-process check fails.
container() {
  docker run --rm --init --label "testq.job=$name" "${limits[@]}" --user "$(id -u):$(id -g)" -e HOME=/tmp "$@" &
  wait $!
}

echo "gate job: $project $(git -C "$dir" rev-parse HEAD) on $(hostname), $cpus CPUs, image $image, target $target_volume"

mkdir -p "$artifacts" "$stamps"
find "$artifacts" -mindepth 1 -maxdepth 1 -type d -mtime +7 -exec rm -rf {} +
# The image lock covers this project's image build.
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
exec {image_lock}>&-

# Disk. The host is shared, and a target volume holds about 10 GB, so target volumes are bounded:
#   - at most $max_targets per project; the least recently used go first, the base branch's (main, v1) last;
#   - below $floor_gb GB free, least recently used Botster target volumes of either project go until the floor is met;
#   - below $fail_gb GB free after that, the gate fails at once instead of filling the disk mid-run.
# A job holds a shared "in use" lock on its target volume (botster-locks/use-<volume>.lock) from before it releases the
# volumes lock until it exits, through every gap between its containers; eviction needs that lock exclusively. A volume
# whose stamp is newer than $fresh_min minutes is not evicted either. One Botster-wide lock covers the stamps, the
# pruning, the in-use locks and the volume initialization of both projects.
max_targets=8 floor_gb=60 fail_gb=40 fresh_min=120
botster_projects=$(dirname "$projdir")
locks_dir=$botster_projects/botster-locks
# Free space in bytes (df -BG rounds up), and the comparison with a threshold in GiB.
free_bytes() { df -B1 --output=avail "$projdir" | tail -n 1 | tr -dc '0-9'; }
below() { (( $(free_bytes) < $1 * 1024 * 1024 * 1024 )); }
# Removes the volume of a stamp file, unless a job holds it (its in-use lock) or a container mounts it.
evict() {
  local volume use
  volume=$(basename -- "$1")
  exec {use}>"$locks_dir/use-$volume.lock"
  if ! flock -n -x "$use"; then exec {use}>&-; return 1; fi
  if ! docker volume rm "$volume" >/dev/null 2>&1; then exec {use}>&-; return 1; fi
  rm -f -- "$1" "$locks_dir/use-$volume.lock"
  exec {use}>&-
  echo "job.sh: removed the target volume $volume ($2)"
}
# Stamp files that may be evicted, least recently used first, the base branch's volumes last.
evictable() {
  find "$@" -maxdepth 0 -type f -mmin +"$fresh_min" -printf '%T@ %p\n' 2>/dev/null | sort -n | cut -d' ' -f2- \
    | awk '/-target-(main|v1)-[0-9a-f]+$/ { last = last $0 "\n"; next } { print } END { printf "%s", last }'
}
mkdir -p "$locks_dir"
exec {volumes_lock}>"$locks_dir/volumes.lock"
flock "$volumes_lock"
touch "$stamps/$target_volume"
# This job's in-use lock, held until it exits (the shell keeps the descriptor; containers and cleanup run inside it).
exec {in_use}>"$locks_dir/use-$target_volume.lock"
flock -s "$in_use"
while IFS= read -r stamp; do
  evict "$stamp" "no gate used it for 7 days" || true
done < <(find "$stamps" -mindepth 1 -maxdepth 1 -type f -name "$project-target-*" -mtime +7)
count=$(find "$stamps" -mindepth 1 -maxdepth 1 -type f -name "$project-target-*" | wc -l)
while IFS= read -r stamp; do
  (( count > max_targets )) || break
  evict "$stamp" "more than $max_targets target volumes of $project" && count=$(( count - 1 ))
done < <(evictable "$stamps"/"$project"-target-*)
if below "$floor_gb"; then
  while IFS= read -r stamp; do
    below "$floor_gb" || break
    evict "$stamp" "the host disk is below $floor_gb GB free" || true
  done < <(evictable "$botster_projects"/botster-*/targets/botster-*-target-*)
fi
free=$(( $(free_bytes) / 1024 / 1024 / 1024 ))
if below "$fail_gb"; then
  echo "job.sh: the host disk has $free GiB free, below $fail_gb GiB, after trimming Botster target volumes; the gate does not start. Free space on the host (other projects' volumes, docker build cache)." >&2
  exit 75
fi
for volume in "$cargo_volume" "$target_volume" "$npm_volume" "${extra_volumes[@]/#/$project-}"; do
  volume=${volume%%:*}
  if ! docker volume inspect "$volume" >/dev/null 2>&1; then
    docker volume create "$volume" >/dev/null
    docker run --rm --label "testq.job=$name" -v "$volume:/v" "$image" chown "$(id -u):$(id -g)" /v &
    wait $!
  fi
done
exec {volumes_lock}>&-
echo "gate job: $free GiB free on the host"

mounts=(-v "$dir:/work" -v "$target_volume:/work/target" -v "$cargo_volume:/cargo" -v "$npm_volume:/npm")
for volume in "${extra_volumes[@]}"; do mounts+=(-v "$project-${volume%%:*}:${volume#*:}"); done
network=()
gate_env=(-e TZ=UTC)
[ -n "$base" ] && gate_env+=(-e "BOTSTER_CI_BASE_REF=$base")
for variable in "${extra_env[@]}"; do gate_env+=(-e "$variable"); done

if (( private_git_deps )); then
  # Fetch every dependency with the host's GitHub CLI token, in a container that runs no repo code but cargo's resolver.
  # The token is a 0600 file mounted read-only and read by a git credential helper for github.com only: never in a process
  # argument, a container's environment, the image, or a volume. The gate container then runs offline.
  secrets=$(mktemp -d)
  chmod 700 "$secrets"
  (umask 077; printf '%s' "$(gh auth token)" > "$secrets/github-token")
  container --name "$name-fetch" "${gate_env[@]}" "${mounts[@]}" -v "$secrets:/run/testq-secrets:ro" -w /work \
    -e GIT_TERMINAL_PROMPT=0 -e GIT_CONFIG_COUNT=1 \
    -e GIT_CONFIG_KEY_0=credential.https://github.com.helper \
    -e 'GIT_CONFIG_VALUE_0=!f() { cat >/dev/null; [ "$1" = get ] || exit 0; echo username=x-access-token; printf "password=%s\n" "$(cat /run/testq-secrets/github-token)"; }; f' \
    "$image" bash ci/remote/fetch.sh
  remove_child "${TMPDIR:-/tmp}" "$secrets"
  secrets=
  # Public sources that are built or unpacked from third-party code (ci/remote/fetch-public.sh: submodules, Zig packages)
  # come in a second container with network and without the token.
  if [ -f "$dir/ci/remote/fetch-public.sh" ]; then
    container --name "$name-fetch-public" "${gate_env[@]}" "${mounts[@]}" -w /work "$image" bash ci/remote/fetch-public.sh
  fi
  network=(--network none -e CARGO_NET_OFFLINE=true)
fi

status=0
# /tmp is a tmpfs: the kernel throttles every writer of a container whose dirty page cache is at its limit, so the tests'
# temp roots (git repositories, small files) stalled for seconds while rustc or another test wrote gigabytes in the same
# container (measured: git init/add/commit 6 ms quiet, 566 ms median and 1.8 s max under such a writer, 13 ms on tmpfs).
# cargo-mutants keeps its large build copies on the target volume (the image's cargo-mutants shim).
container --name "$name" "${network[@]}" "${gate_env[@]}" "${mounts[@]}" --tmpfs "/tmp:rw,exec,mode=1777,size=${tmp_gb}g" \
  -w /work "$image" "$@" || status=$?

# The mutation reports: botster-contracts writes them in the tree, botster-core in the target volume, which only a
# container sees.
if (( status )); then
  remove_child "$artifacts" "$artifacts/$run"
  mkdir -p "$artifacts/$run"
  container --name "$name-artifacts" "${mounts[@]}" -v "$artifacts/$run:/out" "$image" \
    sh -c 'for p in mutants.out mutants-fakes target/mutants.out target/mutants-stage2/mutants.out; do
             [ -e "/work/$p" ] && cp -a "/work/$p" "/out/$(echo "$p" | tr / -)"; done; true' || true
  if [ -n "$(ls -A "$artifacts/$run")" ]; then
    echo "Failure artifacts: $artifacts/$run (kept 7 days)"
  else
    rmdir "$artifacts/$run"
  fi
fi
exit "$status"
