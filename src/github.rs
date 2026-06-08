use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::error::{Result, YougileToGhError};
use crate::http::{HttpClient, HttpRequest, UreqHttpClient};
use crate::models::{CreatedGitHubComment, CreatedGitHubIssue};

const DEFAULT_GITHUB_API_URL: &str = "https://api.github.com";
const DEFAULT_GITHUB_API_VERSION: &str = "2026-03-10";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GitHubIssueDraft {
    pub yougile_task_id: String,
    pub parent_yougile_task_id: Option<String>,
    pub title: String,
    pub body: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assignees: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub comments: Vec<String>,
}

pub trait GitHubSink {
    fn create_issue(&self, draft: &GitHubIssueDraft) -> Result<CreatedGitHubIssue>;

    fn create_issue_comment(&self, issue_number: u64, body: &str) -> Result<CreatedGitHubComment>;

    fn add_sub_issue(&self, parent_issue_number: u64, sub_issue_id: u64) -> Result<()>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitHubRepository {
    owner: String,
    name: String,
}

impl GitHubRepository {
    pub fn parse(value: &str) -> Result<Self> {
        let Some((owner, name)) = value.split_once('/') else {
            return Err(YougileToGhError::InvalidGitHubRepo(value.to_owned()));
        };

        if owner.is_empty() || name.is_empty() || name.contains('/') {
            return Err(YougileToGhError::InvalidGitHubRepo(value.to_owned()));
        }

        Ok(Self {
            owner: owner.to_owned(),
            name: name.to_owned(),
        })
    }
}

#[derive(Clone, Debug)]
pub struct GitHubClient<C = UreqHttpClient> {
    api_url: String,
    token: String,
    repository: GitHubRepository,
    http_client: C,
}

impl GitHubClient<UreqHttpClient> {
    pub fn new(token: impl Into<String>, repository: GitHubRepository) -> Self {
        Self::with_http_client(
            DEFAULT_GITHUB_API_URL,
            token,
            repository,
            UreqHttpClient::new(),
        )
    }
}

impl<C> GitHubClient<C> {
    pub fn with_http_client(
        api_url: impl Into<String>,
        token: impl Into<String>,
        repository: GitHubRepository,
        http_client: C,
    ) -> Self {
        Self {
            api_url: api_url.into().trim_end_matches('/').to_owned(),
            token: token.into(),
            repository,
            http_client,
        }
    }
}

impl<C: HttpClient> GitHubSink for GitHubClient<C> {
    fn create_issue(&self, draft: &GitHubIssueDraft) -> Result<CreatedGitHubIssue> {
        #[derive(Serialize)]
        struct Payload {
            title: String,
            body: String,
            #[serde(skip_serializing_if = "Vec::is_empty")]
            labels: Vec<String>,
            #[serde(skip_serializing_if = "Vec::is_empty")]
            assignees: Vec<String>,
        }

        let payload = Payload {
            title: draft.title.clone(),
            body: draft.body.clone(),
            labels: draft.labels.clone(),
            assignees: draft.assignees.clone(),
        };
        let request = HttpRequest::post(
            self.repo_url("issues"),
            serde_json::to_value(payload)
                .map_err(|source| YougileToGhError::json("GitHub create issue payload", source))?,
        );

        let issue: CreatedGitHubIssue = self.send_json(request, "GitHub create issue")?;
        if issue.id == 0 {
            return Err(YougileToGhError::MissingGitHubIssueId);
        }
        Ok(issue)
    }

    fn create_issue_comment(&self, issue_number: u64, body: &str) -> Result<CreatedGitHubComment> {
        let request = HttpRequest::post(
            self.repo_url(&format!("issues/{issue_number}/comments")),
            json!({ "body": body }),
        );

        self.send_json(request, "GitHub create issue comment")
    }

    fn add_sub_issue(&self, parent_issue_number: u64, sub_issue_id: u64) -> Result<()> {
        let request = HttpRequest::post(
            self.repo_url(&format!("issues/{parent_issue_number}/sub_issues")),
            json!({ "sub_issue_id": sub_issue_id }),
        );
        let _response: serde_json::Value = self.send_json(request, "GitHub add sub-issue")?;
        Ok(())
    }
}

impl<C: HttpClient> GitHubClient<C> {
    fn repo_url(&self, path: &str) -> String {
        format!(
            "{}/repos/{}/{}/{}",
            self.api_url, self.repository.owner, self.repository.name, path
        )
    }

    fn send_json<T: serde::de::DeserializeOwned>(
        &self,
        request: HttpRequest,
        context: &str,
    ) -> Result<T> {
        let request = request
            .with_header("Accept", "application/vnd.github+json")
            .with_header("Authorization", format!("Bearer {}", self.token))
            .with_header("X-GitHub-Api-Version", DEFAULT_GITHUB_API_VERSION)
            .with_header("User-Agent", "yougile-to-gh");

        let response = self.http_client.send(request)?;
        serde_json::from_value(response.body)
            .map_err(|source| YougileToGhError::json(context, source))
    }
}
