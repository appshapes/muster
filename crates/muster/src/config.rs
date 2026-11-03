//! Where the key, the team and the endpoint come from (CLAUDE.md, "The key"; ADR
//! 0008), and the registry of every environment variable muster reads, each with the
//! reason its default cannot be right (invariant 5). `muster whoami` reports what was
//! read, and `muster schema` prints the registry.

use std::path::PathBuf;

use crate::error::{Error, ErrorCode};

/// One environment variable muster reads, for `muster schema` and the reference.
#[derive(Debug, Clone, Copy)]
pub struct EnvVar {
    /// The variable's name.
    pub name: &'static str,
    /// Which commands read it.
    pub read_by: &'static str,
    /// Why the default cannot be right for everyone (CLAUDE.md, invariant 5).
    pub why_not_default: &'static str,
    /// What `muster whoami` checks about it.
    pub whoami_check: &'static str,
}

/// Every environment variable muster reads. Nothing else is configuration.
pub const ENVIRONMENT: &[EnvVar] = &[
    EnvVar {
        name: "LINEAR_API_KEY",
        read_by: "every command that reaches Linear",
        why_not_default: "a key is the one thing muster cannot infer; the environment is where a CI job or a one-off shell puts it, and the file below is where a laptop keeps it",
        whoami_check: "reports `key.source` as `LINEAR_API_KEY` when it was read from here",
    },
    EnvVar {
        name: "MUSTER_KEY_FILE",
        read_by: "every command that reaches Linear, when LINEAR_API_KEY is unset",
        why_not_default: "a laptop with several keys, or a runner that mounts a secret at a path of its own; the default path is the XDG one, `~/.config/muster/key`",
        whoami_check: "fails when it names a file that cannot be read",
    },
    EnvVar {
        name: "XDG_CONFIG_HOME",
        read_by: "every command that reaches Linear, to find the default key file",
        why_not_default: "the XDG Base Directory specification lets a user move every tool's configuration at once; muster follows it rather than inventing a path",
        whoami_check: "reports `key.path` as the file under it",
    },
    EnvVar {
        name: "MUSTER_TEAM",
        read_by: "issue get by number, issue list, issue create (where it wins over the parent's team with --parent), state list, label list, and whoami",
        why_not_default: "a workspace with one team needs nothing; a workspace with several has no default, and this saves `--team` on every call from a checkout that belongs to one team; `--team` wins over it",
        whoami_check: "fails when it names no team of the workspace",
    },
    EnvVar {
        name: "MUSTER_ENDPOINT",
        read_by: "every command that reaches Linear",
        why_not_default: "the tests and `just quickstart` point muster at the fake Linear (ADR 0010); nothing else sets it",
        whoami_check: "warns when the endpoint is not Linear's",
    },
];

/// Linear's endpoint, the default of `MUSTER_ENDPOINT`.
pub const LINEAR_ENDPOINT: &str = "https://api.linear.app/graphql";

/// Where the key was read from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeySource {
    /// `LINEAR_API_KEY`.
    Env,
    /// A file: the one `MUSTER_KEY_FILE` names, or the default path.
    File(PathBuf),
}

/// Where the team came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TeamSource {
    /// `--team`.
    Flag,
    /// `MUSTER_TEAM`.
    Env,
    /// The workspace's only team.
    Inferred,
}

impl TeamSource {
    /// The word `whoami` prints.
    pub fn name(self) -> &'static str {
        match self {
            TeamSource::Flag => "--team",
            TeamSource::Env => "MUSTER_TEAM",
            TeamSource::Inferred => "inferred",
        }
    }
}

/// What was resolved from the environment and the flags, before any request.
#[derive(Debug, Clone)]
pub struct Config {
    /// The endpoint requests go to.
    pub endpoint: String,
    /// The team key asked for, and where it came from; `None` means infer.
    pub team: Option<(String, TeamSource)>,
    /// The path the key is read from when `LINEAR_API_KEY` is unset, when a home or
    /// `MUSTER_KEY_FILE` gave one.
    pub key_path: Option<PathBuf>,
    /// Whether `MUSTER_KEY_FILE` named the path.
    pub key_path_from_env: bool,
    /// Print every request and answer to stderr.
    pub trace: bool,
}

impl Config {
    /// Resolve everything but the key itself from the flags and the environment.
    pub fn load(team_flag: Option<String>, trace: bool) -> Config {
        let endpoint = non_empty("MUSTER_ENDPOINT").unwrap_or_else(|| LINEAR_ENDPOINT.to_string());
        let team = match team_flag {
            Some(key) => Some((key, TeamSource::Flag)),
            None => non_empty("MUSTER_TEAM").map(|key| (key, TeamSource::Env)),
        };
        let (key_path, key_path_from_env) = match non_empty("MUSTER_KEY_FILE") {
            Some(path) => (Some(PathBuf::from(path)), true),
            None => (default_key_path(), false),
        };
        Config {
            endpoint,
            team,
            key_path,
            key_path_from_env,
            trace,
        }
    }

    /// The key and where it came from: `LINEAR_API_KEY`, else the key file, trimmed.
    pub fn key(&self) -> Result<(String, KeySource), Error> {
        if let Some(key) = non_empty("LINEAR_API_KEY") {
            return Ok((key.trim().to_string(), KeySource::Env));
        }
        let Some(path) = &self.key_path else {
            return Err(Error::new(
                ErrorCode::Authentication,
                "no key: LINEAR_API_KEY is unset, MUSTER_KEY_FILE is unset and no home directory is known",
            )
            .at("the-key"));
        };
        match std::fs::read_to_string(path) {
            Ok(text) if !text.trim().is_empty() => {
                Ok((text.trim().to_string(), KeySource::File(path.clone())))
            }
            Ok(_) => Err(Error::new(
                ErrorCode::Authentication,
                format!("the key file {} is empty", path.display()),
            )
            .at("the-key")),
            Err(e) if self.key_path_from_env => Err(Error::new(
                ErrorCode::Authentication,
                format!(
                    "MUSTER_KEY_FILE names {}, which cannot be read: {e}",
                    path.display()
                ),
            )
            .at("the-key")),
            Err(_) => Err(Error::new(
                ErrorCode::Authentication,
                format!(
                    "no key: LINEAR_API_KEY is unset and {} does not exist",
                    path.display()
                ),
            )
            .at("the-key")),
        }
    }

    /// Whether the endpoint is Linear's own.
    pub fn endpoint_is_linear(&self) -> bool {
        self.endpoint == LINEAR_ENDPOINT
    }
}

fn non_empty(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.trim().is_empty())
}

/// `$XDG_CONFIG_HOME/muster/key`, else `~/.config/muster/key`; `None` without a home.
fn default_key_path() -> Option<PathBuf> {
    if let Some(xdg) = non_empty("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(xdg).join("muster").join("key"));
    }
    let home = non_empty("HOME").or_else(|| non_empty("USERPROFILE"))?;
    Some(
        PathBuf::from(home)
            .join(".config")
            .join("muster")
            .join("key"),
    )
}
