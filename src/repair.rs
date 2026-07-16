//! Rewrites the body of an issue imported by an earlier version of this tool.
//!
//! Bodies created before a rendering change keep the old shape until they are
//! rebuilt. Repair reads the `YouGile` task id back out of the issue, fetches
//! the task again, and renders the body with today's renderer, so a repaired
//! issue is indistinguishable from a freshly imported one.

use serde::{Deserialize, Serialize};

use crate::converter::{build_conversion_plan, ConversionMode, ConversionOptions};
use crate::error::{Result, YougileToGhError};
use crate::github::GitHubSink;
use crate::models::GitHubIssue;
use crate::task_url::{resolve_task_url, ProjectTitleCache, TaskUrlContext};
use crate::yougile::{fetch_task_tree, FetchOptions, YougileSource};

/// The line every imported body opens with, naming the task it was built from.
const TASK_ID_MARKER: &str = "Converted from YouGile task `";

/// What the single-issue renderer appends to that line, and the issue-tree one
/// does not — the one thing telling the two bodies apart.
const SINGLE_ISSUE_MARKER: &str = "recursively collected task(s)";

/// Length of a canonical UUID: `aaaabbbb-cccc-dddd-eeee-ffff00001111`.
const UUID_LEN: usize = 36;

/// What repairing one issue would do, or did.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RepairedIssue {
    pub issue_number: u64,
    pub yougile_task_id: String,
    pub html_url: String,
    /// The body as it would be after repair.
    pub body: String,
    /// Whether the rendered body differs from what the issue already has.
    pub changed: bool,
}

/// Reads the `YouGile` task id an imported body was built from.
///
/// The marker line is written by every renderer this tool has shipped, which
/// makes it the one anchor old and new bodies share. Only a line *starting* with
/// the marker counts, so a quoted or discussed copy of it further down cannot
/// redirect a repair at another task, and the id must be a UUID, so a body can't
/// steer the API request that follows.
#[must_use]
pub fn parse_yougile_task_id(body: &str) -> Option<&str> {
    let line = body.lines().find(|line| line.starts_with(TASK_ID_MARKER))?;
    let rest = &line[TASK_ID_MARKER.len()..];
    let task_id = &rest[..rest.find('`')?];

    is_uuid(task_id).then_some(task_id)
}

/// Reports whether the imported body was rendered as one issue holding the whole
/// task tree, rather than as one issue per task.
#[must_use]
fn is_single_issue_body(body: &str) -> bool {
    body.lines()
        .find(|line| line.starts_with(TASK_ID_MARKER))
        .is_some_and(|line| line.contains(SINGLE_ISSUE_MARKER))
}

/// Reports whether `value` is a canonical UUID, the shape of every task id.
fn is_uuid(value: &str) -> bool {
    value.len() == UUID_LEN
        && value.bytes().enumerate().all(|(index, byte)| match index {
            8 | 13 | 18 | 23 => byte == b'-',
            _ => byte.is_ascii_hexdigit(),
        })
}

/// Renders today's body for `issue`, fetching its task from `YouGile` again.
///
/// The mode comes from the body itself rather than from a caller: re-rendering a
/// single-issue body as an issue tree would drop every subtask and message it
/// holds, and no flag should be able to ask for that by accident.
///
/// `task_url_context` builds the task link from the tree this already fetches;
/// without one the body is rendered as before, just without the link.
pub fn plan_issue_repair<S: YougileSource>(
    yougile: &S,
    issue: &GitHubIssue,
    fetch_options: FetchOptions,
    task_url_context: Option<&TaskUrlContext>,
    on_title_error: &mut impl FnMut(&YougileToGhError),
) -> Result<RepairedIssue> {
    let current_body = issue.body.as_deref().unwrap_or_default();
    let task_id = parse_yougile_task_id(current_body).ok_or(YougileToGhError::MissingValue(
        "a YouGile task id in the GitHub issue body",
    ))?;

    let mode = if is_single_issue_body(current_body) {
        ConversionMode::SingleIssue
    } else {
        ConversionMode::IssueTree
    };

    // The whole tree, as an import would: both bodies describe subtasks, so a
    // shallower fetch would render an incomplete one. Only the first draft is
    // used — in issue-tree mode each sub-issue is repaired through its own number.
    let tree = fetch_task_tree(yougile, task_id, fetch_options)?;

    let mut options = ConversionOptions::default();
    // Labels and assignees stay as they are: a repair rewrites the body only,
    // and the issue's own may well have been curated since the import.
    if let Some(context) = task_url_context {
        let mut cache = ProjectTitleCache::new();
        if let Some(url) =
            resolve_task_url(yougile, &tree.task, context, &mut cache, on_title_error)
        {
            options.task_urls.insert(tree.task.id.clone(), url);
        }
    }

    let plan = build_conversion_plan(&tree, mode, &options);
    let draft = plan
        .issues
        .first()
        .ok_or(YougileToGhError::MissingValue("a rendered issue body"))?;

    Ok(RepairedIssue {
        issue_number: issue.number,
        yougile_task_id: tree.task.id.clone(),
        html_url: issue.html_url.clone(),
        changed: draft.body != current_body,
        body: draft.body.clone(),
    })
}

/// Writes a planned repair back to GitHub, skipping bodies already up to date.
pub fn execute_issue_repair<G: GitHubSink>(github: &G, repair: &RepairedIssue) -> Result<()> {
    if !repair.changed {
        return Ok(());
    }

    github.update_issue_body(repair.issue_number, &repair.body)
}
