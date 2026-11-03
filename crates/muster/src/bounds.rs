//! Every limit muster has, named (CLAUDE.md, invariant 3; ADR 0008). `muster schema`
//! lists them, and the generated reference carries the same table. A list that hits
//! `PAGE_CAP` says so with a notice on stderr and exits 0 with what it has.

/// One bound, as `muster schema` prints it.
#[derive(Debug, Clone, Copy)]
pub struct Bound {
    /// The constant's name, as written in this file.
    pub name: &'static str,
    /// Its value.
    pub value: u64,
    /// The unit of the value: `items`, `seconds`, `bytes` or `attempts`.
    pub unit: &'static str,
    /// What it bounds and what happens at the bound.
    pub meaning: &'static str,
}

/// The default `--limit` of every list command.
pub const PAGE_SIZE: usize = 50;
/// The largest `--limit` a list command accepts, and the largest `first` Linear serves
/// in one page. A larger value is a usage error, exit 2.
pub const PAGE_CAP: usize = 250;
/// How many comments `issue get` fetches with an issue, newest first, as Linear orders
/// a connection. A longer thread is reported cut with a notice on stderr.
pub const COMMENTS_PER_ISSUE: usize = 100;
/// How many attachments `issue get` fetches with an issue, newest first, as Linear
/// orders a connection. An issue with more is reported cut with a notice on stderr.
pub const ATTACHMENTS_PER_ISSUE: usize = 50;
/// How many sub-issues `issue get` fetches with an issue, newest first, as Linear orders
/// a connection. A parent with more is reported cut with a notice on stderr.
pub const CHILDREN_PER_ISSUE: usize = 50;
/// How long one request may take, connect to last byte. Past it a query exits 7 and a
/// mutation exits 8.
pub const REQUEST_TIMEOUT_SECS: u64 = 30;
/// The largest description, comment, title, link or subtitle muster sends. A larger
/// one is refused, exit 6, before any request, and the message names which one it was.
pub const BODY_MAX_BYTES: usize = 256 * 1024;
/// The largest answer muster reads. A larger one is muster's failure, exit 1.
pub const RESPONSE_MAX_BYTES: u64 = 16 * 1024 * 1024;
/// How many times a failed request is sent again. Zero: a rate limit or a network
/// failure is reported with its exit code, and the caller decides.
pub const RETRIES: u64 = 0;

/// Every bound above, for `muster schema`.
pub const ALL: &[Bound] = &[
    Bound {
        name: "PAGE_SIZE",
        value: PAGE_SIZE as u64,
        unit: "items",
        meaning: "the default --limit of every list command",
    },
    Bound {
        name: "PAGE_CAP",
        value: PAGE_CAP as u64,
        unit: "items",
        meaning: "the largest --limit a list accepts, which is Linear's largest page; a larger value is a usage error, exit 2; a list cut at its limit writes a `truncated` notice to stderr and exits 0",
    },
    Bound {
        name: "COMMENTS_PER_ISSUE",
        value: COMMENTS_PER_ISSUE as u64,
        unit: "items",
        meaning: "the comments `issue get` fetches with an issue, newest first, as Linear orders them; a longer thread writes a `truncated` notice to stderr and the oldest are the ones left out",
    },
    Bound {
        name: "ATTACHMENTS_PER_ISSUE",
        value: ATTACHMENTS_PER_ISSUE as u64,
        unit: "items",
        meaning: "the attachments `issue get` fetches with an issue, newest first, as Linear orders them; an issue with more writes a `truncated` notice to stderr and the oldest are the ones left out",
    },
    Bound {
        name: "CHILDREN_PER_ISSUE",
        value: CHILDREN_PER_ISSUE as u64,
        unit: "items",
        meaning: "the sub-issues `issue get` fetches with an issue, newest first, as Linear orders them; a parent with more writes a `truncated` notice to stderr and the oldest are the ones left out",
    },
    Bound {
        name: "REQUEST_TIMEOUT_SECS",
        value: REQUEST_TIMEOUT_SECS,
        unit: "seconds",
        meaning: "one request, connect to last byte; past it a query exits 7 and a mutation exits 8",
    },
    Bound {
        name: "BODY_MAX_BYTES",
        value: BODY_MAX_BYTES as u64,
        unit: "bytes",
        meaning: "the largest description, comment, title, link or subtitle sent; a larger one is refused, exit 6, before any request, and the message names which one it was",
    },
    Bound {
        name: "RESPONSE_MAX_BYTES",
        value: RESPONSE_MAX_BYTES,
        unit: "bytes",
        meaning: "the largest answer read; a larger one is muster's failure, exit 1",
    },
    Bound {
        name: "RETRIES",
        value: RETRIES,
        unit: "attempts",
        meaning: "how many times a failed request is sent again: never; the exit code tells the caller what happened and the caller decides",
    },
];
