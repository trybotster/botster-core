//! The data of the libghostty-vt build: the Zig version, the build arguments and the Zig packages that the build needs.
//! `build.rs` includes this file, and a test of the crate reads it too, so there is one list.

/// The Zig version that the pinned Ghostty source requires (`minimum_zig_version` of its `build.zig.zon`).
pub const REQUIRED_ZIG_VERSION: &str = "0.16.0";

/// The arguments of `zig build` for the static libghostty-vt library, without the cache and prefix directories.
pub const GHOSTTY_BUILD_ARGS: &[&str] = &[
    "build",
    "-Demit-lib-vt",
    "-Doptimize=ReleaseFast",
    "-Dsimd=false",
    "-Dcpu=baseline",
    "-Demit-xcframework=false",
];

/// The Zig packages that the lib-vt build needs, by content hash. Zig keeps each one as `p/<hash>.tar.gz` in its global
/// cache. A prefetch step puts them in the package store (see `build.rs`), and the build never fetches them.
///
/// The list is the one that Zig resolves for `GHOSTTY_BUILD_ARGS` at the pinned Ghostty commit. `build.rs` checks that
/// each hash still appears in a `build.zig.zon` of the Ghostty source, so a pin move that changes the list fails loudly.
pub const ZIG_PACKAGES: &[&str] = &[
    "aro-0.0.0-JSD1Qk6lNgDdcDV4Vh7Sfy-34m2TluIVOdPzMmj_0BjX",
    "N-V-__8AAB0eQwD-0MdOEBmz7intriBReIsIDNlukNVoNu6o",
    "N-V-__8AADYiAAB_80AWnH1AxXC0tql9thT-R-DYO1gBqTLc",
    "N-V-__8AAM94BAAFk_hn4UW0x_OBD2g0vOwexeAAyWNNo4eB",
    "N-V-__8AAP5JWgCGP_AD0teWpa4krRvE9VPZzvviGdbmN4jI",
    "translate_c-0.0.0-Q_BUWhVNBwDOEcIqub4VFPJPB6D9dgwzUMHTX5KWr8Xr",
    "uucode-0.2.0-ZZjBPuuFVgC8YZ8eld4fOKsZANLIhTFMzULQxhkLi1C7",
];

/// The packages of `ZIG_PACKAGES` that the `build.zig.zon` files of the Ghostty source name directly. The others
/// (`aro` and the `N-V-...` packages) are dependencies of those packages, named only inside the package archives.
pub const ZIG_PACKAGES_IN_ZON: &[&str] = &[
    "translate_c-0.0.0-Q_BUWhVNBwDOEcIqub4VFPJPB6D9dgwzUMHTX5KWr8Xr",
    "uucode-0.2.0-ZZjBPuuFVgC8YZ8eld4fOKsZANLIhTFMzULQxhkLi1C7",
];
