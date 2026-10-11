//! The stored row of a session in a data directory that no Core holds, for `reuse_pid` (steward ruling R-55 (2), contracts
//! `main` `70dff23`). The harness finds the row's file by Core's own decoder and replaces it as Core's storage does: a new
//! file of mode `0600` in the same directory, synced, renamed over the row, and the directory synced. It opens no data
//! directory, so it takes no lock and raises no host epoch (DP-8).

use botster_core_contract::prelude::SessionId;
use botster_core_host::session::Row;
use std::fs::{File, OpenOptions};
use std::io::{self, Write as _};
use std::os::unix::fs::OpenOptionsExt as _;
use std::path::{Path, PathBuf};

/// The file under `rows` (the registry of a data directory) whose bytes Core's decoder reads as the row of `session`, and
/// that row. `None` when no file holds one. Links are not followed.
///
/// # Errors
/// The registry could not be read.
pub(super) fn find(rows: &Path, session: &SessionId) -> io::Result<Option<(PathBuf, Row)>> {
    for entry in std::fs::read_dir(rows)? {
        let path = entry?.path();
        let kind = std::fs::symlink_metadata(&path)?.file_type();
        if kind.is_dir() {
            if let Some(found) = find(&path, session)? {
                return Ok(Some(found));
            }
        } else if kind.is_file() {
            if let Some(row) = Row::decode(session, &std::fs::read(&path)?) {
                return Ok(Some((path, row)));
            }
        }
    }
    Ok(None)
}

/// Replaces the file `path` with `bytes`, as Core's storage writes a row.
///
/// # Errors
/// The new file could not be written, synced or renamed.
pub(super) fn replace(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let dir = path
        .parent()
        .ok_or_else(|| io::Error::other("a row file has a directory"))?;
    let temp = dir.join(".botster-testkit-row");
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temp)?;
    let placed = file
        .write_all(bytes)
        .and_then(|()| file.sync_all())
        .and_then(|()| std::fs::rename(&temp, path));
    if let Err(error) = placed {
        let _ = std::fs::remove_file(&temp);
        return Err(error);
    }
    File::open(dir)?.sync_all()
}

#[cfg(test)]
mod tests {
    use super::*;
    use botster_core_contract::prelude::*;
    use botster_core_host::session::{unknown_request, RowWorker, ROW_VERSION};
    use std::collections::BTreeMap;
    use std::os::unix::fs::PermissionsExt as _;

    fn row(id: &str) -> Row {
        Row {
            version: ROW_VERSION,
            id: SessionId(id.into()),
            instance: InstanceId(format!("1-{id}")),
            state: SessionState::Running,
            request: unknown_request(),
            labels: BTreeMap::new(),
            token: Some("00".repeat(32)),
            worker: Some(RowWorker {
                pid: 4,
                start_time: 9,
            }),
            payload: None,
            worker_protocol: Some(1),
            worker_features: None,
        }
    }

    /// The row of a session is found by Core's decoder at any depth, past files that are not its row; a missing one is
    /// `None`. A replaced row has the new bytes, the row's mode, and no file is left beside it.
    #[test]
    fn a_row_is_found_by_its_decoder_and_replaced_in_place() {
        let tmp = tempfile::tempdir().unwrap();
        let deep = tmp.path().join("s/a/b");
        std::fs::create_dir_all(&deep).unwrap();
        std::fs::write(tmp.path().join("other"), b"not a row").unwrap();
        std::fs::write(
            tmp.path().join("s/a/x"),
            serde_json::to_vec(&row("s2")).unwrap(),
        )
        .unwrap();
        let path = deep.join("r");
        std::fs::write(&path, serde_json::to_vec(&row("s1")).unwrap()).unwrap();
        std::os::unix::fs::symlink(&path, tmp.path().join("link")).unwrap();

        let (found, stored) = find(tmp.path(), &SessionId("s1".into())).unwrap().unwrap();
        assert_eq!((found.as_path(), &stored), (path.as_path(), &row("s1")));
        assert!(find(tmp.path(), &SessionId("s9".into())).unwrap().is_none());

        let mut changed = row("s1");
        changed.worker = Some(RowWorker {
            pid: 5,
            start_time: 9,
        });
        replace(&path, &serde_json::to_vec(&changed).unwrap()).unwrap();
        let (_, stored) = find(tmp.path(), &SessionId("s1".into())).unwrap().unwrap();
        assert_eq!(stored, changed);
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        let names: Vec<_> = std::fs::read_dir(&deep)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, ["r"]);
    }

    /// A file that cannot take the new bytes leaves the row as it was and no file beside it.
    #[test]
    fn a_failed_replace_leaves_the_row() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("r");
        std::fs::write(&path, b"old").unwrap();
        std::fs::write(tmp.path().join(".botster-testkit-row"), b"busy").unwrap();
        assert!(replace(&path, b"new").is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"old");
        assert!(
            replace(Path::new("/"), b"new").is_err(),
            "a row file has a directory"
        );
    }
}
