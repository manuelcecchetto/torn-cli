use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use url::Url;

pub const REDACTED: &str = "<redacted>";
const SECRET_QUERY_KEYS: &[&str] = &[
    "key",
    "api_key",
    "apikey",
    "apiKey",
    "access_token",
    "token",
    "auth",
    "authorization",
];

pub fn is_secret_query_key(name: &str) -> bool {
    SECRET_QUERY_KEYS
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(name))
}

pub fn redact_secret(_secret: &str) -> String {
    REDACTED.to_string()
}

pub fn redact_known_secrets(text: &str, secrets: &[String]) -> String {
    let mut redacted = text.to_string();
    for secret in secrets.iter().filter(|secret| !secret.is_empty()) {
        redacted = redacted.replace(secret, &redact_secret(secret));
    }
    redact_authorization_lines(&redacted)
}

pub fn redact_authorization_value(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return REDACTED.to_string();
    }

    let mut parts = trimmed.splitn(2, char::is_whitespace);
    let scheme = parts.next().unwrap_or_default();
    if let Some(rest) = parts.next() {
        if scheme.eq_ignore_ascii_case("apikey")
            || scheme.eq_ignore_ascii_case("bearer")
            || scheme.eq_ignore_ascii_case("basic")
        {
            return format!("{scheme} {REDACTED}");
        }
        if !rest.trim().is_empty() {
            return format!("{scheme} {REDACTED}");
        }
    }
    REDACTED.to_string()
}

pub fn redact_header_value(name: &str, value: &HeaderValue) -> String {
    let value = value.to_str().unwrap_or(REDACTED);
    if name.eq_ignore_ascii_case(AUTHORIZATION.as_str()) {
        redact_authorization_value(value)
    } else {
        value.to_string()
    }
}

pub fn redact_headers(headers: &HeaderMap) -> Vec<(String, String)> {
    headers
        .iter()
        .map(|(name, value)| (name.to_string(), redact_header_value(name.as_str(), value)))
        .collect()
}

pub fn redact_url(url: &Url) -> String {
    let query = url
        .query_pairs()
        .map(|(name, value)| {
            let value = if is_secret_query_key(&name) {
                REDACTED.to_string()
            } else {
                value.into_owned()
            };
            format!("{}={}", name, value)
        })
        .collect::<Vec<_>>();

    let mut without_query = url.clone();
    let fragment = without_query.fragment().map(str::to_string);
    without_query.set_query(None);
    without_query.set_fragment(None);

    let mut rendered = without_query.to_string();
    if !query.is_empty() {
        rendered.push('?');
        rendered.push_str(&query.join("&"));
    }
    if let Some(fragment) = fragment {
        rendered.push('#');
        rendered.push_str(&fragment);
    }
    rendered
}

pub fn redact_url_str(input: &str) -> String {
    Url::parse(input)
        .map(|url| redact_url(&url))
        .unwrap_or_else(|_| input.to_string())
}

fn redact_authorization_lines(text: &str) -> String {
    text.lines()
        .map(|line| {
            if let Some((name, value)) = line.split_once(':') {
                if name.trim().eq_ignore_ascii_case("authorization") {
                    return format!("{}: {}", name, redact_authorization_value(value));
                }
            }
            line.to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_short_and_long_secrets() {
        assert_eq!(redact_secret("short"), "<redacted>");
        assert_eq!(redact_secret("abc123456789"), "<redacted>");
    }

    #[test]
    fn redacts_secret_url_query_values() {
        let url = Url::parse("https://ffscouter.com/api/v1/check-key?key=abc123456789&user_id=42")
            .unwrap();
        assert_eq!(
            redact_url(&url),
            "https://ffscouter.com/api/v1/check-key?key=<redacted>&user_id=42"
        );
    }

    #[test]
    fn redacts_authorization_header_values() {
        assert_eq!(
            redact_authorization_value("ApiKey abc123456789"),
            "ApiKey <redacted>"
        );
    }
}
