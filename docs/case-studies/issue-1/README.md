# Issue 1 Case Study: YouGile Task to GitHub Issue Converter

Source issue: <https://github.com/konard/yougile-to-gh/issues/1>

Pull request: <https://github.com/konard/yougile-to-gh/pull/2>

## Issue Data

Issue title: "Make a Rust script, that can convert YouGile task into GitHub issue"

Labels: `documentation`, `enhancement`

Created: 2026-06-08T07:54:51Z

Comments at implementation start: none.

The issue asks for online research, a Rust library, a Rust CLI tool, recursive YouGile task conversion into GitHub issues, preservation of related data in `docs/case-studies/issue-{id}`, a full requirements analysis, and a solution plan with alternatives.

## Requirements

- Provide a Rust library API for converting a YouGile task into GitHub issue data.
- Provide a Rust CLI for the same workflow.
- Recursively collect YouGile subtasks.
- Preserve task description, metadata, checklists, stickers/custom fields, extension data, and chat messages where available through the YouGile REST API.
- Create GitHub issues through authenticated API calls.
- Support an efficient/direct recursive representation in GitHub.
- Record online research and alternatives in this case-study folder.
- Keep the implementation testable without live YouGile or GitHub credentials.

## Online Research

### YouGile REST API

Official admin documentation says YouGile exposes a REST API for automation, uses `Authorization: Bearer <API key>`, supports JSON request bodies, and limits requests to 50 per minute per company. It also states the base URL shape is `https://your-domain.com/api-v2/{resource}` and points to the full interactive API documentation.

Sources:

- <https://docs.yougile.com/docs/admin-guide/api/>
- <https://ru.yougile.com/api-v2>
- OpenAPI document discovered from the Stoplight page: `https://ru.yougile.com/api-json`

Relevant OpenAPI findings:

- `GET /api-v2/tasks/{id}` returns a `TaskDto`.
- `TaskDto.subtasks` is an array of child task IDs.
- `TaskDto.checklists` contains checklist titles and items.
- `TaskDto.stickers`, `deadline`, `timeTracking`, `stopwatch`, `timer`, `deal`, and `extensionData` preserve custom workflow data.
- `GET /api-v2/chats/{chatId}/messages` returns paginated chat history. YouGile platform examples use task IDs as chat IDs for task chats.

### GitHub REST API

GitHub's REST issue API supports creating issues, creating issue comments, and sub-issue endpoints. The sub-issue endpoints make recursive YouGile subtasks representable as native GitHub issue hierarchy instead of only Markdown links.

Sources:

- Issues overview: <https://docs.github.com/en/rest/issues>
- Create issue: <https://docs.github.com/en/rest/issues/issues#create-an-issue>
- Create issue comment: <https://docs.github.com/en/rest/issues/comments#create-an-issue-comment>
- Sub-issues: <https://docs.github.com/en/rest/issues/sub-issues>

Relevant GitHub findings:

- `POST /repos/{owner}/{repo}/issues` creates an issue body with optional labels and assignees.
- `POST /repos/{owner}/{repo}/issues/{issue_number}/comments` can import YouGile chat messages as GitHub comments.
- `POST /repos/{owner}/{repo}/issues/{issue_number}/sub_issues` attaches a created child issue under a parent issue.

## Alternatives Considered

1. Native GitHub sub-issue tree.
   - Pros: closest match for recursive YouGile subtasks, each child has its own issue lifecycle, direct REST support.
   - Cons: requires creating one issue per task and then attaching child issues; depends on repositories/tokens having access to sub-issue endpoints.
   - Status: implemented as `--mode issue-tree` and used as the default.

2. Single GitHub issue with recursive Markdown.
   - Pros: simplest API flow, one GitHub write, works even if sub-issues are unavailable.
   - Cons: subtasks are not independently assignable/closable as GitHub issues.
   - Status: implemented as `--mode single-issue`.

3. Single issue with GitHub task-list checkboxes only.
   - Pros: very compact.
   - Cons: loses comments and rich task metadata unless additional sections are added; deep recursion becomes hard to read.
   - Status: not chosen as a primary mode.

4. Shell out to `gh issue create`.
   - Pros: less HTTP client code.
   - Cons: harder to test, less portable as a library, weak control over sub-issue and comment calls.
   - Status: not chosen.

5. Generate only an export file and ask humans to import manually.
   - Pros: safe preview and audit trail.
   - Cons: does not satisfy the requested converter tool.
   - Status: covered by `--dry-run` but not the main workflow.

## Implemented Plan

- Replace the template sum crate with `yougile-to-gh`.
- Add typed YouGile models that preserve unknown/custom fields with `serde_json::Value`.
- Add a YouGile REST client for task fetches and paginated task chat messages.
- Add recursive task-tree fetching with cycle and max-depth guards.
- Add Markdown renderers for single-issue and issue-tree modes.
- Add a GitHub REST client for issue creation, issue comments, and sub-issue linking.
- Add a conversion planner that can be dry-run without GitHub credentials.
- Add CLI support for live fetching and offline `--task-json` fixtures.
- Add unit and integration tests for rendering, planning, recursion, and CLI dry-run behavior.

## Verification Strategy

- Unit tests use fake YouGile and GitHub clients so no external credentials are needed.
- Integration test runs the compiled CLI with `--task-json --dry-run`.
- Local checks should include `cargo fmt --check`, `cargo clippy --all-targets --all-features`, and `cargo test`.
