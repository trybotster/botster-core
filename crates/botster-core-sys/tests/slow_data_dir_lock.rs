//! The data-directory lock (Core LC-2: the lock is released when the handle is dropped) against the children that other
//! threads of the same process start. Slow tier: every child is an `OwnedChild` in its own process group, ended on every
//! exit path.
//!
//! A fork copies every descriptor of the process into the child, until the child's exec closes the `O_CLOEXEC` ones. A
//! `flock` belongs to the open file description, so such a copy kept the lock of a dropped handle until the child's exec:
//! the must-pass `id_2_instance_survives_adoption` failed 3 of 30 runs with `DataDirInUse` when 13 real ids ran in parallel.
//! The `fcntl` record lock belongs to the process, and a child never holds it.
//!
//! "Another process" is this test binary run again for the one test `slow_lock_probe_entry`, which tries the same lock as
//! `LockFile` and prints whether it was held (the child-entry form of `botster-worker`'s `slow_driver_observer`).
#![cfg(feature = "slow")]

use botster_core_edges::edges::Storage;
use botster_core_sys::lock::{LockError, LockFile};
use botster_core_sys::storage::{DataDir, OpenError};
use botster_test_process::{run_to_completion, Deadline, OwnedChild};
use rustix::fs::FlockOperation;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::time::Duration;

/// The bound of each wait for a child.
const LIMIT: Duration = Duration::from_secs(10);

/// The variable that names the lock file for `slow_lock_probe_entry`.
const PROBE: &str = "BOTSTER_LOCK_PROBE";

/// The child entry of `held_for_another_process`: without `BOTSTER_LOCK_PROBE` it does nothing. With it, it opens the named
/// file and tries the lock of `LockFile` (`fcntl`, `F_SETLK`, `F_WRLCK`, the whole file), and prints the answer.
#[test]
fn slow_lock_probe_entry() {
    let Some(path) = std::env::var_os(PROBE) else {
        return;
    };
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .expect("the probe opens the lock file");
    let answer = match rustix::fs::fcntl_lock(&file, FlockOperation::NonBlockingLockExclusive) {
        Ok(()) => "free",
        Err(_) => "held",
    };
    println!("lock-probe: {answer}");
}

/// Whether another process finds the lock of `path` held now (`slow_lock_probe_entry`).
fn held_for_another_process(path: &Path) -> bool {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", "slow_lock_probe_entry", "--nocapture"])
        .env(PROBE, path);
    let output = run_to_completion(&mut command, Deadline::after(LIMIT)).unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "the probe failed: {output:?}");
    let answers: Vec<&str> = stdout
        .lines()
        .filter_map(|line| line.strip_prefix("lock-probe: "))
        .collect();
    match answers[..] {
        ["held"] => true,
        ["free"] => false,
        _ => panic!("the probe gave no single answer: {stdout}"),
    }
}

/// Threads that start children (`true`) one after another, as other ids of a test process and Core's own spawns do, until
/// the value is dropped or each thread started `MAX_SPAWNS`. Each fork copies the descriptors of this process into the
/// child until its exec. The drop stops and joins the threads, and a thread's failure fails the test there.
struct Spawners {
    stop: Arc<AtomicBool>,
    spawned: Arc<AtomicU64>,
    threads: Vec<std::thread::JoinHandle<()>>,
}

/// The children that one spawner thread starts at most: the bound of its work.
const MAX_SPAWNS: u64 = 20_000;

impl Spawners {
    /// Starts `count` threads, and the receiver of one event from each thread after its first child ended.
    fn start(count: usize) -> (Spawners, Receiver<()>) {
        let stop = Arc::new(AtomicBool::new(false));
        let spawned = Arc::new(AtomicU64::new(0));
        let (started, first) = mpsc::channel();
        let threads = (0..count)
            .map(|_| {
                let (stop, spawned, started) =
                    (Arc::clone(&stop), Arc::clone(&spawned), started.clone());
                std::thread::spawn(move || {
                    for n in 0..MAX_SPAWNS {
                        if stop.load(Ordering::Relaxed) {
                            break;
                        }
                        let mut child = OwnedChild::spawn_group(&mut Command::new("true")).unwrap();
                        assert!(child.status_by(Deadline::after(LIMIT)).success());
                        spawned.fetch_add(1, Ordering::Relaxed);
                        if n == 0 {
                            // The receiver is gone only when the test failed already.
                            let _ = started.send(());
                        }
                    }
                })
            })
            .collect();
        (
            Spawners {
                stop,
                spawned,
                threads,
            },
            first,
        )
    }

    fn spawned(&self) -> u64 {
        self.spawned.load(Ordering::Relaxed)
    }
}

impl Drop for Spawners {
    /// Stops the threads and joins every one of them before it reports a failure, so no thread is left detached with a
    /// child.
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let failed = self
            .threads
            .drain(..)
            .map(std::thread::JoinHandle::join)
            .filter(Result::is_err)
            .count();
        if failed > 0 && !std::thread::panicking() {
            panic!("{failed} spawner threads failed");
        }
    }
}

/// id_2: while other threads start children, a dropped lock is free at once, so the one reopen right after the drop takes
/// it, in each of `CYCLES` cycles. With `flock`, a child between its fork and its exec held the dropped lock, and the first
/// refused reopen fails the test (no retry). The cycles start after a spawner's first child ended, and some child must
/// start during them, or they prove nothing. With v1's `flock`, 95 of 3671 cycles (Mac) and 51 of 7945 cycles (Linux) were
/// refused (the first form of this test, which retried). If the cycles are independent, an estimate that this assumes,
/// 2000 cycles miss a reverted fix with a probability of about 2.5e-6 on Linux and less on the Mac.
#[test]
fn a_dropped_lock_is_free_at_once_while_other_threads_start_children() {
    const CYCLES: u64 = 2000;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("lock");
    let (spawners, first) = Spawners::start(4);
    // timer: deadline — bounds the wait for the first child of a spawner thread
    if let Err(error) = first.recv_timeout(LIMIT) {
        drop(spawners);
        panic!("no spawner thread ended a first child within {LIMIT:?}: {error}");
    }
    let before = spawners.spawned();
    let mut held = LockFile::try_exclusive(&path).unwrap();
    for cycle in 1..=CYCLES {
        drop(held);
        held = match LockFile::try_exclusive(&path) {
            Ok(lock) => lock,
            Err(error) => {
                let spawned = spawners.spawned() - before;
                drop(spawners);
                panic!(
                    "cycle {cycle} of {CYCLES}: the reopen right after the drop failed ({error}) while {spawned} \
                     children started"
                );
            }
        };
    }
    let during = spawners.spawned() - before;
    drop(spawners);
    assert!(
        during > 0,
        "no child started during the {CYCLES} cycles, so they prove nothing"
    );
    assert!(held_for_another_process(&path));
    drop(held);
    assert!(!held_for_another_process(&path));
}

/// Core owns the only open of `<data dir>/lock` in the process: a POSIX record lock ends when the process closes ANY
/// descriptor of the file. So after the open and after each operation of the data directory and its storage, another process
/// still finds the lock held; after the drop it is free.
#[test]
fn core_keeps_its_lock_through_every_operation_of_the_data_directory() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("d");
    let lock = path.join("lock");
    let mut data = DataDir::open(&path).unwrap();
    assert!(held_for_another_process(&lock));
    let storage = data.storage();
    storage.write_row("session/a", b"1").unwrap();
    assert_eq!(
        storage.read_row("session/a").unwrap().as_deref(),
        Some(&b"1"[..])
    );
    assert!(storage
        .list_rows()
        .unwrap()
        .contains(&"session/a".to_string()));
    storage.scan().unwrap();
    storage.delete_row("session/a").unwrap();
    assert!(held_for_another_process(&lock));
    assert!(matches!(DataDir::open(&path), Err(OpenError::InUse)));
    assert!(held_for_another_process(&lock));
    let (held, mut storage, _) = data.into_storage();
    storage.write_row("session/b", b"2").unwrap();
    drop(storage);
    assert!(held_for_another_process(&lock));
    drop(held);
    assert!(!held_for_another_process(&lock));
}

/// A walk of the data directory that reads each node's metadata, and opens no file, keeps the lock: a snapshot of the data
/// directory must stat `lock`, never open it (lead, 2026-10-11, for R-55's data-directory snapshot).
#[test]
fn a_stat_only_walk_of_the_data_directory_keeps_the_lock() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("d");
    let data = DataDir::open(&path).unwrap();
    let mut seen = Vec::new();
    for entry in std::fs::read_dir(&path).unwrap() {
        let entry = entry.unwrap();
        std::fs::symlink_metadata(entry.path()).unwrap();
        seen.push(entry.file_name());
    }
    assert!(seen.iter().any(|name| name == "lock"), "{seen:?}");
    assert!(held_for_another_process(&path.join("lock")));
    drop(data);
}

/// A second open in this process is refused while the first holds the lock, also through a hard link to the lock file, and
/// the refusals close nothing of the holder's: another process still finds the lock held.
#[test]
fn a_second_open_in_this_process_is_refused_also_through_a_hard_link() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("lock");
    let link = dir.path().join("link");
    let first = LockFile::try_exclusive(&path).unwrap();
    std::fs::hard_link(&path, &link).unwrap();
    assert!(matches!(
        LockFile::try_exclusive(&path),
        Err(LockError::Held)
    ));
    assert!(matches!(
        LockFile::try_exclusive(&link),
        Err(LockError::Held)
    ));
    assert!(held_for_another_process(&path));
    drop(first);
    assert!(LockFile::try_exclusive(&link).is_ok());
}
