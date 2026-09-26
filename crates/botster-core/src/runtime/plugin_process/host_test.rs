//! Start rollback: a start that fails before the exit watch owns the child
//! kills the whole group, descendants included, and reaps the leader.
//!
//! Requires `script/prebuild-worker` for the scripted test worker.

use std::ffi::{CString, OsString};
use std::fs::{File, OpenOptions};
use std::io::Read;
use std::os::fd::AsRawFd;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use serde_json::json;

use super::{FaultPoint, PluginProcess, StartFault, START_FAULT};
use crate::runtime::plugin_process::{
    LoadFrame, PluginProcessConfig, PluginProcessError, PluginProcessRlimits,
};

/// Bound for the descendant's death; expiry fails the test.
const EVENT_DEADLINE: Duration = Duration::from_secs(30);

fn worker() -> PathBuf {
    botster_core_test_support::real_worker::WorkerBinary::plugin_test_worker_from_env()
        .unwrap_or_else(|failure| panic!("{failure}"))
        .path
}

fn mkfifo(path: &Path) {
    let path = CString::new(path.as_os_str().as_bytes()).expect("fifo path");
    // SAFETY: mkfifo with a valid NUL-terminated path.
    assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0, "mkfifo");
}

/// A scratch directory with the three FIFOs of the descendant protocol.
struct Fifos {
    dir: PathBuf,
    descendant: PathBuf,
    alive: PathBuf,
    report: PathBuf,
}

impl Fifos {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "botster-plugin-rollback-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        let fifos = Self {
            descendant: dir.join("descendant"),
            alive: dir.join("alive"),
            report: dir.join("report"),
            dir,
        };
        mkfifo(&fifos.descendant);
        mkfifo(&fifos.alive);
        mkfifo(&fifos.report);
        fifos
    }
}

impl Drop for Fifos {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn config(fifos: &Fifos) -> PluginProcessConfig {
    let env = |key: &str, path: &Path| (OsString::from(key), path.as_os_str().to_owned());
    PluginProcessConfig {
        worker_path: worker(),
        cwd: std::env::temp_dir(),
        env: vec![
            env("PLUGIN_TEST_DESCENDANT_FIFO", &fifos.descendant),
            env("PLUGIN_TEST_ALIVE_FIFO", &fifos.alive),
            env("PLUGIN_TEST_REPORT_FIFO", &fifos.report),
        ],
        rlimits: PluginProcessRlimits::default(),
        sandbox: opaque(json!({})),
        memory_cap_bytes: None,
        max_frame_bytes: 1024 * 1024,
        startup_deadline: Duration::from_secs(30),
        shutdown_deadline: Duration::from_secs(30),
        stderr_tail_bytes: 4096,
    }
}

/// Build an opaque payload newtype through its transparent serde form.
fn opaque<T: serde::de::DeserializeOwned>(value: serde_json::Value) -> T {
    serde_json::from_value(value).expect("opaque payload")
}

fn load() -> LoadFrame {
    LoadFrame {
        sources: opaque(json!({})),
        config: opaque(json!({ "mode": "report" })),
    }
}

/// Fail the start at `point` once the child has started its descendant, and
/// return the leader's pid.
fn start_failing_at(point: FaultPoint, fifos: &Fifos) -> u32 {
    let leader: Arc<Mutex<Option<u32>>> = Arc::default();
    let hook_leader = leader.clone();
    let report = fifos.report.clone();
    START_FAULT.with(|slot| {
        *slot.borrow_mut() = Some(StartFault {
            point,
            before: Box::new(move |pid| {
                // The child reports after its descendant runs; this read is
                // the event that orders the fault after the descendant.
                let mut reported = String::new();
                File::open(&report)
                    .and_then(|mut report| report.read_to_string(&mut reported))
                    .expect("read the descendant report");
                assert!(!reported.trim().is_empty(), "the descendant started");
                *hook_leader.lock().expect("leader") = Some(pid);
            }),
        });
    });

    let result = PluginProcess::spawn(&config(fifos), &load());
    assert!(
        matches!(&result, Err(PluginProcessError::Launch(error)) if error.to_string().contains("injected")),
        "the injected fault must fail the start"
    );
    let leader = leader.lock().expect("leader").take();
    leader.expect("the fault hook ran")
}

fn assert_rolled_back(point: FaultPoint, name: &str) {
    let fifos = Fifos::new(name);
    // Holding the input FIFO open keeps the descendant alive until killed.
    let _keep_descendant_alive = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&fifos.descendant)
        .expect("hold the descendant FIFO");
    // Open the alive FIFO's read end first, without blocking, so the child's
    // write open succeeds; then read it blocking until EOF.
    let alive = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(&fifos.alive)
        .expect("open the alive FIFO");

    let leader = start_failing_at(point, &fifos);

    // SAFETY: signal 0 only checks for existence.
    let leader_gone = unsafe { libc::kill(leader as libc::pid_t, 0) } != 0;
    assert!(leader_gone, "the rollback reaped the leader");

    // SAFETY: fcntl get/set on a descriptor this test owns.
    unsafe {
        let flags = libc::fcntl(alive.as_raw_fd(), libc::F_GETFL);
        libc::fcntl(alive.as_raw_fd(), libc::F_SETFL, flags & !libc::O_NONBLOCK);
    }
    let (died, descendant_died) = mpsc::channel();
    std::thread::spawn(move || {
        let mut alive = alive;
        let mut sink = Vec::new();
        let _ = died.send(alive.read_to_end(&mut sink).is_ok());
    });
    // timer: deadline — EOF on the alive FIFO means the descendant died; expiry means only the leader was killed
    let died = descendant_died
        .recv_timeout(EVENT_DEADLINE)
        .expect("the rollback killed the whole group");
    assert!(died);
}

#[test]
fn a_failed_exit_watch_registration_kills_the_group_and_reaps() {
    assert_rolled_back(FaultPoint::RegisterWatch, "register");
}

#[test]
fn a_failed_exit_watch_thread_start_kills_the_group_and_reaps() {
    assert_rolled_back(FaultPoint::SpawnExitWatch, "spawn");
}
