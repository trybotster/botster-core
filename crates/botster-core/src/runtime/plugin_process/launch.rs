//! Start one plugin worker process with only the descriptors, environment,
//! and limits the Hub supplied.

use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::process::{Child, ChildStderr, Command, Stdio};

use super::protocol::{CHILD_FATAL_FD, CHILD_IPC_FD};
use super::{PluginProcessConfig, PluginProcessRlimits};

/// A started worker and the parent's ends of its channels.
pub(super) struct Launched {
    pub child: Child,
    pub ipc: UnixStream,
    pub fatal: OwnedFd,
    pub stderr: ChildStderr,
}

/// Descriptors at or above this value are free for the `pre_exec` shuffle.
const SHUFFLE_FLOOR: libc::c_int = 10;

pub(super) fn launch(config: &PluginProcessConfig) -> io::Result<Launched> {
    let (ipc, child_ipc) = UnixStream::pair()?;
    set_no_sigpipe(&ipc)?;
    let (fatal_read, fatal_write) = fatal_pipe()?;
    let limits = Rlimits::from(&config.rlimits);
    let child_ipc_fd = child_ipc.as_raw_fd();
    let fatal_write_fd = fatal_write.as_raw_fd();

    let mut command = Command::new(&config.worker_path);
    command
        .env_clear()
        .envs(config.env.iter().map(|(key, value)| (key, value)))
        .current_dir(&config.cwd)
        .process_group(0)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    // SAFETY: the closure runs in the forked child before exec and calls only
    // async-signal-safe functions (fcntl, dup2, setrlimit) on values it
    // captured by copy.
    unsafe {
        command.pre_exec(move || {
            // Move both ends above the target numbers first, so placing one
            // cannot close the other when it already sits at 3 or 4.
            let ipc = dup_above(child_ipc_fd)?;
            let fatal = dup_above(fatal_write_fd)?;
            place(ipc, CHILD_IPC_FD)?;
            place(fatal, CHILD_FATAL_FD)?;
            limits.apply()
        });
    }
    let mut child = command.spawn()?;
    // The parent keeps no copy of the child's ends.
    drop(child_ipc);
    drop(fatal_write);
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| io::Error::other("worker stderr was not piped"))?;
    Ok(Launched {
        child,
        ipc,
        fatal: fatal_read,
        stderr,
    })
}

fn dup_above(fd: libc::c_int) -> io::Result<libc::c_int> {
    // SAFETY: fcntl F_DUPFD on a descriptor that is open in this process.
    let copy = unsafe { libc::fcntl(fd, libc::F_DUPFD, SHUFFLE_FLOOR) };
    if copy < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(copy)
}

/// Put `fd` at `target` without close-on-exec. The child's hygiene step
/// closes the high intermediate descriptors.
fn place(fd: libc::c_int, target: libc::c_int) -> io::Result<()> {
    // SAFETY: dup2 between two descriptor numbers; the result is not
    // close-on-exec. fd is always above `target` here, never equal.
    if unsafe { libc::dup2(fd, target) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// A pipe whose ends are both close-on-exec in the parent; the write end is
/// non-blocking, so the child's fatal-cause write can never block.
fn fatal_pipe() -> io::Result<(OwnedFd, OwnedFd)> {
    let mut fds = [0 as libc::c_int; 2];
    // SAFETY: fds is a valid two-element output array.
    if unsafe { libc::pipe(fds.as_mut_ptr()) } < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: pipe returned two new descriptors that nothing else owns.
    let (read, write) = unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) };
    for fd in [read.as_raw_fd(), write.as_raw_fd()] {
        set_fd_flag(fd, libc::F_GETFD, libc::F_SETFD, libc::FD_CLOEXEC)?;
        set_fd_flag(fd, libc::F_GETFL, libc::F_SETFL, libc::O_NONBLOCK)?;
    }
    Ok((read, write))
}

fn set_fd_flag(
    fd: libc::c_int,
    get: libc::c_int,
    set: libc::c_int,
    flag: libc::c_int,
) -> io::Result<()> {
    // SAFETY: fcntl get/set on a descriptor that this process owns.
    let current = unsafe { libc::fcntl(fd, get) };
    if current < 0 || unsafe { libc::fcntl(fd, set, current | flag) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
fn set_no_sigpipe(stream: &UnixStream) -> io::Result<()> {
    let enabled: libc::c_int = 1;
    // SAFETY: setsockopt with a valid option pointer and length.
    let result = unsafe {
        libc::setsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_NOSIGPIPE,
            (&enabled as *const libc::c_int).cast(),
            std::mem::size_of::<libc::c_int>() as libc::socklen_t,
        )
    };
    if result < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Linux sends with MSG_NOSIGNAL instead.
#[cfg(not(any(target_os = "macos", target_os = "ios")))]
fn set_no_sigpipe(_stream: &UnixStream) -> io::Result<()> {
    Ok(())
}

/// Copyable rlimit values for the `pre_exec` closure.
#[derive(Clone, Copy)]
struct Rlimits {
    cpu_seconds: Option<u64>,
    open_files: Option<u64>,
    core_bytes: Option<u64>,
    address_space_bytes: Option<u64>,
}

impl From<&PluginProcessRlimits> for Rlimits {
    fn from(limits: &PluginProcessRlimits) -> Self {
        Self {
            cpu_seconds: limits.cpu_seconds,
            open_files: limits.open_files,
            core_bytes: limits.core_bytes,
            address_space_bytes: limits.address_space_bytes,
        }
    }
}

impl Rlimits {
    fn apply(self) -> io::Result<()> {
        set_rlimit(libc::RLIMIT_CPU, self.cpu_seconds)?;
        set_rlimit(libc::RLIMIT_NOFILE, self.open_files)?;
        set_rlimit(libc::RLIMIT_CORE, self.core_bytes)?;
        set_rlimit(libc::RLIMIT_AS, self.address_space_bytes)
    }
}

#[cfg(target_os = "linux")]
type RlimitResource = libc::__rlimit_resource_t;
#[cfg(not(target_os = "linux"))]
type RlimitResource = libc::c_int;

fn set_rlimit(resource: RlimitResource, value: Option<u64>) -> io::Result<()> {
    let Some(value) = value else {
        return Ok(());
    };
    let value = libc::rlim_t::try_from(value).unwrap_or(libc::RLIM_INFINITY);
    let limit = libc::rlimit {
        rlim_cur: value,
        rlim_max: value,
    };
    // SAFETY: setrlimit with a valid rlimit pointer.
    if unsafe { libc::setrlimit(resource, &limit) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
