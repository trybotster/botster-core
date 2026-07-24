//! Feature-gated build script for the pinned libghostty-vt native adapter path.

use std::env;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

mod build_support;

use build_support::{
    resolve_zig_command, zig_candidates, zig_global_cache_dir, zig_local_cache_dir, ZigCommand,
};

const FEATURE_ENV: &str = "CARGO_FEATURE_LIBGHOSTTY_VT";
const GHOSTTY_SUBMODULE: &str = "crates/botster-terminal-ghostty/vendor/ghostty";

fn main() {
    println!("cargo:rerun-if-env-changed={FEATURE_ENV}");

    if env::var_os(FEATURE_ENV).is_none() {
        return;
    }

    build_ghostty_vt();
}

fn build_ghostty_vt() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let ghostty_dir = manifest_dir.join("vendor/ghostty");

    require_ghostty_source(&ghostty_dir);

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR");
    let repacked_lib = Path::new(&out_dir).join("libghostty-vt.a");
    let zig_lib_path = ghostty_dir.join("zig-out/lib/libghostty-vt.a");
    let zig = resolve_zig(&ghostty_dir);
    let zig_global_cache_dir =
        zig_global_cache_dir(&out_dir, env::var("ZIG_GLOBAL_CACHE_DIR").ok());
    let zig_local_cache_dir = zig_local_cache_dir(&out_dir);

    println!("cargo:warning=building libghostty-vt with {}", zig.label);

    let status = Command::new(&zig.program)
        .args(&zig.prefix_args)
        .args([
            "build",
            "-Demit-lib-vt",
            "-Doptimize=ReleaseFast",
            "-Dsimd=false",
            "-Dcpu=baseline",
            "-Dversion-string=1.3.2-dev",
        ])
        .current_dir(&ghostty_dir)
        .env("DEVELOPER_DIR", "/Library/Developer/CommandLineTools")
        .env("ZIG_GLOBAL_CACHE_DIR", zig_global_cache_dir)
        .env("ZIG_LOCAL_CACHE_DIR", zig_local_cache_dir)
        .status()
        .unwrap_or_else(|_| panic!("failed to run Zig for the libghostty-vt feature"));

    assert!(
        status.success(),
        "botster-terminal-ghostty libghostty-vt feature requires `zig build -Demit-lib-vt -Doptimize=ReleaseFast -Dsimd=false -Dcpu=baseline -Dversion-string=1.3.2-dev` to succeed"
    );

    assert!(
        zig_lib_path.exists(),
        "botster-terminal-ghostty libghostty-vt feature expected Zig to produce zig-out/lib/libghostty-vt.a"
    );

    repack_static_library(&zig_lib_path, &repacked_lib);
    emit_link_directives(&out_dir);
    emit_rerun_directives();
}

fn require_ghostty_source(ghostty_dir: &Path) {
    if ghostty_dir.join("build.zig").exists() && ghostty_dir.join("LICENSE").exists() {
        return;
    }

    panic!(
        "botster-terminal-ghostty libghostty-vt feature requires initialized Ghostty source at {GHOSTTY_SUBMODULE}; run `git submodule update --init {GHOSTTY_SUBMODULE}`"
    );
}

fn resolve_zig(ghostty_dir: &Path) -> ZigCommand {
    let candidates = zig_candidates(
        env::var("BOTSTER_ZIG").ok(),
        env::var("ZIG").ok(),
        env::var("HOME").ok(),
        |path| path.exists(),
    );

    resolve_zig_command(&candidates, |candidate| zig_version(candidate, ghostty_dir))
        .unwrap_or_else(|message| panic!("{message}"))
}

fn zig_version(candidate: &ZigCommand, ghostty_dir: &Path) -> Result<String, String> {
    let output = Command::new(&candidate.program)
        .args(&candidate.prefix_args)
        .arg("version")
        .current_dir(ghostty_dir)
        .output()
        .map_err(|_| "not available".to_owned())?;

    if !output.status.success() {
        return Err(format!(
            "version check failed with exit code {:?}",
            output.status.code()
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn repack_static_library(zig_lib_path: &Path, repacked_lib: &Path) {
    let out_dir = repacked_lib.parent().expect("repacked lib has parent");
    let tmp_dir = out_dir.join("ghostty-repack");
    let _ = fs::remove_dir_all(&tmp_dir);
    let _ = fs::create_dir_all(&tmp_dir);
    let _ = fs::remove_file(repacked_lib);

    let zig_lib_abs = fs::canonicalize(zig_lib_path)
        .unwrap_or_else(|_| panic!("zig-out/lib/libghostty-vt.a not found"));

    if is_thin_archive(&zig_lib_abs) {
        let objects = external_archive_objects(&zig_lib_abs)
            .unwrap_or_else(|| panic!("thin libghostty-vt.a members were not found"));
        archive_objects(repacked_lib, &objects);
        let _ = fs::remove_dir_all(&tmp_dir);
        return;
    }

    if let Some(objects) = external_archive_objects(&zig_lib_abs) {
        archive_objects(repacked_lib, &objects);
        let _ = fs::remove_dir_all(&tmp_dir);
        return;
    }

    let status = Command::new("ar")
        .args(["x", &zig_lib_abs.to_string_lossy()])
        .current_dir(&tmp_dir)
        .status()
        .expect("failed to run `ar x`");
    assert!(status.success(), "ar x failed for libghostty-vt.a");

    let mut objects: Vec<_> = fs::read_dir(&tmp_dir)
        .expect("read Ghostty repack directory")
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            if path.extension().is_some_and(|ext| ext == "o") {
                #[cfg(unix)]
                let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o644));
                Some(path)
            } else {
                None
            }
        })
        .collect();
    objects.sort();

    archive_objects(repacked_lib, &objects);

    let _ = fs::remove_dir_all(&tmp_dir);
}

fn is_thin_archive(archive: &Path) -> bool {
    fs::read(archive)
        .map(|bytes| bytes.starts_with(b"!<thin>\n"))
        .unwrap_or(false)
}

fn external_archive_objects(archive: &Path) -> Option<Vec<PathBuf>> {
    let output = Command::new("ar")
        .arg("t")
        .arg(archive)
        .output()
        .expect("failed to run `ar t`");
    assert!(output.status.success(), "ar t failed for libghostty-vt.a");

    let ghostty_dir = archive
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .expect("zig-out/lib archive lives under Ghostty source root");
    let archive_dir = archive.parent().expect("archive has parent");

    let member_output = String::from_utf8_lossy(&output.stdout);
    let members: Vec<_> = member_output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    let objects: Option<Vec<_>> = members
        .iter()
        .map(|member| resolve_archive_member(ghostty_dir, archive_dir, member))
        .collect();
    let mut objects = objects?;
    objects.sort();
    Some(objects)
}

fn resolve_archive_member(ghostty_dir: &Path, archive_dir: &Path, member: &str) -> Option<PathBuf> {
    let member_path = Path::new(member);
    let candidates = if member_path.is_absolute() {
        vec![member_path.to_path_buf()]
    } else {
        vec![ghostty_dir.join(member_path), archive_dir.join(member_path)]
    };

    candidates.into_iter().find(|path| path.exists())
}

fn archive_objects(repacked_lib: &Path, objects: &[PathBuf]) {
    assert!(
        !objects.is_empty(),
        "libghostty-vt.a did not contain object files"
    );
    let mut ar_cmd = Command::new("ar");
    ar_cmd.args(["rcs"]).arg(repacked_lib);
    for object in objects {
        ar_cmd.arg(object);
    }
    let status = ar_cmd.status().expect("failed to run `ar rcs`");
    assert!(status.success(), "ar rcs failed for libghostty-vt.a");
}

fn emit_link_directives(out_dir: &str) {
    println!("cargo:rustc-link-search=native={out_dir}");
    println!("cargo:rustc-link-lib=static=ghostty-vt");

    if cfg!(target_os = "macos") {
        println!("cargo:rustc-link-lib=c++");
    }
}

fn emit_rerun_directives() {
    println!("cargo:rerun-if-env-changed=BOTSTER_ZIG");
    println!("cargo:rerun-if-env-changed=ZIG");
    println!("cargo:rerun-if-env-changed=ZIG_GLOBAL_CACHE_DIR");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=build_support.rs");
    println!("cargo:rerun-if-changed=vendor/ghostty/build.zig");
    println!("cargo:rerun-if-changed=vendor/ghostty/build.zig.zon");
    println!("cargo:rerun-if-changed=vendor/ghostty/src");
    println!("cargo:rerun-if-changed=vendor/ghostty/include");
    println!("cargo:rerun-if-changed=vendor/ghostty/LICENSE");
}
