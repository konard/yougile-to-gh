use serde::Deserialize;
use serde_json::{json, Value};

use crate::error::{Result, YougileToGhError};
use crate::http::{HttpClient, HttpRequest, UreqHttpClient};
use crate::yougile::normalize_api_base_url;

/// A `YouGile` company (workspace) returned by the authentication API.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct YougileCompany {
    /// Opaque company identifier used when requesting an API key.
    pub id: String,

    /// Human-readable company name (may be empty).
    #[serde(default)]
    pub name: String,

    /// Whether the authenticated user is an administrator of the company.
    #[serde(default, rename = "isAdmin")]
    pub is_admin: bool,
}

/// Outcome of resolving a `YouGile` API token from user credentials.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedToken {
    /// The freshly created `YouGile` API key (bearer token).
    pub token: String,

    /// The company the token grants access to.
    pub company_id: String,
}

/// Client for the `YouGile` authentication endpoints.
///
/// Unlike [`YougileClient`](crate::YougileClient), this client does not require
/// a token: it exchanges user credentials (login + password) for a company list
/// and a freshly created API key via the `AuthKeyController` endpoints.
#[derive(Clone, Debug)]
pub struct YougileAuth<C = UreqHttpClient> {
    api_base_url: String,
    http_client: C,
}

impl YougileAuth<UreqHttpClient> {
    /// Create an authenticator using the default `ureq`-backed HTTP client.
    #[must_use]
    pub fn new(base_url: impl Into<String>) -> Self {
        Self::with_http_client(base_url, UreqHttpClient::new())
    }
}

impl<C> YougileAuth<C> {
    /// Create an authenticator with a custom HTTP client (useful for tests).
    pub fn with_http_client(base_url: impl Into<String>, http_client: C) -> Self {
        Self {
            api_base_url: normalize_api_base_url(&base_url.into()),
            http_client,
        }
    }
}

impl<C: HttpClient> YougileAuth<C> {
    /// List the companies the credentials can access.
    ///
    /// Calls `POST /api-v2/auth/companies`. The optional `name` filter is passed
    /// straight through to the API when provided.
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP request fails or the response cannot be
    /// parsed.
    pub fn list_companies(
        &self,
        login: &str,
        password: &str,
        name: Option<&str>,
    ) -> Result<Vec<YougileCompany>> {
        let mut body = json!({ "login": login, "password": password });
        if let Some(name) = name {
            body["name"] = json!(name);
        }

        let request = HttpRequest::post(format!("{}/auth/companies", self.api_base_url), body)
            .with_header("Accept", "application/json");
        let response = self.http_client.send(request)?;
        parse_companies(&response.body)
    }

    /// Create a fresh API key for `company_id`.
    ///
    /// Calls `POST /api-v2/auth/keys` (`AuthKeyController_create`) and returns
    /// the `key` field from the response.
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP request fails, the response cannot be
    /// parsed, or no key is present in the response.
    pub fn create_api_key(&self, login: &str, password: &str, company_id: &str) -> Result<String> {
        let request = HttpRequest::post(
            format!("{}/auth/keys", self.api_base_url),
            json!({ "login": login, "password": password, "companyId": company_id }),
        )
        .with_header("Accept", "application/json");

        let response = self.http_client.send(request)?;
        response
            .body
            .get("key")
            .and_then(Value::as_str)
            .filter(|key| !key.is_empty())
            .map(ToOwned::to_owned)
            .ok_or(YougileToGhError::YougileMissingApiKey)
    }

    /// Resolve a usable API token from user credentials.
    ///
    /// When `company_id` is supplied it is used directly. Otherwise the company
    /// list is fetched: a single company is selected automatically, an empty
    /// list yields [`YougileToGhError::YougileNoCompanies`], and multiple
    /// companies yield [`YougileToGhError::YougileMultipleCompanies`] listing
    /// the available choices so the caller can pick one.
    ///
    /// # Errors
    ///
    /// Returns an error if company resolution fails or the key cannot be
    /// created.
    pub fn resolve_token(
        &self,
        login: &str,
        password: &str,
        company_id: Option<&str>,
    ) -> Result<ResolvedToken> {
        let company_id = match company_id {
            Some(id) => id.to_owned(),
            None => self.resolve_company_id(login, password)?,
        };

        let token = self.create_api_key(login, password, &company_id)?;
        Ok(ResolvedToken { token, company_id })
    }

    fn resolve_company_id(&self, login: &str, password: &str) -> Result<String> {
        let mut companies = self.list_companies(login, password, None)?;
        match companies.len() {
            0 => Err(YougileToGhError::YougileNoCompanies),
            1 => Ok(companies.remove(0).id),
            _ => Err(YougileToGhError::YougileMultipleCompanies {
                companies: describe_companies(&companies),
            }),
        }
    }
}

/// Extract the company array from a `POST /auth/companies` response.
///
/// The `YouGile` API returns a paginated `{ "paging": ..., "content": [...] }`
/// envelope; some community clients describe it as `{ "companies": [...] }`.
/// Both shapes (and a bare array) are accepted for robustness.
fn parse_companies(body: &Value) -> Result<Vec<YougileCompany>> {
    let array = body
        .get("content")
        .or_else(|| body.get("companies"))
        .unwrap_or(body);

    serde_json::from_value(array.clone())
        .map_err(|source| YougileToGhError::json("YouGile auth companies response", source))
}

fn describe_companies(companies: &[YougileCompany]) -> String {
    companies
        .iter()
        .map(|company| {
            if company.name.is_empty() {
                company.id.clone()
            } else {
                format!("{} ({})", company.id, company.name)
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}
