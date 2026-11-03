//! The client: one named operation per request, the key in the `Authorization`
//! header, the answer as Linear gave it, and every failure classified into an
//! [`ErrorCode`] (ADR 0002, ADR 0008). No retry, one timeout (invariant 3).

pub mod operations;
pub mod select;

use std::time::Duration;

use serde_json::{Value, json};

use crate::bounds::{REQUEST_TIMEOUT_SECS, RESPONSE_MAX_BYTES};
use crate::config::{Config, KeySource};
use crate::error::{Error, ErrorCode, LinearError};
use crate::output::Output;
pub use operations::{Kind, Operation};

/// A connection to one endpoint with one key.
pub struct Linear {
    agent: ureq::Agent,
    endpoint: String,
    key: String,
    /// Where the key came from, for `whoami`.
    pub key_source: KeySource,
    trace: bool,
}

impl std::fmt::Debug for Linear {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Linear")
            .field("endpoint", &self.endpoint)
            .field("key_source", &self.key_source)
            .finish_non_exhaustive()
    }
}

impl Linear {
    /// Read the key as the configuration says and prepare the agent. No request yet.
    pub fn connect(config: &Config) -> Result<Linear, Error> {
        let (key, key_source) = config.key()?;
        let agent_config = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(REQUEST_TIMEOUT_SECS)))
            .max_redirects(0)
            .user_agent(concat!("muster/", env!("CARGO_PKG_VERSION")))
            .build();
        Ok(Linear {
            agent: agent_config.new_agent(),
            endpoint: config.endpoint.clone(),
            key,
            key_source,
            trace: config.trace,
        })
    }

    /// Run one operation and return `data`, or the classified failure.
    pub fn run(&self, op: &Operation, variables: Value) -> Result<Value, Error> {
        let request =
            json!({ "query": op.document(), "operationName": op.name, "variables": variables });
        if self.trace {
            Output::trace(&format!(
                "> POST {} {} {}",
                self.endpoint, op.name, variables
            ));
        }
        let body = request.to_string();
        let sent = self
            .agent
            .post(&self.endpoint)
            .header("Authorization", &self.key)
            .header("Content-Type", "application/json")
            .send(body.as_bytes());
        let mut response = match sent {
            Ok(response) => response,
            Err(e) => return Err(classify_transport(&e, op)),
        };
        let status = response.status().as_u16();
        let header = |name: &str| {
            response
                .headers()
                .get(name)
                .and_then(|v| v.to_str().ok())
                .map(str::to_string)
        };
        let request_id = header("x-request-id");
        let reset = header("x-ratelimit-requests-reset");
        let retry_after = header("retry-after");
        let text = match response
            .body_mut()
            .with_config()
            .limit(RESPONSE_MAX_BYTES)
            .read_to_string()
        {
            Ok(text) => text,
            Err(e) => return Err(classify_transport(&e, op)),
        };
        if self.trace {
            Output::trace(&format!(
                "< {status} {} {}",
                request_id.as_deref().unwrap_or("-"),
                text
            ));
        }
        let answer: Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(_) if status >= 500 => return Err(server_error(status, op, request_id)),
            Err(e) => {
                return Err(Error::new(
                    ErrorCode::Failed,
                    format!("Linear answered {status} with a body that is not JSON: {e}"),
                ));
            }
        };
        let first_error = answer
            .get("errors")
            .and_then(Value::as_array)
            .and_then(|a| a.first())
            .cloned();
        if let Some(err) = first_error {
            let linear = LinearError {
                code: select::str_at(&err, "extensions.code").map(str::to_string),
                message: select::str_at(&err, "message")
                    .unwrap_or("no message")
                    .to_string(),
                request_id: request_id.clone(),
            };
            return Err(classify_linear(status, linear, reset, retry_after, op));
        }
        if status == 429 {
            let linear = LinearError {
                code: Some("RATELIMITED".to_string()),
                message: "HTTP 429".to_string(),
                request_id,
            };
            return Err(rate_limited(linear, reset, retry_after));
        }
        if status >= 500 {
            return Err(server_error(status, op, request_id));
        }
        if status >= 400 {
            return Err(Error::new(
                ErrorCode::Failed,
                format!("Linear answered {status} with no error object"),
            )
            .from_linear(LinearError {
                code: None,
                message: text.chars().take(200).collect(),
                request_id,
            }));
        }
        match answer.get("data") {
            Some(data) if !data.is_null() => Ok(data.clone()),
            _ => Err(Error::new(
                ErrorCode::Failed,
                format!("Linear's answer to {} has no data and no errors", op.name),
            )),
        }
    }
}

fn classify_linear(
    status: u16,
    linear: LinearError,
    reset: Option<String>,
    retry_after: Option<String>,
    op: &Operation,
) -> Error {
    let code = linear.code.as_deref().unwrap_or("");
    if status == 401 || code == "AUTHENTICATION_ERROR" {
        return Error::with_hint(
            ErrorCode::Authentication,
            format!("Linear refused the key: {}", linear.message),
            "check the key in LINEAR_API_KEY or the key file (docs/reference/commands.md#the-key); `muster whoami` says which was read",
        )
        .at("the-key")
        .from_linear(linear);
    }
    if status == 429 || code == "RATELIMITED" {
        return rate_limited(linear, reset, retry_after);
    }
    if linear.message.starts_with("Entity not found") {
        let what = linear
            .message
            .trim_start_matches("Entity not found:")
            .trim()
            .to_string();
        return Error::new(ErrorCode::NotFound, format!("Linear has no such {what}"))
            .from_linear(linear);
    }
    if code == "INPUT_ERROR"
        || code == "INVALID_INPUT"
        || code == "BAD_USER_INPUT"
        || linear.code.is_none() && status == 200
    {
        return Error::new(
            ErrorCode::Refused,
            format!("Linear refused the input: {}", linear.message),
        )
        .from_linear(linear);
    }
    if code.starts_with("GRAPHQL_") {
        return Error::new(
            ErrorCode::Failed,
            format!(
                "Linear rejected muster's {} operation: {}",
                op.name, linear.message
            ),
        )
        .from_linear(linear);
    }
    if status >= 500 {
        return server_error(status, op, linear.request_id);
    }
    Error::new(
        ErrorCode::Failed,
        format!("Linear answered {status} {code}: {}", linear.message),
    )
    .from_linear(linear)
}

fn rate_limited(linear: LinearError, reset: Option<String>, retry_after: Option<String>) -> Error {
    let when = match (
        reset.as_deref().and_then(|r| r.parse::<u64>().ok()),
        retry_after,
    ) {
        (Some(ms), _) => format!(
            "the limit resets at {} (Unix milliseconds {ms})",
            iso_from_millis(ms)
        ),
        (None, Some(secs)) => format!("retry after {secs} seconds"),
        (None, None) => "no reset time was given".to_string(),
    };
    Error::with_hint(
        ErrorCode::RateLimited,
        format!("Linear's rate limit: {when}"),
        "wait until the reset time, then run the command again; muster never waits for you",
    )
    .from_linear(linear)
}

fn server_error(status: u16, op: &Operation, request_id: Option<String>) -> Error {
    let linear = LinearError {
        code: None,
        message: format!("HTTP {status}"),
        request_id,
    };
    match op.kind {
        Kind::Mutation => Error::new(
            ErrorCode::Unknown,
            format!(
                "Linear answered {status} to {}: the request may have landed",
                op.name
            ),
        )
        .from_linear(linear),
        Kind::Query => Error::with_hint(
            ErrorCode::Failed,
            format!("Linear answered {status} to {}", op.name),
            "Linear's side; run the command again in a moment",
        )
        .from_linear(linear),
    }
}

/// A transport failure: before the request was sent it is a network failure, exit 7;
/// after, a query is still exit 7 (nothing changed) and a mutation is exit 8.
fn classify_transport(e: &ureq::Error, op: &Operation) -> Error {
    let (sent, what) = match e {
        ureq::Error::HostNotFound => (false, "the host name does not resolve".to_string()),
        ureq::Error::ConnectionFailed => (false, "the connection failed".to_string()),
        ureq::Error::Tls(_)
        | ureq::Error::Rustls(_)
        | ureq::Error::Pem(_)
        | ureq::Error::TlsRequired
        | ureq::Error::RequireHttpsOnly(_) => (false, format!("TLS failed: {e}")),
        ureq::Error::BadUri(_)
        | ureq::Error::InvalidProxyUrl
        | ureq::Error::ConnectProxyFailed(_) => (false, format!("the endpoint is unusable: {e}")),
        ureq::Error::Timeout(timeout) => {
            let before = matches!(
                timeout,
                ureq::Timeout::Resolve | ureq::Timeout::Connect | ureq::Timeout::SendRequest
            );
            (
                !before,
                format!("timed out ({timeout}) after {REQUEST_TIMEOUT_SECS} seconds"),
            )
        }
        ureq::Error::Io(io) => {
            let before = matches!(
                io.kind(),
                std::io::ErrorKind::ConnectionRefused
                    | std::io::ErrorKind::NotFound
                    | std::io::ErrorKind::AddrNotAvailable
                    | std::io::ErrorKind::NotConnected
            );
            (!before, format!("the connection broke: {io}"))
        }
        ureq::Error::BodyExceedsLimit(_) => {
            return Error::new(
                ErrorCode::Failed,
                format!(
                    "Linear's answer to {} is over RESPONSE_MAX_BYTES ({RESPONSE_MAX_BYTES} bytes)",
                    op.name
                ),
            )
            .at("bounds");
        }
        other => (true, format!("{other}")),
    };
    match (op.kind, sent) {
        (Kind::Mutation, true) => Error::new(
            ErrorCode::Unknown,
            format!(
                "{} was sent and no answer came: {what}; the request may have landed",
                op.name
            ),
        ),
        (Kind::Query, _) | (Kind::Mutation, false) => Error::with_hint(
            ErrorCode::Network,
            format!("{} was not answered: {what}", op.name),
            "check the network and MUSTER_ENDPOINT; nothing changed in Linear, so the command can be run again as it is",
        ),
    }
}

/// The current time as `YYYY-MM-DDTHH:MM:SSZ`, for a timestamp muster sends, such as a
/// team's `retiredAt` (ADR 0014, group 2).
pub fn now_iso() -> String {
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    iso_from_millis(ms)
}

/// `YYYY-MM-DDTHH:MM:SSZ` from Unix milliseconds, with no date crate (ADR 0002).
fn iso_from_millis(ms: u64) -> String {
    let secs = ms / 1000;
    let (days, rem) = (secs / 86_400, secs % 86_400);
    let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    // Civil-from-days, Howard Hinnant's algorithm.
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if mo <= 2 { y + 1 } else { y };
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{m:02}:{s:02}Z")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_dates() {
        assert_eq!(iso_from_millis(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso_from_millis(1_793_670_142_609), "2026-11-03T01:42:22Z");
    }

    #[test]
    fn entity_not_found_is_exit_4_and_input_error_is_exit_6() {
        let nf = LinearError {
            code: Some("INPUT_ERROR".into()),
            message: "Entity not found: Issue".into(),
            request_id: None,
        };
        assert_eq!(
            classify_linear(200, nf, None, None, &operations::ISSUE_BY_ID).code,
            ErrorCode::NotFound
        );
        let refused = LinearError {
            code: Some("INPUT_ERROR".into()),
            message: "title is required".into(),
            request_id: None,
        };
        assert_eq!(
            classify_linear(200, refused, None, None, &operations::ISSUE_CREATE).code,
            ErrorCode::Refused
        );
        let auth = LinearError {
            code: Some("AUTHENTICATION_ERROR".into()),
            message: "no".into(),
            request_id: None,
        };
        assert_eq!(
            classify_linear(401, auth, None, None, &operations::VIEWER).code,
            ErrorCode::Authentication
        );
        let rl = LinearError {
            code: Some("RATELIMITED".into()),
            message: "slow".into(),
            request_id: None,
        };
        let e = classify_linear(
            429,
            rl,
            Some("1793670142609".into()),
            None,
            &operations::VIEWER,
        );
        assert_eq!(e.code, ErrorCode::RateLimited);
        assert!(e.message.contains("2026-11-03T01:42:22Z"), "{}", e.message);
        let gql = LinearError {
            code: Some("GRAPHQL_VALIDATION_FAILED".into()),
            message: "bad field".into(),
            request_id: None,
        };
        assert_eq!(
            classify_linear(400, gql, None, None, &operations::VIEWER).code,
            ErrorCode::Failed
        );
    }

    #[test]
    fn server_errors_depend_on_the_kind() {
        assert_eq!(
            server_error(502, &operations::ISSUES, None).code,
            ErrorCode::Failed
        );
        assert_eq!(
            server_error(502, &operations::ISSUE_CREATE, None).code,
            ErrorCode::Unknown
        );
    }
}
