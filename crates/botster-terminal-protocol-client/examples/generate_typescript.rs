//! Print the generated TypeScript artifact to stdout.
//!
//! ```sh
//! cargo run -p botster-terminal-protocol-client --example generate_typescript \
//!   > crates/botster-terminal-protocol-client/generated/terminal-protocol.ts
//! ```

use std::io::Write;

fn main() {
    let generated = botster_terminal_protocol_client::terminal_protocol_typescript();
    let mut stdout = std::io::stdout().lock();
    stdout
        .write_all(generated.as_bytes())
        .and_then(|()| stdout.flush())
        .expect("write generated TypeScript to stdout");
}
