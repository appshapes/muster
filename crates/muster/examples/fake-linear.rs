//! The fake Linear as a process, for `just quickstart` (ADR 0010): prints its endpoint
//! on the first line of stdout and serves until it is killed. `tests/cli.rs` runs the
//! same module in process.
#![allow(clippy::print_stdout, clippy::expect_used)]

#[path = "../tests/support/fake_linear.rs"]
mod fake_linear;

fn main() {
    let fake = fake_linear::Fake::start().expect("a loopback port");
    println!("{}", fake.endpoint());
    loop {
        std::thread::park();
    }
}
