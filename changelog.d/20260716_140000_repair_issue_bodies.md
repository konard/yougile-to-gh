---
bump: minor
---

### Added
- `--repair-issue <number>` rebuilds the body of an already imported issue from its YouGile task, bringing bodies written by earlier versions up to today's rendering; repeat the flag to repair several, and pair it with `--dry-run` to see each rebuilt body and whether it differs before writing
- `parse_yougile_task_id`, `plan_issue_repair`, `execute_issue_repair` and `RepairedIssue` for repairing issue bodies from a library
- `GitHubSink::fetch_issue` and `update_issue_body`, with a `GitHubIssue` model for an issue read back from GitHub
- `HttpRequest::patch`, for the `PATCH` requests an issue update needs

### Changed
- A repair rewrites the body only: titles, labels and assignees are left as they are, and a body that already matches is not written back
- A repair renders the mode the body was imported with, so re-rendering a single-issue body cannot drop the subtasks and messages it holds
- A task id read from an issue body must be a UUID, and must come from the renderer's own line rather than a quoted copy of it, so a body cannot steer the API request a repair makes
- Repairing several issues reports what was rewritten even when one of them fails partway
