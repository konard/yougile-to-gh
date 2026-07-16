use std::cell::Cell;

use serde_json::json;
use yougile_to_gh::models::{
    YougileBoard, YougileChatMessage, YougileColumn, YougileProject, YougileTask,
};
use yougile_to_gh::{
    resolve_task_url, ProjectTitleCache, Result, TaskUrlContext, YougileSource, YougileToGhError,
};

const COMPANY_ID: &str = "11111111-2222-3333-4444-1a2b3c4d5e6f";
const BASE_URL: &str = "https://ru.yougile.com";

/// Characters a task URL takes from the company id, mirroring `task_url.rs`.
const COMPANY_SEGMENT_LEN: usize = 12;

fn task(sticker: Option<&str>) -> YougileTask {
    serde_json::from_value(json!({
        "id": "task-1",
        "title": "Task",
        "timestamp": 1_700_000_000_000_u64,
        "idTaskProject": sticker,
    }))
    .unwrap()
}

fn task_in_column(column_id: &str) -> YougileTask {
    serde_json::from_value(json!({
        "id": format!("task-in-{column_id}"),
        "title": "Task",
        "timestamp": 1_700_000_000_000_u64,
        "idTaskProject": "ABC-42",
        "columnId": column_id,
    }))
    .unwrap()
}

/// A source counting the column lookups a caller makes.
#[derive(Default)]
struct CountingYougile {
    column_requests: Cell<usize>,
}

impl YougileSource for CountingYougile {
    fn get_task(&self, _task_id: &str) -> Result<YougileTask> {
        Err(YougileToGhError::MissingValue("fake task"))
    }

    fn list_task_messages(&self, _task_id: &str) -> Result<Vec<YougileChatMessage>> {
        Ok(Vec::new())
    }

    fn get_column(&self, column_id: &str) -> Result<YougileColumn> {
        self.column_requests.set(self.column_requests.get() + 1);
        Ok(serde_json::from_value(json!({
            "id": column_id,
            "title": "In progress",
            "boardId": "board-1",
        }))
        .unwrap())
    }

    fn get_board(&self, board_id: &str) -> Result<YougileBoard> {
        Ok(serde_json::from_value(json!({
            "id": board_id,
            "title": "Board",
            "projectId": "project-1",
        }))
        .unwrap())
    }

    fn get_project(&self, project_id: &str) -> Result<YougileProject> {
        Ok(serde_json::from_value(json!({
            "id": project_id,
            "title": "Project Name",
        }))
        .unwrap())
    }
}

/// A source whose lookups fail, standing in for an unreachable API.
struct FailingYougile;

impl YougileSource for FailingYougile {
    fn get_task(&self, _task_id: &str) -> Result<YougileTask> {
        Err(YougileToGhError::MissingValue("fake task"))
    }

    fn list_task_messages(&self, _task_id: &str) -> Result<Vec<YougileChatMessage>> {
        Ok(Vec::new())
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

fn context() -> TaskUrlContext {
    TaskUrlContext::new(BASE_URL, COMPANY_ID).expect("company id should be long enough")
}

#[test]
fn task_url_uses_company_id_tail_and_percent_encoded_project_title() {
    let url = context().task_url(&task(Some("ABC-42")), Some("Проект Один"));

    assert_eq!(
        url.as_deref(),
        Some(
            "https://ru.yougile.com/team/1a2b3c4d5e6f/\
             %D0%9F%D1%80%D0%BE%D0%B5%D0%BA%D1%82-%D0%9E%D0%B4%D0%B8%D0%BD#ABC-42"
        )
    );
}

#[test]
fn task_url_keeps_an_ascii_project_title_readable() {
    let url = context().task_url(&task(Some("ABC-42")), Some("Project Name"));

    assert_eq!(
        url.as_deref(),
        Some("https://ru.yougile.com/team/1a2b3c4d5e6f/Project-Name#ABC-42")
    );
}

#[test]
fn task_url_omits_the_readable_segment_without_a_project_title() {
    let url = context().task_url(&task(Some("ABC-42")), None);

    assert_eq!(
        url.as_deref(),
        Some("https://ru.yougile.com/team/1a2b3c4d5e6f#ABC-42")
    );
}

#[test]
fn task_url_needs_a_sticker_to_select_the_task() {
    assert_eq!(context().task_url(&task(None), Some("Project Name")), None);
    assert_eq!(context().task_url(&task(Some("  ")), None), None);
}

#[test]
fn task_url_context_rejects_a_company_id_that_cannot_identify_a_company() {
    assert_eq!(TaskUrlContext::new(BASE_URL, "short"), None);
    assert_eq!(TaskUrlContext::new(BASE_URL, ""), None);
}

#[test]
fn task_url_context_rejects_a_short_non_ascii_company_id_without_panicking() {
    assert_eq!(TaskUrlContext::new(BASE_URL, "Проектx"), None);
}

#[test]
fn project_title_cache_looks_up_each_column_once() {
    let source = CountingYougile::default();
    let mut cache = ProjectTitleCache::new();

    for _ in 0..3 {
        let title = cache
            .project_title(&source, &task_in_column("column-1"))
            .unwrap();
        assert_eq!(title.as_deref(), Some("Project Name"));
    }
    cache
        .project_title(&source, &task_in_column("column-2"))
        .unwrap();

    assert_eq!(source.column_requests.get(), 2);
}

#[test]
fn project_title_cache_needs_a_column_to_resolve_a_title() {
    let source = CountingYougile::default();
    let mut cache = ProjectTitleCache::new();

    assert_eq!(
        cache.project_title(&source, &task(Some("ABC-42"))).unwrap(),
        None
    );
    assert_eq!(source.column_requests.get(), 0);
}

#[test]
fn project_title_cache_reports_a_failed_lookup_instead_of_swallowing_it() {
    let source = FailingYougile;
    let mut cache = ProjectTitleCache::new();

    let title = cache.project_title(&source, &task_in_column("column-1"));

    assert!(title.is_err());
}

#[test]
fn task_url_survives_a_failed_project_title_lookup_and_reports_it() {
    let source = FailingYougile;
    let mut cache = ProjectTitleCache::new();
    let mut errors = 0;

    let url = resolve_task_url(
        &source,
        &task_in_column("column-1"),
        &context(),
        &mut cache,
        &mut |_| errors += 1,
    );

    assert_eq!(
        url.as_deref(),
        Some("https://ru.yougile.com/team/1a2b3c4d5e6f#ABC-42")
    );
    assert_eq!(errors, 1);
}

#[test]
fn task_url_context_counts_company_id_length_in_characters() {
    let twelve_cyrillic_chars = "абвгдеёжзийк";
    assert_eq!(twelve_cyrillic_chars.chars().count(), COMPANY_SEGMENT_LEN);

    let context = TaskUrlContext::new(BASE_URL, twelve_cyrillic_chars).expect("long enough");

    assert_eq!(
        context.task_url(&task(Some("ABC-42")), None).as_deref(),
        Some("https://ru.yougile.com/team/абвгдеёжзийк#ABC-42")
    );
}

#[test]
fn task_url_keeps_one_slash_after_a_base_url_with_a_trailing_slash() {
    let context = TaskUrlContext::new("https://ru.yougile.com/", COMPANY_ID).unwrap();

    assert_eq!(
        context.task_url(&task(Some("ABC-42")), None).as_deref(),
        Some("https://ru.yougile.com/team/1a2b3c4d5e6f#ABC-42")
    );
}
