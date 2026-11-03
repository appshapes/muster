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
    /// Workflow states of the team
    State {
        /// `list`.
        #[command(subcommand)]
        command: ListCommand,
    },
    /// Labels of the team and of the workspace
    Label {
        /// `list`.
        #[command(subcommand)]
        command: ListCommand,
    },
    /// Projects: list, create
    Project {
        /// The project command.
        #[command(subcommand)]
        command: ProjectCommand,
    },
    /// Users of the workspace
    User {
        /// `list`.
        #[command(subcommand)]
        command: ListCommand,
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
    #[arg(long, value_name = "TEXT")]
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
    /// Whether the roadmap is on
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
    /// The sign-in services allowed, comma-separated (google,saml), or `all`
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
    /// Create a team: its creator becomes a member and owner and Linear makes its default states and labels; a name a team already has is refused unless --allow-duplicate
    Create(Box<TeamCreateArgs>),
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

/// `list` under `state`, `label` and `user`.
#[derive(Subcommand, Debug)]
pub enum ListCommand {
    /// List them, one JSON object per line
    List(ListArgs),
}

/// The one flag of every plain list.
#[derive(Args, Debug)]
pub struct ListArgs {
    /// At most this many, up to PAGE_CAP; a cut list writes a `truncated` notice to stderr
    #[arg(long, default_value_t = PAGE_SIZE, value_name = "N")]
    pub limit: usize,
}

/// The project commands.
#[derive(Subcommand, Debug)]
pub enum ProjectCommand {
    /// List them, one JSON object per line
    List(ListArgs),
    /// Create a project for the team; a name a project of the workspace already has is refused unless --allow-duplicate
    Create(ProjectCreateArgs),
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
