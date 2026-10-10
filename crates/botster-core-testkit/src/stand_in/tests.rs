use super::*;
use crate::worker::{ImpostorField, ImpostorFrame};
use botster_core_contract::prelude::InstanceId;
use botster_core_edges::edges::ProcessIdentity;
use botster_core_link::frame::encode_frame;
use botster_core_link::proof::{token_proof, TOKEN_LEN};
use std::io::ErrorKind;
use std::net::Shutdown;
use std::os::unix::fs::MetadataExt as _;

fn sid() -> SessionId {
    SessionId("s1".into())
}

fn plan(script: Vec<ImpostorFrame>) -> ImpostorPlan {
    ImpostorPlan {
        session: sid(),
        field: ImpostorField::Token,
        script,
        token: [7; TOKEN_LEN],
        worker: ProcessIdentity {
            pid: 1,
            start_time: 1,
        },
    }
}

fn host_hello() -> Hello {
    let instance = InstanceId("i1".into());
    Hello {
        protocol: botster_worker_core::WORKER_PROTOCOL,
        proof: token_proof(&[7; TOKEN_LEN], &instance, 9),
        instance,
        host_epoch: 9,
    }
}

fn frame(kind: FrameType, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    encode_frame(kind, payload, DEFAULT_MAX_PAYLOAD, &mut out).expect("a small frame encodes");
    out
}

fn hello_frame(hello: &Hello) -> Vec<u8> {
    let mut payload = Vec::new();
    hello.encode(&mut payload).expect("a hello encodes");
    frame(FrameType::HELLO, &payload)
}

/// A worker endpoint at `<dir>/w`: a non-blocking listener, as the worker's own.
fn endpoint(dir: &Path) -> (PathBuf, UnixListener) {
    let path = dir.join("w");
    let worker = UnixListener::bind(&path).expect("the worker endpoint binds");
    worker.set_nonblocking(true).expect("non-blocking");
    (path, worker)
}

fn install(dir: &Path, role: Role) -> (PathBuf, UnixListener, StandIn) {
    let (path, worker) = endpoint(dir);
    let stand_in = StandIn::install(sid(), role, &path, &dir.join("aside")).expect("installs");
    (path, worker, stand_in)
}

/// The bytes that the host end can read now, with no wait.
fn readable(host: &mut UnixStream) -> io::Result<Vec<u8>> {
    host.set_nonblocking(true)?;
    let mut out = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        match host.read(&mut buf) {
            Ok(0) => return Ok(out),
            Ok(n) => out.extend(&buf[..n]),
            Err(e) if e.kind() == ErrorKind::WouldBlock && !out.is_empty() => return Ok(out),
            Err(e) => return Err(e),
        }
    }
}

#[test]
fn the_stand_in_takes_the_endpoint_path_and_its_drop_gives_it_back() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (path, _worker) = endpoint(dir.path());
    let aside = dir.path().join("aside");
    let real = inode(&path);
    let mut stand_in = StandIn::install(sid(), Role::Withhold, &path, &aside).expect("installs");
    assert_eq!(stand_in.session(), &sid());
    assert_eq!(inode(&aside), real, "the real endpoint waits aside");
    assert_ne!(inode(&path), real, "the stand-in has the path");
    let _host = UnixStream::connect(&path).expect("the stand-in listens at the path");
    assert!(!stand_in.step());
    assert_eq!(stand_in.links.len(), 1, "the stand-in took the connection");
    drop(stand_in);
    assert!(!aside.exists());
    assert_eq!(inode(&path), real, "the real endpoint is back");
}

fn inode(path: &Path) -> u64 {
    std::fs::symlink_metadata(path).expect("the file").ino()
}

#[test]
fn an_endpoint_that_is_not_there_gets_no_stand_in() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("w");
    let error = StandIn::install(sid(), Role::Withhold, &path, &dir.path().join("aside"))
        .map(|_| ())
        .map_err(|e| e.kind());
    assert_eq!(error, Err(ErrorKind::NotFound));
    assert!(!path.exists());
}

#[test]
fn a_bind_that_fails_puts_the_real_endpoint_back() {
    let dir = tempfile::tempdir().expect("tempdir");
    // A path longer than a socket address holds: the rename works, and the bind fails.
    let long = dir.path().join("e".repeat(120));
    std::fs::write(&long, b"real").expect("writes");
    let aside = dir.path().join("aside");
    assert!(StandIn::install(sid(), Role::Withhold, &long, &aside).is_err());
    assert_eq!(std::fs::read(&long).expect("the endpoint is back"), b"real");
    assert!(!aside.exists());
}

#[test]
fn the_stand_in_answers_the_first_hello_with_the_plan_in_one_write() {
    let dir = tempfile::tempdir().expect("tempdir");
    let plan = plan(vec![
        ImpostorFrame::Notification,
        ImpostorFrame::StateExited,
    ]);
    let hello = host_hello();
    let expected = impostor_answer(&plan, &hello);
    let (path, _worker, mut stand_in) = install(dir.path(), Role::Answer(plan));
    let mut host = UnixStream::connect(&path).expect("connects");
    assert!(!stand_in.step(), "no hello yet");
    // A frame that is not a hello is not answered; the hello after it is, once, in two parts.
    let mut sent = frame(FrameType::WORKER_MSG, b"x");
    sent.extend(hello_frame(&hello));
    let (first, second) = sent.split_at(sent.len() - 3);
    host.write_all(first).expect("writes");
    assert!(!stand_in.step(), "the hello is not whole yet");
    host.write_all(second).expect("writes");
    host.write_all(&hello_frame(&hello)).expect("writes");
    assert!(stand_in.step());
    assert_eq!(readable(&mut host).expect("reads"), expected);
    assert!(stand_in.step(), "the answer stays given");
    assert_eq!(
        readable(&mut host).map_err(|e| e.kind()),
        Err(ErrorKind::WouldBlock),
        "a second hello gets nothing"
    );
}

#[test]
fn a_withholding_stand_in_never_answers() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (path, _worker, mut stand_in) = install(dir.path(), Role::Withhold);
    let mut host = UnixStream::connect(&path).expect("connects");
    host.write_all(&hello_frame(&host_hello())).expect("writes");
    assert!(!stand_in.step());
    assert_eq!(
        readable(&mut host).map_err(|e| e.kind()),
        Err(ErrorKind::WouldBlock)
    );
}

#[test]
fn the_stand_in_closes_its_end_when_the_host_closes() {
    let dir = tempfile::tempdir().expect("tempdir");
    let plan = plan(Vec::new());
    let hello = host_hello();
    let expected = impostor_answer(&plan, &hello);
    let (path, _worker, mut stand_in) = install(dir.path(), Role::Answer(plan));
    let mut host = UnixStream::connect(&path).expect("connects");
    host.write_all(&hello_frame(&hello)).expect("writes");
    host.shutdown(Shutdown::Write).expect("shuts down");
    assert!(stand_in.step());
    assert!(stand_in.links[0].stream.is_none());
    assert_eq!(readable(&mut host).expect("reads"), expected);
    assert_eq!(
        host.read(&mut [0u8; 1]).ok(),
        Some(0),
        "the stand-in closed its end"
    );
}

#[test]
fn a_reset_by_the_host_closes_the_stand_ins_end() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (path, _worker, mut stand_in) = install(dir.path(), Role::Answer(plan(Vec::new())));
    let mut host = UnixStream::connect(&path).expect("connects");
    host.write_all(&hello_frame(&host_hello())).expect("writes");
    assert!(stand_in.step());
    // The host closes with the answer unread: the stand-in's read is a reset (Linux) or an end of file.
    drop(host);
    assert!(stand_in.step());
    assert!(stand_in.links[0].stream.is_none());
}
