//! A snapshot of a data directory, for `host_crash` (steward ruling R-55 (1) conditions 3 and 4, contracts `main`
//! `70dff23`): the harness takes one before it drops a handle's Core and one after it puts back the stale control socket,
//! and the two must be equal. So a step that Core's drop gains beyond the removal of its socket file fails the control
//! loudly, instead of silently turning the model into a graceful drop (LC-12).

use std::collections::BTreeMap;
use std::io;
use std::os::unix::fs::{FileTypeExt as _, PermissionsExt as _};
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};

/// Core's lock file, directly in the data directory (core-sys `storage`). The snapshot never opens it, and records it by its
/// mode and length only. The harness takes the first snapshot while Core holds the lock, and with a process-owned (`fcntl`)
/// lock, which the LC-2 fix gives Core, a close of any descriptor of the file in this process releases the lock. Core never
/// writes the file, so its bytes have nothing to compare.
const LOCK: &str = "lock";

/// One node of a data directory, as the snapshot compares it: its kind, its permission bits, and a regular file's bytes (the
/// lock file's length instead). Inode numbers and times are left out: a restored socket is a new node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Node {
    Dir {
        mode: u32,
    },
    File {
        mode: u32,
        bytes: Vec<u8>,
    },
    /// The top-level lock file ([`LOCK`]), never opened.
    Lock {
        mode: u32,
        len: u64,
    },
    Socket {
        mode: u32,
    },
    Link {
        target: PathBuf,
    },
    Other {
        mode: u32,
    },
}

/// The directory itself (at the empty path) and every node under it, by its path relative to it. Links are not followed.
pub(super) type Snapshot = BTreeMap<PathBuf, Node>;

/// The snapshot of `dir`.
///
/// # Errors
/// A node could not be read.
pub(super) fn snapshot(dir: &Path) -> io::Result<Snapshot> {
    let mut nodes = Snapshot::new();
    // The directory's own kind and mode count too: a drop may change only the socket (R-55 (1) condition 3).
    nodes.insert(PathBuf::new(), node(dir, true)?);
    walk(dir, Path::new(""), &mut nodes)?;
    Ok(nodes)
}

fn walk(root: &Path, relative: &Path, nodes: &mut Snapshot) -> io::Result<()> {
    for entry in std::fs::read_dir(root.join(relative))? {
        let name = entry?.file_name();
        let opened = !(relative.as_os_str().is_empty() && name == LOCK);
        let path = relative.join(name);
        let node = node(&root.join(&path), opened)?;
        if matches!(node, Node::Dir { .. }) {
            walk(root, &path, nodes)?;
        }
        nodes.insert(path, node);
    }
    Ok(())
}

/// The node at `path`; a link is not followed. A regular file that may not be `opened` is a [`Node::Lock`].
fn node(path: &Path, opened: bool) -> io::Result<Node> {
    let meta = std::fs::symlink_metadata(path)?;
    let mode = meta.permissions().mode() & 0o7777;
    let kind = meta.file_type();
    Ok(if kind.is_dir() {
        Node::Dir { mode }
    } else if kind.is_file() && !opened {
        Node::Lock {
            mode,
            len: meta.len(),
        }
    } else if kind.is_file() {
        Node::File {
            mode,
            bytes: std::fs::read(path)?,
        }
    } else if kind.is_socket() {
        Node::Socket { mode }
    } else if kind.is_symlink() {
        Node::Link {
            target: std::fs::read_link(path)?,
        }
    } else {
        Node::Other { mode }
    })
}

/// The socket node that a drop removed: `before` and `after` differ only in that one node, a socket directly in the
/// directory, which `before` has and `after` lacks. Its relative path and mode.
///
/// # Errors
/// Any other difference, or none.
pub(super) fn lost_socket(before: &Snapshot, after: &Snapshot) -> Result<(PathBuf, u32), String> {
    let mut lost = before.iter().filter(|(path, _)| !after.contains_key(*path));
    let gained: Vec<&PathBuf> = after.keys().filter(|p| !before.contains_key(*p)).collect();
    let changed: Vec<&PathBuf> = after
        .iter()
        .filter(|(path, node)| before.get(*path).is_some_and(|b| b != *node))
        .map(|(path, _)| path)
        .collect();
    let socket = match (lost.next(), lost.next()) {
        (Some((path, Node::Socket { mode })), None) if path.parent() == Some(Path::new("")) => {
            Some((path.clone(), *mode))
        }
        _ => None,
    };
    match socket {
        Some(found) if gained.is_empty() && changed.is_empty() => Ok(found),
        _ => Err(format!(
            "the drop changed more than the control socket: lost {:?}, gained {gained:?}, changed {changed:?}",
            before
                .keys()
                .filter(|p| !after.contains_key(*p))
                .collect::<Vec<_>>()
        )),
    }
}

/// Puts back a stale socket node at `path` with `mode`, as a dead host leaves it: bound, then closed, so no process
/// listens on it (R-55 (1) condition 4).
///
/// # Errors
/// The node could not be made or its mode set.
pub(super) fn restore_socket(path: &Path, mode: u32) -> io::Result<()> {
    drop(UnixListener::bind(path)?);
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
}

#[cfg(test)]
mod slow_tests {
    use super::*;
    use std::os::unix::net::UnixStream;

    fn dir() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir(tmp.path().join("rows")).unwrap();
        std::fs::write(tmp.path().join("rows/r"), b"row").unwrap();
        std::fs::write(tmp.path().join("lock"), b"").unwrap();
        restore_socket(&tmp.path().join("c"), 0o600).unwrap();
        tmp
    }

    /// R-55 (1) conditions 3 and 4: a removed control socket is the only difference, and the restored node is a socket with
    /// the same mode and no listener, so the snapshots are equal again.
    #[test]
    fn a_lost_control_socket_is_put_back_as_a_stale_socket() {
        let tmp = dir();
        let before = snapshot(tmp.path()).unwrap();
        assert_eq!(before[Path::new("c")], Node::Socket { mode: 0o600 });
        assert_eq!(
            before[Path::new("rows/r")],
            Node::File {
                mode: before_mode(&tmp, "rows/r"),
                bytes: b"row".to_vec()
            }
        );
        std::fs::remove_file(tmp.path().join("c")).unwrap();
        let after = snapshot(tmp.path()).unwrap();
        let (path, mode) = lost_socket(&before, &after).unwrap();
        assert_eq!((path.as_path(), mode), (Path::new("c"), 0o600));
        restore_socket(&tmp.path().join(&path), mode).unwrap();
        assert_eq!(snapshot(tmp.path()).unwrap(), before);
        let refused = UnixStream::connect(tmp.path().join("c")).unwrap_err();
        assert_eq!(
            refused.kind(),
            io::ErrorKind::ConnectionRefused,
            "no listener"
        );
    }

    fn before_mode(tmp: &tempfile::TempDir, path: &str) -> u32 {
        std::fs::metadata(tmp.path().join(path))
            .unwrap()
            .permissions()
            .mode()
            & 0o7777
    }

    /// Any other difference fails: a changed, a gained or a second lost node, a lost node that is not a socket, a lost
    /// socket below the top level, and no difference.
    #[test]
    fn every_other_difference_fails() {
        let tmp = dir();
        let before = snapshot(tmp.path()).unwrap();
        let fails = |after: &Snapshot| lost_socket(&before, after).unwrap_err();
        assert!(fails(&before).contains("lost []"));
        let without = |path: &str| {
            let mut after = before.clone();
            after.remove(Path::new(path));
            after
        };
        let mut changed = without("c");
        changed.insert(
            "rows/r".into(),
            Node::File {
                mode: 0o600,
                bytes: b"other".to_vec(),
            },
        );
        assert!(fails(&changed).contains("changed [\"rows/r\"]"));
        let mut gained = without("c");
        gained.insert("new".into(), Node::Other { mode: 0 });
        assert!(fails(&gained).contains("gained [\"new\"]"));
        let mut two = without("c");
        two.remove(Path::new("lock"));
        fails(&two);
        fails(&without("lock"));
        let mut deep = before.clone();
        deep.insert("rows/s".into(), Node::Socket { mode: 0o600 });
        let mut deep_after = deep.clone();
        deep_after.remove(Path::new("rows/s"));
        assert!(
            lost_socket(&deep, &deep_after).is_err(),
            "not at the top level"
        );
    }

    /// R-55 (1) condition 3: the directory's own mode is part of the snapshot, so a drop that changes it fails, even when
    /// it also loses the socket.
    #[test]
    fn a_change_of_the_directorys_mode_fails() {
        let tmp = dir();
        let before = snapshot(tmp.path()).unwrap();
        assert_eq!(
            before[Path::new("")],
            Node::Dir {
                mode: before_mode(&tmp, "")
            }
        );
        std::fs::remove_file(tmp.path().join("c")).unwrap();
        let mode = before_mode(&tmp, "") ^ 0o070;
        std::fs::set_permissions(tmp.path(), std::fs::Permissions::from_mode(mode)).unwrap();
        let after = snapshot(tmp.path()).unwrap();
        assert_eq!(after[Path::new("")], Node::Dir { mode });
        assert!(lost_socket(&before, &after)
            .unwrap_err()
            .contains("changed [\"\"]"));
    }

    /// The variable that makes [`lock_probe`] probe the lock of the file that it names.
    const LOCK_PROBE: &str = "BOTSTER_TESTKIT_LOCK_PROBE";

    /// The lock probe, run as a child of [`the_snapshot_never_opens_the_lock_file`] (a process cannot see its own `fcntl`
    /// locks): it tries a process-owned lock of the file that `LOCK_PROBE` names and exits 0 when it gets it, 3 when another
    /// process holds one. Without the variable it does nothing.
    #[test]
    fn lock_probe() {
        let Some(path) = std::env::var_os(LOCK_PROBE) else {
            return;
        };
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .unwrap();
        let code = match rustix::fs::fcntl_lock(
            &file,
            rustix::fs::FlockOperation::NonBlockingLockExclusive,
        ) {
            Ok(()) => 0,
            Err(rustix::io::Errno::AGAIN | rustix::io::Errno::ACCESS) => 3,
            Err(error) => panic!("the probe's lock: {error}"),
        };
        std::process::exit(code);
    }

    /// The snapshot never opens the top-level lock file: a process-owned lock of this process on it is still held after the
    /// snapshot, as a child's probe sees, and the lock is recorded by mode and length. Every other file is read: one at
    /// the top level, and one named `lock` below it.
    #[test]
    fn the_snapshot_never_opens_the_lock_file() {
        let tmp = dir();
        std::fs::write(tmp.path().join("other"), b"x").unwrap();
        std::fs::write(tmp.path().join("rows/lock"), b"y").unwrap();
        let path = tmp.path().join("lock");
        let held = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        rustix::fs::fcntl_lock(&held, rustix::fs::FlockOperation::NonBlockingLockExclusive)
            .unwrap();
        let probe = || {
            let mut command = std::process::Command::new(std::env::current_exe().unwrap());
            command
                .args([
                    "--exact",
                    "real::snapshot::slow_tests::lock_probe",
                    "--nocapture",
                ])
                .env(LOCK_PROBE, &path);
            botster_test_process::OwnedChild::spawn(&mut command)
                .unwrap()
                .status()
                .code()
        };
        assert_eq!(probe(), Some(3), "held before the snapshot");
        let nodes = snapshot(tmp.path()).unwrap();
        assert_eq!(probe(), Some(3), "still held after the snapshot");
        assert_eq!(
            nodes[Path::new("lock")],
            Node::Lock {
                mode: before_mode(&tmp, "lock"),
                len: 0
            }
        );
        assert!(matches!(&nodes[Path::new("other")], Node::File { bytes, .. } if bytes == b"x"));
        assert!(
            matches!(&nodes[Path::new("rows/lock")], Node::File { bytes, .. } if bytes == b"y")
        );
        drop(held);
        assert_eq!(probe(), Some(0), "free after this process closes it");
    }

    /// A link is not followed, and another kind of node keeps its mode.
    #[test]
    fn a_link_is_read_as_its_target() {
        let tmp = dir();
        std::os::unix::fs::symlink("rows", tmp.path().join("l")).unwrap();
        let nodes = snapshot(tmp.path()).unwrap();
        assert_eq!(
            nodes[Path::new("l")],
            Node::Link {
                target: "rows".into()
            }
        );
        assert!(matches!(nodes[Path::new("rows")], Node::Dir { .. }));
    }
}
