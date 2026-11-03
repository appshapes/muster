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
/// How many teams `project list` and `project create` print with a project, under
/// `teams.nodes`. `project list` reports a project in more as cut with a notice on
/// stderr; a project `project create` makes has the one team, so its page is never cut
/// and the command writes no notice. Bounded so that a page of `PAGE_CAP` projects stays
/// inside Linear's query-complexity limit of 10,000: on 2026-11-03 Linear refused
/// `project list --limit 250` at 20 teams per project as "Query too complex" (complexity
/// 13,525), and accepts it at 10.
pub const TEAMS_PER_PROJECT: usize = 10;
/// How many members `project get` prints with a project, under `members.nodes`. A
/// project with more is reported cut with a notice on stderr, and `project update
/// --add-member` and `--remove-member` refuse to replace a member list they could not
/// read whole (ADR 0014, group 8).
pub const MEMBERS_PER_PROJECT: usize = 50;
/// How many milestones `project get` prints with a project, under
/// `projectMilestones.nodes`. A project with more is reported cut with a notice on
/// stderr.
pub const MILESTONES_PER_PROJECT: usize = 50;
/// How many projects `initiative get` prints with an initiative, under
/// `projects.nodes`. An initiative with more is reported cut with a notice on stderr
/// (ADR 0014, group 12).
pub const PROJECTS_PER_INITIATIVE: usize = 50;
/// How long one request may take, connect to last byte. Past it a query exits 7 and a
/// mutation exits 8.
pub const REQUEST_TIMEOUT_SECS: u64 = 30;
/// The largest description, comment, title, name, link or subtitle muster sends. A
/// larger one is refused, exit 6, before any request, and the message names which one it
/// was.
/// A flag's value is bounded by the OS first: Linux refuses one argv string over
/// 128 KiB (`MAX_ARG_STRLEN`) before muster runs, so from `--title`, `--name`, `--url`,
/// `--subtitle` or `--description` this bound is reached only where the OS allows the
/// argument, as macOS does; from a file or stdin it is reached everywhere.
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
        name: "TEAMS_PER_PROJECT",
        value: TEAMS_PER_PROJECT as u64,
        unit: "items",
        meaning: "the teams `project list` and `project create` print with a project under `teams.nodes`; `project list` writes a `truncated` notice to stderr for a project in more, and a project `project create` makes has the one team, so it is never cut; bounded so that a page of PAGE_CAP projects stays inside Linear's query-complexity limit of 10,000",
    },
    Bound {
        name: "MEMBERS_PER_PROJECT",
        value: MEMBERS_PER_PROJECT as u64,
        unit: "items",
        meaning: "the members `project get` prints with a project under `members.nodes`; a project with more writes a `truncated` notice to stderr, and `project update --add-member` or `--remove-member` on it is refused, exit 6, before any request, since Linear replaces the whole member list and muster could not read it whole",
    },
    Bound {
        name: "MILESTONES_PER_PROJECT",
        value: MILESTONES_PER_PROJECT as u64,
        unit: "items",
        meaning: "the milestones `project get` prints with a project under `projectMilestones.nodes`; a project with more writes a `truncated` notice to stderr",
    },
    Bound {
        name: "PROJECTS_PER_INITIATIVE",
        value: PROJECTS_PER_INITIATIVE as u64,
        unit: "items",
        meaning: "the projects `initiative get` prints with an initiative under `projects.nodes`; an initiative with more writes a `truncated` notice to stderr",
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
        meaning: "the largest description, comment, title, name, link or subtitle sent; a larger one is refused, exit 6, before any request, and the message names which one it was; Linux refuses one argv string over 128 KiB before muster runs, so from a flag's value the bound is reached only where the OS allows the argument, from a file or stdin everywhere",
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
