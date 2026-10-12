//! `cargo xtask <command>`: argv in, text out, exit status. All behavior is in the library (`xtask::run`).

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = if args.is_empty() {
        // No command: `run` answers with the usage text before it reads the root.
        xtask::run(std::path::Path::new(""), &args)
    } else {
        xtask::repo_root().and_then(|root| xtask::run(&root, &args))
    };
    if let Err(error) = result {
        xtask::print_error(&error);
        std::process::exit(1);
    }
}
