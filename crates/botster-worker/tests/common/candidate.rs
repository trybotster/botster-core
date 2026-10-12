use std::path::PathBuf;

/// `target/candidate/botster-worker`, found from this test binary (`target/<profile>/deps/<test>`), so it holds for any
/// target directory, prebuilt from this worktree.
pub fn worker_binary() -> PathBuf {
    let exe = std::env::current_exe().expect("the test binary");
    let target = exe
        .parent()
        .and_then(|deps| deps.parent())
        .and_then(|profile| profile.parent())
        .expect("target/<profile>/deps");
    // A worker prebuilt from another worktree into a shared target directory is refused (bug catalog B13).
    botster_test_process::built_in::check_candidate_here(&target.join("candidate"))
        .unwrap_or_else(|error| panic!("{error}"));
    let binary = target.join("candidate").join("botster-worker");
    assert!(
        binary.is_file(),
        "{} is missing: run `cargo xtask prebuild-worker` first",
        binary.display()
    );
    binary
}
