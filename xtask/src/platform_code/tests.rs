use super::*;
use regex::Regex;

fn meta(text: &str) -> syn::Meta {
    syn::parse_str(text).unwrap()
}

fn derive(files: &[(&str, &str)], os: &str) -> Result<Vec<String>, Vec<String>> {
    let files: Vec<(String, String)> = files
        .iter()
        .map(|(path, text)| ((*path).to_string(), (*text).to_string()))
        .collect();
    exclusions(&files, os)
}

/// Whether one of `patterns` excludes the mutant `name`, as `cargo mutants --exclude-re` does.
fn excluded(patterns: &[String], name: &str) -> bool {
    patterns
        .iter()
        .any(|p| Regex::new(p).unwrap().is_match(name))
}

#[test]
fn only_the_target_os_decides_a_predicate() {
    for (predicate, linux, macos) in [
        (r#"target_os = "macos""#, Some(false), Some(true)),
        (r#"target_os = "linux""#, Some(true), Some(false)),
        ("unix", Some(true), Some(true)),
        ("windows", Some(false), Some(false)),
        (r#"target_family = "unix""#, Some(true), Some(true)),
        (r#"target_family = "wasm""#, Some(false), Some(false)),
        (r#"not(target_os = "macos")"#, Some(true), Some(false)),
        (
            r#"all(unix, not(target_os = "macos"))"#,
            Some(true),
            Some(false),
        ),
        (
            r#"any(target_os = "macos", target_os = "linux")"#,
            Some(true),
            Some(true),
        ),
        (
            r#"not(any(target_os = "macos", target_os = "linux"))"#,
            Some(false),
            Some(false),
        ),
        // A predicate that is not the OS alone can be true, so the code counts as compiled.
        ("test", None, None),
        (r#"feature = "slow""#, None, None),
        ("not(test)", None, None),
        (r#"any(target_os = "linux", test)"#, Some(true), None),
        (r#"all(target_os = "linux", test)"#, None, Some(false)),
        (r#"target_arch = "x86_64""#, None, None),
        (
            r#"any(windows, target_os = "freebsd")"#,
            Some(false),
            Some(false),
        ),
    ] {
        let meta = meta(predicate);
        assert_eq!(
            (compiled(&meta, "linux"), compiled(&meta, "macos")),
            (linux, macos),
            "{predicate}"
        );
    }
    for predicate in [
        "windows",
        r#"target_family = "windows""#,
        r#"target_os = "windows""#,
    ] {
        assert_eq!(
            compiled(&meta(predicate), "windows"),
            Some(true),
            "{predicate}"
        );
    }
}

/// The two `pending_output` functions of core-sys `payload.rs` have the same name: lines, not names, tell them apart.
#[test]
fn a_gated_function_is_excluded_by_its_lines_and_only_off_its_os() {
    let text = "\
#[cfg(not(target_os = \"macos\"))]
fn pending_output() -> usize {
    1
}

#[cfg(target_os = \"macos\")]
fn pending_output() -> usize {
    2
}

fn portable() -> usize {
    3
}
";
    let files = [("crates/a/src/payload.rs", text)];
    let linux = derive(&files, "linux").unwrap();
    assert_eq!(linux, [r"^crates/a/src/payload\.rs:(6|7|8|9):"]);
    let macos = derive(&files, "macos").unwrap();
    assert_eq!(macos, [r"^crates/a/src/payload\.rs:(1|2|3|4):"]);
    let name = |line: usize| {
        format!("crates/a/src/payload.rs:{line}:5: replace pending_output -> usize with 0")
    };
    assert!(excluded(&linux, &name(8)) && !excluded(&linux, &name(3)));
    assert!(excluded(&macos, &name(3)) && !excluded(&macos, &name(8)));
    assert!(!excluded(
        &linux,
        "crates/a/src/payload.rs:12:5: replace portable -> usize with 0"
    ));
    assert!(!excluded(
        &linux,
        "crates/a/src/payload.rs:80:5: replace portable -> usize with 0"
    ));
}

/// The three blocks of core-sys `process.rs` `start_time` are statements of one function.
#[test]
fn a_gated_statement_block_impl_and_trait_item_match_arm_and_inline_module_are_excluded() {
    let text = "\
fn start_time() -> u64 {
    #[cfg(target_os = \"macos\")]
    {
        1 + 2
    }
    #[cfg(target_os = \"linux\")]
    {
        3 + 4
    }
}
impl S {
    #[cfg(target_os = \"linux\")]
    fn f() -> u64 { 5 }
    fn g(x: u8) -> u64 {
        match x {
            #[cfg(target_os = \"linux\")]
            0 => 6,
            _ => 7,
        }
    }
}
trait T {
    #[cfg(target_os = \"linux\")]
    fn h() -> u64 { 8 }
}
#[cfg(target_os = \"linux\")]
mod inline {
    fn i() -> u64 { 9 }
}
fn k() {
    #[cfg(target_os = \"linux\")]
    let x = 10;
}
";
    let files = [("src/lib.rs", text)];
    assert_eq!(
        derive(&files, "linux").unwrap(),
        [r"^src/lib\.rs:(2|3|4|5):"]
    );
    assert_eq!(
        derive(&files, "macos").unwrap(),
        [r"^src/lib\.rs:(6|7|8|9|12|13|16|17|23|24|26|27|28|29|31|32):"]
    );
}

/// A compiled trait method or match arm can hold gated code; an expression that only looks like an attribute gates nothing.
#[test]
fn gated_code_inside_a_compiled_trait_method_or_match_arm_is_excluded() {
    let text = "\
trait T {
    fn j() {
        #[cfg(windows)]
        let y = 1;
    }
}
fn m(x: u8) -> u8 {
    match x {
        0 => {
            #[cfg(windows)]
            let z = 2;
            3
        }
        _ => 4,
    }
}
fn n() {
    -[cfg(windows)];
}
";
    assert_eq!(
        derive(&[("src/lib.rs", text)], "linux").unwrap(),
        [r"^src/lib\.rs:(3|4|10|11):"]
    );
}

#[test]
fn code_under_a_predicate_that_is_not_the_os_alone_keeps_its_mutants() {
    let text = "\
#[cfg(feature = \"slow\")]
fn slow() -> u8 { 1 }
#[cfg(any(target_os = \"linux\", test))]
fn stat() -> u8 { 2 }
";
    assert_eq!(
        derive(&[("src/lib.rs", text)], "macos").unwrap(),
        Vec::<String>::new()
    );
    assert_eq!(
        derive(&[("src/lib.rs", text)], "linux").unwrap(),
        Vec::<String>::new()
    );
}

/// botster-test-process `platform.rs` declares `mod linux;` and `mod macos;`: the module's whole file, and the files of its
/// own modules, are excluded where it is not compiled.
#[test]
fn a_gated_module_excludes_its_file_and_its_own_modules() {
    let platform = "\
#[cfg(target_os = \"linux\")]
mod linux;
#[cfg(target_os = \"macos\")]
mod macos;
mod shared;
";
    let files = [
        ("crates/p/src/platform.rs", platform),
        ("crates/p/src/platform/linux.rs", "fn a() -> u8 { 1 }\n"),
        (
            "crates/p/src/platform/macos.rs",
            "mod kq;\nfn b() -> u8 { 2 }\n",
        ),
        ("crates/p/src/platform/macos/kq.rs", "fn c() -> u8 { 3 }\n"),
        ("crates/p/src/platform/shared.rs", "fn d() -> u8 { 4 }\n"),
    ];
    let linux = derive(&files, "linux").unwrap();
    assert_eq!(
        linux,
        [
            r"^crates/p/src/platform/macos\.rs:",
            r"^crates/p/src/platform/macos/kq\.rs:",
        ]
    );
    assert!(excluded(
        &linux,
        "crates/p/src/platform/macos.rs:2:16: replace b -> u8 with 0"
    ));
    assert!(excluded(
        &linux,
        "crates/p/src/platform/macos/kq.rs:1:16: replace c -> u8 with 0"
    ));
    assert!(!excluded(
        &linux,
        "crates/p/src/platform/linux.rs:1:16: replace a -> u8 with 0"
    ));
    assert!(!excluded(
        &linux,
        "crates/p/src/platform/shared.rs:1:16: replace d -> u8 with 0"
    ));
    assert_eq!(
        derive(&files, "macos").unwrap(),
        [r"^crates/p/src/platform/linux\.rs:"]
    );
}

#[test]
fn a_gated_module_is_found_as_a_mod_rs_file_by_a_path_attribute_and_inside_an_inline_module() {
    let lib = "\
#[cfg(windows)]
mod win;
/// A doc comment is an attribute too, and not a path.
#[cfg(windows)]
#[path = \"other/place.rs\"]
mod moved;
mod outer {
    #[cfg(windows)]
    mod inner;
}
";
    let files = [
        ("src/lib.rs", lib),
        ("src/win/mod.rs", "fn a() {}\n"),
        ("src/other/place.rs", "fn b() {}\n"),
        ("src/outer/inner.rs", "fn c() {}\n"),
    ];
    assert_eq!(
        derive(&files, "linux").unwrap(),
        [
            r"^src/other/place\.rs:",
            r"^src/outer/inner\.rs:",
            r"^src/win/mod\.rs:",
        ]
    );
}

/// Code that the derivation cannot place fails it, so no platform-only mutant can stay MISSED unexplained.
#[test]
fn a_gated_module_without_a_file_a_cfg_if_and_a_file_that_does_not_parse_fail_the_derivation() {
    let errors = derive(&[("src/lib.rs", "#[cfg(windows)]\nmod gone;\n")], "linux").unwrap_err();
    assert_eq!(
        errors,
        ["src/lib.rs:1: the file of the platform-gated module `gone` is not found"]
    );
    let errors = derive(
        &[(
            "src/lib.rs",
            "cfg_if::cfg_if! { if #[cfg(unix)] { fn a() {} } }\n",
        )],
        "linux",
    )
    .unwrap_err();
    assert!(errors[0].starts_with("src/lib.rs:1: cfg_if!"), "{errors:?}");
    let errors = derive(&[("src/lib.rs", "fn (")], "linux").unwrap_err();
    assert!(errors[0].contains("does not parse"), "{errors:?}");
}

/// #181 B8: a file that a gated declaration and a compiled declaration both reach is compiled, so it keeps its mutants, on
/// each OS; so do the modules that it declares. A compiled declaration inside an excluded file rescues nothing.
#[test]
fn a_file_that_a_compiled_declaration_also_reaches_is_not_excluded() {
    let lib = "\
#[cfg(target_os = \"macos\")]
#[path = \"./shared.rs\"]
mod mac_shared;
#[cfg(target_os = \"linux\")]
#[path = \"shared.rs\"]
mod linux_shared;
#[cfg(target_os = \"macos\")]
mod mac;
#[path = \"mac/inner.rs\"]
mod also;
";
    // The files of the leaves come first: the rescue of `leaf.rs` takes a second round.
    let files = [
        ("src/mac/inner/leaf.rs", "fn e() -> u8 { 5 }\n"),
        ("src/mac/inner.rs", "mod leaf;\nfn d() -> u8 { 4 }\n"),
        ("src/lib.rs", lib),
        ("src/shared.rs", "mod nested;\nfn a() -> u8 { 1 }\n"),
        ("src/shared/nested.rs", "fn b() -> u8 { 2 }\n"),
        ("src/mac.rs", "mod inner;\nmod only;\nfn c() -> u8 { 3 }\n"),
        ("src/mac/only.rs", "fn f() -> u8 { 6 }\n"),
    ];
    assert_eq!(
        derive(&files, "linux").unwrap(),
        [r"^src/mac\.rs:", r"^src/mac/only\.rs:"]
    );
    assert_eq!(derive(&files, "macos").unwrap(), Vec::<String>::new());
}

/// #181 B8 round 2: inside an inline module, a `#[path]` is relative to the inline module's directory. On Linux the
/// compiled `mod linux { #[path = "shared.rs"] mod live; }` reaches `src/linux/shared.rs`, which the gated `mac` declaration
/// also reaches, so that file keeps its mutants; `src/shared.rs`, beside `lib.rs`, is not the file.
#[test]
fn an_inline_path_is_relative_to_the_inline_module() {
    let lib = "\
#[cfg(target_os = \"macos\")]
#[path = \"linux/shared.rs\"]
mod mac;
#[cfg(target_os = \"linux\")]
mod linux {
    #[path = \"shared.rs\"]
    mod live;
}
";
    let files = [
        ("src/lib.rs", lib),
        ("src/linux/shared.rs", "fn a() -> u8 { 1 }\n"),
        ("src/shared.rs", "fn b() -> u8 { 2 }\n"),
    ];
    let shared = "src/linux/shared.rs:1:16: replace a -> u8 with 0";
    for os in ["linux", "macos"] {
        let patterns = derive(&files, os).unwrap();
        assert!(!excluded(&patterns, shared), "{os}: {patterns:?}");
    }
    assert_eq!(derive(&files, "linux").unwrap(), Vec::<String>::new());
}

/// #181 B8 round 3, plan section 8: a `#[path]` on an inline module is not a listed form, so the derivation fails on every
/// system (also where a gate hides the module) and names the form and the file. It never derives `alt/shared.rs` as
/// macOS-only through the Mac declaration while Linux compiles it.
#[test]
fn a_path_on_an_inline_module_fails_the_derivation_on_every_system() {
    let lib = "\
#[cfg(target_os = \"macos\")]
#[path = \"alt/shared.rs\"]
mod mac;
#[cfg(target_os = \"linux\")]
#[path = \"alt\"]
mod linux {
    #[path = \"shared.rs\"]
    mod live;
}
";
    let files = [
        ("src/lib.rs", lib),
        ("src/alt/shared.rs", "fn a() -> u8 { 1 }\n"),
        ("src/linux/shared.rs", "fn b() -> u8 { 2 }\n"),
    ];
    for os in ["linux", "macos"] {
        assert_eq!(
            derive(&files, os).unwrap_err(),
            ["src/lib.rs:5:1: `#[path]` on the inline module `linux` is not a form that the check resolves (plan section 8): give the module its own file, or remove the attribute"],
            "{os}"
        );
    }
}
