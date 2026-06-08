use serde_json::json;
use yougile_to_gh::{build_conversion_plan, ConversionMode, ConversionOptions, YougileTaskTree};

fn main() {
    let tree: YougileTaskTree = serde_json::from_value(json!({
        "task": {
            "id": "task-1",
            "title": "Port YouGile task",
            "timestamp": 1_700_000_000_000_u64,
            "description": "Preserve task data in GitHub.",
            "subtasks": ["task-2"]
        },
        "messages": [],
        "subtasks": [
            {
                "task": {
                    "id": "task-2",
                    "title": "Nested YouGile subtask",
                    "timestamp": 1_700_000_001_000_u64,
                    "subtasks": []
                },
                "messages": [],
                "subtasks": []
            }
        ]
    }))
    .expect("example fixture should deserialize");

    let plan = build_conversion_plan(
        &tree,
        ConversionMode::IssueTree,
        &ConversionOptions {
            labels: vec!["yougile".to_owned()],
            assignees: Vec::new(),
        },
    );

    println!(
        "{}",
        serde_json::to_string_pretty(&plan).expect("plan should serialize")
    );
}
