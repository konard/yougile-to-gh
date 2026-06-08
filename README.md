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

When `--github-token` / `GITHUB_TOKEN` is not set, the CLI runs
`gh auth token` and uses the authenticated GitHub CLI token. When
`--github-repo` / `GITHUB_REPOSITORY` is not set, it runs
`gh repo view --json nameWithOwner --jq .nameWithOwner` and uses the current
GitHub repository. Pass the flag or environment variable explicitly when
converting into a repository other than the current checkout.

Useful options:

- `--mode issue-tree`: creates one GitHub issue per YouGile task and links subtasks through GitHub sub-issues.
- `--mode single-issue`: creates one GitHub issue body containing the full recursive tree.
- `--dry-run`: prints the GitHub issue plan as JSON without writing to GitHub.
- `--task-json <path>`: reads a captured `YougileTaskTree` JSON fixture instead of calling YouGile.
- `--github-token <token>`: overrides GitHub CLI token detection.
- `--github-repo <owner/repo>`: overrides GitHub CLI repository detection.
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
