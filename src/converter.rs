use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::error::{Result, YougileToGhError};
use crate::github::{GitHubIssueDraft, GitHubSink};
use crate::models::{CreatedGitHubIssue, YougileTask, YougileTaskTree};
use crate::render::{render_issue_tree_body, render_message_as_comment, render_single_issue_body};

const MAX_GITHUB_TITLE_CHARS: usize = 240;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConversionMode {
    SingleIssue,
    IssueTree,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ConversionOptions {
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub assignees: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ConversionPlan {
    pub mode: ConversionMode,
    pub issues: Vec<GitHubIssueDraft>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CreatedIssueRecord {
    pub yougile_task_id: String,
    pub github_issue_id: u64,
    pub github_issue_number: u64,
    pub github_html_url: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CreatedSubIssueLink {
    pub parent_yougile_task_id: String,
    pub child_yougile_task_id: String,
    pub parent_github_issue_number: u64,
    pub child_github_issue_id: u64,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ConversionResult {
    pub created_issues: Vec<CreatedIssueRecord>,
    pub created_sub_issue_links: Vec<CreatedSubIssueLink>,
}

#[must_use]
pub fn build_conversion_plan(
    tree: &YougileTaskTree,
    mode: ConversionMode,
    options: &ConversionOptions,
) -> ConversionPlan {
    match mode {
        ConversionMode::SingleIssue => ConversionPlan {
            mode,
            issues: vec![single_issue_draft(tree, options)],
        },
        ConversionMode::IssueTree => {
            let mut issues = Vec::new();
            let mut seen = HashSet::new();
            collect_issue_tree_drafts(tree, None, options, &mut issues, &mut seen);
            ConversionPlan { mode, issues }
        }
    }
}

pub fn execute_conversion_plan<G: GitHubSink>(
    plan: &ConversionPlan,
    github: &G,
) -> Result<ConversionResult> {
    let mut result = ConversionResult::default();
    let mut created_by_task_id: HashMap<String, CreatedGitHubIssue> = HashMap::new();

    for draft in &plan.issues {
        let created_issue = github.create_issue(draft)?;

        for comment in &draft.comments {
            github.create_issue_comment(created_issue.number, comment)?;
        }

        if let Some(parent_task_id) = &draft.parent_yougile_task_id {
            let Some(parent_issue) = created_by_task_id.get(parent_task_id) else {
                return Err(YougileToGhError::MissingParentIssue {
                    task_id: draft.yougile_task_id.clone(),
                    parent_task_id: parent_task_id.clone(),
                });
            };

            github.add_sub_issue(parent_issue.number, created_issue.id)?;
            result.created_sub_issue_links.push(CreatedSubIssueLink {
                parent_yougile_task_id: parent_task_id.clone(),
                child_yougile_task_id: draft.yougile_task_id.clone(),
                parent_github_issue_number: parent_issue.number,
                child_github_issue_id: created_issue.id,
            });
        }

        result.created_issues.push(CreatedIssueRecord {
            yougile_task_id: draft.yougile_task_id.clone(),
            github_issue_id: created_issue.id,
            github_issue_number: created_issue.number,
            github_html_url: created_issue.html_url.clone(),
        });
        created_by_task_id.insert(draft.yougile_task_id.clone(), created_issue);
    }

    Ok(result)
}

fn single_issue_draft(tree: &YougileTaskTree, options: &ConversionOptions) -> GitHubIssueDraft {
    GitHubIssueDraft {
        yougile_task_id: tree.task.id.clone(),
        parent_yougile_task_id: None,
        title: github_title(&tree.task),
        body: render_single_issue_body(tree),
        labels: options.labels.clone(),
        assignees: options.assignees.clone(),
        comments: Vec::new(),
    }
}

fn collect_issue_tree_drafts(
    tree: &YougileTaskTree,
    parent_yougile_task_id: Option<&str>,
    options: &ConversionOptions,
    issues: &mut Vec<GitHubIssueDraft>,
    seen: &mut HashSet<String>,
) {
    if !seen.insert(tree.task.id.clone()) {
        return;
    }

    issues.push(GitHubIssueDraft {
        yougile_task_id: tree.task.id.clone(),
        parent_yougile_task_id: parent_yougile_task_id.map(str::to_owned),
        title: github_title(&tree.task),
        body: render_issue_tree_body(tree),
        labels: options.labels.clone(),
        assignees: options.assignees.clone(),
        comments: tree
            .messages
            .iter()
            .map(render_message_as_comment)
            .collect(),
    });

    for child in &tree.subtasks {
        collect_issue_tree_drafts(child, Some(&tree.task.id), options, issues, seen);
    }
}

fn github_title(task: &YougileTask) -> String {
    let normalized = task.title.split_whitespace().collect::<Vec<_>>().join(" ");
    let fallback;
    let title = if normalized.is_empty() {
        fallback = format!("YouGile task {}", task.id);
        fallback.as_str()
    } else {
        normalized.as_str()
    };

    truncate_title(title)
}

fn truncate_title(title: &str) -> String {
    if title.chars().count() <= MAX_GITHUB_TITLE_CHARS {
        return title.to_owned();
    }

    let prefix = title
        .chars()
        .take(MAX_GITHUB_TITLE_CHARS.saturating_sub(3))
        .collect::<String>();
    format!("{prefix}...")
}
