use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    client::ApiClient,
    error::AppError,
    log_presets::LogPresetDefinition,
    output::OutputMode,
    request::{ApiRequest, QueryParam, Service},
};

const OFFICIAL_OPENAPI_URL: &str = "https://www.torn.com/swagger/openapi.json";
const PLAYGROUND_USER_LOG_URL: &str = "https://tornapi.tornplayground.eu/user/log";
const PLAYGROUND_LOGTYPES_URL: &str = "https://tornapi.tornplayground.eu/torn/logtypes";
const PLAYGROUND_LOGCATEGORIES_URL: &str = "https://tornapi.tornplayground.eu/torn/logcategories";
const DEFAULT_BOUNDED_LOG_MAX_PAGES: usize = 1_000;
const DEFAULT_UNBOUNDED_LOG_MAX_PAGES: usize = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LogGroupBy {
    Category,
    Type,
    Day,
    Hour,
    Target,
    DataKey,
    ParamKey,
}

impl LogGroupBy {
    pub fn label(self) -> &'static str {
        match self {
            Self::Category => "category",
            Self::Type => "type",
            Self::Day => "day",
            Self::Hour => "hour",
            Self::Target => "target",
            Self::DataKey => "data-key",
            Self::ParamKey => "param-key",
        }
    }
}

#[derive(Debug, Clone)]
pub struct LogsFetchSpec {
    pub since: Option<String>,
    pub to: Option<String>,
    pub log_ids: Vec<String>,
    pub category: Option<String>,
    pub target: Option<String>,
    pub limit: u32,
    pub max_pages: Option<usize>,
    pub extra_params: Vec<QueryParam>,
}

#[derive(Debug, Clone)]
pub struct LogsAnalyzeSpec {
    pub fetch: LogsFetchSpec,
    pub group_by: LogGroupBy,
    pub contains: Vec<String>,
    pub data_keys: Vec<String>,
    pub param_keys: Vec<String>,
    pub top: usize,
    pub include_raw: bool,
}

#[derive(Debug, Clone)]
pub struct LogsPresetAnalyzeSpec {
    pub name: String,
    pub source: String,
    pub preset: LogPresetDefinition,
    pub fetch: LogsFetchSpec,
    pub categories: Vec<String>,
    pub group_by: LogGroupBy,
    pub contains: Vec<String>,
    pub data_keys: Vec<String>,
    pub param_keys: Vec<String>,
    pub top: usize,
    pub include_raw: bool,
}

#[derive(Debug, Clone)]
pub struct LogsCatalogSpec {
    pub category: Option<String>,
    pub expand_categories: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserLogEntry {
    pub id: String,
    pub timestamp: i64,
    pub details: UserLogDetails,
    #[serde(default)]
    pub data: BTreeMap<String, Value>,
    #[serde(default)]
    pub params: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct UserLogDetails {
    pub id: String,
    pub title: String,
    pub category: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogAnalysis {
    pub total_logs: usize,
    pub filtered_logs: usize,
    pub query: LogQuerySummary,
    pub pagination: LogPaginationSummary,
    pub group_by: LogGroupBy,
    pub groups: Vec<LogGroupSummary>,
    pub observed_shapes: Vec<ObservedLogShape>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_logs: Option<Vec<UserLogEntry>>,
    pub sources: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogQuerySummary {
    pub since: Option<i64>,
    pub to: Option<i64>,
    pub log_ids: Vec<String>,
    pub category: Option<String>,
    pub target: Option<String>,
    pub limit: u32,
    pub max_pages: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogPaginationSummary {
    pub pages_fetched: usize,
    pub max_pages: usize,
    pub truncated: bool,
    pub continuation_link_seen: bool,
    pub continuation_direction: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogGroupSummary {
    pub key: String,
    pub count: usize,
    pub first_timestamp: Option<i64>,
    pub last_timestamp: Option<i64>,
    pub categories: Vec<String>,
    pub log_types: Vec<String>,
    pub titles: Vec<String>,
    pub data_keys: Vec<String>,
    pub param_keys: Vec<String>,
    pub example_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservedLogShape {
    pub details: UserLogDetails,
    pub count: usize,
    pub data_keys: Vec<String>,
    pub param_keys: Vec<String>,
    pub data_value_types: BTreeMap<String, Vec<String>>,
    pub param_value_types: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogCatalog {
    pub categories: Vec<LogCatalogCategory>,
    pub all_types: Vec<LogTypeInfo>,
    pub uncategorized_types: Vec<LogTypeInfo>,
    pub category_errors: Vec<String>,
    pub sources: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct UserLogsFetchResult {
    pub entries: Vec<UserLogEntry>,
    pub pagination: LogPaginationSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogCatalogCategory {
    pub id: String,
    pub title: String,
    pub log_types: Vec<LogTypeInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct LogTypeInfo {
    pub id: String,
    pub title: String,
}

#[derive(Debug, Default)]
struct GroupAccumulator {
    count: usize,
    first_timestamp: Option<i64>,
    last_timestamp: Option<i64>,
    categories: BTreeSet<String>,
    log_types: BTreeSet<String>,
    titles: BTreeSet<String>,
    data_keys: BTreeSet<String>,
    param_keys: BTreeSet<String>,
    example_ids: Vec<String>,
}

#[derive(Debug, Default)]
struct ShapeAccumulator {
    details: Option<UserLogDetails>,
    count: usize,
    data_keys: BTreeSet<String>,
    param_keys: BTreeSet<String>,
    data_value_types: BTreeMap<String, BTreeSet<String>>,
    param_value_types: BTreeMap<String, BTreeSet<String>>,
}

pub async fn fetch_user_logs(
    client: &ApiClient,
    spec: &LogsFetchSpec,
) -> Result<Vec<UserLogEntry>, AppError> {
    Ok(fetch_user_logs_with_metadata(client, spec).await?.entries)
}

pub async fn fetch_user_logs_with_metadata(
    client: &ApiClient,
    spec: &LogsFetchSpec,
) -> Result<UserLogsFetchResult, AppError> {
    let first = user_logs_request(spec)?;
    let max_pages = resolved_log_max_pages(spec)?;
    let mut current = first.clone();
    let mut entries = Vec::new();
    let mut seen_entries = BTreeSet::new();
    let mut seen_requests = BTreeSet::new();
    let mut pages_fetched = 0;
    let mut continuation_link_seen = false;
    let mut continuation_direction = None;
    let mut truncated = false;

    for page_index in 0..max_pages {
        let request_fingerprint = request_fingerprint(&current);
        if !seen_requests.insert(request_fingerprint) {
            truncated = true;
            break;
        }

        let response = client.execute(current.clone()).await?;
        pages_fetched += 1;
        let Some(json) = &response.body_json else {
            break;
        };

        for entry in parse_user_logs(json)? {
            if seen_entries.insert(entry.id.clone()) {
                entries.push(entry);
            }
        }

        let Some(continuation) = user_logs_continuation_url(json) else {
            break;
        };
        continuation_link_seen = true;
        continuation_direction.get_or_insert_with(|| continuation.direction.to_string());

        if page_index + 1 >= max_pages {
            truncated = true;
            break;
        }

        current = client.request_from_next_url(&first, &continuation.url)?;
    }

    Ok(UserLogsFetchResult {
        entries,
        pagination: LogPaginationSummary {
            pages_fetched,
            max_pages,
            truncated,
            continuation_link_seen,
            continuation_direction,
        },
    })
}

pub async fn fetch_user_logs_for_preset(
    client: &ApiClient,
    spec: &LogsPresetAnalyzeSpec,
) -> Result<Vec<UserLogEntry>, AppError> {
    Ok(fetch_user_logs_for_preset_with_metadata(client, spec)
        .await?
        .entries)
}

pub async fn fetch_user_logs_for_preset_with_metadata(
    client: &ApiClient,
    spec: &LogsPresetAnalyzeSpec,
) -> Result<UserLogsFetchResult, AppError> {
    let mut seen = BTreeSet::new();
    let mut entries = Vec::new();
    let mut pages_fetched = 0;
    let mut max_pages = 0;
    let mut truncated = false;
    let mut continuation_link_seen = false;
    let mut directions = BTreeSet::new();

    for fetch in preset_fetch_specs(spec) {
        let result = fetch_user_logs_with_metadata(client, &fetch).await?;
        pages_fetched += result.pagination.pages_fetched;
        max_pages += result.pagination.max_pages;
        truncated |= result.pagination.truncated;
        continuation_link_seen |= result.pagination.continuation_link_seen;
        if let Some(direction) = result.pagination.continuation_direction {
            directions.insert(direction);
        }
        for entry in result.entries {
            if seen.insert(entry.id.clone()) {
                entries.push(entry);
            }
        }
    }
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.timestamp));

    Ok(UserLogsFetchResult {
        entries,
        pagination: LogPaginationSummary {
            pages_fetched,
            max_pages,
            truncated,
            continuation_link_seen,
            continuation_direction: match directions.len() {
                0 => None,
                1 => directions.into_iter().next(),
                _ => Some("mixed".to_string()),
            },
        },
    })
}

pub fn preset_fetch_specs(spec: &LogsPresetAnalyzeSpec) -> Vec<LogsFetchSpec> {
    let mut categories = spec.categories.clone();
    categories.retain(|category| !category.trim().is_empty());
    categories.sort_by(
        |left, right| match (left.parse::<u64>(), right.parse::<u64>()) {
            (Ok(left), Ok(right)) => left.cmp(&right),
            _ => left.cmp(right),
        },
    );
    categories.dedup();
    if categories.is_empty() {
        return vec![spec.fetch.clone()];
    }

    categories
        .into_iter()
        .map(|category| {
            let mut fetch = spec.fetch.clone();
            fetch.category = Some(category);
            fetch
        })
        .collect()
}

fn preset_summary_fetch_spec(spec: &LogsPresetAnalyzeSpec) -> LogsFetchSpec {
    let mut fetch = spec.fetch.clone();
    if !spec.categories.is_empty() {
        fetch.category = Some(spec.categories.join(","));
    }
    fetch
}

pub async fn analyze_user_logs(
    client: &ApiClient,
    spec: &LogsAnalyzeSpec,
) -> Result<LogAnalysis, AppError> {
    let result = fetch_user_logs_with_metadata(client, &spec.fetch).await?;
    analyze_log_entries(result.entries, result.pagination, spec)
}

pub async fn analyze_user_logs_with_preset(
    client: &ApiClient,
    spec: &LogsPresetAnalyzeSpec,
) -> Result<LogAnalysis, AppError> {
    let result = fetch_user_logs_for_preset_with_metadata(client, spec).await?;
    analyze_log_entries(
        result.entries,
        result.pagination,
        &LogsAnalyzeSpec {
            fetch: preset_summary_fetch_spec(spec),
            group_by: spec.group_by,
            contains: spec.contains.clone(),
            data_keys: spec.data_keys.clone(),
            param_keys: spec.param_keys.clone(),
            top: spec.top,
            include_raw: spec.include_raw,
        },
    )
}

fn analyze_log_entries(
    logs: Vec<UserLogEntry>,
    pagination: LogPaginationSummary,
    spec: &LogsAnalyzeSpec,
) -> Result<LogAnalysis, AppError> {
    let total_logs = logs.len();
    let filtered = logs
        .iter()
        .filter(|entry| matches_client_filters(entry, spec))
        .cloned()
        .collect::<Vec<_>>();
    let query = query_summary(&spec.fetch)?;
    let groups = summarize_groups(&filtered, spec.group_by, spec.top);
    let observed_shapes = summarize_shapes(&filtered);

    Ok(LogAnalysis {
        total_logs,
        filtered_logs: filtered.len(),
        query,
        pagination,
        group_by: spec.group_by,
        groups,
        observed_shapes,
        raw_logs: spec.include_raw.then_some(filtered),
        sources: research_sources(),
    })
}

pub async fn fetch_log_catalog(
    client: &ApiClient,
    spec: &LogsCatalogSpec,
) -> Result<LogCatalog, AppError> {
    let categories_response = client
        .execute(ApiRequest::get(Service::Torn, "/torn/logcategories")?)
        .await?;
    let all_types_response = client
        .execute(ApiRequest::get(Service::Torn, "/torn/logtypes")?)
        .await?;
    let categories_json = categories_response
        .body_json
        .as_ref()
        .ok_or_else(|| AppError::Json("/torn/logcategories did not return JSON".to_string()))?;
    let all_types_json = all_types_response
        .body_json
        .as_ref()
        .ok_or_else(|| AppError::Json("/torn/logtypes did not return JSON".to_string()))?;
    let mut categories = parse_catalog_items(categories_json, "logcategories")?
        .into_iter()
        .map(|item| LogCatalogCategory {
            id: item.id,
            title: item.title,
            log_types: Vec::new(),
        })
        .collect::<Vec<_>>();
    if let Some(category) = &spec.category {
        categories.retain(|item| item.id == *category || item.title.eq_ignore_ascii_case(category));
    }
    let mut all_types = parse_catalog_items(all_types_json, "logtypes")?;
    all_types.sort();

    let mut category_errors = Vec::new();
    if spec.expand_categories {
        for category in &mut categories {
            let path = format!("/torn/{}/logtypes", category.id);
            match client.execute(ApiRequest::get(Service::Torn, path)?).await {
                Ok(response) => {
                    if let Some(json) = &response.body_json {
                        match parse_catalog_items(json, "logtypes") {
                            Ok(mut items) => {
                                items.sort();
                                category.log_types = items;
                            }
                            Err(error) => category_errors.push(format!(
                                "category {}: could not parse logtypes: {}",
                                category.id, error
                            )),
                        }
                    }
                }
                Err(error) => category_errors.push(format!(
                    "category {}: {}",
                    category.id,
                    error.to_string().replace('\n', " ")
                )),
            }
        }
    }

    let categorized = categories
        .iter()
        .flat_map(|category| category.log_types.iter().map(|item| item.id.clone()))
        .collect::<BTreeSet<_>>();
    let uncategorized_types = all_types
        .iter()
        .filter(|item| !categorized.contains(&item.id))
        .cloned()
        .collect();

    Ok(LogCatalog {
        categories,
        all_types,
        uncategorized_types,
        category_errors,
        sources: research_sources(),
    })
}

pub fn user_logs_request(spec: &LogsFetchSpec) -> Result<ApiRequest, AppError> {
    let mut params = Vec::new();
    if let Some(since) = &spec.since {
        params.push(QueryParam::new(
            "from",
            parse_timestamp_arg(since, Utc::now())?.to_string(),
        ));
    }
    if let Some(to) = &spec.to {
        params.push(QueryParam::new(
            "to",
            parse_timestamp_arg(to, Utc::now())?.to_string(),
        ));
    }
    if !spec.log_ids.is_empty() {
        params.push(QueryParam::new("log", spec.log_ids.join(",")));
    }
    if let Some(category) = &spec.category {
        params.push(QueryParam::new("cat", category));
    }
    if let Some(target) = &spec.target {
        params.push(QueryParam::new("target", target));
    }
    if spec.limit > 0 {
        params.push(QueryParam::new("limit", spec.limit.to_string()));
    }
    params.extend(spec.extra_params.clone());
    Ok(ApiRequest::get(Service::Torn, "/user/log")?.with_params(params))
}

pub fn query_summary(spec: &LogsFetchSpec) -> Result<LogQuerySummary, AppError> {
    Ok(LogQuerySummary {
        since: spec
            .since
            .as_deref()
            .map(|value| parse_timestamp_arg(value, Utc::now()))
            .transpose()?,
        to: spec
            .to
            .as_deref()
            .map(|value| parse_timestamp_arg(value, Utc::now()))
            .transpose()?,
        log_ids: spec.log_ids.clone(),
        category: spec.category.clone(),
        target: spec.target.clone(),
        limit: spec.limit,
        max_pages: resolved_log_max_pages(spec)?,
    })
}

pub fn resolved_log_max_pages(spec: &LogsFetchSpec) -> Result<usize, AppError> {
    let max_pages = spec.max_pages.unwrap_or_else(|| {
        if spec.since.is_some() {
            DEFAULT_BOUNDED_LOG_MAX_PAGES
        } else {
            DEFAULT_UNBOUNDED_LOG_MAX_PAGES
        }
    });
    if max_pages == 0 {
        return Err(AppError::InvalidRequest(
            "log pagination max-pages must be greater than zero".to_string(),
        ));
    }
    Ok(max_pages)
}

fn request_fingerprint(request: &ApiRequest) -> String {
    let mut query = request
        .query
        .iter()
        .map(|param| format!("{}={}", param.name, param.value))
        .collect::<Vec<_>>();
    query.sort();
    format!("{} {}?{}", request.method, request.path, query.join("&"))
}

pub fn parse_timestamp_arg(input: &str, reference: DateTime<Utc>) -> Result<i64, AppError> {
    let trimmed = input.trim();
    if trimmed.eq_ignore_ascii_case("now") {
        return Ok(reference.timestamp());
    }
    if let Ok(timestamp) = trimmed.parse::<i64>() {
        return Ok(timestamp);
    }
    if let Ok(duration) = humantime::parse_duration(trimmed) {
        return Ok(reference.timestamp() - duration.as_secs() as i64);
    }
    if let Ok(timestamp) = DateTime::parse_from_rfc3339(trimmed) {
        return Ok(timestamp.timestamp());
    }
    if let Ok(date) = NaiveDate::parse_from_str(trimmed, "%Y-%m-%d") {
        let timestamp = Utc
            .from_utc_datetime(
                &date
                    .and_hms_opt(0, 0, 0)
                    .ok_or_else(|| AppError::InvalidRequest(format!("invalid date '{trimmed}'")))?,
            )
            .timestamp();
        return Ok(timestamp);
    }
    Err(AppError::InvalidRequest(format!(
        "invalid timestamp '{trimmed}'; use unix seconds, RFC3339, YYYY-MM-DD, 'now', or a relative duration like 7d/24h/30m"
    )))
}

pub fn parse_user_logs(json: &Value) -> Result<Vec<UserLogEntry>, AppError> {
    let Some(log_value) = json.get("log") else {
        return Err(AppError::Json(
            "user log response is missing top-level 'log'".to_string(),
        ));
    };
    match log_value {
        Value::Array(items) => items
            .iter()
            .enumerate()
            .map(|(idx, value)| parse_log_entry(value, Some(idx.to_string())))
            .collect(),
        Value::Object(map) => map
            .iter()
            .map(|(id, value)| parse_log_entry(value, Some(id.clone())))
            .collect(),
        _ => Err(AppError::Json(
            "user log response 'log' must be an array or object".to_string(),
        )),
    }
}

fn parse_log_entry(value: &Value, fallback_id: Option<String>) -> Result<UserLogEntry, AppError> {
    let object = value
        .as_object()
        .ok_or_else(|| AppError::Json("log entry must be a JSON object".to_string()))?;
    let id = object
        .get("id")
        .and_then(value_to_string)
        .or(fallback_id)
        .unwrap_or_else(|| "<unknown>".to_string());
    let timestamp = object
        .get("timestamp")
        .and_then(Value::as_i64)
        .unwrap_or_default();
    let details_value = object.get("details");
    let details = UserLogDetails {
        id: details_value
            .and_then(|details| details.get("id"))
            .or_else(|| object.get("log"))
            .and_then(value_to_string)
            .unwrap_or_else(|| "<unknown>".to_string()),
        title: details_value
            .and_then(|details| details.get("title"))
            .or_else(|| object.get("title"))
            .and_then(Value::as_str)
            .unwrap_or("<unknown>")
            .to_string(),
        category: details_value
            .and_then(|details| details.get("category"))
            .or_else(|| object.get("category"))
            .and_then(Value::as_str)
            .unwrap_or("<unknown>")
            .to_string(),
    };
    Ok(UserLogEntry {
        id,
        timestamp,
        details,
        data: value_object_to_btree(object.get("data")),
        params: value_object_to_btree(object.get("params")),
    })
}

fn value_to_string(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

fn value_object_to_btree(value: Option<&Value>) -> BTreeMap<String, Value> {
    value
        .and_then(Value::as_object)
        .map(|map| {
            map.iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect()
        })
        .unwrap_or_default()
}

fn matches_client_filters(entry: &UserLogEntry, spec: &LogsAnalyzeSpec) -> bool {
    spec.contains.iter().all(|needle| {
        let needle = needle.to_lowercase();
        let haystack = format!(
            "{} {} {} {} {} {}",
            entry.id,
            entry.details.id,
            entry.details.title,
            entry.details.category,
            Value::Object(entry.data.clone().into_iter().collect()),
            Value::Object(entry.params.clone().into_iter().collect())
        )
        .to_lowercase();
        haystack.contains(&needle)
    }) && spec
        .data_keys
        .iter()
        .all(|key| entry.data.contains_key(key))
        && spec
            .param_keys
            .iter()
            .all(|key| entry.params.contains_key(key))
}

fn summarize_groups(
    entries: &[UserLogEntry],
    group_by: LogGroupBy,
    top: usize,
) -> Vec<LogGroupSummary> {
    let mut groups = BTreeMap::<String, GroupAccumulator>::new();
    for entry in entries {
        for key in group_keys(entry, group_by) {
            let group = groups.entry(key).or_default();
            group.count += 1;
            group.first_timestamp = Some(
                group
                    .first_timestamp
                    .map(|first| first.min(entry.timestamp))
                    .unwrap_or(entry.timestamp),
            );
            group.last_timestamp = Some(
                group
                    .last_timestamp
                    .map(|last| last.max(entry.timestamp))
                    .unwrap_or(entry.timestamp),
            );
            group.categories.insert(entry.details.category.clone());
            group.log_types.insert(entry.details.id.clone());
            group.titles.insert(entry.details.title.clone());
            group.data_keys.extend(entry.data.keys().cloned());
            group.param_keys.extend(entry.params.keys().cloned());
            if group.example_ids.len() < 5 {
                group.example_ids.push(entry.id.clone());
            }
        }
    }

    let mut summaries = groups
        .into_iter()
        .map(|(key, group)| LogGroupSummary {
            key,
            count: group.count,
            first_timestamp: group.first_timestamp,
            last_timestamp: group.last_timestamp,
            categories: group.categories.into_iter().collect(),
            log_types: group.log_types.into_iter().collect(),
            titles: group.titles.into_iter().collect(),
            data_keys: group.data_keys.into_iter().collect(),
            param_keys: group.param_keys.into_iter().collect(),
            example_ids: group.example_ids,
        })
        .collect::<Vec<_>>();
    summaries.sort_by(|left, right| right.count.cmp(&left.count).then(left.key.cmp(&right.key)));
    if top > 0 && summaries.len() > top {
        summaries.truncate(top);
    }
    summaries
}

fn group_keys(entry: &UserLogEntry, group_by: LogGroupBy) -> Vec<String> {
    match group_by {
        LogGroupBy::Category => vec![entry.details.category.clone()],
        LogGroupBy::Type => vec![format!("{} {}", entry.details.id, entry.details.title)],
        LogGroupBy::Day => vec![format_timestamp_bucket(entry.timestamp, "%Y-%m-%d")],
        LogGroupBy::Hour => vec![format_timestamp_bucket(
            entry.timestamp,
            "%Y-%m-%dT%H:00:00Z",
        )],
        LogGroupBy::Target => vec![target_key(entry)],
        LogGroupBy::DataKey => keys_or_none(entry.data.keys()),
        LogGroupBy::ParamKey => keys_or_none(entry.params.keys()),
    }
}

fn keys_or_none<'a>(keys: impl Iterator<Item = &'a String>) -> Vec<String> {
    let values = keys.cloned().collect::<Vec<_>>();
    if values.is_empty() {
        vec!["<none>".to_string()]
    } else {
        values
    }
}

fn target_key(entry: &UserLogEntry) -> String {
    [
        "target",
        "target_id",
        "user",
        "user_id",
        "player",
        "player_id",
    ]
    .iter()
    .find_map(|key| {
        entry
            .params
            .get(*key)
            .or_else(|| entry.data.get(*key))
            .and_then(value_to_string)
            .map(|value| format!("{key}:{value}"))
    })
    .unwrap_or_else(|| "<none>".to_string())
}

fn format_timestamp_bucket(timestamp: i64, format: &str) -> String {
    Utc.timestamp_opt(timestamp, 0)
        .single()
        .map(|dt| dt.format(format).to_string())
        .unwrap_or_else(|| "<invalid-timestamp>".to_string())
}

fn summarize_shapes(entries: &[UserLogEntry]) -> Vec<ObservedLogShape> {
    let mut shapes = BTreeMap::<UserLogDetails, ShapeAccumulator>::new();
    for entry in entries {
        let shape = shapes.entry(entry.details.clone()).or_default();
        shape.details = Some(entry.details.clone());
        shape.count += 1;
        for (key, value) in &entry.data {
            shape.data_keys.insert(key.clone());
            shape
                .data_value_types
                .entry(key.clone())
                .or_default()
                .insert(value_type(value).to_string());
        }
        for (key, value) in &entry.params {
            shape.param_keys.insert(key.clone());
            shape
                .param_value_types
                .entry(key.clone())
                .or_default()
                .insert(value_type(value).to_string());
        }
    }

    let mut out = shapes
        .into_values()
        .filter_map(|shape| {
            Some(ObservedLogShape {
                details: shape.details?,
                count: shape.count,
                data_keys: shape.data_keys.into_iter().collect(),
                param_keys: shape.param_keys.into_iter().collect(),
                data_value_types: set_map_to_vec(shape.data_value_types),
                param_value_types: set_map_to_vec(shape.param_value_types),
            })
        })
        .collect::<Vec<_>>();
    out.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then(left.details.cmp(&right.details))
    });
    out
}

fn value_type(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(number) if number.is_i64() || number.is_u64() => "integer",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn set_map_to_vec(map: BTreeMap<String, BTreeSet<String>>) -> BTreeMap<String, Vec<String>> {
    map.into_iter()
        .map(|(key, values)| (key, values.into_iter().collect()))
        .collect()
}

fn parse_catalog_items(json: &Value, key: &str) -> Result<Vec<LogTypeInfo>, AppError> {
    let Some(value) = json.get(key) else {
        return Err(AppError::Json(format!(
            "catalog response is missing top-level '{key}'"
        )));
    };
    match value {
        Value::Array(items) => items
            .iter()
            .map(|item| {
                let id = item
                    .get("id")
                    .and_then(value_to_string)
                    .ok_or_else(|| AppError::Json(format!("{key} item is missing id")))?;
                let title = item
                    .get("title")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                Ok(LogTypeInfo { id, title })
            })
            .collect(),
        Value::Object(map) => Ok(map
            .iter()
            .map(|(id, title)| LogTypeInfo {
                id: id.clone(),
                title: title
                    .as_str()
                    .map(str::to_string)
                    .unwrap_or_else(|| title.to_string()),
            })
            .collect()),
        _ => Err(AppError::Json(format!(
            "catalog response '{key}' must be an array or object"
        ))),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LogContinuation {
    direction: &'static str,
    url: String,
}

fn user_logs_continuation_url(value: &Value) -> Option<LogContinuation> {
    // Torn's /user/log returns the newest page first for normal bounded windows.
    // The older page inside the requested from/to window is exposed as `prev`,
    // while `next` points back toward newer entries. Prefer `prev` so a
    // `--since ... --to ...` log scan walks back until the lower bound is done.
    pagination_link(value, "prev").or_else(|| pagination_link(value, "next"))
}

fn pagination_link(value: &Value, direction: &'static str) -> Option<LogContinuation> {
    value
        .pointer(&format!("/_metadata/links/{direction}"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|link| !link.is_empty())
        .map(|link| LogContinuation {
            direction,
            url: link.to_string(),
        })
}

fn research_sources() -> Vec<String> {
    vec![
        OFFICIAL_OPENAPI_URL.to_string(),
        PLAYGROUND_USER_LOG_URL.to_string(),
        PLAYGROUND_LOGTYPES_URL.to_string(),
        PLAYGROUND_LOGCATEGORIES_URL.to_string(),
    ]
}

pub fn render_log_entries(entries: &[UserLogEntry], mode: OutputMode) -> Result<String, AppError> {
    match mode {
        OutputMode::JsonPretty | OutputMode::Raw => {
            serde_json::to_string_pretty(entries).map_err(AppError::from)
        }
        OutputMode::JsonCompact => serde_json::to_string(entries).map_err(AppError::from),
        OutputMode::Csv => Ok(render_entries_csv(entries)),
        OutputMode::Auto | OutputMode::Table => Ok(render_entries_table(entries)),
    }
}

pub fn render_analysis(analysis: &LogAnalysis, mode: OutputMode) -> Result<String, AppError> {
    match mode {
        OutputMode::JsonPretty | OutputMode::Raw => {
            serde_json::to_string_pretty(analysis).map_err(AppError::from)
        }
        OutputMode::JsonCompact => serde_json::to_string(analysis).map_err(AppError::from),
        OutputMode::Csv => Ok(render_groups_csv(&analysis.groups)),
        OutputMode::Auto | OutputMode::Table => Ok(render_analysis_table(analysis)),
    }
}

pub fn render_catalog(catalog: &LogCatalog, mode: OutputMode) -> Result<String, AppError> {
    match mode {
        OutputMode::JsonPretty | OutputMode::Raw => {
            serde_json::to_string_pretty(catalog).map_err(AppError::from)
        }
        OutputMode::JsonCompact => serde_json::to_string(catalog).map_err(AppError::from),
        OutputMode::Csv => Ok(render_catalog_csv(catalog)),
        OutputMode::Auto | OutputMode::Table => Ok(render_catalog_table(catalog)),
    }
}

pub fn render_catalog_types(catalog: &LogCatalog, mode: OutputMode) -> Result<String, AppError> {
    match mode {
        OutputMode::JsonPretty | OutputMode::Raw => {
            serde_json::to_string_pretty(&catalog.all_types).map_err(AppError::from)
        }
        OutputMode::JsonCompact => {
            serde_json::to_string(&catalog.all_types).map_err(AppError::from)
        }
        OutputMode::Csv => Ok(render_type_list_csv(&catalog.all_types)),
        OutputMode::Auto | OutputMode::Table => Ok(render_type_list_table(&catalog.all_types)),
    }
}

pub fn render_catalog_categories(
    catalog: &LogCatalog,
    mode: OutputMode,
) -> Result<String, AppError> {
    let categories = catalog
        .categories
        .iter()
        .map(|category| LogTypeInfo {
            id: category.id.clone(),
            title: category.title.clone(),
        })
        .collect::<Vec<_>>();
    match mode {
        OutputMode::JsonPretty | OutputMode::Raw => {
            serde_json::to_string_pretty(&categories).map_err(AppError::from)
        }
        OutputMode::JsonCompact => serde_json::to_string(&categories).map_err(AppError::from),
        OutputMode::Csv => Ok(render_type_list_csv(&categories)),
        OutputMode::Auto | OutputMode::Table => Ok(render_type_list_table(&categories)),
    }
}

fn render_entries_table(entries: &[UserLogEntry]) -> String {
    let mut lines =
        vec!["timestamp\tlog_type\tcategory\ttitle\tdata_keys\tparam_keys\tid".to_string()];
    lines.extend(entries.iter().map(|entry| {
        format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}",
            entry.timestamp,
            entry.details.id,
            entry.details.category,
            entry.details.title,
            entry.data.keys().cloned().collect::<Vec<_>>().join(","),
            entry.params.keys().cloned().collect::<Vec<_>>().join(","),
            entry.id
        )
    }));
    lines.join("\n")
}

fn render_entries_csv(entries: &[UserLogEntry]) -> String {
    let mut lines = vec!["timestamp,log_type,category,title,data_keys,param_keys,id".to_string()];
    lines.extend(entries.iter().map(|entry| {
        [
            entry.timestamp.to_string(),
            entry.details.id.clone(),
            entry.details.category.clone(),
            entry.details.title.clone(),
            entry.data.keys().cloned().collect::<Vec<_>>().join("|"),
            entry.params.keys().cloned().collect::<Vec<_>>().join("|"),
            entry.id.clone(),
        ]
        .into_iter()
        .map(csv_escape)
        .collect::<Vec<_>>()
        .join(",")
    }));
    lines.join("\n")
}

fn render_analysis_table(analysis: &LogAnalysis) -> String {
    let mut lines = vec![
        format!("total_logs\t{}", analysis.total_logs),
        format!("filtered_logs\t{}", analysis.filtered_logs),
        format!("pages_fetched\t{}", analysis.pagination.pages_fetched),
        format!("max_pages\t{}", analysis.pagination.max_pages),
        format!("truncated\t{}", analysis.pagination.truncated),
        format!(
            "continuation_direction\t{}",
            analysis
                .pagination
                .continuation_direction
                .as_deref()
                .unwrap_or("")
        ),
        format!("group_by\t{:?}", analysis.group_by),
        "".to_string(),
        "GROUPS".to_string(),
        "count\tkey\tfirst\tlast\tcategories\tlog_types\tdata_keys\tparam_keys".to_string(),
    ];
    lines.extend(analysis.groups.iter().map(|group| {
        format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            group.count,
            group.key,
            group
                .first_timestamp
                .map(|value| value.to_string())
                .unwrap_or_default(),
            group
                .last_timestamp
                .map(|value| value.to_string())
                .unwrap_or_default(),
            group.categories.join("|"),
            group.log_types.join("|"),
            group.data_keys.join("|"),
            group.param_keys.join("|")
        )
    }));
    lines.push("".to_string());
    lines.push("OBSERVED LOG FIELD SHAPES".to_string());
    lines.push("count\tlog_type\tcategory\ttitle\tdata_keys\tparam_keys".to_string());
    lines.extend(analysis.observed_shapes.iter().map(|shape| {
        format!(
            "{}\t{}\t{}\t{}\t{}\t{}",
            shape.count,
            shape.details.id,
            shape.details.category,
            shape.details.title,
            shape.data_keys.join("|"),
            shape.param_keys.join("|")
        )
    }));
    lines.join("\n")
}

fn render_groups_csv(groups: &[LogGroupSummary]) -> String {
    let mut lines = vec![
        "count,key,first_timestamp,last_timestamp,categories,log_types,titles,data_keys,param_keys,example_ids"
            .to_string(),
    ];
    lines.extend(groups.iter().map(|group| {
        [
            group.count.to_string(),
            group.key.clone(),
            group
                .first_timestamp
                .map(|value| value.to_string())
                .unwrap_or_default(),
            group
                .last_timestamp
                .map(|value| value.to_string())
                .unwrap_or_default(),
            group.categories.join("|"),
            group.log_types.join("|"),
            group.titles.join("|"),
            group.data_keys.join("|"),
            group.param_keys.join("|"),
            group.example_ids.join("|"),
        ]
        .into_iter()
        .map(csv_escape)
        .collect::<Vec<_>>()
        .join(",")
    }));
    lines.join("\n")
}

fn render_type_list_table(items: &[LogTypeInfo]) -> String {
    let mut lines = vec!["id\ttitle".to_string()];
    lines.extend(
        items
            .iter()
            .map(|item| format!("{}\t{}", item.id, item.title)),
    );
    lines.join("\n")
}

fn render_type_list_csv(items: &[LogTypeInfo]) -> String {
    let mut lines = vec!["id,title".to_string()];
    lines.extend(items.iter().map(|item| {
        [item.id.clone(), item.title.clone()]
            .into_iter()
            .map(csv_escape)
            .collect::<Vec<_>>()
            .join(",")
    }));
    lines.join("\n")
}

fn render_catalog_table(catalog: &LogCatalog) -> String {
    let mut lines = vec!["category_id\tcategory\tlog_type_id\tlog_type".to_string()];
    for category in &catalog.categories {
        if category.log_types.is_empty() {
            lines.push(format!("{}\t{}\t\t", category.id, category.title));
        } else {
            lines.extend(category.log_types.iter().map(|log_type| {
                format!(
                    "{}\t{}\t{}\t{}",
                    category.id, category.title, log_type.id, log_type.title
                )
            }));
        }
    }
    if !catalog.category_errors.is_empty() {
        lines.push("".to_string());
        lines.push("CATEGORY ERRORS".to_string());
        lines.extend(catalog.category_errors.iter().cloned());
    }
    lines.join("\n")
}

fn render_catalog_csv(catalog: &LogCatalog) -> String {
    let mut lines = vec!["category_id,category,log_type_id,log_type".to_string()];
    for category in &catalog.categories {
        if category.log_types.is_empty() {
            lines.push(
                [
                    category.id.clone(),
                    category.title.clone(),
                    String::new(),
                    String::new(),
                ]
                .into_iter()
                .map(csv_escape)
                .collect::<Vec<_>>()
                .join(","),
            );
        } else {
            lines.extend(category.log_types.iter().map(|log_type| {
                [
                    category.id.clone(),
                    category.title.clone(),
                    log_type.id.clone(),
                    log_type.title.clone(),
                ]
                .into_iter()
                .map(csv_escape)
                .collect::<Vec<_>>()
                .join(",")
            }));
        }
    }
    lines.join("\n")
}

fn csv_escape(value: String) -> String {
    if value
        .chars()
        .any(|ch| matches!(ch, ',' | '"' | '\n' | '\r'))
    {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn sample_entry() -> UserLogEntry {
        UserLogEntry {
            id: "abc".to_string(),
            timestamp: 1_700_000_000,
            details: UserLogDetails {
                id: "105".to_string(),
                title: "Item used".to_string(),
                category: "Items".to_string(),
            },
            data: BTreeMap::from([("item".to_string(), json!("Xanax"))]),
            params: BTreeMap::from([("target_id".to_string(), json!(123))]),
        }
    }

    #[test]
    fn parses_v2_array_user_logs() {
        let logs = parse_user_logs(&json!({
            "log": [{
                "id": "abc",
                "timestamp": 1700000000,
                "details": {"id": 105, "title": "Item used", "category": "Items"},
                "data": {"item": "Xanax"},
                "params": {"target_id": 123}
            }]
        }))
        .unwrap();
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].details.id, "105");
        assert_eq!(logs[0].data["item"], json!("Xanax"));
    }

    #[test]
    fn parses_legacy_object_user_logs() {
        let logs = parse_user_logs(&json!({
            "log": {
                "abc": {
                    "timestamp": 1700000000,
                    "log": 105,
                    "title": "Item used",
                    "category": "Items",
                    "data": {"item": "Xanax"},
                    "params": {}
                }
            }
        }))
        .unwrap();
        assert_eq!(logs[0].id, "abc");
        assert_eq!(logs[0].details.category, "Items");
    }

    #[test]
    fn summarizes_groups_and_observed_fields() {
        let entry = sample_entry();
        let groups = summarize_groups(std::slice::from_ref(&entry), LogGroupBy::Category, 10);
        assert_eq!(groups[0].key, "Items");
        assert_eq!(groups[0].data_keys, vec!["item".to_string()]);

        let shapes = summarize_shapes(&[entry]);
        assert_eq!(shapes[0].details.id, "105");
        assert_eq!(
            shapes[0].data_value_types["item"],
            vec!["string".to_string()]
        );
    }

    #[test]
    fn parses_timestamp_variants() {
        let reference = Utc.timestamp_opt(1_700_000_000, 0).single().unwrap();
        assert_eq!(
            parse_timestamp_arg("1700000000", reference).unwrap(),
            1_700_000_000
        );
        assert_eq!(parse_timestamp_arg("1h", reference).unwrap(), 1_699_996_400);
        assert_eq!(
            parse_timestamp_arg("2023-11-14", reference).unwrap(),
            1_699_920_000
        );
    }

    #[test]
    fn bounded_log_queries_auto_page_by_default() {
        let bounded = LogsFetchSpec {
            since: Some("3d".to_string()),
            to: Some("now".to_string()),
            log_ids: Vec::new(),
            category: None,
            target: None,
            limit: 100,
            max_pages: None,
            extra_params: Vec::new(),
        };
        assert_eq!(
            resolved_log_max_pages(&bounded).unwrap(),
            DEFAULT_BOUNDED_LOG_MAX_PAGES
        );

        let unbounded = LogsFetchSpec {
            since: None,
            to: None,
            max_pages: None,
            ..bounded.clone()
        };
        assert_eq!(
            resolved_log_max_pages(&unbounded).unwrap(),
            DEFAULT_UNBOUNDED_LOG_MAX_PAGES
        );

        let upper_bound_only = LogsFetchSpec {
            since: None,
            to: Some("now".to_string()),
            max_pages: None,
            ..bounded
        };
        assert_eq!(
            resolved_log_max_pages(&upper_bound_only).unwrap(),
            DEFAULT_UNBOUNDED_LOG_MAX_PAGES
        );
    }

    #[test]
    fn user_log_continuation_prefers_prev_over_next() {
        let continuation = user_logs_continuation_url(&json!({
            "_metadata": {
                "links": {
                    "next": "https://api.torn.com/v2/user/log?from=200",
                    "prev": "https://api.torn.com/v2/user/log?to=100"
                }
            }
        }))
        .unwrap();

        assert_eq!(continuation.direction, "prev");
        assert_eq!(continuation.url, "https://api.torn.com/v2/user/log?to=100");
    }

    #[test]
    fn user_log_continuation_falls_back_to_next() {
        let continuation = user_logs_continuation_url(&json!({
            "_metadata": {
                "links": {
                    "next": "https://api.torn.com/v2/user/log?from=200",
                    "prev": null
                }
            }
        }))
        .unwrap();

        assert_eq!(continuation.direction, "next");
        assert_eq!(
            continuation.url,
            "https://api.torn.com/v2/user/log?from=200"
        );
    }

    #[test]
    fn parses_catalog_arrays_and_maps() {
        let array = parse_catalog_items(
            &json!({"logtypes": [{"id": 1, "title": "One"}]}),
            "logtypes",
        )
        .unwrap();
        assert_eq!(array[0].id, "1");
        let map = parse_catalog_items(&json!({"logtypes": {"2": "Two"}}), "logtypes").unwrap();
        assert_eq!(map[0].title, "Two");
    }
}
