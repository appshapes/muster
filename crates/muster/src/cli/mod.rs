//! The command tree (ADR 0007), as `clap` derives it. `muster schema` walks this same
//! tree, so the generated reference is never out of step with `--help`.

pub mod run;
pub mod schema;

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::bounds::PAGE_SIZE;

/// `muster`: the global flags and the command.
#[derive(Parser, Debug)]
#[command(
    name = "muster",
    version,
    about = "A command-line client for Linear, for coding agents first: one JSON object per line out, an exit code per failure, the key from a file",
    disable_help_subcommand = true,
    propagate_version = true
)]
pub struct Cli {
    /// Render the answer as text for a person; one JSON object per line is the default
    #[arg(long, global = true)]
    pub human: bool,
    /// Print every request and answer to stderr; the key is never printed
    #[arg(long, global = true)]
    pub trace: bool,
    /// The team key (APP); else MUSTER_TEAM; else the workspace's only team
    #[arg(long, global = true, value_name = "KEY")]
    pub team: Option<String>,
    /// The command.
    #[command(subcommand)]
    pub command: Command,
}

/// The commands.
#[derive(Subcommand, Debug)]
pub enum Command {
    /// Who the key belongs to, the workspace, the team muster will use, and checks on what was read
    Whoami,
    /// The workspace: get
    Workspace {
        /// The workspace command.
        #[command(subcommand)]
        command: WorkspaceCommand,
    },
    /// Teams: list, get, create, retire, unretire, delete, unarchive
    Team {
        /// The team command.
        #[command(subcommand)]
        command: TeamCommand,
    },
    /// Members of the team: list, add, remove
    Member {
        /// The member command.
        #[command(subcommand)]
        command: MemberCommand,
    },
    /// Workflow states of the team: list, create, update, archive
    State {
        /// The state command.
        #[command(subcommand)]
        command: StateCommand,
    },
    /// Labels: list, create, update, retire, restore, delete
    Label {
        /// The label command.
        #[command(subcommand)]
        command: LabelCommand,
    },
    /// Projects: list, create
    Project {
        /// The project command.
        #[command(subcommand)]
        command: ProjectCommand,
    },
    /// Initiatives, the workspace's strategic objectives: list, get, create, update, archive, unarchive, add-project, remove-project
    Initiative {
        /// The initiative command.
        #[command(subcommand)]
        command: InitiativeCommand,
    },
    /// A team's cycles, Linear's time-boxed sprints: list, get, update
    Cycle {
        /// The cycle command.
        #[command(subcommand)]
        command: CycleCommand,
    },
    /// Documents, Linear's pages of markdown on a project or an issue: list, get, create, update, delete, unarchive
    Document {
        /// The document command.
        #[command(subcommand)]
        command: DocumentCommand,
    },
    /// Templates for issues, projects and documents: list, get, create, update, delete
    Template {
        /// The template command.
        #[command(subcommand)]
        command: TemplateCommand,
    },
    /// A project's milestones: list, create, update, delete
    Milestone {
        /// The milestone command.
        #[command(subcommand)]
        command: MilestoneCommand,
    },
    /// A project's status posts, which Linear calls project updates: list, create
    ProjectUpdate {
        /// The project update command.
        #[command(subcommand)]
        command: ProjectUpdateCommand,
    },
    /// The workspace's project statuses: list, create, update, archive, unarchive
    ProjectStatus {
        /// The project status command.
        #[command(subcommand)]
        command: ProjectStatusCommand,
    },
    /// Users: list, get, update, change-role, suspend, unsuspend, revoke-sessions
    User {
        /// The user command.
        #[command(subcommand)]
        command: UserCommand,
    },
    /// Issues: get, list, create, comment, attach, close, update
    Issue {
        /// The issue command.
        #[command(subcommand)]
        command: IssueCommand,
    },
    /// The command tree, exit codes, notices, bounds and environment variables, as JSON or as the reference page
    Schema(SchemaArgs),
    /// Record the shape of every read operation's answer from the configured endpoint into a directory, values stripped (maintainers; ADR 0010)
    Fixtures(FixturesArgs),
}

/// The workspace commands (ADR 0014, group 1).
#[derive(Subcommand, Debug)]
pub enum WorkspaceCommand {
    /// The workspace's settings, its security and auth settings, its counts and its subscription, as Linear returns them
    Get,
    /// Change the workspace's settings; a flag left out leaves its setting alone, a boolean takes true or false, and most need a workspace admin
    Update(Box<WorkspaceUpdateArgs>),
}

/// `workspace update`: every setting of `OrganizationUpdateInput` that is public, not
/// deprecated and readable back with `workspace get` (ADR 0014, group 1).
#[derive(Args, Debug, Default)]
pub struct WorkspaceUpdateArgs {
    /// A new name
    #[arg(long, value_name = "TEXT")]
    pub name: Option<String>,
    /// A new URL key, the `appshapes` of linear.app/appshapes; Linear keeps the last three previous keys redirecting
    #[arg(long, value_name = "KEY")]
    pub url_key: Option<String>,
    /// A new logo, by URL
    #[arg(long, value_name = "URL")]
    pub logo_url: Option<String>,
    /// The template for the branch name Linear suggests for an issue, or `none` for Linear's default
    #[arg(long, value_name = "TEXT|none")]
    pub git_branch_format: Option<String>,
    /// Whether the git integration posts a linkback comment on a private repository's pull request
    #[arg(long, value_name = "true|false")]
    pub git_linkback_messages: Option<bool>,
    /// Whether the git integration posts a linkback comment on a public repository's pull request
    #[arg(long, value_name = "true|false")]
    pub git_public_linkback_messages: Option<bool>,
    /// Whether a linkback comment carries the issue's description
    #[arg(long, value_name = "true|false")]
    pub git_linkback_descriptions: Option<bool>,
    /// Whether copying an issue, or opening it in a coding tool, suggests a branch name
    #[arg(long, value_name = "true|false")]
    pub suggested_branch_name: Option<bool>,
    /// Whether initiatives are on (Linear's `roadmapEnabled`, the switch under Settings > Initiatives); `initiative create` is refused while off
    #[arg(long, value_name = "true|false")]
    pub roadmap: Option<bool>,
    /// How many weeks apart Linear asks for project updates
    #[arg(long, value_name = "N")]
    pub project_update_reminder_weeks: Option<u32>,
    /// The weekday project update reminders go out
    #[arg(long, value_name = "DAY", value_enum)]
    pub project_update_reminders_day: Option<Day>,
    /// The hour project update reminders go out, 0 to 23
    #[arg(long, value_name = "0-23", value_parser = clap::value_parser!(u8).range(0..=23))]
    pub project_update_reminders_hour: Option<u8>,
    /// The month the fiscal year starts, 0 for January to 11 for December
    #[arg(long, value_name = "0-11", value_parser = clap::value_parser!(u8).range(0..=11))]
    pub fiscal_year_start_month: Option<u8>,
    /// Whether the activity feed is on
    #[arg(long, value_name = "true|false")]
    pub feed: Option<bool>,
    /// Whether Linear writes an AI guide for a new pull request
    #[arg(long, value_name = "true|false")]
    pub pull_request_tour: Option<bool>,
    /// How Linear links an issue to a pull request that names none
    #[arg(long, value_name = "MODE", value_enum)]
    pub pull_request_issue_mode: Option<PullRequestIssueMode>,
    /// Whether a resolved comment thread gets an AI summary
    #[arg(long, value_name = "true|false")]
    pub ai_thread_summaries: Option<bool>,
    /// Whether an issue's discussion gets an AI summary
    #[arg(long, value_name = "true|false")]
    pub ai_discussion_summaries: Option<bool>,
    /// The content types an upload may have, comma-separated (image/png,application/pdf)
    #[arg(long, value_name = "TYPE,...", value_delimiter = ',')]
    pub allowed_file_upload_content_types: Option<Vec<String>>,
    /// The least role that may invite users
    #[arg(long, value_name = "ROLE", value_enum)]
    pub invitations_role: Option<Role>,
    /// The least role that may create a team
    #[arg(long, value_name = "ROLE", value_enum)]
    pub team_creation_role: Option<Role>,
    /// The least role that may manage workspace labels
    #[arg(long, value_name = "ROLE", value_enum)]
    pub label_management_role: Option<Role>,
    /// The least role that may create a personal API key
    #[arg(long, value_name = "ROLE", value_enum)]
    pub personal_api_keys_role: Option<Role>,
    /// The least role that may grant or revoke the admin role
    #[arg(long, value_name = "ROLE", value_enum)]
    pub admin_management_role: Option<Role>,
    /// The least role that may manage workspace templates
    #[arg(long, value_name = "ROLE", value_enum)]
    pub template_management_role: Option<Role>,
    /// The least role that may install an integration
    #[arg(long, value_name = "ROLE", value_enum)]
    pub integration_creation_role: Option<Role>,
    /// The least role that may import data
    #[arg(long, value_name = "ROLE", value_enum)]
    pub import_role: Option<Role>,
    /// The least role that may manage API settings
    #[arg(long, value_name = "ROLE", value_enum)]
    pub api_settings_role: Option<Role>,
    /// The least role that may create a workspace-wide initiative
    #[arg(long, value_name = "ROLE", value_enum)]
    pub workspace_initiatives_role: Option<Role>,
    /// The sign-in services allowed, comma-separated (google,saml), or `all` for every one; a value that names none is a usage error, exit 2
    #[arg(long, value_name = "SERVICE,...|all")]
    pub allowed_auth_services: Option<String>,
    /// Whether a new user whose email domain this workspace claims is shown no other workspace at sign-up
    #[arg(long, value_name = "true|false")]
    pub hide_non_primary_organizations: Option<bool>,
}

/// A weekday, as Linear's `Day` enum names it.
#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Day {
    /// Sunday.
    #[value(name = "Sunday")]
    Sunday,
    /// Monday.
    #[value(name = "Monday")]
    Monday,
    /// Tuesday.
    #[value(name = "Tuesday")]
    Tuesday,
    /// Wednesday.
    #[value(name = "Wednesday")]
    Wednesday,
    /// Thursday.
    #[value(name = "Thursday")]
    Thursday,
    /// Friday.
    #[value(name = "Friday")]
    Friday,
    /// Saturday.
    #[value(name = "Saturday")]
    Saturday,
}

impl Day {
    /// The value Linear takes.
    pub fn linear(self) -> &'static str {
        match self {
            Day::Sunday => "Sunday",
            Day::Monday => "Monday",
            Day::Tuesday => "Tuesday",
            Day::Wednesday => "Wednesday",
            Day::Thursday => "Thursday",
            Day::Friday => "Friday",
            Day::Saturday => "Saturday",
        }
    }
}

/// A workspace role a person holds, as Linear's `UserRoleType` names it; `app` is not
/// a role a person takes.
#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// The highest role; the owner of the workspace.
    Owner,
    /// A workspace administrator.
    Admin,
    /// A member.
    User,
    /// A guest, limited to the teams they are in.
    Guest,
}

impl Role {
    /// The value Linear takes.
    pub fn linear(self) -> &'static str {
        match self {
            Role::Owner => "owner",
            Role::Admin => "admin",
            Role::User => "user",
            Role::Guest => "guest",
        }
    }
}

/// How Linear links an issue to a pull request that names none.
#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum PullRequestIssueMode {
    /// Never.
    Off,
    /// Suggest one.
    Suggest,
    /// Link one when the pull request merges.
    #[value(name = "autoOnMerge")]
    AutoOnMerge,
}

impl PullRequestIssueMode {
    /// The value Linear takes.
    pub fn linear(self) -> &'static str {
        match self {
            PullRequestIssueMode::Off => "off",
            PullRequestIssueMode::Suggest => "suggest",
            PullRequestIssueMode::AutoOnMerge => "autoOnMerge",
        }
    }
}

/// The team commands (ADR 0007; ADR 0014, group 2).
#[derive(Subcommand, Debug)]
pub enum TeamCommand {
    /// List them, one JSON object per line
    List(ListArgs),
    /// One team with every setting, by key; without a key, the team muster would use
    Get(TeamRefArgs),
    /// Create a team: its creator becomes a member and Linear makes its default states and labels; a name a team already has is refused unless --allow-duplicate
    Create(Box<TeamCreateArgs>),
    /// Change a team's settings; a flag left out leaves its setting alone, a boolean takes true or false, and most need a team owner or a workspace admin
    Update(Box<TeamUpdateArgs>),
    /// Retire a team: it keeps its issues and accepts no new ones (`retiredAt` set to now)
    Retire(TeamRefArgs),
    /// Take a retired team back into use (`retiredAt` cleared)
    Unretire(TeamRefArgs),
    /// Archive a team and schedule its data for deletion after Linear's grace period; `team unarchive` cancels that within the period
    Delete(TeamRefArgs),
    /// Cancel a team's deletion within Linear's grace period and bring it back
    Unarchive(TeamRefArgs),
}

/// A team named by key, or the usual team when none is given.
#[derive(Args, Debug)]
pub struct TeamRefArgs {
    /// The team key (APP); when absent, --team, MUSTER_TEAM or the workspace's only team
    #[arg(value_name = "KEY")]
    pub key: Option<String>,
}

/// `team create`.
#[derive(Args, Debug, Default)]
pub struct TeamCreateArgs {
    /// The name
    #[arg(long, value_name = "TEXT")]
    pub name: String,
    /// The key, the prefix of the team's identifiers (APP); Linear makes one from the name when absent
    #[arg(long, value_name = "KEY")]
    pub key: Option<String>,
    /// The description, plain text
    #[arg(long, value_name = "TEXT")]
    pub description: Option<String>,
    /// The team's timezone, an IANA name (Europe/London)
    #[arg(long, value_name = "TZ")]
    pub timezone: Option<String>,
    /// Whether the team uses cycles
    #[arg(long, value_name = "true|false")]
    pub cycles_enabled: Option<bool>,
    /// The parent team, by key, which makes this a sub-team
    #[arg(long, value_name = "KEY")]
    pub parent: Option<String>,
    /// A team, by key, whose settings the new team copies
    #[arg(long, value_name = "KEY")]
    pub copy_settings_from: Option<String>,
    /// Create it even when a team has the same name
    #[arg(long)]
    pub allow_duplicate: bool,
}

/// `team update`: every setting of `TeamUpdateInput` that is public, not deprecated
/// and readable back with `team get` (ADR 0014, group 2).
#[derive(Args, Debug, Default)]
pub struct TeamUpdateArgs {
    /// The team key (APP); when absent, --team, MUSTER_TEAM or the workspace's only team
    #[arg(value_name = "KEY")]
    pub key_of: Option<String>,
    /// A new name
    #[arg(long, value_name = "TEXT")]
    pub name: Option<String>,
    /// A new key, the prefix of the team's identifiers; Linear keeps the old one redirecting
    #[arg(long, value_name = "KEY")]
    pub key: Option<String>,
    /// A new description, plain text
    #[arg(long, value_name = "TEXT")]
    pub description: Option<String>,
    /// The team's icon
    #[arg(long, value_name = "TEXT")]
    pub icon: Option<String>,
    /// The team's colour, a hex string (#4ea7fc)
    #[arg(long, value_name = "HEX")]
    pub color: Option<String>,
    /// Whether the team is private, visible to its members alone
    #[arg(long, value_name = "true|false")]
    pub private: Option<bool>,
    /// The team's timezone, an IANA name (Europe/London)
    #[arg(long, value_name = "TZ")]
    pub timezone: Option<String>,
    /// The parent team, by key, or `none` to take the team out from under its parent
    #[arg(long, value_name = "KEY")]
    pub parent: Option<String>,
    /// Whether the team uses cycles; Linear makes the cycles from the settings below
    #[arg(long, value_name = "true|false")]
    pub cycles_enabled: Option<bool>,
    /// The weekday a cycle starts, 0 for Sunday to 6 for Saturday
    #[arg(long, value_name = "0-6", value_parser = clap::value_parser!(u8).range(0..=6))]
    pub cycle_start_day: Option<u8>,
    /// How many weeks a cycle lasts
    #[arg(long, value_name = "WEEKS")]
    pub cycle_duration: Option<u32>,
    /// How many weeks of cooldown follow a cycle
    #[arg(long, value_name = "WEEKS")]
    pub cycle_cooldown: Option<u32>,
    /// Whether an issue that is started is put in the active cycle
    #[arg(long, value_name = "true|false")]
    pub cycle_auto_assign_started: Option<bool>,
    /// Whether an issue that is completed is put in the active cycle
    #[arg(long, value_name = "true|false")]
    pub cycle_auto_assign_completed: Option<bool>,
    /// Whether only issues in a cycle may be active
    #[arg(long, value_name = "true|false")]
    pub cycle_lock_to_active: Option<bool>,
    /// When cycles begin, a time (2026-11-10T00:00:00Z)
    #[arg(long, value_name = "TIME")]
    pub cycle_enabled_start_date: Option<String>,
    /// How many upcoming cycles Linear keeps made
    #[arg(long, value_name = "N")]
    pub upcoming_cycle_count: Option<u32>,
    /// Whether new issues from outside the team land in triage
    #[arg(long, value_name = "true|false")]
    pub triage_enabled: Option<bool>,
    /// Whether an issue needs a priority before it leaves triage
    #[arg(long, value_name = "true|false")]
    pub require_priority_to_leave_triage: Option<bool>,
    /// The state a new issue starts in, by name
    #[arg(long, value_name = "NAME")]
    pub default_issue_state: Option<String>,
    /// How issues are estimated
    #[arg(long, value_name = "TYPE", value_enum)]
    pub issue_estimation_type: Option<EstimationType>,
    /// Whether an estimate may be zero
    #[arg(long, value_name = "true|false")]
    pub issue_estimation_allow_zero: Option<bool>,
    /// Whether the estimate scale has its extra points
    #[arg(long, value_name = "true|false")]
    pub issue_estimation_extended: Option<bool>,
    /// The estimate an issue has when none is set
    #[arg(long, value_name = "N")]
    pub default_issue_estimate: Option<u32>,
    /// The months after which an idle issue is closed, or `none` to never close one
    #[arg(long, value_name = "MONTHS|none")]
    pub auto_close_period: Option<String>,
    /// The canceled state an auto-closed issue goes to, by name
    #[arg(long, value_name = "NAME")]
    pub auto_close_state: Option<String>,
    /// Whether a parent issue closes when every sub-issue is closed
    #[arg(long, value_name = "true|false")]
    pub auto_close_parent_issues: Option<bool>,
    /// Whether the sub-issues close when their parent is closed
    #[arg(long, value_name = "true|false")]
    pub auto_close_child_issues: Option<bool>,
    /// The months after which a closed issue is archived
    #[arg(long, value_name = "MONTHS")]
    pub auto_archive_period: Option<u32>,
    /// Whether a new user joins the team by default; a workspace admin's setting
    #[arg(long, value_name = "true|false")]
    pub join_by_default: Option<bool>,
    /// Whether every member of the workspace may join the team
    #[arg(long, value_name = "true|false")]
    pub all_members_can_join: Option<bool>,
    /// Whether initiatives show in the team's sidebar
    #[arg(long, value_name = "true|false")]
    pub initiatives_enabled: Option<bool>,
    /// Whether an issue's recent history entries are grouped
    #[arg(long, value_name = "true|false")]
    pub group_issue_history: Option<bool>,
    /// The least team role that may change the team's settings
    #[arg(long, value_name = "ROLE", value_enum)]
    pub team_management_role: Option<TeamRole>,
    /// The least team role that may add or remove members
    #[arg(long, value_name = "ROLE", value_enum)]
    pub member_management_role: Option<TeamRole>,
    /// The least team role that may manage the team's labels
    #[arg(long, value_name = "ROLE", value_enum)]
    pub label_management_role: Option<TeamRole>,
    /// The least team role that may manage the team's templates
    #[arg(long, value_name = "ROLE", value_enum)]
    pub template_management_role: Option<TeamRole>,
    /// The least team role that may share an issue outside the team
    #[arg(long, value_name = "ROLE", value_enum)]
    pub issue_sharing_role: Option<TeamRole>,
}

/// A role inside a team, as Linear's `TeamRoleType` names it.
#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum TeamRole {
    /// A team owner.
    Owner,
    /// A team member.
    Member,
}

impl TeamRole {
    /// The value Linear takes.
    pub fn linear(self) -> &'static str {
        match self {
            TeamRole::Owner => "owner",
            TeamRole::Member => "member",
        }
    }
}

/// How a team estimates issues, as Linear's `issueEstimationType` spells it.
#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum EstimationType {
    /// No estimates.
    #[value(name = "notUsed")]
    NotUsed,
    /// Powers of two.
    Exponential,
    /// The Fibonacci scale.
    Fibonacci,
    /// One to five.
    Linear,
    /// T-shirt sizes.
    #[value(name = "tShirt")]
    TShirt,
}

impl EstimationType {
    /// The value Linear takes.
    pub fn linear(self) -> &'static str {
        match self {
            EstimationType::NotUsed => "notUsed",
            EstimationType::Exponential => "exponential",
            EstimationType::Fibonacci => "fibonacci",
            EstimationType::Linear => "linear",
            EstimationType::TShirt => "tShirt",
        }
    }
}

/// The member commands (ADR 0014, group 3): the team's memberships.
#[derive(Subcommand, Debug)]
pub enum MemberCommand {
    /// List the team's members, one JSON object per line, each a membership with its user
    List(ListArgs),
    /// Add a user to the team
    Add(MemberArgs),
    /// Remove a user from the team; a user may remove themselves, a team owner or a workspace admin anyone. The membership is found in the team's first PAGE_CAP members: a user who is not among them is exit 4 when the team has no more, and exit 6, refused before any mutation, when it has
    Remove(MemberArgs),
}

/// `member add` and `member remove`.
#[derive(Args, Debug)]
pub struct MemberArgs {
    /// The user: `me`, an email address or a name
    #[arg(long, value_name = "WHO")]
    pub user: String,
}

/// The user commands (ADR 0007; ADR 0014, group 4).
#[derive(Subcommand, Debug)]
pub enum UserCommand {
    /// List the workspace's users, one JSON object per line; the active ones unless --all
    List(UserListArgs),
    /// One user with their role flags, profile, status and when they were last seen
    Get(UserRefArgs),
    /// Change a user's profile; your own, or anyone's as a workspace admin; a flag left out leaves its field alone
    Update(Box<UserUpdateArgs>),
    /// Change a user's workspace role; a workspace admin's command, and the caller's role must be at least the new one
    ChangeRole(UserRoleArgs),
    /// Suspend a user: their account is deactivated and their sessions end; a workspace admin's command
    Suspend(UserRefArgs),
    /// Reactivate a suspended user; a workspace admin's command
    Unsuspend(UserRefArgs),
    /// End every session of a user, so that they sign in again; a workspace admin's command
    RevokeSessions(UserRefArgs),
}

/// `user list`.
#[derive(Args, Debug)]
pub struct UserListArgs {
    /// At most this many, up to PAGE_CAP; a cut list writes a `truncated` notice to stderr
    #[arg(long, default_value_t = PAGE_SIZE, value_name = "N")]
    pub limit: usize,
    /// The suspended users too, not only the active ones
    #[arg(long)]
    pub all: bool,
}

/// A user named as `--assignee` names one.
#[derive(Args, Debug)]
pub struct UserRefArgs {
    /// The user: `me`, an email address or a name
    #[arg(value_name = "WHO")]
    pub who: String,
}

/// `user change-role`.
#[derive(Args, Debug)]
pub struct UserRoleArgs {
    /// The user: `me`, an email address or a name
    #[arg(value_name = "WHO")]
    pub who: String,
    /// The new role
    #[arg(long, value_name = "ROLE", value_enum)]
    pub role: Role,
}

/// `user update`: the profile fields of `UserUpdateInput`.
#[derive(Args, Debug, Default)]
pub struct UserUpdateArgs {
    /// The user: `me`, an email address or a name
    #[arg(value_name = "WHO")]
    pub who: String,
    /// A new full name
    #[arg(long, value_name = "TEXT")]
    pub name: Option<String>,
    /// A new display name, unique in the workspace
    #[arg(long, value_name = "TEXT")]
    pub display_name: Option<String>,
    /// A new job title
    #[arg(long, value_name = "TEXT")]
    pub title: Option<String>,
    /// A new short bio
    #[arg(long, value_name = "TEXT")]
    pub description: Option<String>,
    /// The user's timezone, an IANA name (Europe/London)
    #[arg(long, value_name = "TZ")]
    pub timezone: Option<String>,
    /// The emoji of the user's status
    #[arg(long, value_name = "TEXT")]
    pub status_emoji: Option<String>,
    /// The text of the user's status
    #[arg(long, value_name = "TEXT")]
    pub status_label: Option<String>,
    /// When the status clears, a time (2026-11-10T09:00:00Z), or `none` to keep it
    #[arg(long, value_name = "TIME|none")]
    pub status_until: Option<String>,
    /// A new avatar, by URL
    #[arg(long, value_name = "URL")]
    pub avatar_url: Option<String>,
}

/// The label commands (ADR 0007; ADR 0014, group 6).
#[derive(Subcommand, Debug)]
pub enum LabelCommand {
    /// List the labels of the team and of the workspace, one JSON object per line
    List(ListArgs),
    /// Create a label for the team, or for the workspace with --workspace; a name a label of the same scope has is refused unless --allow-duplicate
    Create(LabelCreateArgs),
    /// Change a label's name, colour, description or parent
    Update(LabelUpdateArgs),
    /// Retire a label: it stays on its issues and cannot be applied to a new one
    Retire(LabelRefArgs),
    /// Restore a retired label
    Restore(LabelRefArgs),
    /// Delete a label, which removes it from every issue; no undo in Linear, so retire first where history matters
    Delete(LabelRefArgs),
}

/// A label named as `--label` names one: in the team or the workspace.
#[derive(Args, Debug)]
pub struct LabelRefArgs {
    /// The label, by name
    #[arg(value_name = "NAME")]
    pub name: String,
}

/// `label create`.
#[derive(Args, Debug, Default)]
pub struct LabelCreateArgs {
    /// The name
    #[arg(long, value_name = "TEXT")]
    pub name: String,
    /// A workspace label, for every team, instead of one for the team
    #[arg(long)]
    pub workspace: bool,
    /// The colour, a hex string (#eb5757)
    #[arg(long, value_name = "HEX")]
    pub color: Option<String>,
    /// The description, plain text
    #[arg(long, value_name = "TEXT")]
    pub description: Option<String>,
    /// The group this label goes in, by name
    #[arg(long, value_name = "NAME")]
    pub parent: Option<String>,
    /// Make a group, which holds labels rather than going on issues
    #[arg(long)]
    pub group: bool,
    /// Create it even when a label of the same scope has the name
    #[arg(long)]
    pub allow_duplicate: bool,
}

/// `label update`.
#[derive(Args, Debug, Default)]
pub struct LabelUpdateArgs {
    /// The label, by name
    #[arg(value_name = "NAME")]
    pub label: String,
    /// A new name
    #[arg(long, value_name = "TEXT")]
    pub name: Option<String>,
    /// A new colour, a hex string (#eb5757)
    #[arg(long, value_name = "HEX")]
    pub color: Option<String>,
    /// A new description, plain text
    #[arg(long, value_name = "TEXT")]
    pub description: Option<String>,
    /// A new group, by name, or `none` to take the label out of its group
    #[arg(long, value_name = "NAME")]
    pub parent: Option<String>,
}

/// The state commands (ADR 0007; ADR 0014, group 7): the team's workflow.
#[derive(Subcommand, Debug)]
pub enum StateCommand {
    /// List the team's states by position, one JSON object per line
    List(ListArgs),
    /// Add a state to the team's workflow; its type cannot change afterwards; a name a state of the team has is refused unless --allow-duplicate
    Create(StateCreateArgs),
    /// Change a state's name, colour, description or position
    Update(StateUpdateArgs),
    /// Archive a state; Linear refuses it while the state holds an issue that is not archived, and nothing unarchives a state
    Archive(StateRefArgs),
}

/// A state named as `--state` names one, in the team.
#[derive(Args, Debug)]
pub struct StateRefArgs {
    /// The state, by name
    #[arg(value_name = "NAME")]
    pub name: String,
}

/// `state create`.
#[derive(Args, Debug, Default)]
pub struct StateCreateArgs {
    /// The name
    #[arg(long, value_name = "TEXT")]
    pub name: String,
    /// The type, which says how Linear treats the state; `triage` is Linear's own
    #[arg(long, value_name = "TYPE", value_enum)]
    pub r#type: StateType,
    /// The colour, a hex string (#bec2c8)
    #[arg(long, value_name = "HEX")]
    pub color: String,
    /// The description, plain text
    #[arg(long, value_name = "TEXT")]
    pub description: Option<String>,
    /// Where the state sits among the team's, a finite number; Linear's default is the end
    #[arg(long, value_name = "N")]
    pub position: Option<f64>,
    /// Create it even when a state of the team has the name
    #[arg(long)]
    pub allow_duplicate: bool,
}

/// `state update`.
#[derive(Args, Debug, Default)]
pub struct StateUpdateArgs {
    /// The state, by name
    #[arg(value_name = "NAME")]
    pub state: String,
    /// A new name
    #[arg(long, value_name = "TEXT")]
    pub name: Option<String>,
    /// A new colour, a hex string (#bec2c8)
    #[arg(long, value_name = "HEX")]
    pub color: Option<String>,
    /// A new description, plain text
    #[arg(long, value_name = "TEXT")]
    pub description: Option<String>,
    /// A new position among the team's states, a finite number
    #[arg(long, value_name = "N")]
    pub position: Option<f64>,
}

/// A workflow state's type, as Linear's `WorkflowState.type` spells the ones a team
/// may create (`triage` and `duplicate` are Linear's own).
#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StateType {
    /// Not yet planned.
    #[default]
    Backlog,
    /// Planned, not started.
    Unstarted,
    /// In progress.
    Started,
    /// Done.
    Completed,
    /// Not going to be done.
    Canceled,
}

impl StateType {
    /// The value Linear takes.
    pub fn linear(self) -> &'static str {
        match self {
            StateType::Backlog => "backlog",
            StateType::Unstarted => "unstarted",
            StateType::Started => "started",
            StateType::Completed => "completed",
            StateType::Canceled => "canceled",
        }
    }
}
/// The one flag of every plain list.
#[derive(Args, Debug)]
pub struct ListArgs {
    /// At most this many, up to PAGE_CAP; a cut list writes a `truncated` notice to stderr
    #[arg(long, default_value_t = PAGE_SIZE, value_name = "N")]
    pub limit: usize,
}

/// The project commands (ADR 0013; ADR 0014, group 8).
#[derive(Subcommand, Debug)]
pub enum ProjectCommand {
    /// List them, one JSON object per line
    List(ListArgs),
    /// One project with its content, lifecycle, health, members and milestones, by name
    Get(ProjectRefArgs),
    /// Create a project for the team; a name a project of the workspace already has is refused unless --allow-duplicate
    Create(ProjectCreateArgs),
    /// Change a project's name, description, content, status, lead, dates, priority, teams, members, colour or icon
    Update(Box<ProjectUpdateArgs>),
    /// Trash a project; Linear keeps it for thirty days, and `project unarchive` brings it back
    Delete(ProjectRefArgs),
    /// Bring a trashed or archived project back
    Unarchive(ProjectRefArgs),
}

/// A project named as `--project` names one.
#[derive(Args, Debug)]
pub struct ProjectRefArgs {
    /// The project, by name
    #[arg(value_name = "NAME")]
    pub name: String,
}

/// `project update`.
#[derive(Args, Debug, Default)]
pub struct ProjectUpdateArgs {
    /// The project, by name
    #[arg(value_name = "NAME")]
    pub project: String,
    /// A new name
    #[arg(long, value_name = "TEXT")]
    pub name: Option<String>,
    /// A new description, plain text
    #[arg(long, value_name = "TEXT")]
    pub description: Option<String>,
    /// The project's content, markdown
    #[arg(long, value_name = "TEXT", conflicts_with = "content_file")]
    pub content: Option<String>,
    /// The content from a file, or from stdin when PATH is `-`
    #[arg(long, value_name = "PATH")]
    pub content_file: Option<String>,
    /// A new status, by name, one of the workspace's project statuses
    #[arg(long, value_name = "NAME")]
    pub status: Option<String>,
    /// A new lead: `me`, an email address, a name, or `none`
    #[arg(long, value_name = "WHO")]
    pub lead: Option<String>,
    /// The planned start, a date (2026-11-10), or `none`
    #[arg(long, value_name = "DATE")]
    pub start_date: Option<String>,
    /// The planned end, a date (2026-12-01), or `none`
    #[arg(long, value_name = "DATE")]
    pub target_date: Option<String>,
    /// A new priority: 0 none, 1 urgent, 2 high, 3 medium, 4 low
    #[arg(long, value_name = "0-4", value_parser = clap::value_parser!(u8).range(0..=4))]
    pub priority: Option<u8>,
    /// The teams the project spans, by key, comma-separated; replaces the list
    #[arg(long, value_name = "KEY,...", value_delimiter = ',')]
    pub teams: Option<Vec<String>>,
    /// A member to add: `me`, an email address or a name; repeatable
    #[arg(long, value_name = "WHO")]
    pub add_member: Vec<String>,
    /// A member to remove: `me`, an email address or a name; repeatable
    #[arg(long, value_name = "WHO")]
    pub remove_member: Vec<String>,
    /// A new colour, a hex string (#4ea7fc)
    #[arg(long, value_name = "HEX")]
    pub color: Option<String>,
    /// A new icon
    #[arg(long, value_name = "TEXT")]
    pub icon: Option<String>,
}

/// The project status commands (ADR 0014, group 9): the workspace's project
/// lifecycle, which `project update --status` names.
#[derive(Subcommand, Debug)]
pub enum ProjectStatusCommand {
    /// List the workspace's project statuses by position, one JSON object per line
    List(ListArgs),
    /// Add a project status to the workspace; a name a status already has is refused unless --allow-duplicate
    Create(ProjectStatusCreateArgs),
    /// Change a project status's name, colour, description, position, type or whether a project may stay in it indefinitely
    Update(ProjectStatusUpdateArgs),
    /// Archive a project status; Linear refuses one with active projects, or the last of its type
    Archive(ProjectStatusRefArgs),
    /// Bring an archived project status back
    Unarchive(ProjectStatusRefArgs),
}

/// A project status named as `--status` names one.
#[derive(Args, Debug)]
pub struct ProjectStatusRefArgs {
    /// The status, by name
    #[arg(value_name = "NAME")]
    pub name: String,
}

/// `project-status create`.
#[derive(Args, Debug, Default)]
pub struct ProjectStatusCreateArgs {
    /// The name
    #[arg(long, value_name = "TEXT")]
    pub name: String,
    /// The type, which says how Linear treats projects in the status
    #[arg(long, value_name = "TYPE", value_enum)]
    pub r#type: ProjectStatusType,
    /// The colour, a hex string (#4ea7fc)
    #[arg(long, value_name = "HEX")]
    pub color: String,
    /// Where the status sits in the workspace's project flow: one finite number Linear orders every status by, across types, so 2.5 goes between the defaults at 2 and 3
    #[arg(long, value_name = "N")]
    pub position: f64,
    /// The description, plain text
    #[arg(long, value_name = "TEXT")]
    pub description: Option<String>,
    /// Whether a project may stay in the status indefinitely
    #[arg(long, value_name = "true|false")]
    pub indefinite: Option<bool>,
    /// Create it even when a status has the name
    #[arg(long)]
    pub allow_duplicate: bool,
}

/// `project-status update`.
#[derive(Args, Debug, Default)]
pub struct ProjectStatusUpdateArgs {
    /// The status, by name
    #[arg(value_name = "NAME")]
    pub status: String,
    /// A new name
    #[arg(long, value_name = "TEXT")]
    pub name: Option<String>,
    /// A new colour, a hex string (#4ea7fc)
    #[arg(long, value_name = "HEX")]
    pub color: Option<String>,
    /// A new description, plain text
    #[arg(long, value_name = "TEXT")]
    pub description: Option<String>,
    /// A new position in the workspace's project flow, one finite number Linear orders every status by, across types
    #[arg(long, value_name = "N")]
    pub position: Option<f64>,
    /// A new type
    #[arg(long, value_name = "TYPE", value_enum)]
    pub r#type: Option<ProjectStatusType>,
    /// Whether a project may stay in the status indefinitely
    #[arg(long, value_name = "true|false")]
    pub indefinite: Option<bool>,
}

/// A project status's type, as Linear's `ProjectStatusType` names it.
#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProjectStatusType {
    /// Not yet planned.
    #[default]
    Backlog,
    /// Planned, not started.
    Planned,
    /// In progress.
    Started,
    /// On hold.
    Paused,
    /// Done.
    Completed,
    /// Not going to be done.
    Canceled,
}

impl ProjectStatusType {
    /// The value Linear takes.
    pub fn linear(self) -> &'static str {
        match self {
            ProjectStatusType::Backlog => "backlog",
            ProjectStatusType::Planned => "planned",
            ProjectStatusType::Started => "started",
            ProjectStatusType::Paused => "paused",
            ProjectStatusType::Completed => "completed",
            ProjectStatusType::Canceled => "canceled",
        }
    }
}

/// The initiative commands (ADR 0014, group 12).
#[derive(Subcommand, Debug)]
pub enum InitiativeCommand {
    /// List the workspace's initiatives, one JSON object per line
    List(InitiativeListArgs),
    /// One initiative with its content and its projects, by name
    Get(InitiativeRefArgs),
    /// Create an initiative; a name an initiative already has is refused unless --allow-duplicate, and so is a workspace with initiatives off (`workspace update --roadmap true` switches them on)
    Create(Box<InitiativeCreateArgs>),
    /// Change an initiative's name, description, owner, lead team, status, target date, priority or content
    Update(Box<InitiativeUpdateArgs>),
    /// Archive an initiative
    Archive(InitiativeRefArgs),
    /// Bring an archived initiative back
    Unarchive(InitiativeRefArgs),
    /// Put a project in an initiative
    AddProject(InitiativeProjectArgs),
    /// Take a project out of an initiative
    RemoveProject(InitiativeProjectArgs),
}

/// `initiative list`.
#[derive(Args, Debug)]
pub struct InitiativeListArgs {
    /// Only initiatives in this status
    #[arg(long, value_name = "STATUS", value_enum)]
    pub status: Option<InitiativeStatus>,
    /// At most this many, up to PAGE_CAP; a cut list writes a `truncated` notice to stderr
    #[arg(long, default_value_t = PAGE_SIZE, value_name = "N")]
    pub limit: usize,
}

/// An initiative, by name.
#[derive(Args, Debug)]
pub struct InitiativeRefArgs {
    /// The initiative, by name
    #[arg(value_name = "NAME")]
    pub name: String,
}

/// `initiative add-project` and `initiative remove-project`.
#[derive(Args, Debug)]
pub struct InitiativeProjectArgs {
    /// The initiative, by name
    #[arg(value_name = "NAME")]
    pub name: String,
    /// The project, by name
    #[arg(long, value_name = "NAME")]
    pub project: String,
}

/// `initiative create`.
#[derive(Args, Debug, Default)]
pub struct InitiativeCreateArgs {
    /// The name
    #[arg(long, value_name = "TEXT")]
    pub name: String,
    /// The description, plain text
    #[arg(long, value_name = "TEXT")]
    pub description: Option<String>,
    /// The owner: `me`, an email address or a name
    #[arg(long, value_name = "WHO")]
    pub owner: Option<String>,
    /// The team that leads it, by key; a team initiative is a Linear Business plan feature, refused, exit 6, below it
    #[arg(long, value_name = "KEY")]
    pub lead_team: Option<String>,
    /// The status; Linear's default when absent
    #[arg(long, value_name = "STATUS", value_enum)]
    pub status: Option<InitiativeStatus>,
    /// The planned end, a date (2026-12-01)
    #[arg(long, value_name = "DATE")]
    pub target_date: Option<String>,
    /// The priority: 0 none, 1 urgent, 2 high, 3 medium, 4 low
    #[arg(long, value_name = "0-4", value_parser = clap::value_parser!(u8).range(0..=4))]
    pub priority: Option<u8>,
    /// The content, markdown
    #[arg(long, value_name = "TEXT", conflicts_with = "content_file")]
    pub content: Option<String>,
    /// The content from a file, or from stdin when PATH is `-`
    #[arg(long, value_name = "PATH")]
    pub content_file: Option<String>,
    /// Create it even when an initiative has the name
    #[arg(long)]
    pub allow_duplicate: bool,
}

/// `initiative update`.
#[derive(Args, Debug, Default)]
pub struct InitiativeUpdateArgs {
    /// The initiative, by name
    #[arg(value_name = "NAME")]
    pub initiative: String,
    /// A new name
    #[arg(long, value_name = "TEXT")]
    pub name: Option<String>,
    /// A new description, plain text
    #[arg(long, value_name = "TEXT")]
    pub description: Option<String>,
    /// A new owner: `me`, an email address, a name, or `none`
    #[arg(long, value_name = "WHO")]
    pub owner: Option<String>,
    /// A new lead team, by key, or `none`; a team initiative is a Linear Business plan feature, refused, exit 6, below it
    #[arg(long, value_name = "KEY")]
    pub lead_team: Option<String>,
    /// A new status
    #[arg(long, value_name = "STATUS", value_enum)]
    pub status: Option<InitiativeStatus>,
    /// A new planned end, a date (2026-12-01), or `none`
    #[arg(long, value_name = "DATE")]
    pub target_date: Option<String>,
    /// A new priority: 0 none, 1 urgent, 2 high, 3 medium, 4 low
    #[arg(long, value_name = "0-4", value_parser = clap::value_parser!(u8).range(0..=4))]
    pub priority: Option<u8>,
    /// A new content, markdown
    #[arg(long, value_name = "TEXT", conflicts_with = "content_file")]
    pub content: Option<String>,
    /// The content from a file, or from stdin when PATH is `-`
    #[arg(long, value_name = "PATH")]
    pub content_file: Option<String>,
}

/// An initiative's lifecycle status, as Linear's `InitiativeStatus` spells it.
#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitiativeStatus {
    /// Proposed.
    #[value(name = "Proposed")]
    Proposed,
    /// Planned.
    #[value(name = "Planned")]
    Planned,
    /// Active.
    #[value(name = "Active")]
    Active,
    /// Completed.
    #[value(name = "Completed")]
    Completed,
    /// Canceled.
    #[value(name = "Canceled")]
    Canceled,
}

impl InitiativeStatus {
    /// The value Linear takes.
    pub fn linear(self) -> &'static str {
        match self {
            InitiativeStatus::Proposed => "Proposed",
            InitiativeStatus::Planned => "Planned",
            InitiativeStatus::Active => "Active",
            InitiativeStatus::Completed => "Completed",
            InitiativeStatus::Canceled => "Canceled",
        }
    }
}

/// The cycle commands (ADR 0014, group 13). Linear makes a team's cycles from its
/// settings (`team update --cycles-enabled true`), so there is no create; `cycleArchive`
/// unlinks every issue with no inverse and stays out.
#[derive(Subcommand, Debug)]
pub enum CycleCommand {
    /// List the team's cycles, one JSON object per line, newest first
    List(CycleListArgs),
    /// One cycle with its issues, by number
    Get(CycleRefArgs),
    /// Change a cycle's name, description, start or end
    Update(Box<CycleUpdateArgs>),
}

/// `cycle list`.
#[derive(Args, Debug)]
pub struct CycleListArgs {
    /// Only the active cycle
    #[arg(long, conflicts_with_all = ["upcoming", "past"])]
    pub active: bool,
    /// Only the cycles still to come
    #[arg(long, conflicts_with_all = ["active", "past"])]
    pub upcoming: bool,
    /// Only the cycles that have ended
    #[arg(long, conflicts_with_all = ["active", "upcoming"])]
    pub past: bool,
    /// At most this many, up to PAGE_CAP; a cut list writes a `truncated` notice to stderr
    #[arg(long, default_value_t = PAGE_SIZE, value_name = "N")]
    pub limit: usize,
}

/// A cycle of the team, by number.
#[derive(Args, Debug)]
pub struct CycleRefArgs {
    /// The cycle's number in its team
    #[arg(value_name = "NUMBER")]
    pub number: u64,
}

/// `cycle update`.
#[derive(Args, Debug, Default)]
pub struct CycleUpdateArgs {
    /// The cycle's number in its team
    #[arg(value_name = "NUMBER")]
    pub number: u64,
    /// A new name
    #[arg(long, value_name = "TEXT")]
    pub name: Option<String>,
    /// A new description
    #[arg(long, value_name = "TEXT")]
    pub description: Option<String>,
    /// A new start, a time (2026-11-10T00:00:00Z)
    #[arg(long, value_name = "TIME")]
    pub starts_at: Option<String>,
    /// A new end, a time (2026-11-24T00:00:00Z)
    #[arg(long, value_name = "TIME")]
    pub ends_at: Option<String>,
}

/// The document commands (ADR 0014, group 14).
#[derive(Subcommand, Debug)]
pub enum DocumentCommand {
    /// List documents, one JSON object per line, without their content
    List(DocumentListArgs),
    /// One document with its content, by title
    Get(DocumentRefArgs),
    /// Create a document; a title a document already has is refused unless --allow-duplicate
    Create(Box<DocumentCreateArgs>),
    /// Change a document's title, content, project or issue
    Update(Box<DocumentUpdateArgs>),
    /// Put a document in the trash; `document unarchive` brings it back
    Delete(DocumentRefArgs),
    /// Bring a trashed document back
    Unarchive(DocumentRefArgs),
}

/// `document list`.
#[derive(Args, Debug)]
pub struct DocumentListArgs {
    /// Only documents of this project, by name
    #[arg(long, value_name = "NAME")]
    pub project: Option<String>,
    /// Only documents of this issue, by identifier (APP-12)
    #[arg(long, value_name = "ISSUE")]
    pub issue: Option<String>,
    /// At most this many, up to PAGE_CAP; a cut list writes a `truncated` notice to stderr
    #[arg(long, default_value_t = PAGE_SIZE, value_name = "N")]
    pub limit: usize,
}

/// A document, by title.
#[derive(Args, Debug)]
pub struct DocumentRefArgs {
    /// The document, by title
    #[arg(value_name = "TITLE")]
    pub title: String,
}

/// `document create`.
#[derive(Args, Debug, Default)]
pub struct DocumentCreateArgs {
    /// The title
    #[arg(long, value_name = "TEXT")]
    pub title: String,
    /// The content, markdown
    #[arg(long, value_name = "TEXT", conflicts_with = "content_file")]
    pub content: Option<String>,
    /// The content, markdown, from a file, or from stdin when PATH is `-`
    #[arg(long, value_name = "PATH")]
    pub content_file: Option<String>,
    /// The project it belongs to, by name
    #[arg(long, value_name = "NAME")]
    pub project: Option<String>,
    /// The issue it belongs to, by identifier (APP-12)
    #[arg(long, value_name = "ISSUE")]
    pub issue: Option<String>,
    /// Create it even when a document has the title
    #[arg(long)]
    pub allow_duplicate: bool,
}

/// `document update`.
#[derive(Args, Debug, Default)]
pub struct DocumentUpdateArgs {
    /// The document, by title
    #[arg(value_name = "TITLE")]
    pub document: String,
    /// A new title
    #[arg(long, value_name = "TEXT")]
    pub title: Option<String>,
    /// New content, markdown
    #[arg(long, value_name = "TEXT", conflicts_with = "content_file")]
    pub content: Option<String>,
    /// New content, markdown, from a file, or from stdin when PATH is `-`
    #[arg(long, value_name = "PATH")]
    pub content_file: Option<String>,
    /// A new project, by name, or `none`
    #[arg(long, value_name = "NAME")]
    pub project: Option<String>,
    /// A new issue, by identifier, or `none`
    #[arg(long, value_name = "ISSUE")]
    pub issue: Option<String>,
}

/// The template commands (ADR 0014, group 15).
#[derive(Subcommand, Debug)]
pub enum TemplateCommand {
    /// List the team's and the workspace's templates, one JSON object per line
    List(TemplateListArgs),
    /// One template with its data, by name; `templateData` is a string of JSON, as Linear returns it
    Get(TemplateRefArgs),
    /// Create a template from a JSON data file; a name a template already has is refused unless --allow-duplicate
    Create(Box<TemplateCreateArgs>),
    /// Change a template's name, description or data
    Update(Box<TemplateUpdateArgs>),
    /// Delete a template; there is no undo
    Delete(TemplateRefArgs),
}

/// `template list`.
#[derive(Args, Debug)]
pub struct TemplateListArgs {
    /// Only templates of this kind
    #[arg(long, value_name = "TYPE", value_enum)]
    pub r#type: Option<TemplateType>,
    /// At most this many, up to PAGE_CAP; a cut list writes a `truncated` notice to stderr
    #[arg(long, default_value_t = PAGE_SIZE, value_name = "N")]
    pub limit: usize,
}

/// A template, by name.
#[derive(Args, Debug)]
pub struct TemplateRefArgs {
    /// The template, by name
    #[arg(value_name = "NAME")]
    pub name: String,
}

/// `template create`.
#[derive(Args, Debug, Default)]
pub struct TemplateCreateArgs {
    /// The name
    #[arg(long, value_name = "TEXT")]
    pub name: String,
    /// What the template makes
    #[arg(long, value_name = "TYPE", value_enum)]
    pub r#type: TemplateType,
    /// The template's data, JSON as Linear's `templateData`, from a file, or from stdin when PATH is `-`
    #[arg(long, value_name = "PATH")]
    pub data_file: String,
    /// A template every team shares, rather than the team's own
    #[arg(long)]
    pub workspace: bool,
    /// The description
    #[arg(long, value_name = "TEXT")]
    pub description: Option<String>,
    /// Create it even when a template has the name
    #[arg(long)]
    pub allow_duplicate: bool,
}

/// `template update`.
#[derive(Args, Debug, Default)]
pub struct TemplateUpdateArgs {
    /// The template, by name
    #[arg(value_name = "NAME")]
    pub template: String,
    /// A new name
    #[arg(long, value_name = "TEXT")]
    pub name: Option<String>,
    /// A new description
    #[arg(long, value_name = "TEXT")]
    pub description: Option<String>,
    /// New data, JSON as Linear's `templateData`, from a file, or from stdin when PATH is `-`
    #[arg(long, value_name = "PATH")]
    pub data_file: Option<String>,
}

/// What a template makes, as Linear's `type` spells it.
#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TemplateType {
    /// An issue.
    #[default]
    Issue,
    /// A project.
    Project,
    /// A document.
    Document,
}

impl TemplateType {
    /// The value Linear takes and returns.
    pub fn linear(self) -> &'static str {
        match self {
            TemplateType::Issue => "issue",
            TemplateType::Project => "project",
            TemplateType::Document => "document",
        }
    }
}

/// The milestone commands (ADR 0014, group 10): the steps inside a project.
#[derive(Subcommand, Debug)]
pub enum MilestoneCommand {
    /// List a project's milestones, one JSON object per line
    List(MilestoneListArgs),
    /// Add a milestone to a project; a name a milestone of the project has is refused unless --allow-duplicate
    Create(MilestoneCreateArgs),
    /// Change a milestone's name, description or target date
    Update(MilestoneUpdateArgs),
    /// Delete a milestone; no undo in Linear
    Delete(MilestoneRefArgs),
}

/// `milestone list`.
#[derive(Args, Debug)]
pub struct MilestoneListArgs {
    /// The project, by name
    #[arg(long, value_name = "NAME")]
    pub project: String,
    /// At most this many, up to PAGE_CAP; a cut list writes a `truncated` notice to stderr
    #[arg(long, default_value_t = PAGE_SIZE, value_name = "N")]
    pub limit: usize,
}

/// A milestone named in its project.
#[derive(Args, Debug)]
pub struct MilestoneRefArgs {
    /// The milestone, by name
    #[arg(value_name = "NAME")]
    pub name: String,
    /// The project, by name
    #[arg(long, value_name = "NAME")]
    pub project: String,
}

/// `milestone create`.
#[derive(Args, Debug, Default)]
pub struct MilestoneCreateArgs {
    /// The project, by name
    #[arg(long, value_name = "NAME")]
    pub project: String,
    /// The name
    #[arg(long, value_name = "TEXT")]
    pub name: String,
    /// The description, markdown
    #[arg(long, value_name = "TEXT")]
    pub description: Option<String>,
    /// The planned date, a date (2026-12-01)
    #[arg(long, value_name = "DATE")]
    pub target_date: Option<String>,
    /// Create it even when a milestone of the project has the name
    #[arg(long)]
    pub allow_duplicate: bool,
}

/// `milestone update`.
#[derive(Args, Debug, Default)]
pub struct MilestoneUpdateArgs {
    /// The milestone, by name
    #[arg(value_name = "NAME")]
    pub milestone: String,
    /// The project, by name
    #[arg(long, value_name = "NAME")]
    pub project: String,
    /// A new name
    #[arg(long, value_name = "TEXT")]
    pub name: Option<String>,
    /// A new description, markdown
    #[arg(long, value_name = "TEXT")]
    pub description: Option<String>,
    /// A new planned date, a date (2026-12-01), or `none`
    #[arg(long, value_name = "DATE")]
    pub target_date: Option<String>,
}

/// The project update commands (ADR 0014, group 11): the status posts on a project,
/// which `project update` does not touch.
#[derive(Subcommand, Debug)]
pub enum ProjectUpdateCommand {
    /// List a project's status posts, newest first, one JSON object per line
    List(ProjectUpdateListArgs),
    /// Post a status on a project, with a body, a health, or both
    Create(ProjectUpdateCreateArgs),
}

/// `project-update list`.
#[derive(Args, Debug)]
pub struct ProjectUpdateListArgs {
    /// The project, by name
    #[arg(long, value_name = "NAME")]
    pub project: String,
    /// At most this many, up to PAGE_CAP; a cut list writes a `truncated` notice to stderr
    #[arg(long, default_value_t = PAGE_SIZE, value_name = "N")]
    pub limit: usize,
}

/// `project-update create`.
#[derive(Args, Debug, Default)]
pub struct ProjectUpdateCreateArgs {
    /// The project, by name
    #[arg(long, value_name = "NAME")]
    pub project: String,
    /// The post, markdown
    #[arg(long, value_name = "TEXT", conflicts_with = "body_file")]
    pub body: Option<String>,
    /// The post from a file, or from stdin when PATH is `-`
    #[arg(long, value_name = "PATH")]
    pub body_file: Option<String>,
    /// The project's health as of this post
    #[arg(long, value_name = "HEALTH", value_enum)]
    pub health: Option<Health>,
}

/// A project's health, as Linear's `ProjectUpdateHealthType` spells it.
#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Health {
    /// Going to plan.
    #[value(name = "onTrack")]
    OnTrack,
    /// In danger of slipping.
    #[value(name = "atRisk")]
    AtRisk,
    /// Slipped.
    #[value(name = "offTrack")]
    OffTrack,
}

impl Health {
    /// The value Linear takes.
    pub fn linear(self) -> &'static str {
        match self {
            Health::OnTrack => "onTrack",
            Health::AtRisk => "atRisk",
            Health::OffTrack => "offTrack",
        }
    }
}

/// `project create`.
#[derive(Args, Debug)]
pub struct ProjectCreateArgs {
    /// The name
    #[arg(long, value_name = "TEXT")]
    pub name: String,
    /// The description, plain text
    #[arg(long, value_name = "TEXT")]
    pub description: Option<String>,
    /// Create it even when a project of the workspace has the same name
    #[arg(long)]
    pub allow_duplicate: bool,
}

/// The issue commands. The two with many flags are boxed so that the enum stays small
/// (clippy's `large_enum_variant`); `clap` reads through the box.
#[derive(Subcommand, Debug)]
pub enum IssueCommand {
    /// One issue with its comments, attachments and sub-issues
    Get(GetArgs),
    /// Issues of the team: the open ones unless --state or --all; most recently updated first
    List(IssueListArgs),
    /// Create an issue in the team; a title an open issue of the team already has is refused unless --allow-duplicate
    Create(Box<CreateArgs>),
    /// Add a comment to an issue
    Comment(CommentArgs),
    /// Add a link to an issue: a URL with a title, which Linear shows as an attachment
    Attach(AttachArgs),
    /// Move an issue to the team's completed state, or to --state, or with --cancel to its canceled state, after an optional comment; prints the comment, then the issue
    Close(CloseArgs),
    /// Change an issue's title, state, labels, priority, assignee, project, parent or description
    Update(Box<UpdateArgs>),
}

/// `issue get`.
#[derive(Args, Debug)]
pub struct GetArgs {
    /// The identifier (APP-12), the number alone (12, in the team) or the id
    #[arg(value_name = "ISSUE")]
    pub issue: String,
    /// Leave the comments and attachments out
    #[arg(long)]
    pub no_comments: bool,
}

/// `issue list`.
#[derive(Args, Debug)]
pub struct IssueListArgs {
    /// Only issues in this state, by name; repeatable
    #[arg(long, value_name = "NAME")]
    pub state: Vec<String>,
    /// Every state, the completed and canceled ones included
    #[arg(long, conflicts_with = "state")]
    pub all: bool,
    /// Only issues carrying this label
    #[arg(long, value_name = "NAME")]
    pub label: Option<String>,
    /// Only issues assigned to this user: `me`, an email address, a name, or `none`
    #[arg(long, value_name = "WHO")]
    pub assignee: Option<String>,
    /// Only issues whose title contains this text, case-insensitively; how a title is resolved
    #[arg(long, value_name = "TEXT")]
    pub search: Option<String>,
    /// Only the sub-issues of this issue: the identifier (APP-12), the number alone (12, in the team) or the id; or `none` for issues with no parent
    #[arg(long, value_name = "ISSUE")]
    pub parent: Option<String>,
    /// Only issues in this project, by name, or `none` for issues in no project
    #[arg(long, value_name = "NAME")]
    pub project: Option<String>,
    /// Only issues in this milestone, by name, or `none` for issues in no milestone
    #[arg(long, value_name = "NAME")]
    pub milestone: Option<String>,
    /// Only issues in this cycle of the team: a number, `active`, `next`, or `none` for issues in no cycle
    #[arg(long, value_name = "NUMBER|active|next|none")]
    pub cycle: Option<String>,
    /// At most this many, up to PAGE_CAP; a cut list writes a `truncated` notice to stderr
    #[arg(long, default_value_t = PAGE_SIZE, value_name = "N")]
    pub limit: usize,
}

/// `issue create`.
#[derive(Args, Debug)]
pub struct CreateArgs {
    /// The title
    #[arg(long, value_name = "TEXT")]
    pub title: String,
    /// The description, markdown
    #[arg(long, value_name = "TEXT", conflicts_with = "description_file")]
    pub description: Option<String>,
    /// The description from a file, or from stdin when PATH is `-`
    #[arg(long, value_name = "PATH")]
    pub description_file: Option<String>,
    /// The state, by name; the team's default state when absent
    #[arg(long, value_name = "NAME")]
    pub state: Option<String>,
    /// A label, by name; repeatable
    #[arg(long, value_name = "NAME")]
    pub label: Vec<String>,
    /// The priority: 0 none, 1 urgent, 2 high, 3 medium, 4 low
    #[arg(long, value_name = "0-4", value_parser = clap::value_parser!(u8).range(0..=4))]
    pub priority: Option<u8>,
    /// The project, by name
    #[arg(long, value_name = "NAME")]
    pub project: Option<String>,
    /// The milestone, by name, in the project --project names
    #[arg(long, value_name = "NAME", requires = "project")]
    pub milestone: Option<String>,
    /// The cycle of the team to put it in: a number, `active` or `next`
    #[arg(long, value_name = "NUMBER|active|next")]
    pub cycle: Option<String>,
    /// The assignee: `me`, an email address or a name
    #[arg(long, value_name = "WHO")]
    pub assignee: Option<String>,
    /// The parent, which makes this a sub-issue: the identifier (APP-12), the number alone (12, in the team) or the id; the team is the parent's unless --team or MUSTER_TEAM names one
    #[arg(long, value_name = "ISSUE")]
    pub parent: Option<String>,
    /// Create it even when an open issue of the team has the same title
    #[arg(long)]
    pub allow_duplicate: bool,
}

/// `issue comment`.
#[derive(Args, Debug)]
pub struct CommentArgs {
    /// The identifier (APP-12), the number alone (12, in the team) or the id
    #[arg(value_name = "ISSUE")]
    pub issue: String,
    /// The comment, markdown
    #[arg(
        long,
        value_name = "TEXT",
        conflicts_with = "body_file",
        required_unless_present = "body_file"
    )]
    pub body: Option<String>,
    /// The comment from a file, or from stdin when PATH is `-`
    #[arg(long, value_name = "PATH")]
    pub body_file: Option<String>,
}

/// `issue attach`.
#[derive(Args, Debug)]
pub struct AttachArgs {
    /// The identifier (APP-12), the number alone (12, in the team) or the id
    #[arg(value_name = "ISSUE")]
    pub issue: String,
    /// The link; a URL the issue already has is updated in place, with the same id, not added again
    #[arg(long, value_name = "URL")]
    pub url: String,
    /// The title Linear shows for the link; required, because Linear requires one and muster invents none
    #[arg(long, value_name = "TEXT")]
    pub title: String,
    /// A line under the title
    #[arg(long, value_name = "TEXT")]
    pub subtitle: Option<String>,
}

/// `issue close`.
#[derive(Args, Debug)]
pub struct CloseArgs {
    /// The identifier (APP-12), the number alone (12, in the team) or the id
    #[arg(value_name = "ISSUE")]
    pub issue: String,
    /// The state to move to, by name, instead of the team's completed state
    #[arg(long, value_name = "NAME", conflicts_with = "cancel")]
    pub state: Option<String>,
    /// Move to the team's canceled state instead
    #[arg(long)]
    pub cancel: bool,
    /// A closing comment, markdown, added before the move
    #[arg(long, value_name = "TEXT", conflicts_with = "comment_file")]
    pub comment: Option<String>,
    /// The closing comment from a file, or from stdin when PATH is `-`
    #[arg(long, value_name = "PATH")]
    pub comment_file: Option<String>,
}

/// `issue update`.
#[derive(Args, Debug)]
pub struct UpdateArgs {
    /// The identifier (APP-12), the number alone (12, in the team) or the id
    #[arg(value_name = "ISSUE")]
    pub issue: String,
    /// A new title
    #[arg(long, value_name = "TEXT")]
    pub title: Option<String>,
    /// A new state, by name
    #[arg(long, value_name = "NAME")]
    pub state: Option<String>,
    /// A label to add, by name; repeatable
    #[arg(long, value_name = "NAME")]
    pub add_label: Vec<String>,
    /// A label to remove, by name; repeatable
    #[arg(long, value_name = "NAME")]
    pub remove_label: Vec<String>,
    /// A new priority: 0 none, 1 urgent, 2 high, 3 medium, 4 low
    #[arg(long, value_name = "0-4", value_parser = clap::value_parser!(u8).range(0..=4))]
    pub priority: Option<u8>,
    /// A new assignee: `me`, an email address, a name, or `none` to unassign
    #[arg(long, value_name = "WHO")]
    pub assignee: Option<String>,
    /// A new project, by name, or `none` to remove it
    #[arg(long, value_name = "NAME")]
    pub project: Option<String>,
    /// A new milestone, by name, in the issue's project (or the one --project names), or `none` to remove it
    #[arg(long, value_name = "NAME")]
    pub milestone: Option<String>,
    /// A new cycle, in the issue's team: a number, `active`, `next`, or `none` to take it out of its cycle
    #[arg(long, value_name = "NUMBER|active|next|none")]
    pub cycle: Option<String>,
    /// A new parent, which makes this a sub-issue, or `none` to take it out from under its parent
    #[arg(long, value_name = "ISSUE")]
    pub parent: Option<String>,
    /// A new description, markdown
    #[arg(long, value_name = "TEXT", conflicts_with = "description_file")]
    pub description: Option<String>,
    /// The new description from a file, or from stdin when PATH is `-`
    #[arg(long, value_name = "PATH")]
    pub description_file: Option<String>,
}

/// `schema`.
#[derive(Args, Debug)]
pub struct SchemaArgs {
    /// `json`, one object; or `markdown`, the text of docs/reference/commands.md
    #[arg(long, value_enum, default_value_t = SchemaFormat::Json)]
    pub format: SchemaFormat,
}

/// The two renderings of the schema.
#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaFormat {
    /// One JSON object.
    Json,
    /// The reference page.
    Markdown,
}

/// `fixtures`.
#[derive(Args, Debug)]
pub struct FixturesArgs {
    /// The directory to write one `<Operation>.json` per read operation into
    #[arg(value_name = "DIR")]
    pub dir: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn the_tree_is_well_formed() {
        Cli::command().debug_assert();
    }
}
