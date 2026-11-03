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

/// The fields every project answer carries.
pub const PROJECT_FIELDS: &str = "fragment ProjectFields on Project {
  id name url description priority startDate targetDate
  status { id name type }
  lead { id name displayName }
  teams(first: $teams) { nodes { id key } pageInfo { hasNextPage } }
}";

/// The fields every workspace answer carries (ADR 0014, group 1): the settings
/// `workspace update` can change, the security and auth settings as Linear's JSON
/// objects, the counts and the subscription.
pub const ORGANIZATION_FIELDS: &str = "fragment OrganizationFields on Organization {
  id name urlKey logoUrl createdAt updatedAt
  gitBranchFormat gitLinkbackMessagesEnabled gitPublicLinkbackMessagesEnabled gitLinkbackDescriptionsEnabled suggestedBranchNameEnabled
  roadmapEnabled feedEnabled pullRequestTourEnabled pullRequestIssueMode aiThreadSummariesEnabled aiDiscussionSummariesEnabled
  projectUpdateReminderFrequencyInWeeks projectUpdateRemindersDay projectUpdateRemindersHour
  initiativeUpdateReminderFrequencyInWeeks initiativeUpdateRemindersDay initiativeUpdateRemindersHour
  fiscalYearStartMonth defaultHomeView
  samlEnabled scimEnabled hipaaComplianceEnabled releaseChannel
  securitySettings authSettings allowedFileUploadContentTypes
  previousUrlKeys deletionRequestedAt trialStartsAt trialEndsAt
  userCount createdIssueCount
  subscription { id type seats seatsMinimum seatsMaximum collectionMethod nextBillingAt canceledAt cancelAt pendingChangeType }
}";

/// The fields every team answer carries (ADR 0014, group 2): every setting
/// `team update` can change, read back, with the team's states and parent by id and
/// name. `private` and `joinByDefault` are left out: Linear marks the first deprecated
/// on `Team` (`visibility` replaces it) and the second Internal.
pub const TEAM_FIELDS: &str = "fragment TeamFields on Team {
  id key name displayName description icon color createdAt updatedAt archivedAt retiredAt
  visibility timezone issueCount
  cyclesEnabled cycleStartDay cycleDuration cycleCooldownTime cycleIssueAutoAssignStarted cycleIssueAutoAssignCompleted cycleLockToActive upcomingCycleCount
  triageEnabled requirePriorityToLeaveTriage
  defaultIssueState { id name type } triageIssueState { id name type }
  inheritIssueEstimation issueEstimationType issueEstimationAllowZero issueEstimationExtended defaultIssueEstimate
  autoClosePeriod autoCloseStateId autoCloseParentIssues autoCloseChildIssues autoArchivePeriod
  allMembersCanJoin initiativesEnabled groupIssueHistory securitySettings scimManaged scimGroupName
  parent { id key name }
  activeCycle { id number name }
}";

/// The fields every team membership answer carries (ADR 0014, group 3).
pub const MEMBERSHIP_FIELDS: &str = "fragment MembershipFields on TeamMembership {
  id owner sortOrder createdAt
  user { id name displayName email active admin }
  team { id key }
}";

/// The fields every user answer carries (ADR 0014, group 4): the role flags, the
/// profile `user update` changes, and when the user was last seen.
pub const USER_FIELDS: &str = "fragment UserFields on User {
  id name displayName email avatarUrl url
  active admin owner guest app isMe disableReason
  title description timezone statusEmoji statusLabel statusUntilAt
  lastSeen createdAt updatedAt
}";

/// The fields every label answer carries (ADR 0014, group 6): what `label list`
/// prints, with `retiredAt`, which `label retire` sets, and `createdAt`.
pub const LABEL_FIELDS: &str = "fragment LabelFields on IssueLabel {
  id name color description isGroup retiredAt createdAt
  team { id key } parent { id name }
}";

/// The fields every workflow state answer carries (ADR 0014, group 7).
pub const STATE_FIELDS: &str = "fragment StateFields on WorkflowState {
  id name type position color description team { id key }
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
  teams(first: $first) { nodes { id key name description private visibility cyclesEnabled retiredAt timezone } pageInfo { hasNextPage } }
}"
);

op!(
    WORKFLOW_STATES,
    "WorkflowStates",
    Kind::Query,
    "query WorkflowStates($first: Int!, $filter: WorkflowStateFilter) {
  workflowStates(first: $first, filter: $filter) { nodes { ...StateFields } pageInfo { hasNextPage } }
}",
    &[STATE_FIELDS]
);

op!(
    ISSUE_LABELS,
    "IssueLabels",
    Kind::Query,
    "query IssueLabels($first: Int!, $filter: IssueLabelFilter) {
  issueLabels(first: $first, filter: $filter) { nodes { ...LabelFields } pageInfo { hasNextPage } }
}",
    &[LABEL_FIELDS]
);

op!(
    PROJECTS,
    "Projects",
    Kind::Query,
    "query Projects($first: Int!, $filter: ProjectFilter, $teams: Int!) {
  projects(first: $first, filter: $filter) { nodes { ...ProjectFields } pageInfo { hasNextPage } }
}",
    &[PROJECT_FIELDS]
);

op!(
    USERS,
    "Users",
    Kind::Query,
    "query Users($first: Int!, $filter: UserFilter, $includeDisabled: Boolean) {
  users(first: $first, filter: $filter, includeDisabled: $includeDisabled) {
    nodes { id name displayName email active admin owner guest app isMe }
    pageInfo { hasNextPage }
  }
}"
);

op!(ISSUE_BY_ID, "IssueById", Kind::Query, "query IssueById($id: String!, $comments: Int!, $attachments: Int!, $children: Int!) {
  issue(id: $id) {
    ...IssueFields
    comments(first: $comments, orderBy: createdAt) { nodes { ...CommentFields } pageInfo { hasNextPage } }
    attachments(first: $attachments) { nodes { id title subtitle url sourceType createdAt } pageInfo { hasNextPage } }
    children(first: $children, orderBy: createdAt) { nodes { id identifier title state { id name type } } pageInfo { hasNextPage } }
  }
}", &[ISSUE_FIELDS, COMMENT_FIELDS]);

op!(ISSUES, "Issues", Kind::Query, "query Issues($first: Int!, $filter: IssueFilter, $orderBy: PaginationOrderBy) {
  issues(first: $first, filter: $filter, orderBy: $orderBy) { nodes { ...IssueFields } pageInfo { hasNextPage } }
}", &[ISSUE_FIELDS]);

op!(
    TEAM_BY_KEY,
    "TeamByKey",
    Kind::Query,
    "query TeamByKey($key: String!, $includeArchived: Boolean) {
  teams(first: 1, filter: { key: { eqIgnoreCase: $key } }, includeArchived: $includeArchived) { nodes { ...TeamFields } }
}",
    &[TEAM_FIELDS]
);

op!(
    TEAM_MEMBERS,
    "TeamMembers",
    Kind::Query,
    "query TeamMembers($key: String!, $first: Int!) {
  teams(first: 1, filter: { key: { eqIgnoreCase: $key } }) {
    nodes { id key memberships(first: $first) { nodes { ...MembershipFields } pageInfo { hasNextPage } } }
  }
}",
    &[MEMBERSHIP_FIELDS]
);

op!(
    USER_BY_ID,
    "UserById",
    Kind::Query,
    "query UserById($id: String!) {
  user(id: $id) { ...UserFields }
}",
    &[USER_FIELDS]
);

op!(
    WORKSPACE,
    "Workspace",
    Kind::Query,
    "query Workspace {
  organization { ...OrganizationFields }
}",
    &[ORGANIZATION_FIELDS]
);

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

op!(
    PROJECT_CREATE,
    "ProjectCreate",
    Kind::Mutation,
    "mutation ProjectCreate($input: ProjectCreateInput!, $teams: Int!) {
  projectCreate(input: $input) { success project { ...ProjectFields } }
}",
    &[PROJECT_FIELDS]
);

op!(
    ATTACHMENT_CREATE,
    "AttachmentCreate",
    Kind::Mutation,
    "mutation AttachmentCreate($input: AttachmentCreateInput!) {
  attachmentCreate(input: $input) { success attachment { id title subtitle url sourceType createdAt issue { id identifier } } }
}"
);

op!(
    WORKSPACE_UPDATE,
    "WorkspaceUpdate",
    Kind::Mutation,
    "mutation WorkspaceUpdate($input: OrganizationUpdateInput!) {
  organizationUpdate(input: $input) { success organization { ...OrganizationFields } }
}",
    &[ORGANIZATION_FIELDS]
);

op!(
    TEAM_CREATE,
    "TeamCreate",
    Kind::Mutation,
    "mutation TeamCreate($input: TeamCreateInput!, $copySettingsFromTeamId: String) {
  teamCreate(input: $input, copySettingsFromTeamId: $copySettingsFromTeamId) { success team { ...TeamFields } }
}",
    &[TEAM_FIELDS]
);

op!(
    TEAM_UPDATE,
    "TeamUpdate",
    Kind::Mutation,
    "mutation TeamUpdate($id: String!, $input: TeamUpdateInput!) {
  teamUpdate(id: $id, input: $input) { success team { ...TeamFields } }
}",
    &[TEAM_FIELDS]
);

op!(
    TEAM_DELETE,
    "TeamDelete",
    Kind::Mutation,
    "mutation TeamDelete($id: String!) {
  teamDelete(id: $id) { success entityId }
}"
);

op!(
    TEAM_UNARCHIVE,
    "TeamUnarchive",
    Kind::Mutation,
    "mutation TeamUnarchive($id: String!) {
  teamUnarchive(id: $id) { success entity { ...TeamFields } }
}",
    &[TEAM_FIELDS]
);

op!(
    MEMBERSHIP_CREATE,
    "TeamMembershipCreate",
    Kind::Mutation,
    "mutation TeamMembershipCreate($input: TeamMembershipCreateInput!) {
  teamMembershipCreate(input: $input) { success teamMembership { ...MembershipFields } }
}",
    &[MEMBERSHIP_FIELDS]
);

op!(
    MEMBERSHIP_DELETE,
    "TeamMembershipDelete",
    Kind::Mutation,
    "mutation TeamMembershipDelete($id: String!) {
  teamMembershipDelete(id: $id) { success entityId }
}"
);

op!(
    USER_UPDATE,
    "UserUpdate",
    Kind::Mutation,
    "mutation UserUpdate($id: String!, $input: UserUpdateInput!) {
  userUpdate(id: $id, input: $input) { success user { ...UserFields } }
}",
    &[USER_FIELDS]
);

op!(
    USER_CHANGE_ROLE,
    "UserChangeRole",
    Kind::Mutation,
    "mutation UserChangeRole($id: String!, $role: UserRoleType!) {
  userChangeRole(id: $id, role: $role) { success }
}"
);

op!(
    USER_SUSPEND,
    "UserSuspend",
    Kind::Mutation,
    "mutation UserSuspend($id: String!) {
  userSuspend(id: $id) { success }
}"
);

op!(
    USER_UNSUSPEND,
    "UserUnsuspend",
    Kind::Mutation,
    "mutation UserUnsuspend($id: String!) {
  userUnsuspend(id: $id) { success }
}"
);

op!(
    USER_REVOKE_SESSIONS,
    "UserRevokeAllSessions",
    Kind::Mutation,
    "mutation UserRevokeAllSessions($id: String!) {
  userRevokeAllSessions(id: $id) { success }
}"
);

op!(
    LABEL_CREATE,
    "IssueLabelCreate",
    Kind::Mutation,
    "mutation IssueLabelCreate($input: IssueLabelCreateInput!) {
  issueLabelCreate(input: $input) { success issueLabel { ...LabelFields } }
}",
    &[LABEL_FIELDS]
);

op!(
    LABEL_UPDATE,
    "IssueLabelUpdate",
    Kind::Mutation,
    "mutation IssueLabelUpdate($id: String!, $input: IssueLabelUpdateInput!) {
  issueLabelUpdate(id: $id, input: $input) { success issueLabel { ...LabelFields } }
}",
    &[LABEL_FIELDS]
);

op!(
    LABEL_RETIRE,
    "IssueLabelRetire",
    Kind::Mutation,
    "mutation IssueLabelRetire($id: String!) {
  issueLabelRetire(id: $id) { success issueLabel { ...LabelFields } }
}",
    &[LABEL_FIELDS]
);

op!(
    LABEL_RESTORE,
    "IssueLabelRestore",
    Kind::Mutation,
    "mutation IssueLabelRestore($id: String!) {
  issueLabelRestore(id: $id) { success issueLabel { ...LabelFields } }
}",
    &[LABEL_FIELDS]
);

op!(
    LABEL_DELETE,
    "IssueLabelDelete",
    Kind::Mutation,
    "mutation IssueLabelDelete($id: String!) {
  issueLabelDelete(id: $id) { success entityId }
}"
);

op!(
    STATE_CREATE,
    "WorkflowStateCreate",
    Kind::Mutation,
    "mutation WorkflowStateCreate($input: WorkflowStateCreateInput!) {
  workflowStateCreate(input: $input) { success workflowState { ...StateFields } }
}",
    &[STATE_FIELDS]
);

op!(
    STATE_UPDATE,
    "WorkflowStateUpdate",
    Kind::Mutation,
    "mutation WorkflowStateUpdate($id: String!, $input: WorkflowStateUpdateInput!) {
  workflowStateUpdate(id: $id, input: $input) { success workflowState { ...StateFields } }
}",
    &[STATE_FIELDS]
);

op!(
    STATE_ARCHIVE,
    "WorkflowStateArchive",
    Kind::Mutation,
    "mutation WorkflowStateArchive($id: String!) {
  workflowStateArchive(id: $id) { success entity { ...StateFields } }
}",
    &[STATE_FIELDS]
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
    &WORKSPACE,
    &TEAM_BY_KEY,
    &TEAM_MEMBERS,
    &USER_BY_ID,
];

/// Every write operation.
pub static WRITES: &[&Operation] = &[
    &ISSUE_CREATE,
    &ISSUE_UPDATE,
    &COMMENT_CREATE,
    &ATTACHMENT_CREATE,
    &PROJECT_CREATE,
    &WORKSPACE_UPDATE,
    &TEAM_CREATE,
    &TEAM_UPDATE,
    &TEAM_DELETE,
    &TEAM_UNARCHIVE,
    &MEMBERSHIP_CREATE,
    &MEMBERSHIP_DELETE,
    &USER_UPDATE,
    &USER_CHANGE_ROLE,
    &USER_SUSPEND,
    &USER_UNSUSPEND,
    &USER_REVOKE_SESSIONS,
    &LABEL_CREATE,
    &LABEL_UPDATE,
    &LABEL_RETIRE,
    &LABEL_RESTORE,
    &LABEL_DELETE,
    &STATE_CREATE,
    &STATE_UPDATE,
    &STATE_ARCHIVE,
];

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
            for frag in [
                "IssueFields",
                "CommentFields",
                "ProjectFields",
                "OrganizationFields",
                "TeamFields",
                "MembershipFields",
                "UserFields",
                "LabelFields",
                "StateFields",
            ] {
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
