---
bump: minor
---

### Added
- Each issue body now starts with a link back to its YouGile task (`**YouGile:** [ABC-42](...)`), so the original task is one click away
- `TaskUrlContext`, `resolve_task_url`, `resolve_project_title` and `ProjectTitleCache` for building task URLs from a company id, a project title and the task's project sticker
- `YougileSource::get_column`, `get_board` and `get_project`, with matching `YougileColumn`, `YougileBoard` and `YougileProject` models
- `ConversionOptions::task_urls`, mapping task ids to the links rendered at the top of each issue body
- The company id resolved during login is now persisted to `.lenv` as `YOUGILE_COMPANY_ID`, so task links keep working on later runs without passing `--yougile-company-id`

### Changed
- `render_single_issue_body` and `render_issue_tree_body` take the task URL to render as a second argument
- Task links reuse one project title lookup per column, keeping a task tree at three requests instead of three per task
- A failed project title lookup now warns on stderr and still emits the link, instead of silently dropping the project name
