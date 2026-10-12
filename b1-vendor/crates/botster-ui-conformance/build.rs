//! Rebuilds the crate when a transcript changes, because the transcripts are embedded in the crate.

fn main() {
    println!("cargo:rerun-if-changed=../../conformance/ui");
}
