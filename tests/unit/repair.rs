use std::cell::RefCell;

use serde_json::json;
use yougile_to_gh::models::{CreatedGitHubComment, CreatedGitHubIssue};
use yougile_to_gh::models::{
    GitHubIssue, YougileBoard, YougileChatMessage, YougileColumn, YougileProject, YougileTask,
};
use yougile_to_gh::{
    execute_issue_repair, parse_yougile_task_id, plan_issue_repair, FetchOptions, GitHubIssueDraft,
    GitHubSink, RepairedIssue, Result, TaskUrlContext, YougileSource, YougileToGhError,
};

const TASK_ID: &str = "aaaabbbb-cccc-dddd-eeee-ffff00001111";

/// A body in the shape the tool produced before it rendered task links.
fn old_body() -> String {
    format!("Converted from YouGile task `{TASK_ID}`.\n\n## Task: Old title\n")
}

/// A body rendered as one issue holding the whole task tree, which the
/// issue-tree renderer would strip back to the root task.
fn single_issue_body() -> String {
    format!(
        "Converted from YouGile task `{TASK_ID}` with 3 recursively collected task(s).\
         \n\n## Task: Old title\n"
    )
}

fn issue(body: Option<&str>) -> GitHubIssue {
    GitHubIssue {
        number: 340,
        title: "Old title".to_owned(),
        html_url: "https://github.test/owner/repo/issues/340".to_owned(),
        body: body.map(str::to_owned),
    }
}

fn context() -> TaskUrlContext {
    TaskUrlContext::new(
        "https://ru.yougile.com",
        "11111111-2222-3333-4444-1a2b3c4d5e6f",
    )
    .expect("company id should be long enough")
}

fn plan(issue: &GitHubIssue, context: Option<&TaskUrlContext>) -> Result<RepairedIssue> {
    plan_issue_repair(
        &StubYougile,
        issue,
        FetchOptions::default(),
        context,
        &mut |_| {},
    )
}

struct StubYougile;

/// The root task carries a subtask, so a single-issue body has content an
/// issue-tree rendering would drop.
const SUBTASK_ID: &str = "11112222-3333-4444-5555-666677778888";

impl YougileSource for StubYougile {
    fn get_task(&self, task_id: &str) -> Result<YougileTask> {
        if task_id == SUBTASK_ID {
            return Ok(serde_json::from_value(json!({
                "id": SUBTASK_ID,
                "title": "Subtask title",
                "timestamp": 1_700_000_001_000_u64,
                "subtasks": [],
            }))
            .unwrap());
        }

        Ok(serde_json::from_value(json!({
            "id": task_id,
            "title": "Task title",
            "timestamp": 1_700_000_000_000_u64,
            "idTaskProject": "SER-47",
            "subtasks": [SUBTASK_ID],
        }))
        .unwrap())
    }

    fn list_task_messages(&self, _task_id: &str) -> Result<Vec<YougileChatMessage>> {
        Ok(Vec::new())
    }

    fn get_column(&self, _column_id: &str) -> Result<YougileColumn> {
        Err(YougileToGhError::MissingValue("stub column"))
    }

    fn get_board(&self, _board_id: &str) -> Result<YougileBoard> {
        Err(YougileToGhError::MissingValue("stub board"))
    }

    fn get_project(&self, _project_id: &str) -> Result<YougileProject> {
        Err(YougileToGhError::MissingValue("stub project"))
    }
}

#[derive(Default)]
struct RecordingGitHub {
    updated_bodies: RefCell<Vec<(u64, String)>>,
}

impl GitHubSink for RecordingGitHub {
    fn create_issue(&self, _draft: &GitHubIssueDraft) -> Result<CreatedGitHubIssue> {
        Err(YougileToGhError::MissingValue("stub create"))
    }

    fn create_issue_comment(
        &self,
        _issue_number: u64,
        _body: &str,
    ) -> Result<CreatedGitHubComment> {
        Err(YougileToGhError::MissingValue("stub comment"))
    }

    fn add_sub_issue(&self, _parent_issue_number: u64, _sub_issue_id: u64) -> Result<()> {
        Err(YougileToGhError::MissingValue("stub sub-issue"))
    }

    fn fetch_issue(&self, _issue_number: u64) -> Result<GitHubIssue> {
        Err(YougileToGhError::MissingValue("stub fetch"))
    }

    fn update_issue_body(&self, issue_number: u64, body: &str) -> Result<()> {
        self.updated_bodies
            .borrow_mut()
            .push((issue_number, body.to_owned()));
        Ok(())
    }
}

#[test]
fn task_id_is_read_from_an_imported_body() {
    assert_eq!(parse_yougile_task_id(&old_body()), Some(TASK_ID));
}

#[test]
fn task_id_is_read_from_a_body_that_already_carries_a_link() {
    let body = format!(
        "**YouGile:** [SER-47](https://ru.yougile.com/team/1a2b3c4d5e6f#SER-47)\n\n{}",
        old_body()
    );

    assert_eq!(parse_yougile_task_id(&body), Some(TASK_ID));
}

#[test]
fn a_body_without_the_marker_yields_no_task_id() {
    assert_eq!(parse_yougile_task_id("Hand written issue"), None);
    assert_eq!(parse_yougile_task_id(""), None);
}

#[test]
fn an_unterminated_or_empty_task_id_is_rejected() {
    assert_eq!(
        parse_yougile_task_id("Converted from YouGile task `abc"),
        None
    );
    assert_eq!(
        parse_yougile_task_id("Converted from YouGile task ``"),
        None
    );
}

#[test]
fn a_task_id_that_is_not_a_uuid_is_rejected() {
    // A body is editable by anyone with write access, and its task id lands in
    // an API path, so anything but a UUID must not get that far.
    for id in [
        "../../projects/secret",
        "http://evil.test/x",
        "not-a-uuid",
        "aaaabbbb-cccc-dddd-eeee-ffff0000111",
        "aaaabbbb_cccc_dddd_eeee_ffff00001111",
        "gaaabbbb-cccc-dddd-eeee-ffff00001111",
    ] {
        let body = format!("Converted from YouGile task `{id}`.");
        assert_eq!(parse_yougile_task_id(&body), None, "accepted {id:?}");
    }
}

#[test]
fn a_quoted_marker_cannot_redirect_a_repair_at_another_task() {
    let other_id = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee";
    let body = format!(
        "> Converted from YouGile task `{other_id}`\n\nConverted from YouGile task `{TASK_ID}`."
    );

    assert_eq!(parse_yougile_task_id(&body), Some(TASK_ID));
}

#[test]
fn a_single_issue_body_is_repaired_as_a_single_issue() {
    // Re-rendering it as an issue tree would drop the subtasks and messages the
    // body holds, which no repair may do.
    let repair = plan(&issue(Some(&single_issue_body())), None).expect("body names a task");

    assert!(repair.body.contains("recursively collected task(s)"));
    assert!(repair.body.contains("Subtask title"));
}

#[test]
fn an_issue_tree_body_is_repaired_as_an_issue_tree() {
    let repair = plan(&issue(Some(&old_body())), None).expect("body names a task");

    assert!(!repair.body.contains("recursively collected task(s)"));
}

#[test]
fn repairing_renders_todays_body_and_reports_the_change() {
    let repair = plan(&issue(Some(&old_body())), None).expect("body names a task");

    assert_eq!(repair.issue_number, 340);
    assert_eq!(repair.yougile_task_id, TASK_ID);
    assert!(repair.changed);
    assert!(repair.body.contains("## Task: Task title"));
}

#[test]
fn repairing_renders_the_task_link_when_a_company_is_known() {
    let repair = plan(&issue(Some(&old_body())), Some(&context())).expect("body names a task");

    assert!(repair
        .body
        .starts_with("**YouGile:** [SER-47](https://ru.yougile.com/team/1a2b3c4d5e6f#SER-47)"));
}

#[test]
fn repairing_without_a_company_still_renders_a_body() {
    let repair = plan(&issue(Some(&old_body())), None).expect("body names a task");

    assert!(!repair.body.contains("**YouGile:**"));
    assert!(repair.body.starts_with("Converted from YouGile task"));
}

#[test]
fn an_issue_whose_body_names_no_task_cannot_be_repaired() {
    assert!(plan(&issue(Some("Hand written issue")), None).is_err());
}

#[test]
fn an_issue_with_no_body_at_all_cannot_be_repaired() {
    assert!(plan(&issue(None), None).is_err());
}

#[test]
fn executing_a_repair_writes_the_rendered_body_back() {
    let github = RecordingGitHub::default();
    let repair = plan(&issue(Some(&old_body())), None).unwrap();

    execute_issue_repair(&github, &repair).unwrap();

    let updates = github.updated_bodies.borrow();
    assert_eq!(updates.len(), 1);
    assert_eq!(updates[0].0, 340);
    assert_eq!(updates[0].1, repair.body);
}

#[test]
fn a_body_already_up_to_date_is_not_written_back() {
    let github = RecordingGitHub::default();
    let rendered = plan(&issue(Some(&old_body())), None).unwrap().body;

    // Feed the rendered body back in: the issue now matches what repair builds.
    let repair = plan(&issue(Some(&rendered)), None).unwrap();

    assert!(!repair.changed);
    execute_issue_repair(&github, &repair).unwrap();
    assert!(github.updated_bodies.borrow().is_empty());
}
