//! The `muster` binary (ADR 0007, ADR 0008): parse the command line, run the command,
//! write the answer or the error object, return the exit code. Nothing here prompts.
#![deny(missing_docs, rustdoc::broken_intra_doc_links)]

use std::process::ExitCode;

use clap::Parser;
use muster::cli::{Cli, run};
use muster::output::{Format, Output};

fn main() -> ExitCode {
    let cli = Cli::parse();
    let out = Output {
        format: if cli.human {
            Format::Human
        } else {
            Format::Json
        },
    };
    match run::run(&cli, &out) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            out.error(&error);
            error.exit_code()
        }
    }
}
