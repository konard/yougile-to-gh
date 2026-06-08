use serde_json::json;
use yougile_to_gh::models::YougileTaskTree;
use yougile_to_gh::render::{
    render_issue_tree_body, render_message_as_comment, render_single_issue_body,
};

pub fn sample_tree() -> YougileTaskTree {
    serde_json::from_value(json!({
        "task": {
            "id": "root",
            "title": "Root task",
            "timestamp": 1_700_000_000_000_u64,
            "description": "Root description",
            "completed": false,
            "subtasks": ["child"],
            "assigned": ["yougile-user-1"],
            "createdBy": "creator-1",
            "checklists": [
                {
                    "title": "Acceptance",
                    "items": [
                        { "title": "Preserve description", "isCompleted": true },
                        { "title": "Preserve subtasks", "isCompleted": false }
                    ]
                }
            ],
            "stickers": {
                "priority": "high"
            }
        },
        "messages": [
            {
                "id": 1_700_000_000_100_u64,
                "fromUserId": "commenter-1",
                "text": "Root chat message",
                "textHtml": "<p>Root chat message</p>",
                "label": "discussion",
                "editTimestamp": 1_700_000_000_200_u64,
                "reactions": {}
            }
        ],
        "subtasks": [
            {
                "task": {
                    "id": "child",
                    "title": "Child task",
                    "timestamp": 1_700_000_001_000_u64,
                    "description": "Child description",
                    "completed": true,
                    "subtasks": ["grandchild"]
                },
                "messages": [
                    {
                        "id": 1_700_000_001_100_u64,
                        "fromUserId": "commenter-2",
                        "text": "Child chat message",
                        "textHtml": "<p>Child chat message</p>",
                        "label": "",
                        "editTimestamp": 1_700_000_001_200_u64,
                        "reactions": {}
                    }
                ],
                "subtasks": [
                    {
                        "task": {
                            "id": "grandchild",
                            "title": "Grandchild task",
                            "timestamp": 1_700_000_002_000_u64,
                            "completed": false,
                            "subtasks": []
                        },
                        "messages": [],
                        "subtasks": []
                    }
                ]
            }
        ]
    }))
    .unwrap()
}

#[test]
fn single_issue_body_contains_recursive_task_data() {
    let body = render_single_issue_body(&sample_tree());

    assert!(body.contains("Root description"));
    assert!(body.contains("Child description"));
    assert!(body.contains("Grandchild task"));
    assert!(body.contains("- [x] Preserve description"));
    assert!(body.contains("\"priority\": \"high\""));
    assert!(body.contains("Root chat message"));
}

#[test]
fn issue_tree_body_mentions_sub_issue_strategy() {
    let body = render_issue_tree_body(&sample_tree());

    assert!(body.contains("Child tasks are created as GitHub issues"));
    assert!(body.contains("- [x] Child task (`child`)"));
    assert!(body.contains("  - [ ] Grandchild task (`grandchild`)"));
}

#[test]
fn message_comment_prefers_plain_text() {
    let tree = sample_tree();
    let comment = render_message_as_comment(&tree.messages[0]);

    assert!(comment.contains("Imported YouGile chat message"));
    assert!(comment.contains("Root chat message"));
    assert!(!comment.contains("<p>Root chat message</p>"));
}
