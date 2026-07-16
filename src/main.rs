use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

use clap::ArgAction;
use command_stream::{CommandResult, RunOptions, StdinOption};
use lino_arguments::{LinoEnv, Parser, ValueEnum};
use tokio::runtime::Builder;
use yougile_to_gh::{
    build_conversion_plan, execute_conversion_plan, fetch_task_tree, resolve_task_url,
    ConversionMode, ConversionOptions, FetchOptions, GitHubClient, GitHubRepository,
    ProjectTitleCache, Result, TaskUrlContext, YougileAuth, YougileClient, YougileTaskTree,
    YougileToGhError,
};

/// Default `.lenv` file used to persist a resolved `YouGile` token.
const DEFAULT_LENV_PATH: &str = ".lenv";

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
#[allow(clippy::struct_excessive_bools)]
struct Args {
    #[arg(
        long,
        env = "YOUGILE_BASE_URL",
        default_value = "https://ru.yougile.com"
    )]
    yougile_base_url: String,

    #[arg(long, env = "YOUGILE_TOKEN")]
    yougile_token: Option<String>,

    #[arg(long, env = "YOUGILE_LOGIN")]
    yougile_login: Option<String>,

    #[arg(long, env = "YOUGILE_PASSWORD")]
    yougile_password: Option<String>,

    #[arg(long, env = "YOUGILE_COMPANY_ID")]
    yougile_company_id: Option<String>,

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

    /// Path to the `.lenv` file used to persist a resolved `YouGile` token.
    #[arg(long, env = "YOUGILE_LENV_PATH", default_value = DEFAULT_LENV_PATH)]
    lenv_path: PathBuf,

    /// Do not persist a credential-resolved `YouGile` token to the `.lenv` file.
    #[arg(long)]
    no_save_token: bool,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        process::exit(1);
    }
}

fn run() -> Result<()> {
    // Load `.lenv` (and `.env`) into the process environment before clap reads
    // its `env = ...` defaults, so a previously persisted token is reused.
    lino_arguments::init();
    let args = Args::parse();
    let loaded = load_task_tree(&args)?;
    let options = ConversionOptions {
        labels: args.label.clone(),
        assignees: args.assignee.clone(),
        task_urls: collect_task_urls(&args, &loaded),
    };
    let plan = build_conversion_plan(&loaded.tree, args.mode.into(), &options);

    if args.dry_run {
        print_json(&plan)?;
        return Ok(());
    }

    let token = resolve_github_token(args.github_token.as_deref())?;
    let repo = resolve_github_repo(args.github_repo.as_deref())?;
    let repository = GitHubRepository::parse(&repo)?;
    let github = GitHubClient::with_http_client(
        args.github_api_url,
        token,
        repository,
        yougile_to_gh::http::UreqHttpClient::new(),
    );
    let result = execute_conversion_plan(&plan, &github)?;
    print_json(&result)
}

/// A task tree, plus what is needed to link its tasks back to `YouGile`.
///
/// The client and company id are absent for a `--task-json` run, which never
/// reaches the API and therefore renders no links.
struct LoadedTaskTree {
    tree: YougileTaskTree,
    client: Option<YougileClient>,
    company_id: Option<String>,
}

fn load_task_tree(args: &Args) -> Result<LoadedTaskTree> {
    if let Some(path) = &args.task_json {
        let content = fs::read_to_string(path)
            .map_err(|source| YougileToGhError::io(path.display().to_string(), source))?;
        let tree = serde_json::from_str(&content)
            .map_err(|source| YougileToGhError::json(path.display().to_string(), source))?;
        return Ok(LoadedTaskTree {
            tree,
            client: None,
            company_id: None,
        });
    }

    let task_id = required(args.task_id.as_deref(), "YOUGILE_TASK_ID or --task-id")?;
    let (token, company_id) = resolve_yougile_token(args)?;
    let client = YougileClient::new(&args.yougile_base_url, token)
        .include_deleted(args.include_deleted)
        .include_system_messages(args.include_system_messages);

    let tree = fetch_task_tree(
        &client,
        task_id,
        FetchOptions {
            max_depth: args.max_depth,
        },
    )?;
    Ok(LoadedTaskTree {
        tree,
        client: Some(client),
        company_id,
    })
}

/// Build the `YouGile` link for every task in the tree, keyed by task id.
///
/// Links need the company id, so they are skipped when it is unknown — a link
/// is a convenience, and lacking one must not fail the conversion.
fn collect_task_urls(args: &Args, loaded: &LoadedTaskTree) -> BTreeMap<String, String> {
    let mut urls = BTreeMap::new();
    let Some(client) = loaded.client.as_ref() else {
        return urls;
    };
    let Some(company_id) = non_empty(args.yougile_company_id.as_deref())
        .or_else(|| non_empty(loaded.company_id.as_deref()))
    else {
        return urls;
    };
    let Some(context) = TaskUrlContext::new(&args.yougile_base_url, company_id) else {
        return urls;
    };

    let mut cache = ProjectTitleCache::new();
    // Warn once: the same unreachable API would repeat this for every task.
    let mut warned = false;
    let mut warn = |error: &YougileToGhError| {
        if !warned {
            warned = true;
            eprintln!("warning: YouGile task links omit the project name: {error}");
        }
    };

    collect_task_urls_recursive(
        client,
        &loaded.tree,
        &context,
        &mut cache,
        &mut warn,
        &mut urls,
    );
    urls
}

fn collect_task_urls_recursive(
    client: &YougileClient,
    tree: &YougileTaskTree,
    context: &TaskUrlContext,
    cache: &mut ProjectTitleCache,
    on_title_error: &mut impl FnMut(&YougileToGhError),
    urls: &mut BTreeMap<String, String>,
) {
    if let Some(url) = resolve_task_url(client, &tree.task, context, cache, on_title_error) {
        urls.insert(tree.task.id.clone(), url);
    }

    for subtask in &tree.subtasks {
        collect_task_urls_recursive(client, subtask, context, cache, on_title_error, urls);
    }
}

/// Resolve a usable `YouGile` API token, and the company it belongs to.
///
/// A token supplied directly (via `--yougile-token` / `YOUGILE_TOKEN`, which
/// also covers values loaded from `.lenv`) wins; its company is only known if
/// it was configured alongside. Otherwise, when a login and password are
/// available, the token is created through the `YouGile` authentication API and
/// persisted to the `.lenv` file for reuse, together with the company it
/// resolved to, which task URLs are built from.
fn resolve_yougile_token(args: &Args) -> Result<(String, Option<String>)> {
    if let Some(token) = non_empty(args.yougile_token.as_deref()) {
        return Ok((
            token.to_owned(),
            non_empty(args.yougile_company_id.as_deref()).map(str::to_owned),
        ));
    }

    let login = required(
        args.yougile_login.as_deref(),
        "YOUGILE_TOKEN/--yougile-token or YOUGILE_LOGIN/--yougile-login",
    )?;
    let password = required(
        args.yougile_password.as_deref(),
        "YOUGILE_PASSWORD or --yougile-password",
    )?;

    let auth = YougileAuth::new(&args.yougile_base_url);
    let resolved = auth.resolve_token(
        login,
        password,
        non_empty(args.yougile_company_id.as_deref()),
    )?;

    if !args.no_save_token {
        save_credentials_to_lenv(&args.lenv_path, &resolved.token, &resolved.company_id)?;
    }

    Ok((resolved.token, Some(resolved.company_id)))
}

/// Persist the resolved `YouGile` token and company to the `.lenv` file,
/// preserving any existing entries.
fn save_credentials_to_lenv(path: &Path, token: &str, company_id: &str) -> Result<()> {
    let path_display = path.display().to_string();
    let path_str = path
        .to_str()
        .ok_or(YougileToGhError::MissingValue("a UTF-8 --lenv-path"))?;

    let mut lenv = LinoEnv::new(path_str);
    lenv.read()
        .map_err(|source| YougileToGhError::io(path_display.clone(), source))?;
    lenv.set("YOUGILE_TOKEN", token);
    lenv.set("YOUGILE_COMPANY_ID", company_id);
    lenv.write()
        .map_err(|source| YougileToGhError::io(path_display, source))?;
    Ok(())
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

fn resolve_github_token(value: Option<&str>) -> Result<String> {
    if let Some(value) = non_empty(value) {
        return Ok(value.to_owned());
    }

    run_gh_detection(
        &["auth", "token"],
        "gh auth token",
        "GitHub token",
        "GITHUB_TOKEN or --github-token",
    )
}

fn resolve_github_repo(value: Option<&str>) -> Result<String> {
    if let Some(value) = non_empty(value) {
        return Ok(value.to_owned());
    }

    run_gh_detection(
        &[
            "repo",
            "view",
            "--json",
            "nameWithOwner",
            "--jq",
            ".nameWithOwner",
        ],
        "gh repo view --json nameWithOwner --jq .nameWithOwner",
        "GitHub repository",
        "GITHUB_REPOSITORY or --github-repo",
    )
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn run_gh_detection(
    args: &[&str],
    display_command: &'static str,
    value_name: &'static str,
    fallback: &'static str,
) -> Result<String> {
    trace_gh(display_command);

    let command = gh_command(args);
    let output =
        run_command_stream(&command).map_err(|source| YougileToGhError::GitHubCliDetection {
            value: value_name,
            command: display_command,
            fallback,
            message: source.to_string(),
        })?;

    let value = output.stdout.trim();
    if output.is_success() && !value.is_empty() {
        return Ok(value.to_owned());
    }

    let message = if output.is_success() {
        "command produced empty stdout".to_owned()
    } else {
        let details = output.stderr.trim();
        let status = if output.code < 0 {
            "terminated by signal".to_owned()
        } else {
            output.code.to_string()
        };

        if details.is_empty() {
            format!("exit status {status}")
        } else {
            format!("exit status {status}: {details}")
        }
    };

    Err(YougileToGhError::GitHubCliDetection {
        value: value_name,
        command: display_command,
        fallback,
        message,
    })
}

fn gh_command(args: &[&str]) -> String {
    std::iter::once("gh")
        .chain(args.iter().copied())
        .map(command_stream::quote)
        .collect::<Vec<_>>()
        .join(" ")
}

fn run_command_stream(command: &str) -> command_stream::Result<CommandResult> {
    let runtime = Builder::new_current_thread().enable_io().build()?;
    runtime.block_on(command_stream::exec(
        command,
        RunOptions {
            mirror: false,
            capture: true,
            stdin: StdinOption::Null,
            shell_operators: false,
            trace: false,
            ..Default::default()
        },
    ))
}

fn trace_gh(command: &str) {
    if env_flag("YOUGILE_TO_GH_TRACE_GH") {
        eprintln!("running {command}");
    }
}

fn env_flag(name: &str) -> bool {
    matches!(
        env::var(name).as_deref(),
        Ok("1" | "true" | "TRUE" | "yes" | "YES")
    )
}
