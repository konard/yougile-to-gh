use std::collections::{HashMap, HashSet};

use serde::de::DeserializeOwned;

use crate::error::{Result, YougileToGhError};
use crate::http::{HttpClient, HttpRequest, UreqHttpClient};
use crate::models::{YougileChatMessage, YougilePage, YougileTask, YougileTaskTree};

const DEFAULT_PAGE_LIMIT: u64 = 1000;

pub trait YougileSource {
    fn get_task(&self, task_id: &str) -> Result<YougileTask>;

    fn list_task_messages(&self, task_id: &str) -> Result<Vec<YougileChatMessage>>;
}

#[derive(Clone, Debug)]
pub struct YougileClient<C = UreqHttpClient> {
    api_base_url: String,
    token: String,
    http_client: C,
    include_deleted: bool,
    include_system_messages: bool,
    page_limit: u64,
}

impl YougileClient<UreqHttpClient> {
    pub fn new(base_url: impl Into<String>, token: impl Into<String>) -> Self {
        Self::with_http_client(base_url, token, UreqHttpClient::new())
    }
}

impl<C> YougileClient<C> {
    pub fn with_http_client(
        base_url: impl Into<String>,
        token: impl Into<String>,
        http_client: C,
    ) -> Self {
        Self {
            api_base_url: normalize_api_base_url(&base_url.into()),
            token: token.into(),
            http_client,
            include_deleted: false,
            include_system_messages: false,
            page_limit: DEFAULT_PAGE_LIMIT,
        }
    }

    #[must_use]
    pub const fn include_deleted(mut self, include_deleted: bool) -> Self {
        self.include_deleted = include_deleted;
        self
    }

    #[must_use]
    pub const fn include_system_messages(mut self, include_system_messages: bool) -> Self {
        self.include_system_messages = include_system_messages;
        self
    }

    #[must_use]
    pub const fn page_limit(mut self, page_limit: u64) -> Self {
        self.page_limit = page_limit;
        self
    }
}

impl<C: HttpClient> YougileSource for YougileClient<C> {
    fn get_task(&self, task_id: &str) -> Result<YougileTask> {
        self.get_json(&format!("{}/tasks/{task_id}", self.api_base_url))
    }

    fn list_task_messages(&self, task_id: &str) -> Result<Vec<YougileChatMessage>> {
        let mut offset = 0;
        let mut messages = Vec::new();

        loop {
            let page: YougilePage<YougileChatMessage> = self.get_json(&format!(
                "{}/chats/{task_id}/messages?limit={}&offset={offset}&includeDeleted={}&includeSystem={}",
                self.api_base_url,
                self.page_limit,
                self.include_deleted,
                self.include_system_messages
            ))?;

            let item_count = page.content.len();
            messages.extend(page.content);

            if !page.paging.next || item_count == 0 {
                break;
            }

            offset = page.paging.offset + page.paging.limit;
        }

        messages.sort_by_key(|message| message.id);
        Ok(messages)
    }
}

impl<C: HttpClient> YougileClient<C> {
    fn get_json<T: DeserializeOwned>(&self, url: &str) -> Result<T> {
        let request = HttpRequest::get(url)
            .with_header("Accept", "application/json")
            .with_header("Authorization", format!("Bearer {}", self.token));

        let response = self.http_client.send(request)?;
        serde_json::from_value(response.body).map_err(|source| YougileToGhError::json(url, source))
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FetchOptions {
    pub max_depth: Option<usize>,
}

pub fn fetch_task_tree<S: YougileSource>(
    source: &S,
    root_task_id: &str,
    options: FetchOptions,
) -> Result<YougileTaskTree> {
    let mut path = HashSet::new();
    let mut cache = HashMap::new();
    fetch_task_tree_inner(source, root_task_id, 0, options, &mut path, &mut cache)
}

fn fetch_task_tree_inner<S: YougileSource>(
    source: &S,
    task_id: &str,
    depth: usize,
    options: FetchOptions,
    path: &mut HashSet<String>,
    cache: &mut HashMap<String, YougileTaskTree>,
) -> Result<YougileTaskTree> {
    if let Some(max_depth) = options.max_depth {
        if depth > max_depth {
            return Err(YougileToGhError::MaxDepthExceeded {
                task_id: task_id.to_owned(),
                max_depth,
            });
        }
    }

    if path.contains(task_id) {
        return Err(YougileToGhError::TaskCycle(task_id.to_owned()));
    }

    if let Some(cached) = cache.get(task_id) {
        return Ok(cached.clone());
    }

    path.insert(task_id.to_owned());
    let task = source.get_task(task_id)?;
    let messages = source.list_task_messages(task_id)?;
    let mut subtasks = Vec::with_capacity(task.subtasks.len());

    for child_task_id in &task.subtasks {
        subtasks.push(fetch_task_tree_inner(
            source,
            child_task_id,
            depth + 1,
            options,
            path,
            cache,
        )?);
    }

    path.remove(task_id);

    let tree = YougileTaskTree {
        task,
        messages,
        subtasks,
    };
    cache.insert(task_id.to_owned(), tree.clone());
    Ok(tree)
}

fn normalize_api_base_url(base_url: &str) -> String {
    let trimmed = base_url.trim_end_matches('/');
    if trimmed.ends_with("/api-v2") {
        trimmed.to_owned()
    } else {
        format!("{trimmed}/api-v2")
    }
}
