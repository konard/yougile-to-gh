use std::io;

use thiserror::Error;

pub type Result<T> = std::result::Result<T, YougileToGhError>;

#[derive(Debug, Error)]
pub enum YougileToGhError {
    #[error("HTTP request failed for {url}: {message}")]
    HttpTransport { url: String, message: String },

    #[error("HTTP {status} for {url}: {body}")]
    HttpStatus {
        status: u16,
        url: String,
        body: String,
    },

    #[error("failed to parse JSON from {context}: {source}")]
    Json {
        context: String,
        #[source]
        source: serde_json::Error,
    },

    #[error("failed to read {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: io::Error,
    },

    #[error("missing required value: {0}")]
    MissingValue(&'static str),

    #[error("could not detect {value} with `{command}`: {message}; set {fallback}")]
    GitHubCliDetection {
        value: &'static str,
        command: &'static str,
        fallback: &'static str,
        message: String,
    },

    #[error("GitHub repository must use owner/repo format, got {0:?}")]
    InvalidGitHubRepo(String),

    #[error("task graph contains a recursive cycle at YouGile task {0}")]
    TaskCycle(String),

    #[error("maximum task recursion depth {max_depth} exceeded at YouGile task {task_id}")]
    MaxDepthExceeded { task_id: String, max_depth: usize },

    #[error("GitHub response for created issue did not include a numeric issue id")]
    MissingGitHubIssueId,

    #[error("parent YouGile task {parent_task_id} was not created before child task {task_id}")]
    MissingParentIssue {
        task_id: String,
        parent_task_id: String,
    },
}

impl YougileToGhError {
    pub fn json(context: impl Into<String>, source: serde_json::Error) -> Self {
        Self::Json {
            context: context.into(),
            source,
        }
    }

    pub fn io(path: impl Into<String>, source: io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}
