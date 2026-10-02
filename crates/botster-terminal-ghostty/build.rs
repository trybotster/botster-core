//! Builds the pinned libghostty-vt as a static library and links it.
//!
//! The library is built from the Ghostty source in `vendor/ghostty` (a submodule at the pinned commit). The build uses
//! **no network**:
//!
//! - The Zig packages that the lib-vt build needs are in an immutable *package store*, filled once by the prefetch
//!   step (`BOTSTER_ZIG_PACKAGES`, default `~/.cache/botster/zig-packages`, layout `p/<hash>.tar.gz`). This script
//!   copies the seven files that the build needs into a Zig global cache under `OUT_DIR`, where Zig also writes its
//!   artifacts. It never writes the package store and never fetches a package. A missing file stops the build and
//!   names the file, the store and the prefetch command.
//! - `zig build` runs under an OS network denial (`sandbox-exec` on macOS, `unshare --net` on Linux), in every build,
//!   so a package that is missing could not be fetched even by mistake. A host whose build environment already has no
//!   network and no working `unshare` (the Linux gate container) sets `BOTSTER_ZIG_NETWORK_DENIED=1` to say so; the
//!   script then runs Zig without the wrapper. Without that variable a missing wrapper stops the build.
//!
//! This script is a driver: it reads the environment and runs processes, which the machine crates must not do.

#![allow(clippy::disallowed_methods)]

#[allow(dead_code)]
mod build_data;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use build_data::{GHOSTTY_BUILD_ARGS, REQUIRED_ZIG_VERSION, ZIG_PACKAGES, ZIG_PACKAGES_IN_ZON};

const GHOSTTY_SUBMODULE: &str = "crates/botster-terminal-ghostty/vendor/ghostty";
const PREFETCH_HINT: &str = "run crates/botster-terminal-ghostty/prefetch-zig.sh once, with network, to fill the package store";

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=build_data.rs");
    println!("cargo:rerun-if-env-changed=BOTSTER_ZIG");
    println!("cargo:rerun-if-env-changed=BOTSTER_ZIG_PACKAGES");
    println!("cargo:rerun-if-env-changed=BOTSTER_ZIG_NETWORK_DENIED");
    for path in ["build.zig", "build.zig.zon", "src", "include", "pkg", "LICENSE"] {
        println!("cargo:rerun-if-changed=vendor/ghostty/{path}");
    }

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let ghostty = manifest_dir.join("vendor/ghostty");
    require_ghostty_source(&ghostty);
    check_package_list(&ghostty);

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let global_cache = out_dir.join("zig-global");
    stage_packages(&global_cache);

    let zig = resolve_zig(&ghostty);
    let prefix = out_dir.join("zig-prefix");
    run_zig_build(&zig, &ghostty, &out_dir, &global_cache, &prefix);

    let library = prefix.join("lib/libghostty-vt.a");
    assert!(library.exists(), "zig build did not install {}", library.display());
    // The header directory, for a consumer that wants the C headers of the same build.
    println!("cargo:rustc-link-search=native={}", prefix.join("lib").display());
    println!("cargo:rustc-link-lib=static=ghostty-vt");
    println!("cargo:include={}", ghostty.join("include").display());
}

fn require_ghostty_source(ghostty: &Path) {
    if ghostty.join("build.zig").exists() && ghostty.join("LICENSE").exists() {
        return;
    }
    panic!(
        "the Ghostty source is not at {GHOSTTY_SUBMODULE}; run `git submodule update --init {GHOSTTY_SUBMODULE}`"
    );
}

/// Every package of `ZIG_PACKAGES_IN_ZON` must still appear in a `build.zig.zon` of the source. A pin move that
/// changes those dependencies makes this fail, so the list in `build_data.rs` cannot drift silently. The packages that
/// only those packages depend on cannot be checked here; a changed one makes the offline build fail with a Zig error.
fn check_package_list(ghostty: &Path) {
    let mut zon = String::new();
    for path in ["build.zig.zon", "pkg/translate-c/build.zig.zon"] {
        zon.push_str(&fs::read_to_string(ghostty.join(path)).unwrap_or_default());
    }
    for hash in ZIG_PACKAGES_IN_ZON {
        assert!(
            zon.contains(hash),
            "the Zig package {hash} is not in the build.zig.zon files of the Ghostty source: the pin changed its \
             dependencies, so update ZIG_PACKAGES in build_data.rs and the prefetch step"
        );
    }
}

fn package_store() -> PathBuf {
    if let Some(dir) = env::var_os("BOTSTER_ZIG_PACKAGES") {
        return PathBuf::from(dir);
    }
    let home = env::var_os("HOME").expect("HOME is not set, and BOTSTER_ZIG_PACKAGES is not set");
    PathBuf::from(home).join(".cache/botster/zig-packages")
}

/// Copy the packages that the build needs from the immutable store into the Zig global cache under `OUT_DIR`.
fn stage_packages(global_cache: &Path) {
    let store = package_store();
    let packages = global_cache.join("p");
    fs::create_dir_all(&packages).expect("create the Zig package directory");

    let missing: Vec<String> = ZIG_PACKAGES
        .iter()
        .filter(|hash| !store.join("p").join(format!("{hash}.tar.gz")).exists())
        .map(|hash| (*hash).to_owned())
        .collect();
    assert!(
        missing.is_empty(),
        "the Zig package store {} lacks {} package(s): {}. The build fetches nothing. To fix it, {PREFETCH_HINT}.",
        store.display(),
        missing.len(),
        missing.join(", ")
    );

    for hash in ZIG_PACKAGES {
        let name = format!("{hash}.tar.gz");
        let target = packages.join(&name);
        if !target.exists() {
            fs::copy(store.join("p").join(&name), &target)
                .unwrap_or_else(|e| panic!("copy the Zig package {name}: {e}"));
        }
    }
}

fn resolve_zig(ghostty: &Path) -> PathBuf {
    let program = env::var_os("BOTSTER_ZIG").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("zig"));
    let output = Command::new(&program)
        .arg("version")
        .current_dir(ghostty)
        .output()
        .unwrap_or_else(|e| panic!("cannot run Zig ({}): {e}. Set BOTSTER_ZIG to a Zig {REQUIRED_ZIG_VERSION}.", program.display()));
    let version = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    assert!(
        output.status.success() && version == REQUIRED_ZIG_VERSION,
        "the pinned Ghostty needs Zig {REQUIRED_ZIG_VERSION} exactly, and {} is {version:?}",
        program.display()
    );
    program
}

/// The command prefix that denies the network to the command that follows, or nothing when the host says that it has
/// no network.
fn network_denial() -> Vec<String> {
    if env::var_os("BOTSTER_ZIG_NETWORK_DENIED").is_some_and(|v| v == "1") {
        return Vec::new();
    }
    if cfg!(target_os = "macos") {
        return vec![
            "sandbox-exec".into(),
            "-p".into(),
            "(version 1)(allow default)(deny network*)".into(),
        ];
    }
    if cfg!(target_os = "linux") {
        let probe = Command::new("unshare").args(["--net", "--map-root-user", "true"]).status();
        if probe.is_ok_and(|s| s.success()) {
            return vec!["unshare".into(), "--net".into(), "--map-root-user".into()];
        }
        panic!(
            "cannot deny the network to Zig: `unshare --net` does not work here. Run where it works, or set \
             BOTSTER_ZIG_NETWORK_DENIED=1 on a host whose build environment has no network"
        );
    }
    panic!("no network denial is known for this operating system; set BOTSTER_ZIG_NETWORK_DENIED=1 on a host without network");
}

fn run_zig_build(zig: &Path, ghostty: &Path, out_dir: &Path, global_cache: &Path, prefix: &Path) {
    let mut denial = network_denial().into_iter();
    let mut command = match denial.next() {
        Some(wrapper) => {
            let mut command = Command::new(wrapper);
            command.args(denial);
            command.arg(zig);
            command
        }
        None => Command::new(zig),
    };
    command
        .args(GHOSTTY_BUILD_ARGS)
        .arg("--cache-dir")
        .arg(out_dir.join("zig-local"))
        .arg("--global-cache-dir")
        .arg(global_cache)
        .arg("--prefix")
        .arg(prefix)
        .current_dir(ghostty);
    println!("cargo:warning=building libghostty-vt with Zig {REQUIRED_ZIG_VERSION} and no network");
    let status = command.status().unwrap_or_else(|e| panic!("cannot run the Zig build: {e}"));
    assert!(status.success(), "the libghostty-vt build failed ({status})");
}
