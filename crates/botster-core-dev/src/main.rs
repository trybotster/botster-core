//! Dev-only executable for exercising `botster-core` during engine work.

fn main() {
    match botster_core_dev::run_engine_smoke() {
        Ok(report) => {
            for line in report.lines() {
                println!("{line}");
            }
        }
        Err(error) => {
            eprintln!("botster-core dev harness failed: {error}");
            std::process::exit(1);
        }
    }
}
