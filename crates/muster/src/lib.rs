#![doc = include_str!("../README.md")]
#![deny(missing_docs, rustdoc::broken_intra_doc_links)]

pub mod bounds;
pub mod cli;
pub mod config;
pub mod error;
pub mod linear;
pub mod output;

pub use config::{Config, EnvVar, KeySource, TeamSource};
pub use error::{Error, ErrorCode, LinearError};
pub use linear::{Linear, Operation};
pub use output::{Format, Output};

/// The version of this crate, as `muster --version` and `muster schema` print it.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Where the reference page lives, the base of every `docs` link in an error object.
pub const DOCS_URL: &str =
    "https://github.com/appshapes/muster/blob/master/docs/reference/commands.md";
