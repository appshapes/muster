//! The one module that writes to stdout and stderr, and the one that reads a body
//! from a file or from stdin named `-` (ADR 0008). Everything else returns values.

use std::io::{Read, Write};

use serde_json::Value;

use crate::bounds::BODY_MAX_BYTES;
use crate::error::{Error, ErrorCode};

/// How the answer is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// One JSON object per line. The default.
    Json,
    /// Text for a person, the same data.
    Human,
}

/// The writer.
#[derive(Debug, Clone, Copy)]
pub struct Output {
    /// The format.
    pub format: Format,
}

impl Output {
    /// One answer: a JSON line, or its text rendering.
    #[expect(
        clippy::print_stdout,
        reason = "the one place the answer is written (ADR 0008)"
    )]
    pub fn emit(&self, value: &Value) {
        match self.format {
            Format::Json => println!("{value}"),
            Format::Human => print!("{}", render(value)),
        }
        let _ = std::io::stdout().flush();
    }

    /// Text that is the whole answer, such as the reference page: written as it is.
    #[expect(
        clippy::print_stdout,
        reason = "the one place a text answer is written (ADR 0008)"
    )]
    pub fn emit_text(&self, text: &str) {
        print!("{text}");
        let _ = std::io::stdout().flush();
    }

    /// The error object, on stderr, in the same format as the answer.
    #[expect(
        clippy::print_stderr,
        reason = "the one place the error object is written (ADR 0008)"
    )]
    pub fn error(&self, error: &Error) {
        match self.format {
            Format::Json => eprintln!("{}", error.to_json()),
            Format::Human => eprint!("{}", render(&error.to_json())),
        }
    }

    /// A notice, on stderr: a list cut at its limit, a thread cut at its bound.
    #[expect(
        clippy::print_stderr,
        reason = "the one place a notice is written (ADR 0008)"
    )]
    pub fn notice(&self, code: &str, message: &str, hint: &str) {
        let value =
            serde_json::json!({ "notice": { "code": code, "message": message, "hint": hint } });
        match self.format {
            Format::Json => eprintln!("{value}"),
            Format::Human => eprint!("{}", render(&value)),
        }
    }

    /// One `--trace` line, on stderr.
    #[expect(
        clippy::print_stderr,
        reason = "the one place a trace line is written (ADR 0008)"
    )]
    pub fn trace(line: &str) {
        eprintln!("{line}");
    }
}

/// A body from a file, or from stdin when the path is `-`; at most [`BODY_MAX_BYTES`].
/// Reading stdin to its end is not a prompt: nothing is asked and nothing waits on a
/// person when stdin is a file or a pipe, which is what an agent gives it.
#[expect(
    clippy::disallowed_methods,
    reason = "stdin read to EOF as a body, never a prompt (ADR 0008)"
)]
pub fn read_body(path: &str) -> Result<String, Error> {
    let limit = BODY_MAX_BYTES as u64 + 1;
    let mut text = String::new();
    let read = if path == "-" {
        std::io::stdin()
            .lock()
            .take(limit)
            .read_to_string(&mut text)
    } else {
        match std::fs::File::open(path) {
            Ok(file) => file.take(limit).read_to_string(&mut text),
            Err(e) => {
                return Err(Error::with_hint(
                    ErrorCode::Usage,
                    format!("the body file {path} cannot be read: {e}"),
                    "name a file that exists, or `-` to read the body from stdin",
                ));
            }
        }
    };
    if let Err(e) = read {
        return Err(Error::new(
            ErrorCode::Usage,
            format!("the body {path} is not UTF-8 text: {e}"),
        ));
    }
    check_body(Text::Body, &text)?;
    Ok(text)
}

/// What [`check_body`] bounds: the word the message names it by, and the hint that
/// fits it, since a title cannot be split over several comments as a body can.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Text {
    /// A description or a comment: `--description`, `--body`, `--comment`, or the file
    /// or stdin their `-file` twins read.
    Body,
    /// An issue's or a link's `--title`.
    Title,
    /// A link's `--url`, or the workspace's `--logo-url`.
    Link,
    /// A link's `--subtitle`.
    Subtitle,
    /// A project's or the workspace's `--name`.
    Name,
}

impl Text {
    /// The words the message names it by.
    pub fn name(self) -> &'static str {
        match self {
            Text::Body => "the body",
            Text::Title => "the title",
            Text::Link => "the link",
            Text::Subtitle => "the subtitle",
            Text::Name => "the name",
        }
    }

    /// What to do when it is over the bound.
    pub fn hint(self) -> &'static str {
        match self {
            Text::Body => "shorten the body, or split it over several comments",
            Text::Title => {
                "shorten the title; the detail belongs in the description or in a comment"
            }
            Text::Link => "shorten the link; muster sends a URL as it is and never cuts one",
            Text::Subtitle => "shorten the subtitle; the detail belongs in a comment on the issue",
            Text::Name => "shorten the name; the detail belongs in the description",
        }
    }
}

/// Refuse a text over [`BODY_MAX_BYTES`], before any request (invariant 3). The
/// message names what was too long and the hint says what to do with it.
pub fn check_body(what: Text, text: &str) -> Result<(), Error> {
    if text.len() > BODY_MAX_BYTES {
        return Err(Error::with_hint(
            ErrorCode::Refused,
            format!(
                "{} is {} bytes; BODY_MAX_BYTES is {BODY_MAX_BYTES}",
                what.name(),
                text.len()
            ),
            what.hint(),
        )
        .at("bounds"));
    }
    Ok(())
}

/// The text rendering of a value: keys and values, nested by indentation, lists as
/// `- ` items. The same data as the JSON line, nothing added.
pub fn render(value: &Value) -> String {
    let mut out = String::new();
    render_into(value, 0, &mut out);
    out
}

fn render_into(value: &Value, depth: usize, out: &mut String) {
    let pad = "  ".repeat(depth);
    match value {
        Value::Object(map) => {
            for (key, item) in map {
                match item {
                    Value::Object(inner) if !inner.is_empty() => {
                        out.push_str(&format!("{pad}{key}:\n"));
                        render_into(item, depth + 1, out);
                    }
                    Value::Array(items) if !items.is_empty() => {
                        out.push_str(&format!("{pad}{key}:\n"));
                        render_into(item, depth + 1, out);
                    }
                    _ => out.push_str(&format!("{pad}{key}: {}\n", scalar(item))),
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                match item {
                    Value::Object(_) | Value::Array(_) => {
                        out.push_str(&format!("{pad}-\n"));
                        render_into(item, depth + 1, out);
                    }
                    _ => out.push_str(&format!("{pad}- {}\n", scalar(item))),
                }
            }
        }
        _ => out.push_str(&format!("{pad}{}\n", scalar(value))),
    }
}

fn scalar(value: &Value) -> String {
    match value {
        Value::String(s) if s.contains('\n') => {
            let mut lines = s.lines();
            let first = lines.next().unwrap_or_default().to_string();
            let rest: Vec<String> = lines.map(|l| format!("    {l}")).collect();
            if rest.is_empty() {
                first
            } else {
                format!("{first}\n{}", rest.join("\n"))
            }
        }
        Value::String(s) => s.clone(),
        Value::Null => "null".to_string(),
        Value::Object(map) if map.is_empty() => "{}".to_string(),
        Value::Array(items) if items.is_empty() => "[]".to_string(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_text_over_the_bound_is_refused_by_the_name_the_caller_gave_it() {
        let at_bound = "x".repeat(BODY_MAX_BYTES);
        let over = "x".repeat(BODY_MAX_BYTES + 1);
        for what in [
            Text::Body,
            Text::Title,
            Text::Link,
            Text::Subtitle,
            Text::Name,
        ] {
            assert_eq!(check_body(what, &at_bound), Ok(()), "{what:?} at the bound");
            let e = check_body(what, &over).err();
            assert_eq!(e.as_ref().map(|e| e.code), Some(ErrorCode::Refused));
            assert_eq!(e.as_ref().map(|e| e.anchor), Some("bounds"));
            let message = e.as_ref().map(|e| e.message.as_str()).unwrap_or_default();
            assert!(
                message.starts_with(what.name()) && message.contains("BODY_MAX_BYTES"),
                "{what:?}: {message}"
            );
            let hint = e.as_ref().map(|e| e.hint.as_str()).unwrap_or_default();
            assert!(
                hint.starts_with(&format!("shorten {}", what.name())),
                "{what:?}: {hint}"
            );
            assert_eq!(
                hint.contains("several comments"),
                what == Text::Body,
                "only a body can be split over comments: {what:?}: {hint}"
            );
        }
    }
}
