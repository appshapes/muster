//! Reading what muster needs out of an answer, and building Linear's filter objects.
//! The answer itself is printed as Linear returned it; these helpers only pick ids and
//! names out of it (ADR 0008).

use serde_json::{Map, Value, json};

use crate::error::{Error, ErrorCode};

/// The value at a dotted path, `issue.state.name`, or `Null`.
pub fn at<'a>(value: &'a Value, path: &str) -> &'a Value {
    let mut cur = value;
    for key in path.split('.') {
        cur = match cur {
            Value::Object(map) => map.get(key).unwrap_or(&Value::Null),
            Value::Array(items) => key
                .parse::<usize>()
                .ok()
                .and_then(|i| items.get(i))
                .unwrap_or(&Value::Null),
            _ => &Value::Null,
        };
    }
    cur
}

/// The string at a dotted path, or `None`.
pub fn str_at<'a>(value: &'a Value, path: &str) -> Option<&'a str> {
    at(value, path).as_str()
}

/// The string at a dotted path, or an exit-1 failure naming what was missing: an
/// answer without a field muster needs is an answer it does not understand.
pub fn need_str(value: &Value, path: &str, what: &str) -> Result<String, Error> {
    str_at(value, path).map(str::to_string).ok_or_else(|| {
        Error::new(
            ErrorCode::Failed,
            format!("Linear's answer to {what} has no `{path}`"),
        )
    })
}

/// The `nodes` array at a dotted path, or an empty list.
pub fn nodes(value: &Value, path: &str) -> Vec<Value> {
    at(value, &format!("{path}.nodes"))
        .as_array()
        .cloned()
        .unwrap_or_default()
}

/// Whether the connection at a dotted path has more pages.
pub fn has_next_page(value: &Value, path: &str) -> bool {
    at(value, &format!("{path}.pageInfo.hasNextPage"))
        .as_bool()
        .unwrap_or(false)
}

/// Case-insensitive match of `name` against `field` of each node; the first hit.
pub fn find_by<'a>(items: &'a [Value], field: &str, name: &str) -> Option<&'a Value> {
    items
        .iter()
        .find(|item| str_at(item, field).is_some_and(|v| v.eq_ignore_ascii_case(name)))
}

/// The `field` of every node, for a hint that lists what exists.
pub fn names(items: &[Value], field: &str) -> String {
    let list: Vec<&str> = items
        .iter()
        .filter_map(|item| str_at(item, field))
        .collect();
    if list.is_empty() {
        "nothing".to_string()
    } else {
        list.join(", ")
    }
}

/// A filter object, built key by key; empty when nothing was added.
#[derive(Debug, Default, Clone)]
pub struct Filter(Map<String, Value>);

impl Filter {
    /// An empty filter.
    pub fn new() -> Filter {
        Filter::default()
    }

    /// Add one clause.
    pub fn with(mut self, key: &str, clause: Value) -> Filter {
        self.0.insert(key.to_string(), clause);
        self
    }

    /// The team clause, `{ team: { key: { eq } } }`.
    pub fn team(self, key: &str) -> Filter {
        self.with("team", json!({ "key": { "eq": key } }))
    }

    /// The filter as a value, or `Null` when empty, which Linear reads as no filter.
    pub fn build(self) -> Value {
        if self.0.is_empty() {
            Value::Null
        } else {
            Value::Object(self.0)
        }
    }
}

/// Whether a reference looks like a Linear id (a UUID).
pub fn is_uuid(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    parts.len() == 5
        && parts
            .iter()
            .zip([8, 4, 4, 4, 12])
            .all(|(p, n)| p.len() == n && p.chars().all(|c| c.is_ascii_hexdigit()))
}

/// Whether a reference looks like an identifier, `APP-12`.
pub fn is_identifier(s: &str) -> bool {
    match s.rsplit_once('-') {
        Some((key, number)) => {
            !key.is_empty()
                && key.chars().all(|c| c.is_ascii_alphanumeric())
                && !number.is_empty()
                && number.chars().all(|c| c.is_ascii_digit())
        }
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_and_nodes() {
        let v = json!({ "issue": { "state": { "name": "Done" }, "labels": { "nodes": [{ "name": "Bug" }], "pageInfo": { "hasNextPage": true } } } });
        assert_eq!(str_at(&v, "issue.state.name"), Some("Done"));
        assert_eq!(str_at(&v, "issue.state.nope"), None);
        assert_eq!(nodes(&v, "issue.labels").len(), 1);
        assert!(has_next_page(&v, "issue.labels"));
        assert!(!has_next_page(&v, "issue.state"));
        assert!(need_str(&v, "issue.missing", "the test").is_err());
    }

    #[test]
    fn references() {
        assert!(is_uuid("1f979a91-84ec-4817-9397-3cdc6ca67612"));
        assert!(!is_uuid("APP-12"));
        assert!(is_identifier("APP-12"));
        assert!(is_identifier("app-1"));
        assert!(!is_identifier("12"));
        assert!(!is_identifier("APP-"));
        assert!(!is_identifier("1f979a91-84ec-4817-9397-3cdc6ca67612"));
    }

    #[test]
    fn filters() {
        assert_eq!(Filter::new().build(), Value::Null);
        let f = Filter::new()
            .team("APP")
            .with("number", json!({ "eq": 12 }))
            .build();
        assert_eq!(at(&f, "team.key.eq"), "APP");
        assert_eq!(at(&f, "number.eq"), 12);
    }
}
