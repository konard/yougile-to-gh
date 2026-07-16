//! Builds a browser URL that opens a `YouGile` task.
//!
//! A task URL looks like
//! `https://ru.yougile.com/team/1a2b3c4d5e6f/Project-Name#ABC-42`, where the
//! company segment is the tail of the company id, the readable segment is the
//! project title, and the fragment is the task's project sticker. The fragment
//! is what actually selects the task; the project title is a readable label.

use std::collections::HashMap;
use std::fmt::Write as _;

use crate::error::{Result, YougileToGhError};
use crate::models::YougileTask;
use crate::yougile::YougileSource;

/// Length of the company segment: the last group of the company UUID, as the
/// `YouGile` UI addresses a company by it — `.../team/1a2b3c4d5e6f/...`.
const COMPANY_SEGMENT_LEN: usize = 12;

/// Everything needed to build task URLs for one company.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskUrlContext {
    web_base_url: String,
    company_segment: String,
}

impl TaskUrlContext {
    /// Builds a context from the web base URL and the full company id.
    ///
    /// `web_base_url` addresses the `YouGile` UI, so unlike the base URL the API
    /// client takes, it must stay as it is rather than gain an `/api-v2` suffix.
    /// The company id is accepted with or without the dashes of a UUID.
    ///
    /// Returns `None` when the company id is too short to identify a company,
    /// which keeps a malformed id from producing a link that goes nowhere.
    #[must_use]
    pub fn new(web_base_url: &str, company_id: &str) -> Option<Self> {
        // Counted in characters, not bytes, so that a non-ASCII id is rejected
        // rather than sliced through the middle of a character.
        let company_id: Vec<char> = company_id.trim().replace('-', "").chars().collect();
        let tail_start = company_id.len().checked_sub(COMPANY_SEGMENT_LEN)?;

        Some(Self {
            web_base_url: web_base_url.trim_end_matches('/').to_owned(),
            company_segment: company_id[tail_start..].iter().collect(),
        })
    }

    /// Builds the URL opening `task`, or `None` when the task has no sticker.
    ///
    /// Without a sticker the URL cannot select the task, so no link is better
    /// than one that lands on an arbitrary board.
    #[must_use]
    pub fn task_url(&self, task: &YougileTask, project_title: Option<&str>) -> Option<String> {
        let sticker = task.id_task_project.as_deref()?.trim();
        if sticker.is_empty() {
            return None;
        }

        let mut url = format!("{}/team/{}", self.web_base_url, self.company_segment);
        if let Some(title) = project_title
            .map(str::trim)
            .filter(|title| !title.is_empty())
        {
            url.push('/');
            url.push_str(&encode_path_segment(&title.replace(' ', "-")));
        }
        url.push('#');
        url.push_str(sticker);
        Some(url)
    }
}

/// Resolves the project title of a task through its column and board.
///
/// `Ok(None)` means the chain is incomplete — the task sits outside a column,
/// board or project — while `Err` reports a failed lookup, letting a caller tell
/// "nothing to resolve" from "the API did not answer".
pub fn resolve_project_title<S: YougileSource>(
    source: &S,
    task: &YougileTask,
) -> Result<Option<String>> {
    let Some(column_id) = task.column_id.as_deref() else {
        return Ok(None);
    };
    let column = source.get_column(column_id)?;

    let Some(board_id) = column.board_id.as_deref() else {
        return Ok(None);
    };
    let board = source.get_board(board_id)?;

    let Some(project_id) = board.project_id.as_deref() else {
        return Ok(None);
    };
    Ok(Some(source.get_project(project_id)?.title))
}

/// Resolves project titles, reusing the answer for tasks sharing a column.
///
/// Sibling tasks usually sit in one column, so caching by column id keeps a
/// whole tree at three requests instead of three per task.
#[derive(Debug, Default)]
pub struct ProjectTitleCache {
    titles_by_column_id: HashMap<String, Option<String>>,
}

impl ProjectTitleCache {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A resolved absence is cached like any other answer, but a failed request
    /// is not, so one bad response does not deny every later task its title.
    pub fn project_title<S: YougileSource>(
        &mut self,
        source: &S,
        task: &YougileTask,
    ) -> Result<Option<String>> {
        let Some(column_id) = task.column_id.as_deref() else {
            return Ok(None);
        };
        if let Some(title) = self.titles_by_column_id.get(column_id) {
            return Ok(title.clone());
        }

        let title = resolve_project_title(source, task)?;
        self.titles_by_column_id
            .insert(column_id.to_owned(), title.clone());
        Ok(title)
    }
}

/// Percent-encodes one URL path segment, keeping RFC 3986 unreserved characters.
fn encode_path_segment(segment: &str) -> String {
    let mut encoded = String::with_capacity(segment.len());
    for byte in segment.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                encoded.push(byte as char);
            }
            _ => write!(encoded, "%{byte:02X}").expect("writing to String cannot fail"),
        }
    }
    encoded
}

/// Resolves the task URL for `task`, including the project title when available.
///
/// A failed title lookup is reported through `on_title_error` and then dropped:
/// the title only makes the URL readable, so losing it must not cost the link.
pub fn resolve_task_url<S: YougileSource>(
    source: &S,
    task: &YougileTask,
    context: &TaskUrlContext,
    cache: &mut ProjectTitleCache,
    on_title_error: &mut impl FnMut(&YougileToGhError),
) -> Option<String> {
    let project_title = cache.project_title(source, task).unwrap_or_else(|error| {
        on_title_error(&error);
        None
    });
    context.task_url(task, project_title.as_deref())
}
