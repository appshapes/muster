//! The one error type, its codes and the error object muster writes to stderr
//! (CLAUDE.md, invariants 2 and 4; ADR 0008). `docs/reference/commands.md#exit-codes`
//! is the table, generated from [`ErrorCode::ALL`].

use std::fmt;
use std::process::ExitCode;

use serde_json::{Value, json};

use crate::DOCS_URL;

/// A failure class, with its exit code. The table is append-only (ADR 0008).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    /// Muster's own failure, or an answer it does not understand.
    Failed,
    /// A command line muster cannot act on; `clap` uses the same code.
    Usage,
    /// No key, or Linear refused the key.
    Authentication,
    /// Linear has no such issue, team, state, label, project or user.
    NotFound,
    /// Linear's rate limit: the request was not served.
    RateLimited,
    /// Linear's validation refused the input, a muster guard did, or the workspace's
    /// plan or the key's role does not allow the operation.
    Refused,
    /// The request was not sent: name resolution, connection, TLS or a timeout before
    /// sending; or a query whose answer never came.
    Network,
    /// A mutation was sent and no answer came: the request may have landed.
    Unknown,
}

impl ErrorCode {
    /// Every code, in exit-code order, for `muster schema` and the test that holds the
    /// reference to it.
    pub const ALL: &'static [ErrorCode] = &[
        ErrorCode::Failed,
        ErrorCode::Usage,
        ErrorCode::Authentication,
        ErrorCode::NotFound,
        ErrorCode::RateLimited,
        ErrorCode::Refused,
        ErrorCode::Network,
        ErrorCode::Unknown,
    ];

    /// The process exit code.
    pub fn exit(self) -> u8 {
        match self {
            ErrorCode::Failed => 1,
            ErrorCode::Usage => 2,
            ErrorCode::Authentication => 3,
            ErrorCode::NotFound => 4,
            ErrorCode::RateLimited => 5,
            ErrorCode::Refused => 6,
            ErrorCode::Network => 7,
            ErrorCode::Unknown => 8,
        }
    }

    /// The `code` field of the error object.
    pub fn name(self) -> &'static str {
        match self {
            ErrorCode::Failed => "failed",
            ErrorCode::Usage => "usage",
            ErrorCode::Authentication => "authentication",
            ErrorCode::NotFound => "not_found",
            ErrorCode::RateLimited => "rate_limited",
            ErrorCode::Refused => "refused",
            ErrorCode::Network => "network",
            ErrorCode::Unknown => "unknown",
        }
    }

    /// What the code means, for the reference table.
    pub fn meaning(self) -> &'static str {
        match self {
            ErrorCode::Failed => {
                "muster's own failure, or an answer from Linear it does not understand"
            }
            ErrorCode::Usage => {
                "the command line cannot be acted on: an unknown flag, a missing value, a --limit over the cap, a team that must be named"
            }
            ErrorCode::Authentication => {
                "no key was found, or Linear refused it (HTTP 401, AUTHENTICATION_ERROR)"
            }
            ErrorCode::NotFound => {
                "no such issue, team, state, label, project or user (Linear's \"Entity not found\", or no match for a number, a title or a name)"
            }
            ErrorCode::RateLimited => {
                "Linear's rate limit; the request was not served (HTTP 429 or RATELIMITED); the message carries the reset time"
            }
            ErrorCode::Refused => {
                "the input was refused: Linear's validation (INPUT_ERROR), a body over BODY_MAX_BYTES, a muster guard such as a duplicate title, or what the workspace's plan or the key's role does not allow (FORBIDDEN, FEATURE_NOT_ACCESSIBLE)"
            }
            ErrorCode::Network => {
                "the request was not sent (name resolution, connection, TLS, a timeout before sending), or a query's answer never came; nothing changed in Linear, unless the message says a mutation sent before the query landed"
            }
            ErrorCode::Unknown => {
                "a mutation was sent and no answer came: the request may have landed; check before repeating it"
            }
        }
    }

    /// What to do, for the reference table; a specific error carries a more specific
    /// hint.
    pub fn hint(self) -> &'static str {
        match self {
            ErrorCode::Failed => {
                "read the message; run again with --trace and report the request and the answer as an issue on this repository"
            }
            ErrorCode::Usage => {
                "read the message; `muster <command> --help` and `muster schema` list every flag"
            }
            ErrorCode::Authentication => {
                "put a Linear API key in LINEAR_API_KEY or in ~/.config/muster/key (docs/reference/commands.md#the-key); `muster whoami` says which source was read"
            }
            ErrorCode::NotFound => {
                "check the name or identifier; `muster team list`, `state list`, `label list`, `project list`, `user list` and `issue list --search` show what exists"
            }
            ErrorCode::RateLimited => {
                "wait until the reset time in the message, then run the command again; muster never waits for you"
            }
            ErrorCode::Refused => {
                "fix the input the message names; for a duplicate title pass --allow-duplicate if a second issue is wanted; for a plan or role refusal `muster whoami` shows the key's role"
            }
            ErrorCode::Network => {
                "check the network and MUSTER_ENDPOINT; the command can be run again as it is"
            }
            ErrorCode::Unknown => {
                "read the issue back (`muster issue get`, `issue list --search`) before running the mutation again"
            }
        }
    }
}

/// What Linear said, when it answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinearError {
    /// Linear's `extensions.code`, such as `INPUT_ERROR`, when present.
    pub code: Option<String>,
    /// Linear's `message`.
    pub message: String,
    /// Linear's `x-request-id` header, when present.
    pub request_id: Option<String>,
}

/// A failure, as the error object on stderr carries it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    /// The failure class.
    pub code: ErrorCode,
    /// What happened, in one sentence.
    pub message: String,
    /// What to do next.
    pub hint: String,
    /// The anchor in `docs/reference/commands.md` that explains it.
    pub anchor: &'static str,
    /// What Linear said, when it answered. Boxed so that `Result<_, Error>` stays small
    /// on every path (clippy's `result_large_err`).
    pub linear: Option<Box<LinearError>>,
}

impl Error {
    /// A failure with the code's own hint and the exit-codes anchor.
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Error {
        Error {
            code,
            message: message.into(),
            hint: code.hint().to_string(),
            anchor: "exit-codes",
            linear: None,
        }
    }

    /// The same, with a hint of its own.
    pub fn with_hint(
        code: ErrorCode,
        message: impl Into<String>,
        hint: impl Into<String>,
    ) -> Error {
        Error {
            code,
            message: message.into(),
            hint: hint.into(),
            anchor: "exit-codes",
            linear: None,
        }
    }

    /// Point the `docs` link at another anchor of the reference.
    pub fn at(mut self, anchor: &'static str) -> Error {
        self.anchor = anchor;
        self
    }

    /// Attach what Linear said.
    pub fn from_linear(mut self, linear: LinearError) -> Error {
        self.linear = Some(Box::new(linear));
        self
    }

    /// The process exit code.
    pub fn exit_code(&self) -> ExitCode {
        ExitCode::from(self.code.exit())
    }

    /// The error object, as written to stderr: `{"error": {...}}`.
    pub fn to_json(&self) -> Value {
        let mut error = json!({
            "code": self.code.name(),
            "exit": self.code.exit(),
            "message": self.message,
            "hint": self.hint,
            "docs": format!("{DOCS_URL}#{}", self.anchor),
        });
        if let (Some(linear), Some(map)) = (&self.linear, error.as_object_mut()) {
            map.insert(
                "linear".to_string(),
                json!({ "code": linear.code, "message": linear.message, "requestId": linear.request_id }),
            );
        }
        json!({ "error": error })
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} ({}, exit {}): {}",
            self.code.name(),
            self.message,
            self.code.exit(),
            self.hint
        )
    }
}

impl std::error::Error for Error {}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Error {
        Error::new(
            ErrorCode::Failed,
            format!("an answer could not be read as JSON: {e}"),
        )
    }
}
