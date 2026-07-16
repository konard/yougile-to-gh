use std::fmt::Write as _;

use serde_json::Value;

use crate::models::{Checklist, YougileChatMessage, YougileTask, YougileTaskTree};

macro_rules! line {
    ($out:expr) => {
        $out.push('\n');
    };
    ($out:expr, $($arg:tt)*) => {
        writeln!($out, $($arg)*).expect("writing to String cannot fail");
    };
}

/// Renders the `YouGile` link that opens the task, followed by a blank line.
///
/// The link leads the body so the original task is one click away from the top
/// of the issue.
fn render_task_link(body: &mut String, task: &YougileTask, task_url: Option<&str>) {
    let Some(url) = task_url.map(str::trim).filter(|url| !url.is_empty()) else {
        return;
    };

    // The sticker names the task for a reader. Any caller may supply URLs, so a
    // task without one still gets a label rather than a bare link.
    let label = task
        .id_task_project
        .as_deref()
        .map(str::trim)
        .filter(|sticker| !sticker.is_empty())
        .unwrap_or("Open task");

    line!(body, "**YouGile:** [{label}]({url})");
    line!(body);
}

#[must_use]
pub fn render_single_issue_body(tree: &YougileTaskTree, task_url: Option<&str>) -> String {
    let mut body = String::new();
    render_task_link(&mut body, &tree.task, task_url);
    line!(
        &mut body,
        "Converted from YouGile task `{}` with {} recursively collected task(s).",
        tree.task.id,
        tree.task_count()
    );
    line!(&mut body);
    render_task_recursive(&mut body, tree, 0);
    body
}

#[must_use]
pub fn render_issue_tree_body(tree: &YougileTaskTree, task_url: Option<&str>) -> String {
    let mut body = String::new();
    render_task_link(&mut body, &tree.task, task_url);
    line!(&mut body, "Converted from YouGile task `{}`.", tree.task.id);
    line!(&mut body);
    render_task_details(&mut body, &tree.task, "Task", 2);

    if !tree.subtasks.is_empty() {
        line!(&mut body, "## YouGile Subtasks");
        line!(
            &mut body,
            "Child tasks are created as GitHub issues and attached as sub-issues."
        );
        line!(&mut body);
        render_subtask_list(&mut body, tree, 0);
        line!(&mut body);
    }

    body
}

#[must_use]
pub fn render_message_as_comment(message: &YougileChatMessage) -> String {
    let mut body = String::new();
    line!(
        &mut body,
        "Imported YouGile chat message `{}` from user `{}`.",
        message.id,
        message.from_user_id
    );

    if message.is_deleted() {
        line!(&mut body);
        line!(&mut body, "**Deleted in YouGile.**");
    }

    if let Some(label) = &message.label {
        if !label.trim().is_empty() {
            line!(&mut body);
            line!(&mut body, "Label: `{}`", label.trim());
        }
    }

    if let Some(edit_timestamp) = message.edit_timestamp {
        line!(&mut body);
        line!(&mut body, "Edited timestamp: `{edit_timestamp}`");
    }

    line!(&mut body);
    let text = message_body(message);
    if text.is_empty() {
        line!(&mut body, "_Empty YouGile message._");
    } else {
        body.push_str(&text);
        if !body.ends_with('\n') {
            body.push('\n');
        }
    }

    body
}

fn render_task_recursive(body: &mut String, tree: &YougileTaskTree, depth: usize) {
    let label = if depth == 0 { "Task" } else { "Subtask" };
    let heading_level = (2 + depth).min(6);
    render_task_details(body, &tree.task, label, heading_level);

    if !tree.messages.is_empty() {
        line!(
            body,
            "{} Chat Messages",
            "#".repeat((heading_level + 1).min(6))
        );
        line!(body);
        for message in &tree.messages {
            render_message_section(body, message);
        }
    }

    for subtask in &tree.subtasks {
        line!(body);
        render_task_recursive(body, subtask, depth + 1);
    }
}

fn render_task_details(body: &mut String, task: &YougileTask, label: &str, heading_level: usize) {
    let heading = "#".repeat(heading_level);
    let section_heading = "#".repeat((heading_level + 1).min(6));
    let sub_section_heading = "#".repeat((heading_level + 2).min(6));

    line!(body, "{heading} {label}: {}", task.title);
    line!(body);
    render_metadata(body, task, &section_heading);

    if let Some(description) = task.description.as_deref().map(str::trim) {
        if !description.is_empty() {
            line!(body, "{section_heading} Description");
            line!(body);
            line!(body, "{description}");
            line!(body);
        }
    }

    if !task.checklists.is_empty() {
        line!(body, "{section_heading} Checklists");
        line!(body);
        for checklist in &task.checklists {
            render_checklist(body, checklist, &sub_section_heading);
        }
    }

    render_json_fields(body, task, &section_heading);
}

fn render_metadata(body: &mut String, task: &YougileTask, heading: &str) {
    line!(body, "{heading} Metadata");
    line!(body);
    line!(body, "| Field | Value |");
    line!(body, "| --- | --- |");
    table_row(body, "YouGile task ID", &format!("`{}`", task.id));
    table_row(body, "Created timestamp", &format!("`{}`", task.timestamp));
    table_row(body, "Completed", checkbox_value(task.is_completed()));
    table_row(
        body,
        "Archived",
        checkbox_value(task.archived.unwrap_or(false)),
    );
    table_row(body, "Deleted", checkbox_value(task.is_deleted()));

    if let Some(column_id) = &task.column_id {
        table_row(body, "Column ID", &format!("`{column_id}`"));
    }
    if let Some(created_by) = &task.created_by {
        table_row(body, "Created by", &format!("`{created_by}`"));
    }
    if !task.assigned.is_empty() {
        table_row(
            body,
            "Assigned YouGile users",
            &inline_code_list(&task.assigned),
        );
    }
    if let Some(task_id) = &task.id_task_common {
        table_row(body, "Company task ID", &format!("`{task_id}`"));
    }
    if let Some(task_id) = &task.id_task_project {
        table_row(body, "Project task ID", &format!("`{task_id}`"));
    }
    if let Some(entity_type) = &task.entity_type {
        table_row(body, "Entity type", &format!("`{entity_type}`"));
    }

    line!(body);
}

fn render_checklist(body: &mut String, checklist: &Checklist, heading: &str) {
    line!(body, "{heading} {}", checklist.title);
    if checklist.items.is_empty() {
        line!(body);
        line!(body, "_No checklist items._");
        line!(body);
        return;
    }

    for item in &checklist.items {
        line!(
            body,
            "- {} {}",
            status_checkbox(item.is_completed),
            item.title
        );
    }
    line!(body);
}

fn render_json_fields(body: &mut String, task: &YougileTask, heading: &str) {
    render_optional_json(body, "Deadline", task.deadline.as_ref(), heading);
    render_optional_json(body, "Time Tracking", task.time_tracking.as_ref(), heading);
    render_optional_json(body, "Stickers", task.stickers.as_ref(), heading);
    render_optional_json(body, "Stopwatch", task.stopwatch.as_ref(), heading);
    render_optional_json(body, "Timer", task.timer.as_ref(), heading);
    render_optional_json(body, "Deal", task.deal.as_ref(), heading);
    render_optional_json(
        body,
        "Extension Data",
        task.extension_data.as_ref(),
        heading,
    );

    if !task.extra.is_empty() {
        let extra = serde_json::to_value(&task.extra).unwrap_or(Value::Null);
        render_optional_json(body, "Additional YouGile Fields", Some(&extra), heading);
    }
}

fn render_optional_json(body: &mut String, title: &str, value: Option<&Value>, heading: &str) {
    let Some(value) = value else {
        return;
    };

    if !value_has_data(value) {
        return;
    }

    let pretty = serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string());
    line!(body, "{heading} {title}");
    line!(body);
    line!(body, "```json");
    body.push_str(&pretty);
    if !body.ends_with('\n') {
        body.push('\n');
    }
    line!(body, "```");
    line!(body);
}

fn render_subtask_list(body: &mut String, tree: &YougileTaskTree, depth: usize) {
    for subtask in &tree.subtasks {
        let indent = "  ".repeat(depth);
        line!(
            body,
            "{}- {} {} (`{}`)",
            indent,
            status_checkbox(subtask.task.is_completed()),
            subtask.task.title,
            subtask.task.id
        );
        render_subtask_list(body, subtask, depth + 1);
    }
}

fn render_message_section(body: &mut String, message: &YougileChatMessage) {
    line!(body, "#### Message `{}`", message.id);
    line!(body);
    body.push_str(&render_message_as_comment(message));
    if !body.ends_with('\n') {
        body.push('\n');
    }
}

fn message_body(message: &YougileChatMessage) -> String {
    let text = message.text.trim();
    if !text.is_empty() {
        return text.to_owned();
    }

    message
        .text_html
        .as_deref()
        .map(str::trim)
        .filter(|html| !html.is_empty())
        .unwrap_or_default()
        .to_owned()
}

fn table_row(body: &mut String, field: &str, value: &str) {
    line!(
        body,
        "| {} | {} |",
        escape_table(field),
        escape_table(value)
    );
}

fn inline_code_list(values: &[String]) -> String {
    values
        .iter()
        .map(|value| format!("`{value}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

const fn checkbox_value(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
    }
}

const fn status_checkbox(value: bool) -> &'static str {
    if value {
        "[x]"
    } else {
        "[ ]"
    }
}

fn value_has_data(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Array(values) => !values.is_empty(),
        Value::Object(values) => !values.is_empty(),
        Value::Bool(_) | Value::Number(_) | Value::String(_) => true,
    }
}

fn escape_table(value: &str) -> String {
    value.replace('|', "\\|")
}
