use std::cell::RefCell;
use std::rc::Rc;

use serde_json::{json, Value};
use yougile_to_gh::auth::YougileAuth;
use yougile_to_gh::http::{HttpClient, HttpMethod, HttpRequest, HttpResponse};
use yougile_to_gh::{Result, YougileToGhError};

type Recorder = Rc<RefCell<Vec<HttpRequest>>>;

/// Records every request and replays canned JSON responses keyed by the URL
/// suffix (e.g. `/auth/companies`).
struct FakeHttp {
    responses: Vec<(&'static str, HttpResponse)>,
    requests: Recorder,
}

impl FakeHttp {
    fn new(responses: Vec<(&'static str, Value)>) -> Self {
        Self {
            responses: responses
                .into_iter()
                .map(|(suffix, body)| (suffix, HttpResponse { status: 200, body }))
                .collect(),
            requests: Recorder::default(),
        }
    }

    fn recorder(&self) -> Recorder {
        Rc::clone(&self.requests)
    }
}

impl HttpClient for FakeHttp {
    fn send(&self, request: HttpRequest) -> Result<HttpResponse> {
        let response = self
            .responses
            .iter()
            .find(|(suffix, _)| request.url.ends_with(suffix))
            .map_or_else(
                || panic!("no canned response for {}", request.url),
                |(_, response)| response.clone(),
            );
        self.requests.borrow_mut().push(request);
        Ok(response)
    }
}

#[test]
fn create_api_key_posts_credentials_and_returns_key() {
    let http = FakeHttp::new(vec![("/auth/keys", json!({ "key": "yg-token-123" }))]);
    let auth = YougileAuth::with_http_client("https://ru.yougile.com", http);

    let key = auth
        .create_api_key("user@example.com", "secret", "company-1")
        .unwrap();

    assert_eq!(key, "yg-token-123");
}

#[test]
fn resolve_token_selects_single_company_automatically() {
    let http = FakeHttp::new(vec![
        (
            "/auth/companies",
            json!({
                "paging": { "count": 1, "limit": 50, "offset": 0, "next": false },
                "content": [ { "id": "company-1", "name": "Acme", "isAdmin": true } ]
            }),
        ),
        ("/auth/keys", json!({ "key": "yg-token-abc" })),
    ]);
    let auth = YougileAuth::with_http_client("https://ru.yougile.com", http);

    let resolved = auth
        .resolve_token("user@example.com", "secret", None)
        .unwrap();

    assert_eq!(resolved.token, "yg-token-abc");
    assert_eq!(resolved.company_id, "company-1");
}

#[test]
fn resolve_token_uses_explicit_company_without_listing() {
    // Note: no `/auth/companies` response is registered, so the listing call
    // would panic. The test passing proves the explicit company short-circuits it.
    let http = FakeHttp::new(vec![("/auth/keys", json!({ "key": "yg-token-explicit" }))]);
    let auth = YougileAuth::with_http_client("https://ru.yougile.com", http);

    let resolved = auth
        .resolve_token("user@example.com", "secret", Some("company-explicit"))
        .unwrap();

    assert_eq!(resolved.token, "yg-token-explicit");
    assert_eq!(resolved.company_id, "company-explicit");
}

#[test]
fn resolve_token_errors_when_multiple_companies_match() {
    let http = FakeHttp::new(vec![(
        "/auth/companies",
        json!({
            "content": [
                { "id": "company-1", "name": "Acme" },
                { "id": "company-2", "name": "Globex" }
            ]
        }),
    )]);
    let auth = YougileAuth::with_http_client("https://ru.yougile.com", http);

    let error = auth
        .resolve_token("user@example.com", "secret", None)
        .unwrap_err();

    match error {
        YougileToGhError::YougileMultipleCompanies { companies } => {
            assert!(companies.contains("company-1 (Acme)"));
            assert!(companies.contains("company-2 (Globex)"));
        }
        other => panic!("expected YougileMultipleCompanies, got {other:?}"),
    }
}

#[test]
fn resolve_token_errors_when_no_companies_match() {
    let http = FakeHttp::new(vec![("/auth/companies", json!({ "content": [] }))]);
    let auth = YougileAuth::with_http_client("https://ru.yougile.com", http);

    let error = auth
        .resolve_token("user@example.com", "secret", None)
        .unwrap_err();

    assert!(matches!(error, YougileToGhError::YougileNoCompanies));
}

#[test]
fn create_api_key_errors_when_key_missing() {
    let http = FakeHttp::new(vec![("/auth/keys", json!({ "error": "bad credentials" }))]);
    let auth = YougileAuth::with_http_client("https://ru.yougile.com", http);

    let error = auth
        .create_api_key("user@example.com", "secret", "company-1")
        .unwrap_err();

    assert!(matches!(error, YougileToGhError::YougileMissingApiKey));
}

#[test]
fn list_companies_accepts_companies_key_envelope() {
    let http = FakeHttp::new(vec![(
        "/auth/companies",
        json!({ "companies": [ { "id": "c1", "name": "Solo" } ] }),
    )]);
    let auth = YougileAuth::with_http_client("https://ru.yougile.com", http);

    let companies = auth
        .list_companies("user@example.com", "secret", None)
        .unwrap();

    assert_eq!(companies.len(), 1);
    assert_eq!(companies[0].id, "c1");
    assert_eq!(companies[0].name, "Solo");
}

#[test]
fn auth_targets_api_v2_endpoints_with_post_and_payload() {
    let http = FakeHttp::new(vec![("/auth/keys", json!({ "key": "k" }))]);
    let recorder = http.recorder();
    // A trailing slash in the base URL must still produce a single `/api-v2`.
    let auth = YougileAuth::with_http_client("https://ru.yougile.com/", http);

    auth.create_api_key("user@example.com", "secret", "company-9")
        .unwrap();

    let requests = recorder.borrow();
    let request = &requests[0];
    assert_eq!(request.method, HttpMethod::Post);
    assert!(
        request.url.ends_with("/api-v2/auth/keys"),
        "unexpected url: {}",
        request.url
    );
    let body = request
        .body
        .as_ref()
        .expect("auth request must carry a JSON body");
    assert_eq!(body["companyId"], json!("company-9"));
    assert_eq!(body["login"], json!("user@example.com"));
}
