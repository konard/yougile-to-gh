pub mod auth;
pub mod converter;
pub mod error;
pub mod github;
pub mod http;
pub mod models;
pub mod render;
pub mod repair;
pub mod task_url;
pub mod yougile;

pub use auth::{ResolvedToken, YougileAuth, YougileCompany};
pub use converter::{
    build_conversion_plan, execute_conversion_plan, ConversionMode, ConversionOptions,
    ConversionPlan, ConversionResult,
};
pub use error::{Result, YougileToGhError};
pub use github::{GitHubClient, GitHubIssueDraft, GitHubRepository, GitHubSink};
pub use models::{
    GitHubIssue, YougileBoard, YougileChatMessage, YougileColumn, YougileProject, YougileTask,
    YougileTaskTree,
};
pub use repair::{execute_issue_repair, parse_yougile_task_id, plan_issue_repair, RepairedIssue};
pub use task_url::{resolve_project_title, resolve_task_url, ProjectTitleCache, TaskUrlContext};
pub use yougile::{fetch_task_tree, FetchOptions, YougileClient, YougileSource};
