use std::{
    collections::VecDeque,
    fmt,
    sync::Arc,
    time::{Duration, Instant},
};

use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue, USER_AGENT};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::Mutex;
use url::Url;

use crate::{
    config::Config,
    error::AppError,
    redaction::{redact_headers, redact_known_secrets, redact_url},
    request::{ApiRequest, HttpMethod, QueryParam, Service},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryPolicy {
    pub max_attempts: usize,
    pub backoff: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 1,
            backoff: Duration::from_millis(250),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PaginationOptions {
    pub max_pages: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct ApiClient {
    http: reqwest::Client,
    config: Config,
    retry_policy: RetryPolicy,
    rate_limiter: Option<Arc<RateLimiter>>,
}

#[derive(Clone)]
pub struct PreparedRequest {
    pub method: HttpMethod,
    pub url: Url,
    pub headers: HeaderMap,
    pub body: Option<Value>,
}

impl PreparedRequest {
    pub fn redacted_url(&self) -> String {
        redact_url(&self.url)
    }

    pub fn redacted_headers(&self) -> Vec<(String, String)> {
        redact_headers(&self.headers)
    }
}

impl fmt::Debug for PreparedRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PreparedRequest")
            .field("method", &self.method)
            .field("url", &self.redacted_url())
            .field("headers", &self.redacted_headers())
            .field("body", &self.body.as_ref().map(|_| "<present>"))
            .finish()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse {
    pub service: Service,
    pub status: u16,
    pub body_text: String,
    pub body_json: Option<Value>,
    pub from_cache: bool,
    pub elapsed_ms: u128,
}

impl ApiClient {
    pub fn new(config: Config) -> Result<Self, AppError> {
        let http = reqwest::Client::builder()
            .user_agent(config.user_agent.clone())
            .build()
            .map_err(AppError::network)?;
        Ok(Self {
            http,
            config,
            retry_policy: RetryPolicy::default(),
            rate_limiter: Some(Arc::new(RateLimiter::per_minute(95))),
        })
    }

    pub fn with_retry_policy(mut self, retry_policy: RetryPolicy) -> Self {
        self.retry_policy = retry_policy;
        self
    }

    pub fn with_rate_limit_per_minute(mut self, max_requests: usize) -> Self {
        self.rate_limiter = Some(Arc::new(RateLimiter::per_minute(max_requests)));
        self
    }

    pub fn without_rate_limiter(mut self) -> Self {
        self.rate_limiter = None;
        self
    }

    pub fn prepare(&self, request: &ApiRequest) -> Result<PreparedRequest, AppError> {
        prepare_request(&self.config, request)
    }

    pub async fn execute(&self, request: ApiRequest) -> Result<ApiResponse, AppError> {
        let mut last_error = None;
        let attempts = self.retry_policy.max_attempts.max(1);
        for attempt in 1..=attempts {
            if let Some(rate_limiter) = &self.rate_limiter {
                rate_limiter.acquire().await;
            }
            match self.execute_once(&request).await {
                Ok(response) => return Ok(response),
                Err(error)
                    if request.method.is_safe_for_retry()
                        && attempt < attempts
                        && error.is_retryable() =>
                {
                    last_error = Some(error);
                    tokio::time::sleep(self.retry_policy.backoff).await;
                }
                Err(error) => return Err(error),
            }
        }
        Err(last_error.unwrap_or_else(|| AppError::Network("request failed".to_string())))
    }

    pub async fn execute_pages(
        &self,
        first: ApiRequest,
        options: PaginationOptions,
    ) -> Result<Vec<ApiResponse>, AppError> {
        if !matches!(first.method, HttpMethod::Get) {
            return Err(AppError::InvalidRequest(
                "pagination is only supported for GET requests".to_string(),
            ));
        }
        let max_pages = options.max_pages.unwrap_or(10);
        if max_pages == 0 {
            return Err(AppError::InvalidRequest(
                "pagination max_pages must be greater than zero".to_string(),
            ));
        }

        let mut current = first.clone();
        let mut responses = Vec::new();
        for _ in 0..max_pages {
            let response = self.execute(current.clone()).await?;
            let next = response.body_json.as_ref().and_then(next_page_url);
            responses.push(response);
            let Some(next) = next else {
                return Ok(responses);
            };
            current = self.request_from_next_url(&first, &next)?;
        }

        Err(AppError::InvalidRequest(format!(
            "pagination stopped after {max_pages} pages before the API returned no next link"
        )))
    }

    pub(crate) fn request_from_next_url(
        &self,
        original: &ApiRequest,
        next: &str,
    ) -> Result<ApiRequest, AppError> {
        let url = Url::parse(next).map_err(|err| {
            AppError::InvalidRequest(format!("invalid pagination next URL from API: {err}"))
        })?;
        let base_url = match original.service {
            Service::Torn => &self.config.torn.base_url,
            Service::Ffscouter => &self.config.ffscouter.base_url,
        };
        let path = strip_base_path(base_url, url.path());
        let query = url
            .query_pairs()
            .filter(|(name, _)| !crate::redaction::is_secret_query_key(name))
            .map(|(name, value)| QueryParam::new(name.into_owned(), value.into_owned()))
            .collect();
        Ok(ApiRequest {
            service: original.service,
            method: original.method,
            path,
            query,
            body: None,
            use_auth: original.use_auth,
            cache_policy: original.cache_policy.clone(),
        })
    }

    async fn execute_once(&self, request: &ApiRequest) -> Result<ApiResponse, AppError> {
        let prepared = self.prepare(request)?;
        let mut builder = self
            .http
            .request(reqwest::Method::from(prepared.method), prepared.url.clone())
            .headers(prepared.headers.clone());
        if let Some(body) = &prepared.body {
            builder = builder.json(body);
        }

        let start = Instant::now();
        let response = builder.send().await.map_err(AppError::network)?;
        let status = response.status().as_u16();
        let body_text = response.text().await.map_err(AppError::network)?;
        let elapsed_ms = start.elapsed().as_millis();
        if !(200..300).contains(&status) {
            return Err(AppError::HttpStatus {
                status,
                body: redact_known_secrets(&body_text, &self.config.secret_values()),
            });
        }
        let secrets = self.config.secret_values();
        let body_json = serde_json::from_str::<Value>(&body_text).ok();
        if let Some(error) = api_error(request.service, body_json.as_ref(), &secrets) {
            return Err(error);
        }
        let redacted_body_json = body_json.map(|value| redact_json_secrets(value, &secrets));
        let redacted_body_text = redact_known_secrets(&body_text, &secrets);
        Ok(ApiResponse {
            service: request.service,
            status,
            body_text: redacted_body_text,
            body_json: redacted_body_json,
            from_cache: false,
            elapsed_ms,
        })
    }
}

pub fn prepare_request(config: &Config, request: &ApiRequest) -> Result<PreparedRequest, AppError> {
    reject_secret_query_params(request)?;
    let mut url = build_url(config, request.service, &request.path, &request.query)?;
    let mut headers = HeaderMap::new();
    headers.insert(
        USER_AGENT,
        HeaderValue::from_str(&config.user_agent).map_err(|err| {
            AppError::InvalidRequest(format!("invalid configured user-agent: {err}"))
        })?,
    );

    if request.use_auth {
        match request.service {
            Service::Torn => {
                let key = config
                    .torn
                    .api_key
                    .as_ref()
                    .ok_or(AppError::MissingApiKey {
                        service: Service::Torn,
                    })?;
                headers.insert(
                    AUTHORIZATION,
                    HeaderValue::from_str(&format!("ApiKey {}", key.expose_secret())).map_err(
                        |err| {
                            AppError::InvalidRequest(format!("invalid Torn API key header: {err}"))
                        },
                    )?,
                );
            }
            Service::Ffscouter => {
                let key = config
                    .ffscouter
                    .api_key
                    .as_ref()
                    .ok_or(AppError::MissingApiKey {
                        service: Service::Ffscouter,
                    })?;
                upsert_query_param(&mut url, "key", key.expose_secret());
            }
        }
    }

    Ok(PreparedRequest {
        method: request.method,
        url,
        headers,
        body: request.body.clone(),
    })
}

pub fn build_url(
    config: &Config,
    service: Service,
    path: &str,
    query: &[QueryParam],
) -> Result<Url, AppError> {
    let mut url = match service {
        Service::Torn => config.torn.base_url.clone(),
        Service::Ffscouter => config.ffscouter.base_url.clone(),
    };
    let base_path = url.path().trim_end_matches('/');
    let request_path = path.trim_start_matches('/');
    let full_path = if base_path.is_empty() {
        format!("/{request_path}")
    } else if request_path.is_empty() {
        base_path.to_string()
    } else {
        format!("{base_path}/{request_path}")
    };
    url.set_path(&full_path);
    url.set_query(None);
    if !query.is_empty() {
        url.query_pairs_mut()
            .extend_pairs(query.iter().map(|param| (&param.name, &param.value)));
    }
    Ok(url)
}

fn reject_secret_query_params(request: &ApiRequest) -> Result<(), AppError> {
    if let Some(param) = request
        .query
        .iter()
        .find(|param| crate::redaction::is_secret_query_key(&param.name))
    {
        return Err(AppError::InvalidRequest(format!(
            "query parameter '{}' may contain credentials; provide API keys through config, env, or dedicated CLI flags instead",
            param.name
        )));
    }
    Ok(())
}

fn upsert_query_param(url: &mut Url, name: &str, value: &str) {
    let mut pairs = url
        .query_pairs()
        .filter(|(existing, _)| existing != name)
        .map(|(name, value)| (name.into_owned(), value.into_owned()))
        .collect::<Vec<_>>();
    pairs.push((name.to_string(), value.to_string()));
    url.query_pairs_mut()
        .clear()
        .extend_pairs(pairs.iter().map(|(name, value)| (name, value)));
}

fn value_to_error_code(value: &Value, secrets: &[String]) -> Option<String> {
    match value {
        Value::Number(number) => Some(number.to_string()),
        Value::String(text) => Some(redact_known_secrets(text, secrets)),
        _ => None,
    }
}

fn redact_json_secrets(value: Value, secrets: &[String]) -> Value {
    match value {
        Value::String(text) => Value::String(redact_known_secrets(&text, secrets)),
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(|item| redact_json_secrets(item, secrets))
                .collect(),
        ),
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(key, value)| (key, redact_json_secrets(value, secrets)))
                .collect(),
        ),
        other => other,
    }
}

fn api_error(service: Service, body: Option<&Value>, secrets: &[String]) -> Option<AppError> {
    let body = body?;
    let error = body.get("error")?;
    match error {
        Value::String(message) => Some(AppError::ApiError {
            service,
            code: body
                .get("code")
                .and_then(|value| value_to_error_code(value, secrets)),
            message: redact_known_secrets(message, secrets),
        }),
        Value::Object(map) => {
            let code = map
                .get("code")
                .and_then(|value| value_to_error_code(value, secrets));
            let message = map
                .get("error")
                .or_else(|| map.get("message"))
                .and_then(Value::as_str)
                .unwrap_or("API returned an error payload");
            Some(AppError::ApiError {
                service,
                code,
                message: redact_known_secrets(message, secrets),
            })
        }
        _ => None,
    }
}

fn next_page_url(value: &Value) -> Option<String> {
    value
        .pointer("/_metadata/links/next")
        .and_then(Value::as_str)
        .filter(|next| !next.trim().is_empty())
        .map(str::to_string)
}

fn strip_base_path(base_url: &Url, path: &str) -> String {
    let base_path = base_url.path().trim_end_matches('/');
    if !base_path.is_empty() && path.starts_with(base_path) {
        let stripped = path[base_path.len()..].trim_start_matches('/');
        format!("/{stripped}")
    } else if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    }
}

#[derive(Debug)]
struct RateLimiter {
    max_requests: usize,
    window: Duration,
    calls: Mutex<VecDeque<Instant>>,
}

impl RateLimiter {
    fn per_minute(max_requests: usize) -> Self {
        Self {
            max_requests,
            window: Duration::from_secs(60),
            calls: Mutex::new(VecDeque::new()),
        }
    }

    async fn acquire(&self) {
        if self.max_requests == 0 {
            return;
        }
        loop {
            let sleep_for = {
                let mut calls = self.calls.lock().await;
                let now = Instant::now();
                while calls
                    .front()
                    .is_some_and(|instant| now.duration_since(*instant) >= self.window)
                {
                    calls.pop_front();
                }
                if calls.len() < self.max_requests {
                    calls.push_back(now);
                    return;
                }
                calls
                    .front()
                    .map(|oldest| self.window.saturating_sub(now.duration_since(*oldest)))
                    .unwrap_or_default()
            };
            tokio::time::sleep(sleep_for).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, path::PathBuf};

    use super::*;
    use crate::{
        config::{ConfigLoadOptions, ConfigOverrides},
        request::QueryParam,
    };

    fn test_config() -> Config {
        let mut env = HashMap::new();
        env.insert("TORN_API_KEY".to_string(), "torn-secret".to_string());
        env.insert("FFSCOUTER_API_KEY".to_string(), "ff-secret".to_string());
        Config::load(&ConfigLoadOptions {
            no_env: true,
            current_dir: PathBuf::from("."),
            overrides: ConfigOverrides {
                torn_base_url: Some("https://api.torn.com/v2".to_string()),
                ffscouter_base_url: Some("https://ffscouter.test/api/v1".to_string()),
                ..ConfigOverrides::default()
            },
            process_env: Some(env),
            ..ConfigLoadOptions::default()
        })
        .unwrap()
    }

    #[test]
    fn torn_auth_uses_authorization_header_not_query() {
        let request = ApiRequest::get(Service::Torn, "/user/basic").unwrap();
        let prepared = prepare_request(&test_config(), &request).unwrap();
        assert_eq!(prepared.url.as_str(), "https://api.torn.com/v2/user/basic");
        assert_eq!(
            prepared.headers.get(AUTHORIZATION).unwrap(),
            "ApiKey torn-secret"
        );
        assert!(!prepared.url.as_str().contains("torn-secret"));
    }

    #[test]
    fn ff_auth_is_query_param_and_redacted_for_display() {
        let request = ApiRequest::get(Service::Ffscouter, "/check-key").unwrap();
        let prepared = prepare_request(&test_config(), &request).unwrap();
        assert!(prepared.url.as_str().contains("key=ff-secret"));
        assert!(!prepared.redacted_url().contains("ff-secret"));
    }

    #[test]
    fn prepared_request_debug_redacts_auth_material() {
        let request = ApiRequest::get(Service::Ffscouter, "/check-key").unwrap();
        let prepared = prepare_request(&test_config(), &request).unwrap();
        let debug = format!("{prepared:?}");
        assert!(debug.contains("key=<redacted>"));
        assert!(!debug.contains("ff-secret"));

        let request = ApiRequest::get(Service::Torn, "/user/basic").unwrap();
        let prepared = prepare_request(&test_config(), &request).unwrap();
        let debug = format!("{prepared:?}");
        assert!(debug.contains("ApiKey <redacted>"));
        assert!(!debug.contains("torn-secret"));
    }

    #[test]
    fn query_params_are_preserved() {
        let request = ApiRequest::get(Service::Torn, "/user/basic")
            .unwrap()
            .with_params(vec![QueryParam::new("limit", "20")]);
        let prepared = prepare_request(&test_config(), &request).unwrap();
        assert_eq!(
            prepared.url.as_str(),
            "https://api.torn.com/v2/user/basic?limit=20"
        );
    }

    #[test]
    fn rejects_user_supplied_secret_query_params() {
        let request = ApiRequest::get(Service::Ffscouter, "/check-key?key=raw-secret").unwrap();
        let error = prepare_request(&test_config(), &request).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("provide API keys through config")
        );
        assert!(!error.to_string().contains("raw-secret"));
    }

    #[test]
    fn pagination_next_url_strips_base_path_and_secret_query() {
        let client = ApiClient::new(test_config()).unwrap();
        let original = ApiRequest::get(Service::Torn, "/user/events").unwrap();
        let next = client
            .request_from_next_url(
                &original,
                "https://api.torn.com/v2/user/events?offset=100&key=leaked",
            )
            .unwrap();
        assert_eq!(next.path, "/user/events");
        assert_eq!(next.query, vec![QueryParam::new("offset", "100")]);
    }

    #[test]
    fn response_json_redaction_replaces_secret_values() {
        let value = serde_json::json!({
            "key": "ff-secret",
            "nested": {"message": "token ff-secret"}
        });
        let redacted = redact_json_secrets(value, &["ff-secret".to_string()]);
        let text = redacted.to_string();
        assert!(text.contains("<redacted>"));
        assert!(!text.contains("ff-secret"));
    }

    #[test]
    fn api_error_redacts_known_secrets() {
        let body = serde_json::json!({
            "error": {
                "code": "ff-secret",
                "message": "invalid key ff-secret"
            }
        });
        let error = api_error(Service::Ffscouter, Some(&body), &["ff-secret".to_string()])
            .unwrap()
            .to_string();
        assert!(error.contains("<redacted>"));
        assert!(!error.contains("ff-secret"));
    }
}
