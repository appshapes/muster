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
    /// Teams of the workspace
    Team {
        /// `list`.
        #[command(subcommand)]
        command: ListCommand,
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
}

/// `list` under `team`, `state`, `label`, `project` and `user`.
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
