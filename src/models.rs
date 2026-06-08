use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

fn deserialize_vec_or_default<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Ok(Option::<Vec<T>>::deserialize(deserializer)?.unwrap_or_default())
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChecklistItem {
    pub title: String,
    pub is_completed: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Checklist {
    pub title: String,
    #[serde(default, deserialize_with = "deserialize_vec_or_default")]
    pub items: Vec<ChecklistItem>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct YougileTask {
    pub id: String,
    pub title: String,
    pub timestamp: u64,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deleted: Option<bool>,

    #[serde(default, rename = "columnId", skip_serializing_if = "Option::is_none")]
    pub column_id: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archived: Option<bool>,

    #[serde(
        default,
        rename = "archivedTimestamp",
        skip_serializing_if = "Option::is_none"
    )]
    pub archived_timestamp: Option<u64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed: Option<bool>,

    #[serde(
        default,
        rename = "completedTimestamp",
        skip_serializing_if = "Option::is_none"
    )]
    pub completed_timestamp: Option<u64>,

    #[serde(default, deserialize_with = "deserialize_vec_or_default")]
    pub subtasks: Vec<String>,

    #[serde(default, deserialize_with = "deserialize_vec_or_default")]
    pub assigned: Vec<String>,

    #[serde(default, rename = "createdBy", skip_serializing_if = "Option::is_none")]
    pub created_by: Option<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub checklists: Vec<Checklist>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deadline: Option<Value>,

    #[serde(
        default,
        rename = "timeTracking",
        skip_serializing_if = "Option::is_none"
    )]
    pub time_tracking: Option<Value>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stickers: Option<Value>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,

    #[serde(
        default,
        rename = "idTaskCommon",
        skip_serializing_if = "Option::is_none"
    )]
    pub id_task_common: Option<String>,

    #[serde(
        default,
        rename = "idTaskProject",
        skip_serializing_if = "Option::is_none"
    )]
    pub id_task_project: Option<String>,

    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub entity_type: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stopwatch: Option<Value>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timer: Option<Value>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deal: Option<Value>,

    #[serde(
        default,
        rename = "extensionData",
        skip_serializing_if = "Option::is_none"
    )]
    pub extension_data: Option<Value>,

    #[serde(flatten, default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extra: BTreeMap<String, Value>,
}

impl YougileTask {
    #[must_use]
    pub fn is_completed(&self) -> bool {
        self.completed.unwrap_or(false)
    }

    #[must_use]
    pub fn is_deleted(&self) -> bool {
        self.deleted.unwrap_or(false)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct YougileChatMessage {
    pub id: u64,
    pub from_user_id: String,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub text: String,

    #[serde(default, rename = "textHtml", skip_serializing_if = "Option::is_none")]
    pub text_html: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,

    #[serde(
        default,
        rename = "editTimestamp",
        skip_serializing_if = "Option::is_none"
    )]
    pub edit_timestamp: Option<u64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deleted: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reactions: Option<Value>,

    #[serde(flatten, default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extra: BTreeMap<String, Value>,
}

impl YougileChatMessage {
    #[must_use]
    pub fn is_deleted(&self) -> bool {
        self.deleted.unwrap_or(false)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct YougileTaskTree {
    pub task: YougileTask,
    #[serde(default, deserialize_with = "deserialize_vec_or_default")]
    pub messages: Vec<YougileChatMessage>,
    #[serde(default, deserialize_with = "deserialize_vec_or_default")]
    pub subtasks: Vec<Self>,
}

impl YougileTaskTree {
    #[must_use]
    pub fn task_count(&self) -> usize {
        1 + self.subtasks.iter().map(Self::task_count).sum::<usize>()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PagingMetadata {
    pub count: u64,
    pub limit: u64,
    pub offset: u64,
    pub next: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(bound(deserialize = "T: Deserialize<'de>", serialize = "T: Serialize"))]
pub struct YougilePage<T> {
    pub paging: PagingMetadata,
    #[serde(default, deserialize_with = "deserialize_vec_or_default")]
    pub content: Vec<T>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CreatedGitHubIssue {
    pub id: u64,
    pub number: u64,
    pub html_url: String,
    pub url: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CreatedGitHubComment {
    pub id: u64,
    pub html_url: String,
    pub url: String,
}
