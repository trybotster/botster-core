//! Pinned Ghostty source and build data shared by the build script and crate.

/// Repository that owns the vendored Ghostty source.
pub const GHOSTTY_SOURCE_REPOSITORY: &str = "https://github.com/trybotster/ghostty";

/// Exact vendored Ghostty commit.
pub const GHOSTTY_SOURCE_COMMIT: &str = "eb72ec61304ea256be1d86ed8fa961c84e43ecbd";

/// Ghostty application version at the pinned commit.
pub const GHOSTTY_APP_VERSION: &str = "1.3.2-dev";

/// libghostty-vt version with the exact source commit in SemVer build data.
pub const GHOSTTY_LIB_VERSION: &str = "0.1.0-dev+eb72ec61304ea256be1d86ed8fa961c84e43ecbd";

/// Version of the JSON ABI manifest returned by `ghostty_type_json`.
pub const GHOSTTY_ABI_SCHEMA_VERSION: u64 = 1;

/// Return the stable Zig arguments for the pinned native library build.
#[allow(dead_code)]
pub const fn ghostty_build_args() -> [&'static str; 8] {
    [
        "build",
        "-Demit-lib-vt",
        "-Doptimize=ReleaseFast",
        "-Dsimd=false",
        "-Dcpu=baseline",
        "-Dversion-string=1.3.2-dev",
        "-Dlib-version-string=0.1.0-dev+eb72ec61304ea256be1d86ed8fa961c84e43ecbd",
        "-Demit-xcframework=false",
    ]
}
