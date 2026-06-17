use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{redaction::is_secret_query_key, request::ApiRequest};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CachePolicy {
    #[default]
    Default,
    Disabled,
    Fresh,
    Ttl(Duration),
}

impl CachePolicy {
    pub fn effective_ttl(&self, default_ttl: Duration) -> Option<Duration> {
        match self {
            Self::Default => Some(default_ttl),
            Self::Ttl(ttl) => Some(*ttl),
            Self::Disabled | Self::Fresh => None,
        }
    }

    pub fn bypass_read(&self) -> bool {
        matches!(self, Self::Disabled | Self::Fresh)
    }

    pub fn write_after_fetch(&self) -> bool {
        !matches!(self, Self::Disabled)
    }
}

pub fn cache_key(request: &ApiRequest) -> String {
    let mut query = request
        .query
        .iter()
        .filter(|param| !is_secret_query_key(&param.name))
        .map(|param| (param.name.as_str(), param.value.as_str()))
        .collect::<Vec<_>>();
    query.sort_unstable();

    let body_hash = request
        .body
        .as_ref()
        .map(hash_json_value)
        .unwrap_or_default();

    let mut hasher = Sha256::new();
    hasher.update(request.service.to_string().as_bytes());
    hasher.update(b"\0");
    hasher.update(request.method.to_string().as_bytes());
    hasher.update(b"\0");
    hasher.update(request.path.as_bytes());
    hasher.update(b"\0");
    for (name, value) in query {
        hasher.update(name.as_bytes());
        hasher.update(b"=");
        hasher.update(value.as_bytes());
        hasher.update(b"\0");
    }
    hasher.update(body_hash.as_bytes());
    hex::encode(hasher.finalize())
}

fn hash_json_value(value: &serde_json::Value) -> String {
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use crate::request::{HttpMethod, QueryParam, Service};

    use super::*;

    #[test]
    fn cache_key_excludes_secret_query_values() {
        let first = ApiRequest {
            service: Service::Ffscouter,
            method: HttpMethod::Get,
            path: "/check-key".to_string(),
            query: vec![
                QueryParam::new("key", "secret-one"),
                QueryParam::new("user_id", "123"),
            ],
            body: None,
            use_auth: true,
            cache_policy: CachePolicy::Default,
        };
        let mut second = first.clone();
        second.query[0] = QueryParam::new("key", "secret-two");

        assert_eq!(cache_key(&first), cache_key(&second));
        assert!(!cache_key(&first).contains("secret-one"));
    }
}
