//! Every GraphQL operation muster sends, named (CLAUDE.md, "Linear's names"; ADR 0002).
//! The fake Linear answers by the name, and `just record-fixtures` records each read
//! operation's shape by the name (ADR 0010). One operation per command where Linear
//! allows it; a new one is appended at the end of its section.

/// Whether an operation reads or writes: a write whose answer never came exits 8.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A read. A failure to get its answer is a network failure, exit 7.
    Query,
    /// A write. A failure to get its answer is an unknown outcome, exit 8.
    Mutation,
}

/// One named operation.
#[derive(Debug, Clone, Copy)]
pub struct Operation {
    /// The `operationName` sent with the request.
    pub name: &'static str,
    /// Read or write.
    pub kind: Kind,
    /// The query or mutation, without its fragments.
    pub body: &'static str,
    /// The fragments the body uses, appended to it when sent.
    pub fragments: &'static [&'static str],
}

impl Operation {
    /// The document as sent: the body, then each fragment.
    pub fn document(&self) -> String {
        let mut doc = String::from(self.body);
        for fragment in self.fragments {
            doc.push('\n');
            doc.push_str(fragment);
        }
        doc
    }
}

/// The fields every issue answer carries. Linear's names, as they are (ADR 0008).
pub const ISSUE_FIELDS: &str = "fragment IssueFields on Issue {
  id identifier number title description url priority priorityLabel estimate dueDate branchName
  createdAt updatedAt startedAt completedAt canceledAt archivedAt
  state { id name type }
  team { id key name }
  project { id name }
  cycle { id number name }
  assignee { id name displayName email }
  creator { id name displayName }
  parent { id identifier }
  labels { nodes { id name } }
}";

/// The fields every comment answer carries.
pub const COMMENT_FIELDS: &str = "fragment CommentFields on Comment {
  id body url createdAt updatedAt editedAt
  user { id name displayName }
  parent { id }
}";

macro_rules! op {
    ($ident:ident, $name:literal, $kind:expr, $body:expr) => {
        /// See the module documentation.
        pub static $ident: Operation = Operation {
            name: $name,
            kind: $kind,
            body: $body,
            fragments: &[],
        };
    };
    ($ident:ident, $name:literal, $kind:expr, $body:expr, $fragments:expr) => {
        /// See the module documentation.
        pub static $ident: Operation = Operation {
            name: $name,
            kind: $kind,
            body: $body,
            fragments: $fragments,
        };
    };
}

// ---- reads ---------------------------------------------------------------------------

op!(
    VIEWER,
    "Viewer",
    Kind::Query,
    "query Viewer {
  viewer { id name displayName email admin active }
  organization { id name urlKey }
}"
);

op!(
    TEAMS,
    "Teams",
    Kind::Query,
    "query Teams($first: Int!) {
  teams(first: $first) { nodes { id key name description private } pageInfo { hasNextPage } }
}"
);

op!(
    WORKFLOW_STATES,
    "WorkflowStates",
    Kind::Query,
    "query WorkflowStates($first: Int!, $filter: WorkflowStateFilter) {
  workflowStates(first: $first, filter: $filter) {
    nodes { id name type position color description team { id key } }
    pageInfo { hasNextPage }
  }
}"
);

op!(
    ISSUE_LABELS,
    "IssueLabels",
    Kind::Query,
    "query IssueLabels($first: Int!, $filter: IssueLabelFilter) {
  issueLabels(first: $first, filter: $filter) {
    nodes { id name color description isGroup team { id key } parent { id name } }
    pageInfo { hasNextPage }
  }
}"
);

op!(PROJECTS, "Projects", Kind::Query, "query Projects($first: Int!, $filter: ProjectFilter) {
  projects(first: $first, filter: $filter) {
    nodes { id name url description priority startDate targetDate status { id name type } lead { id name displayName } teams { nodes { id key } } }
    pageInfo { hasNextPage }
  }
}");

op!(
    USERS,
    "Users",
    Kind::Query,
    "query Users($first: Int!, $filter: UserFilter) {
  users(first: $first, filter: $filter) {
    nodes { id name displayName email active admin isMe }
    pageInfo { hasNextPage }
  }
}"
);

op!(ISSUE_BY_ID, "IssueById", Kind::Query, "query IssueById($id: String!, $comments: Int!, $attachments: Int!) {
  issue(id: $id) {
    ...IssueFields
    comments(first: $comments, orderBy: createdAt) { nodes { ...CommentFields } pageInfo { hasNextPage } }
    attachments(first: $attachments) { nodes { id title subtitle url sourceType createdAt } pageInfo { hasNextPage } }
  }
}", &[ISSUE_FIELDS, COMMENT_FIELDS]);

op!(ISSUES, "Issues", Kind::Query, "query Issues($first: Int!, $filter: IssueFilter, $orderBy: PaginationOrderBy) {
  issues(first: $first, filter: $filter, orderBy: $orderBy) { nodes { ...IssueFields } pageInfo { hasNextPage } }
}", &[ISSUE_FIELDS]);

// ---- writes --------------------------------------------------------------------------

op!(
    ISSUE_CREATE,
    "IssueCreate",
    Kind::Mutation,
    "mutation IssueCreate($input: IssueCreateInput!) {
  issueCreate(input: $input) { success issue { ...IssueFields } }
}",
    &[ISSUE_FIELDS]
);

op!(
    ISSUE_UPDATE,
    "IssueUpdate",
    Kind::Mutation,
    "mutation IssueUpdate($id: String!, $input: IssueUpdateInput!) {
  issueUpdate(id: $id, input: $input) { success issue { ...IssueFields } }
}",
    &[ISSUE_FIELDS]
);

op!(
    COMMENT_CREATE,
    "CommentCreate",
    Kind::Mutation,
    "mutation CommentCreate($input: CommentCreateInput!) {
  commentCreate(input: $input) { success comment { ...CommentFields issue { id identifier } } }
}",
    &[COMMENT_FIELDS]
);

/// Every read operation, in the order `just record-fixtures` records them.
pub static READS: &[&Operation] = &[
    &VIEWER,
    &TEAMS,
    &WORKFLOW_STATES,
    &ISSUE_LABELS,
    &PROJECTS,
    &USERS,
    &ISSUE_BY_ID,
    &ISSUES,
];

/// Every write operation.
pub static WRITES: &[&Operation] = &[&ISSUE_CREATE, &ISSUE_UPDATE, &COMMENT_CREATE];

/// An operation by its name, for the fixture test and the fake.
pub fn by_name(name: &str) -> Option<&'static Operation> {
    READS
        .iter()
        .chain(WRITES.iter())
        .copied()
        .find(|op| op.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_document_names_its_operation() {
        for op in READS.iter().chain(WRITES.iter()) {
            let document = op.document();
            let head = document.lines().next().unwrap_or_default();
            assert!(
                head.contains(op.name),
                "{} does not name itself: {head}",
                op.name
            );
            let word = match op.kind {
                Kind::Query => "query ",
                Kind::Mutation => "mutation ",
            };
            assert!(
                head.starts_with(word),
                "{} is not a {word}: {head}",
                op.name
            );
        }
    }

    #[test]
    fn every_fragment_used_is_defined() {
        for op in READS.iter().chain(WRITES.iter()) {
            for frag in ["IssueFields", "CommentFields"] {
                let document = op.document();
                let used = document.contains(&format!("...{frag}"));
                let defined = document.contains(&format!("fragment {frag} on"));
                assert!(
                    !used || defined,
                    "{} uses {frag} without defining it",
                    op.name
                );
            }
        }
    }

    #[test]
    fn names_are_unique_and_findable() {
        let mut seen = std::collections::BTreeSet::new();
        for op in READS.iter().chain(WRITES.iter()) {
            assert!(seen.insert(op.name), "{} is listed twice", op.name);
            assert!(by_name(op.name).is_some());
        }
        assert!(by_name("Nope").is_none());
    }
}
