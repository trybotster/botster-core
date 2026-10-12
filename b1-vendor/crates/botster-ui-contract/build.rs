//! Rebuilds the crate when a vector file changes, because the vectors are embedded in the crate.

fn main() {
    println!("cargo:rerun-if-changed=vectors");
}
