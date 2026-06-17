use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use url::Url;

use crate::{cache::CachePolicy, error::AppError, redaction::is_secret_query_key};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Service {
    Torn,
    Ffscouter,
}

impl fmt::Display for Service {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Torn => write!(f, "torn"),
            Self::Ffscouter => write!(f, "ffscouter"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
}

impl HttpMethod {
    pub fn is_safe_for_retry(self) -> bool {
        matches!(self, Self::Get)
    }

    pub fn is_cacheable_by_default(self) -> bool {
        matches!(self, Self::Get)
    }
}

impl fmt::Display for HttpMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Get => write!(f, "GET"),
            Self::Post => write!(f, "POST"),
            Self::Put => write!(f, "PUT"),
            Self::Delete => write!(f, "DELETE"),
        }
    }
}

impl From<HttpMethod> for reqwest::Method {
    fn from(value: HttpMethod) -> Self {
        match value {
            HttpMethod::Get => Self::GET,
            HttpMethod::Post => Self::POST,
            HttpMethod::Put => Self::PUT,
            HttpMethod::Delete => Self::DELETE,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct QueryParam {
    pub name: String,
    pub value: String,
}

impl QueryParam {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }
}

impl fmt::Debug for QueryParam {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = if is_secret_query_key(&self.name) {
            "<redacted>"
        } else {
            self.value.as_str()
        };
        f.debug_struct("QueryParam")
            .field("name", &self.name)
            .field("value", &value)
            .finish()
    }
}

impl FromStr for QueryParam {
    type Err = String;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let (name, value) = input
            .split_once('=')
            .ok_or_else(|| "expected NAME=VALUE".to_string())?;
        if name.trim().is_empty() {
            return Err("query parameter name cannot be empty".to_string());
        }
        Ok(Self::new(name.trim(), value))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TornSection {
    User,
    Faction,
    Torn,
    Market,
    Company,
    Racing,
    Forum,
    Property,
    Key,
}

impl TornSection {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Faction => "faction",
            Self::Torn => "torn",
            Self::Market => "market",
            Self::Company => "company",
            Self::Racing => "racing",
            Self::Forum => "forum",
            Self::Property => "property",
            Self::Key => "key",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiRequest {
    pub service: Service,
    pub method: HttpMethod,
    pub path: String,
    pub query: Vec<QueryParam>,
    pub body: Option<Value>,
    pub use_auth: bool,
    pub cache_policy: CachePolicy,
}

impl ApiRequest {
    pub fn new(
        service: Service,
        method: HttpMethod,
        path: impl AsRef<str>,
    ) -> Result<Self, AppError> {
        let (path, query) = split_path_and_query(path.as_ref())?;
        Ok(Self {
            service,
            method,
            path,
            query,
            body: None,
            use_auth: true,
            cache_policy: CachePolicy::Default,
        })
    }

    pub fn get(service: Service, path: impl AsRef<str>) -> Result<Self, AppError> {
        Self::new(service, HttpMethod::Get, path)
    }

    pub fn post(service: Service, path: impl AsRef<str>) -> Result<Self, AppError> {
        Self::new(service, HttpMethod::Post, path)
    }

    pub fn with_params(mut self, explicit: Vec<QueryParam>) -> Self {
        self.query = merge_query_params(self.query, explicit);
        self
    }

    pub fn with_param(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.query.push(QueryParam::new(name, value));
        self
    }

    pub fn with_body(mut self, body: Value) -> Self {
        self.body = Some(body);
        self
    }

    pub fn without_auth(mut self) -> Self {
        self.use_auth = false;
        self
    }

    pub fn with_cache_policy(mut self, cache_policy: CachePolicy) -> Self {
        self.cache_policy = cache_policy;
        self
    }

    pub fn torn_endpoint(
        section: TornSection,
        endpoint: impl AsRef<str>,
    ) -> Result<Self, AppError> {
        let endpoint = endpoint.as_ref().trim_matches('/');
        Self::get(Service::Torn, format!("/{}/{}", section.as_str(), endpoint))
    }

    pub fn torn_selection(
        section: TornSection,
        selections: &[impl AsRef<str>],
    ) -> Result<Self, AppError> {
        let selections = selections
            .iter()
            .map(|selection| selection.as_ref().trim())
            .filter(|selection| !selection.is_empty())
            .collect::<Vec<_>>()
            .join(",");
        if selections.is_empty() {
            return Err(AppError::InvalidRequest(
                "at least one Torn selection is required".to_string(),
            ));
        }
        Ok(Self::get(Service::Torn, format!("/{}", section.as_str()))?
            .with_param("selections", selections))
    }

    pub fn torn_id_endpoint(
        section: TornSection,
        id: impl AsRef<str>,
        endpoint: impl AsRef<str>,
    ) -> Result<Self, AppError> {
        let id = id.as_ref().trim_matches('/');
        let endpoint = endpoint.as_ref().trim_matches('/');
        if id.is_empty() || endpoint.is_empty() {
            return Err(AppError::InvalidRequest(
                "Torn id endpoint requires both id and endpoint".to_string(),
            ));
        }
        Self::get(
            Service::Torn,
            format!("/{}/{}/{}", section.as_str(), id, endpoint),
        )
    }
}

pub fn split_path_and_query(input: &str) -> Result<(String, Vec<QueryParam>), AppError> {
    let input = input.trim();
    if input.is_empty() {
        return Err(AppError::InvalidRequest(
            "request path cannot be empty".to_string(),
        ));
    }

    if input.starts_with("http://") || input.starts_with("https://") {
        let url = Url::parse(input)?;
        let path = normalize_path(url.path())?;
        let query = url
            .query_pairs()
            .map(|(name, value)| QueryParam::new(name.into_owned(), value.into_owned()))
            .collect();
        return Ok((path, query));
    }

    let (path, query_text) = input.split_once('?').unwrap_or((input, ""));
    let path = normalize_path(path)?;
    let query = url::form_urlencoded::parse(query_text.as_bytes())
        .map(|(name, value)| QueryParam::new(name.into_owned(), value.into_owned()))
        .collect();
    Ok((path, query))
}

fn normalize_path(path: &str) -> Result<String, AppError> {
    let path = path.trim();
    if path.is_empty() || path == "/" {
        return Err(AppError::InvalidRequest(
            "request path cannot be empty".to_string(),
        ));
    }
    let path = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    };
    if path.contains("..") {
        return Err(AppError::InvalidRequest(
            "request path must not contain '..'".to_string(),
        ));
    }
    Ok(path)
}

pub fn merge_query_params(
    mut embedded: Vec<QueryParam>,
    explicit: Vec<QueryParam>,
) -> Vec<QueryParam> {
    if explicit.is_empty() {
        return embedded;
    }
    let explicit_names = explicit
        .iter()
        .map(|param| param.name.as_str())
        .collect::<std::collections::HashSet<_>>();
    embedded.retain(|param| !explicit_names.contains(param.name.as_str()));
    embedded.extend(explicit);
    embedded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merges_explicit_params_over_embedded_query() {
        let request = ApiRequest::get(Service::Torn, "/user?selections=basic&limit=10")
            .unwrap()
            .with_params(vec![
                QueryParam::new("selections", "bars"),
                QueryParam::new("offset", "20"),
            ]);

        assert_eq!(
            request.query,
            vec![
                QueryParam::new("limit", "10"),
                QueryParam::new("selections", "bars"),
                QueryParam::new("offset", "20"),
            ]
        );
    }

    #[test]
    fn builds_torn_selection_request() {
        let request = ApiRequest::torn_selection(TornSection::User, &["basic", "bars"]).unwrap();
        assert_eq!(request.path, "/user");
        assert_eq!(
            request.query,
            vec![QueryParam::new("selections", "basic,bars")]
        );
    }
}
