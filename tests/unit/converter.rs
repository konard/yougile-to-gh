use std::cell::RefCell;

use yougile_to_gh::converter::{
    build_conversion_plan, execute_conversion_plan, ConversionMode, ConversionOptions,
};
use yougile_to_gh::github::{GitHubIssueDraft, GitHubSink};
use yougile_to_gh::models::{CreatedGitHubComment, CreatedGitHubIssue};
use yougile_to_gh::{Result, YougileTaskTree};

use crate::render::sample_tree;

#[test]
fn single_issue_plan_preserves_recursive_subtasks_in_one_body() {
    let tree = sample_tree();
    let plan = build_conversion_plan(
        &tree,
        ConversionMode::SingleIssue,
        &ConversionOptions::default(),
    );

    assert_eq!(plan.issues.len(), 1);
    assert_eq!(plan.issues[0].yougile_task_id, "root");
    assert!(plan.issues[0].body.contains("Child task"));
    assert!(plan.issues[0].body.contains("Grandchild task"));
    assert!(plan.issues[0].comments.is_empty());
}

#[test]
fn issue_tree_plan_links_child_drafts_to_parent_tasks() {
    let tree = sample_tree();
    let options = ConversionOptions {
        labels: vec!["migration".to_owned()],
        assignees: vec!["octocat".to_owned()],
    };
    let plan = build_conversion_plan(&tree, ConversionMode::IssueTree, &options);

    assert_eq!(plan.issues.len(), 3);
    assert_eq!(plan.issues[0].yougile_task_id, "root");
    assert_eq!(plan.issues[1].yougile_task_id, "child");
    assert_eq!(
        plan.issues[1].parent_yougile_task_id.as_deref(),
        Some("root")
    );
    assert_eq!(
        plan.issues[2].parent_yougile_task_id.as_deref(),
        Some("child")
    );
    assert_eq!(plan.issues[0].labels, ["migration"]);
    assert_eq!(plan.issues[0].assignees, ["octocat"]);
    assert_eq!(plan.issues[0].comments.len(), 1);
}

#[test]
fn execution_creates_issues_comments_and_sub_issue_links_in_order() {
    let tree = sample_tree();
    let plan = build_conversion_plan(
        &tree,
        ConversionMode::IssueTree,
        &ConversionOptions::default(),
    );
    let github = FakeGitHub::default();

    let result = execute_conversion_plan(&plan, &github).unwrap();

    assert_eq!(result.created_issues.len(), 3);
    assert_eq!(result.created_sub_issue_links.len(), 2);
    assert_eq!(
        github.created_issue_titles(),
        ["Root task", "Child task", "Grandchild task"]
    );
    assert_eq!(github.comments.borrow().len(), 2);
    assert_eq!(github.sub_issue_links.borrow().len(), 2);
}

#[derive(Default)]
struct FakeGitHub {
    issues: RefCell<Vec<GitHubIssueDraft>>,
    comments: RefCell<Vec<(u64, String)>>,
    sub_issue_links: RefCell<Vec<(u64, u64)>>,
}

impl FakeGitHub {
    fn created_issue_titles(&self) -> Vec<String> {
        self.issues
            .borrow()
            .iter()
            .map(|issue| issue.title.clone())
            .collect()
    }
}

impl GitHubSink for FakeGitHub {
    fn create_issue(&self, draft: &GitHubIssueDraft) -> Result<CreatedGitHubIssue> {
        let issue_index = self.issues.borrow().len() + 1;
        self.issues.borrow_mut().push(draft.clone());

        Ok(CreatedGitHubIssue {
            id: 1000 + u64::try_from(issue_index).unwrap(),
            number: u64::try_from(issue_index).unwrap(),
            html_url: format!("https://github.test/owner/repo/issues/{issue_index}"),
            url: format!("https://api.github.test/repos/owner/repo/issues/{issue_index}"),
        })
    }

    fn create_issue_comment(&self, issue_number: u64, body: &str) -> Result<CreatedGitHubComment> {
        let comment_index = self.comments.borrow().len() + 1;
        self.comments
            .borrow_mut()
            .push((issue_number, body.to_owned()));

        Ok(CreatedGitHubComment {
            id: u64::try_from(comment_index).unwrap(),
            html_url: format!("https://github.test/comment/{comment_index}"),
            url: format!("https://api.github.test/comment/{comment_index}"),
        })
    }

    fn add_sub_issue(&self, parent_issue_number: u64, sub_issue_id: u64) -> Result<()> {
        self.sub_issue_links
            .borrow_mut()
            .push((parent_issue_number, sub_issue_id));
        Ok(())
    }
}

#[test]
fn fixture_shape_is_valid() {
    let tree: YougileTaskTree = serde_json::from_value(serde_json::json!(sample_tree())).unwrap();
    assert_eq!(tree.task_count(), 3);
}
