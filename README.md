# yougile-to-gh

Rust library and CLI for converting a YouGile task tree into GitHub issue content.

The converter fetches a root YouGile task, recursively follows `subtasks`, imports task chat messages, and creates either:

- one GitHub issue containing the full recursive task tree, or
- one GitHub issue per YouGile task, linked with GitHub sub-issues.

## CLI

Dry-run from a previously captured task tree:

```bash
cargo run -- --task-json task-tree.json --dry-run --mode issue-tree
```

Live recursive conversion:

```bash
YOUGILE_TOKEN=... \
cargo run -- \
  --yougile-base-url https://ru.yougile.com \
  --task-id YOU_GILE_TASK_ID \
  --mode issue-tree \
  --label imported-from-yougile
```

### YouGile authentication

A token may be supplied directly through `--yougile-token` / `YOUGILE_TOKEN`.
When no token is available, the CLI authenticates with your YouGile login and
password and creates an API key for you:

```bash
cargo run -- \
  --yougile-login you@example.com \
  --yougile-password secret \
  --task-id YOU_GILE_TASK_ID
```

The credential exchange uses the YouGile `AuthKeyController` endpoints
(`POST /api-v2/auth/companies` to list companies and `POST /api-v2/auth/keys`
to create the key). When your account belongs to a single company it is
selected automatically; otherwise pass `--yougile-company-id`
(or set `YOUGILE_COMPANY_ID`) with one of the listed company ids.

### Task links

Each issue body starts with a link back to the YouGile task, so the original is
one click away:

```markdown
**YouGile:** [ABC-42](https://ru.yougile.com/team/1a2b3c4d5e6f/Project-Name#ABC-42)
```

The link needs the company id, which is taken from `--yougile-company-id` /
`YOUGILE_COMPANY_ID`, or from the company resolved while authenticating with a
login and password — that one is saved to `.lenv`, so later runs keep their
links. Links are skipped when the company is unknown, as for `--task-json` runs,
which never reach the API.

The readable segment is the project title, resolved through the task's column and
board and reused across tasks sharing a column. A failed lookup warns on stderr
and still emits the link without the title. Tasks without a project sticker get
no link at all, since the `#ABC-42` fragment is what selects the task.

The resolved token and its company id are saved to a `.lenv` file
(`--lenv-path`, default `.lenv`)
so later runs reuse it instead of re-authenticating. CLI options, environment
variables, `.lenv`, and `.env` are loaded through the
[`lino-arguments`](https://github.com/link-foundation/lino-arguments) library,
with precedence: CLI arguments > environment variables > `.lenv` > `.env` >
defaults. Pass `--no-save-token` to skip persisting the token. The `.lenv`
file stores credentials — keep it out of version control.

When `--github-token` / `GITHUB_TOKEN` is not set, the CLI runs
`gh auth token` and uses the authenticated GitHub CLI token. When
`--github-repo` / `GITHUB_REPOSITORY` is not set, it runs
`gh repo view --json nameWithOwner --jq .nameWithOwner` and uses the current
GitHub repository. These commands are executed through `command-stream` with
stdin disabled and output captured. Pass the flag or environment variable
explicitly when converting into a repository other than the current checkout.

Useful options:

- `--mode issue-tree`: creates one GitHub issue per YouGile task and links subtasks through GitHub sub-issues.
- `--mode single-issue`: creates one GitHub issue body containing the full recursive tree.
- `--dry-run`: prints the GitHub issue plan as JSON without writing to GitHub.
- `--task-json <path>`: reads a captured `YougileTaskTree` JSON fixture instead of calling YouGile.
- `--github-token <token>`: overrides GitHub CLI token detection.
- `--github-repo <owner/repo>`: overrides GitHub CLI repository detection.
- `--yougile-login <login>` / `--yougile-password <password>`: authenticate with credentials when no token is set.
- `--yougile-company-id <id>`: selects the YouGile company when the account can access more than one.
- `--lenv-path <path>`: path of the `.lenv` file used to persist a credential-resolved token (default `.lenv`).
- `--no-save-token`: do not write a credential-resolved token and company id to the `.lenv` file.
- `--max-depth <n>`: limits recursive subtask traversal.
- `--include-deleted`: includes deleted YouGile chat messages where the API returns them.
- `--include-system-messages`: includes system chat messages.

Set `YOUGILE_TO_GH_TRACE_GH=1` to print the `gh` detection commands while
debugging. The trace prints command names only, not the detected token.

## Library

```rust
use yougile_to_gh::{
    ConversionMode, ConversionOptions, YougileClient, build_conversion_plan,
    fetch_task_tree, FetchOptions,
};

let yougile = YougileClient::new("https://ru.yougile.com", "YOUGILE_TOKEN");
let tree = fetch_task_tree(&yougile, "YOUGILE_TASK_ID", FetchOptions::default())?;
let plan = build_conversion_plan(
    &tree,
    ConversionMode::IssueTree,
    &ConversionOptions::default(),
);
# Ok::<(), yougile_to_gh::YougileToGhError>(())
```

Use `execute_conversion_plan` with a `GitHubClient` to write the generated plan to GitHub.

## Task JSON Fixture Shape

`--task-json` accepts the same structure produced by the library:

```json
{
  "task": {
    "id": "root",
    "title": "Root task",
    "timestamp": 1700000000000,
    "subtasks": ["child"]
  },
  "messages": [],
  "subtasks": [
    {
      "task": {
        "id": "child",
        "title": "Child task",
        "timestamp": 1700000001000,
        "subtasks": []
      },
      "messages": [],
      "subtasks": []
    }
  ]
}
```

## Development

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

The repository uses changelog fragments in `changelog.d/`; do not edit the package version manually.
