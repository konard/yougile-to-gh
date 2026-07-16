use std::collections::HashMap;

use serde_json::json;
use yougile_to_gh::models::{
    YougileBoard, YougileChatMessage, YougileColumn, YougileProject, YougileTask,
};
use yougile_to_gh::yougile::{fetch_task_tree, FetchOptions, YougileSource};
use yougile_to_gh::{Result, YougileToGhError};

#[test]
fn fetch_task_tree_collects_subtasks_and_messages_recursively() {
    let source = FakeYougile::new([
        task("root", "Root", &["child"]),
        task("child", "Child", &[]),
    ]);

    let tree = fetch_task_tree(&source, "root", FetchOptions::default()).unwrap();

    assert_eq!(tree.task.id, "root");
    assert_eq!(tree.messages.len(), 1);
    assert_eq!(tree.subtasks[0].task.id, "child");
    assert_eq!(tree.task_count(), 2);
}

#[test]
fn fetch_task_tree_rejects_cycles() {
    let source = FakeYougile::new([
        task("root", "Root", &["child"]),
        task("child", "Child", &["root"]),
    ]);

    let error = fetch_task_tree(&source, "root", FetchOptions::default()).unwrap_err();

    assert!(matches!(error, YougileToGhError::TaskCycle(task_id) if task_id == "root"));
}

#[test]
fn fetch_task_tree_enforces_max_depth() {
    let source = FakeYougile::new([
        task("root", "Root", &["child"]),
        task("child", "Child", &["grandchild"]),
        task("grandchild", "Grandchild", &[]),
    ]);

    let error = fetch_task_tree(&source, "root", FetchOptions { max_depth: Some(1) }).unwrap_err();

    assert!(
        matches!(error, YougileToGhError::MaxDepthExceeded { task_id, max_depth } if task_id == "grandchild" && max_depth == 1)
    );
}

struct FakeYougile {
    tasks: HashMap<String, YougileTask>,
}

impl FakeYougile {
    fn new(tasks: impl IntoIterator<Item = YougileTask>) -> Self {
        Self {
            tasks: tasks
                .into_iter()
                .map(|task| (task.id.clone(), task))
                .collect(),
        }
    }
}

impl YougileSource for FakeYougile {
    fn get_task(&self, task_id: &str) -> Result<YougileTask> {
        self.tasks
            .get(task_id)
            .cloned()
            .ok_or(YougileToGhError::MissingValue("fake task"))
    }

    fn list_task_messages(&self, task_id: &str) -> Result<Vec<YougileChatMessage>> {
        Ok(vec![serde_json::from_value(json!({
            "id": 1_700_000_000_000_u64,
            "fromUserId": "user",
            "text": format!("message for {task_id}"),
            "textHtml": "",
            "label": "",
            "editTimestamp": 1_700_000_000_001_u64,
            "reactions": {}
        }))
        .unwrap()])
    }

    fn get_column(&self, _column_id: &str) -> Result<YougileColumn> {
        Err(YougileToGhError::MissingValue("fake column"))
    }

    fn get_board(&self, _board_id: &str) -> Result<YougileBoard> {
        Err(YougileToGhError::MissingValue("fake board"))
    }

    fn get_project(&self, _project_id: &str) -> Result<YougileProject> {
        Err(YougileToGhError::MissingValue("fake project"))
    }
}

fn task(id: &str, title: &str, subtasks: &[&str]) -> YougileTask {
    serde_json::from_value(json!({
        "id": id,
        "title": title,
        "timestamp": 1_700_000_000_000_u64,
        "subtasks": subtasks
    }))
    .unwrap()
}
