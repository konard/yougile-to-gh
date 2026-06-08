use std::fs;
use std::path::PathBuf;
use std::process;

use clap::{ArgAction, Parser, ValueEnum};
use yougile_to_gh::{
    build_conversion_plan, execute_conversion_plan, fetch_task_tree, ConversionMode,
    ConversionOptions, FetchOptions, GitHubClient, GitHubRepository, Result, YougileClient,
    YougileTaskTree, YougileToGhError,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum CliMode {
    SingleIssue,
    IssueTree,
}

impl From<CliMode> for ConversionMode {
    fn from(value: CliMode) -> Self {
        match value {
            CliMode::SingleIssue => Self::SingleIssue,
            CliMode::IssueTree => Self::IssueTree,
        }
    }
}

#[derive(Debug, Parser)]
#[command(
    name = "yougile-to-gh",
    about = "Convert a YouGile task tree into GitHub issue(s)"
)]
struct Args {
    #[arg(
        long,
        env = "YOUGILE_BASE_URL",
        default_value = "https://ru.yougile.com"
    )]
    yougile_base_url: String,

    #[arg(long, env = "YOUGILE_TOKEN")]
    yougile_token: Option<String>,

    #[arg(long, env = "YOUGILE_TASK_ID")]
    task_id: Option<String>,

    #[arg(long)]
    task_json: Option<PathBuf>,

    #[arg(long, env = "GITHUB_TOKEN")]
    github_token: Option<String>,

    #[arg(long, env = "GITHUB_REPOSITORY")]
    github_repo: Option<String>,

    #[arg(long, env = "GITHUB_API_URL", default_value = "https://api.github.com")]
    github_api_url: String,

    #[arg(long, value_enum, default_value = "issue-tree")]
    mode: CliMode,

    #[arg(long)]
    dry_run: bool,

    #[arg(long, action = ArgAction::Append)]
    label: Vec<String>,

    #[arg(long, action = ArgAction::Append)]
    assignee: Vec<String>,

    #[arg(long)]
    max_depth: Option<usize>,

    #[arg(long)]
    include_deleted: bool,

    #[arg(long)]
    include_system_messages: bool,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        process::exit(1);
    }
}

fn run() -> Result<()> {
    let args = Args::parse();
    let tree = load_task_tree(&args)?;
    let options = ConversionOptions {
        labels: args.label,
        assignees: args.assignee,
    };
    let plan = build_conversion_plan(&tree, args.mode.into(), &options);

    if args.dry_run {
        print_json(&plan)?;
        return Ok(());
    }

    let token = required(
        args.github_token.as_deref(),
        "GITHUB_TOKEN or --github-token",
    )?;
    let repo = required(
        args.github_repo.as_deref(),
        "GITHUB_REPOSITORY or --github-repo",
    )?;
    let repository = GitHubRepository::parse(repo)?;
    let github = GitHubClient::with_http_client(
        args.github_api_url,
        token.to_owned(),
        repository,
        yougile_to_gh::http::UreqHttpClient::new(),
    );
    let result = execute_conversion_plan(&plan, &github)?;
    print_json(&result)
}

fn load_task_tree(args: &Args) -> Result<YougileTaskTree> {
    if let Some(path) = &args.task_json {
        let content = fs::read_to_string(path)
            .map_err(|source| YougileToGhError::io(path.display().to_string(), source))?;
        return serde_json::from_str(&content)
            .map_err(|source| YougileToGhError::json(path.display().to_string(), source));
    }

    let task_id = required(args.task_id.as_deref(), "YOUGILE_TASK_ID or --task-id")?;
    let token = required(
        args.yougile_token.as_deref(),
        "YOUGILE_TOKEN or --yougile-token",
    )?;
    let client = YougileClient::new(&args.yougile_base_url, token)
        .include_deleted(args.include_deleted)
        .include_system_messages(args.include_system_messages);

    fetch_task_tree(
        &client,
        task_id,
        FetchOptions {
            max_depth: args.max_depth,
        },
    )
}

fn print_json<T: serde::Serialize>(value: &T) -> Result<()> {
    let stdout = std::io::stdout();
    let handle = stdout.lock();
    serde_json::to_writer_pretty(handle, value)
        .map_err(|source| YougileToGhError::json("stdout", source))?;
    println!();
    Ok(())
}

fn required<'a>(value: Option<&'a str>, name: &'static str) -> Result<&'a str> {
    value
        .filter(|value| !value.trim().is_empty())
        .ok_or(YougileToGhError::MissingValue(name))
}
