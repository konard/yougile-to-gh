use serde_json::Value;

use crate::error::{Result, YougileToGhError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HttpMethod {
    Get,
    Post,
    Patch,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpHeader {
    pub name: String,
    pub value: String,
}

impl HttpHeader {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpRequest {
    pub method: HttpMethod,
    pub url: String,
    pub headers: Vec<HttpHeader>,
    pub body: Option<Value>,
}

impl HttpRequest {
    pub fn get(url: impl Into<String>) -> Self {
        Self {
            method: HttpMethod::Get,
            url: url.into(),
            headers: Vec::new(),
            body: None,
        }
    }

    pub fn post(url: impl Into<String>, body: Value) -> Self {
        Self {
            method: HttpMethod::Post,
            url: url.into(),
            headers: Vec::new(),
            body: Some(body),
        }
    }

    pub fn patch(url: impl Into<String>, body: Value) -> Self {
        Self {
            method: HttpMethod::Patch,
            url: url.into(),
            headers: Vec::new(),
            body: Some(body),
        }
    }

    #[must_use]
    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push(HttpHeader::new(name, value));
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpResponse {
    pub status: u16,
    pub body: Value,
}

pub trait HttpClient {
    fn send(&self, request: HttpRequest) -> Result<HttpResponse>;
}

#[derive(Clone, Debug)]
pub struct UreqHttpClient {
    agent: ureq::Agent,
}

impl UreqHttpClient {
    #[must_use]
    pub fn new() -> Self {
        Self {
            agent: ureq::Agent::new(),
        }
    }
}

impl Default for UreqHttpClient {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpClient for UreqHttpClient {
    fn send(&self, request: HttpRequest) -> Result<HttpResponse> {
        let url = request.url.clone();
        let mut request_builder = match request.method {
            HttpMethod::Get => self.agent.get(&url),
            HttpMethod::Post => self.agent.post(&url),
            HttpMethod::Patch => self.agent.patch(&url),
        };

        for header in request.headers {
            request_builder = request_builder.set(&header.name, &header.value);
        }

        let response_result = match request.body {
            Some(body) => request_builder.send_json(body),
            None => request_builder.call(),
        };

        match response_result {
            Ok(response) => parse_response(&url, response),
            Err(ureq::Error::Status(status, response)) => {
                let body = response.into_string().unwrap_or_default();
                Err(YougileToGhError::HttpStatus { status, url, body })
            }
            Err(error) => Err(YougileToGhError::HttpTransport {
                url,
                message: error.to_string(),
            }),
        }
    }
}

fn parse_response(url: &str, response: ureq::Response) -> Result<HttpResponse> {
    let status = response.status();
    let text = response
        .into_string()
        .map_err(|error| YougileToGhError::HttpTransport {
            url: url.to_owned(),
            message: error.to_string(),
        })?;

    let body = if text.trim().is_empty() {
        Value::Null
    } else {
        serde_json::from_str(&text).map_err(|source| YougileToGhError::json(url, source))?
    };

    Ok(HttpResponse { status, body })
}
