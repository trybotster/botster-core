//! The fixture's conformance binary. Like the real harness, it names an ignored trial `[<kind>] conf::<id>`: it answers
//! the libtest list calls of nextest (`--list --format terse`, with `--ignored` for the ignored tests only). It lists a
//! selected test and the ignored trial `[pending-real] conf::pending_id`, which passes the check only while
//! `conf::pending_id` is pending-real. A run without `--list` runs nothing and succeeds.

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if !args.iter().any(|arg| arg == "--list") {
        return;
    }
    if !args.iter().any(|arg| arg == "--ignored") {
        println!("selected: test");
    }
    println!("[pending-real] conf::pending_id: test");
}
