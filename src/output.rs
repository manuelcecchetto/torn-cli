use chrono::{DateTime, Utc};
use crossterm::style::{Color, Stylize, style};
use serde_json::{Map, Value};

use crate::{
    client::ApiResponse,
    error::AppError,
    request::{ApiRequest, Service},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    Auto,
    JsonCompact,
    JsonPretty,
    Raw,
    Table,
    Csv,
}

pub fn render_response(response: &ApiResponse, mode: OutputMode) -> Result<String, AppError> {
    render_response_with_color(response, mode, false)
}

pub fn render_response_colored(
    response: &ApiResponse,
    mode: OutputMode,
) -> Result<String, AppError> {
    render_response_with_color(response, mode, true)
}

pub fn render_response_for_request(
    request: &ApiRequest,
    response: &ApiResponse,
    mode: OutputMode,
) -> Result<String, AppError> {
    render_response_for_request_with_color(request, response, mode, false)
}

pub fn render_response_for_request_colored(
    request: &ApiRequest,
    response: &ApiResponse,
    mode: OutputMode,
) -> Result<String, AppError> {
    render_response_for_request_with_color(request, response, mode, true)
}

fn render_response_for_request_with_color(
    request: &ApiRequest,
    response: &ApiResponse,
    mode: OutputMode,
    color: bool,
) -> Result<String, AppError> {
    match mode {
        OutputMode::JsonPretty => render_schema_pretty(request, response, color),
        _ => render_response_with_color(response, mode, color),
    }
}

fn render_response_with_color(
    response: &ApiResponse,
    mode: OutputMode,
    color: bool,
) -> Result<String, AppError> {
    match mode {
        OutputMode::Raw => Ok(response.body_text.clone()),
        OutputMode::JsonPretty => render_json(response, true, color),
        OutputMode::JsonCompact | OutputMode::Auto => render_json(response, false, false),
        OutputMode::Table => render_table(response, color),
        OutputMode::Csv => render_csv(response),
    }
}

fn render_schema_pretty(
    request: &ApiRequest,
    response: &ApiResponse,
    color: bool,
) -> Result<String, AppError> {
    let Some(json) = &response.body_json else {
        return Ok(response.body_text.clone());
    };
    Ok(
        pretty_value_for_request(request, json, color).unwrap_or_else(|| {
            render_json(response, true, color).unwrap_or_else(|_| response.body_text.clone())
        }),
    )
}

fn pretty_value_for_request(request: &ApiRequest, value: &Value, color: bool) -> Option<String> {
    match request.service {
        Service::Ffscouter => pretty_ffscouter_value(&request.path, value, color),
        Service::Torn => pretty_torn_value(&request.path, value, color),
    }
    .or_else(|| pretty_wrapped_value(value, color))
}

fn pretty_torn_value(path: &str, value: &Value, color: bool) -> Option<String> {
    let object = value.as_object()?;
    if let Some(Value::Object(profile)) = object.get("profile") {
        return Some(render_profile(profile, color));
    }
    if let Some(Value::Object(basic)) = object.get("basic") {
        return Some(render_named_record("faction", basic, color));
    }
    if let Some(Value::Array(members)) = object.get("members") {
        return Some(render_object_array(
            "members",
            members,
            color,
            &MEMBER_COLUMNS,
        ));
    }
    if let Some(Value::Object(bars)) = object.get("bars") {
        return Some(render_bars(bars, color));
    }
    if let Some(Value::Object(cooldowns)) = object.get("cooldowns") {
        return Some(render_simple_duration_map("cooldowns", cooldowns, color));
    }
    if let Some(Value::Object(travel)) = object.get("travel") {
        return Some(render_named_record("travel", travel, color));
    }
    if let Some(Value::Object(info)) = object.get("info") {
        return Some(render_named_record("key info", info, color));
    }
    if let Some(timestamp) = object.get("timestamp") {
        return Some(render_timestamp_value("timestamp", timestamp, color));
    }

    for key in [
        "attacks",
        "revives",
        "events",
        "messages",
        "notifications",
        "news",
        "log",
        "logs",
        "items",
        "inventory",
        "properties",
        "stocks",
        "trades",
        "reports",
        "bounties",
        "races",
        "applications",
        "employees",
    ] {
        if let Some(Value::Array(items)) = object.get(key) {
            return Some(render_object_array(key, items, color, columns_for_key(key)));
        }
    }

    let path_hint = path.trim_matches('/').replace('/', " ");
    pretty_wrapped_value_with_label(value, path_hint.as_str(), color)
}

fn pretty_ffscouter_value(path: &str, value: &Value, color: bool) -> Option<String> {
    match path {
        "/get-stats" => value
            .as_array()
            .map(|items| render_object_array("ffscouter stats", items, color, &FF_STATS_COLUMNS)),
        "/get-targets" => value
            .as_object()
            .and_then(|object| object.get("targets"))
            .and_then(Value::as_array)
            .map(|items| {
                render_object_array("ffscouter targets", items, color, &FF_TARGET_COLUMNS)
            }),
        "/check-key" => value
            .as_object()
            .map(|object| render_named_record("ffscouter key", object, color)),
        "/losses/orders/quote" => value
            .as_object()
            .map(|object| render_named_record("losses quote", object, color)),
        "/activity/player" | "/activity/faction" => {
            pretty_wrapped_value_with_label(value, "activity", color)
        }
        "/get-stats-history" => pretty_wrapped_value_with_label(value, "stats history", color),
        "/player-flights" => pretty_wrapped_value_with_label(value, "flights", color),
        _ => pretty_wrapped_value(value, color),
    }
}

fn pretty_wrapped_value(value: &Value, color: bool) -> Option<String> {
    pretty_wrapped_value_with_label(value, "response", color)
}

fn pretty_wrapped_value_with_label(
    value: &Value,
    fallback_label: &str,
    color: bool,
) -> Option<String> {
    match value {
        Value::Object(object) => {
            let keys = object
                .keys()
                .filter(|key| key.as_str() != "_metadata")
                .cloned()
                .collect::<Vec<_>>();
            if keys.len() == 1 {
                let key = &keys[0];
                return pretty_section(key, object.get(key)?, color);
            }
            Some(render_named_record(fallback_label, object, color))
        }
        Value::Array(items) => Some(render_object_array(
            fallback_label,
            items,
            color,
            &DEFAULT_COLUMNS,
        )),
        _ => Some(value_to_cell(value, color)),
    }
}

fn pretty_section(label: &str, value: &Value, color: bool) -> Option<String> {
    match value {
        Value::Array(items) => Some(render_object_array(
            label,
            items,
            color,
            columns_for_key(label),
        )),
        Value::Object(object) if is_profile_object(object) => Some(render_profile(object, color)),
        Value::Object(object) => Some(render_named_record(label, object, color)),
        other => Some(format!(
            "{}\n{}",
            heading(label, color),
            value_to_cell(other, color)
        )),
    }
}

fn is_profile_object(object: &Map<String, Value>) -> bool {
    object.contains_key("name") && object.contains_key("status")
}

fn render_profile(profile: &Map<String, Value>, color: bool) -> String {
    let mut lines = Vec::new();
    let name = object_string(profile, "name").unwrap_or_else(|| "unknown".to_string());
    let id = object_cell(profile, "id", false);
    let level = object_cell(profile, "level", false);
    let title = object_string(profile, "title").or_else(|| object_string(profile, "rank"));
    let mut headline = if id.is_empty() {
        name.clone()
    } else {
        format!("{name} [{id}]")
    };
    if !level.is_empty() {
        headline.push_str(&format!("  level {level}"));
    }
    if let Some(title) = title.filter(|value| !value.is_empty()) {
        headline.push_str(&format!("  {title}"));
    }
    lines.push(heading(&headline, color));

    if let Some(status) = profile.get("status") {
        lines.push(label_value("status", status, color));
    }
    if let Some(last_action) = profile.get("last_action") {
        lines.push(label_value("last_action", last_action, color));
    }
    if let Some(life) = profile.get("life") {
        lines.push(label_value("life", life, color));
    }
    for key in [
        "property",
        "faction_id",
        "role",
        "gender",
        "age",
        "karma",
        "friends",
        "enemies",
        "forum_posts",
    ] {
        if profile.contains_key(key) {
            let line = label_value(key, &profile[key], color);
            if !line.ends_with('\t') && !line.ends_with("\t") {
                lines.push(line);
            }
        }
    }
    lines.join("\n")
}

fn render_named_record(label: &str, object: &Map<String, Value>, color: bool) -> String {
    let mut lines = vec![heading(label, color)];
    for key in preferred_object_keys(object) {
        if let Some(value) = object.get(key) {
            lines.push(label_value(key, value, color));
        }
    }
    lines.join("\n")
}

fn render_bars(bars: &Map<String, Value>, color: bool) -> String {
    let mut rows = Vec::new();
    for key in ["energy", "nerve", "happy", "life"] {
        if let Some(Value::Object(bar)) = bars.get(key) {
            let current = object_cell(bar, "current", false);
            let maximum = object_cell(bar, "maximum", false);
            rows.push(vec![key.to_string(), format!("{current}/{maximum}")]);
        }
    }
    if let Some(chain) = bars.get("chain") {
        rows.push(vec!["chain".to_string(), value_to_cell(chain, false)]);
    }
    format!(
        "{}\n{}",
        heading("bars", color),
        render_rows(&["bar", "value"], rows, color)
    )
}

fn render_simple_duration_map(label: &str, object: &Map<String, Value>, color: bool) -> String {
    let mut rows = Vec::new();
    for (key, value) in object {
        let raw = value_to_cell(value, false);
        let formatted = value
            .as_i64()
            .filter(|seconds| *seconds > 0)
            .map(format_seconds)
            .unwrap_or(raw);
        rows.push(vec![key.clone(), formatted]);
    }
    format!(
        "{}\n{}",
        heading(label, color),
        render_rows(&["name", "time"], rows, color)
    )
}

fn render_timestamp_value(label: &str, value: &Value, color: bool) -> String {
    format!(
        "{}\n{}",
        heading(label, color),
        value_to_cell_for_key(value, "timestamp", color)
    )
}

fn render_object_array(label: &str, items: &[Value], color: bool, preferred: &[&str]) -> String {
    if items.is_empty() {
        return format!("{}\n(empty)", heading(label, color));
    }
    let columns = choose_columns(items, preferred);
    if columns.is_empty() {
        let rows = items
            .iter()
            .enumerate()
            .map(|(idx, value)| vec![idx.to_string(), value_to_cell(value, false)])
            .collect::<Vec<_>>();
        return format!(
            "{} ({})\n{}",
            heading(label, color),
            items.len(),
            render_rows(&["#", "value"], rows, color)
        );
    }
    let rows = items
        .iter()
        .filter_map(Value::as_object)
        .map(|object| {
            columns
                .iter()
                .map(|column| {
                    object
                        .get(*column)
                        .map(|value| value_to_cell_for_key(value, column, false))
                        .unwrap_or_default()
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    format!(
        "{} ({})\n{}",
        heading(label, color),
        items.len(),
        render_rows(&columns, rows, color)
    )
}

fn choose_columns<'a>(items: &'a [Value], preferred: &'a [&'a str]) -> Vec<&'a str> {
    let object_keys = items
        .iter()
        .filter_map(Value::as_object)
        .flat_map(|object| object.keys().map(String::as_str))
        .collect::<Vec<_>>();
    let mut columns = preferred
        .iter()
        .copied()
        .filter(|column| object_keys.iter().any(|key| key == column))
        .collect::<Vec<_>>();
    for key in object_keys {
        if columns.len() >= 8 {
            break;
        }
        if !columns.contains(&key) && !key.starts_with('_') {
            columns.push(key);
        }
    }
    columns
}

fn render_rows(headers: &[&str], rows: Vec<Vec<String>>, color: bool) -> String {
    let mut widths = headers
        .iter()
        .map(|header| header.len())
        .collect::<Vec<_>>();
    for row in &rows {
        for (idx, cell) in row.iter().enumerate() {
            if let Some(width) = widths.get_mut(idx) {
                *width = (*width).max(cell.chars().count().min(80));
            }
        }
    }
    let header = headers
        .iter()
        .enumerate()
        .map(|(idx, header)| pad_cell(header, widths[idx], colorize_key_for_table(header, color)))
        .collect::<Vec<_>>()
        .join("  ");
    let mut lines = vec![header];
    lines.extend(rows.into_iter().map(|row| {
        row.into_iter()
            .enumerate()
            .map(|(idx, cell)| {
                let header = headers.get(idx).copied().unwrap_or_default();
                let colored = if color {
                    colorize_value_for_key(&cell, header)
                } else {
                    cell.clone()
                };
                pad_cell(
                    &cell,
                    widths.get(idx).copied().unwrap_or(cell.len()),
                    colored,
                )
            })
            .collect::<Vec<_>>()
            .join("  ")
    }));
    lines.join("\n")
}

fn pad_cell(raw: &str, width: usize, rendered: String) -> String {
    let len = raw.chars().count().min(80);
    if len >= width {
        rendered
    } else {
        format!("{rendered}{}", " ".repeat(width - len))
    }
}

fn label_value(key: &str, value: &Value, color: bool) -> String {
    format!(
        "{}\t{}",
        colorize_key_for_table(key, color),
        value_to_cell_for_key(value, key, color)
    )
}

fn preferred_object_keys(object: &Map<String, Value>) -> Vec<&str> {
    let mut keys = [
        "id",
        "name",
        "level",
        "status",
        "last_action",
        "life",
        "access",
        "user",
        "selections",
        "state",
        "description",
        "details",
        "until",
        "destination",
        "method",
        "time_left",
        "arrival_at",
        "departed_at",
        "current",
        "maximum",
        "money_onhand",
        "points",
        "networth",
    ]
    .into_iter()
    .filter(|key| object.contains_key(*key))
    .collect::<Vec<_>>();
    for key in object.keys().map(String::as_str) {
        if !keys.contains(&key) && !key.starts_with('_') {
            keys.push(key);
        }
    }
    keys
}

fn object_string(object: &Map<String, Value>, key: &str) -> Option<String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn object_cell(object: &Map<String, Value>, key: &str, color: bool) -> String {
    object
        .get(key)
        .map(|value| value_to_cell_for_key(value, key, color))
        .unwrap_or_default()
}

const DEFAULT_COLUMNS: [&str; 24] = [
    "id",
    "player_id",
    "name",
    "level",
    "status",
    "last_action",
    "timestamp",
    "time",
    "started",
    "ended",
    "result",
    "attacker",
    "defender",
    "respect_gain",
    "quantity",
    "price",
    "value",
    "type",
    "title",
    "description",
    "state",
    "until",
    "source",
    "last_updated",
];
const MEMBER_COLUMNS: [&str; 9] = [
    "id",
    "name",
    "level",
    "position",
    "status",
    "last_action",
    "is_revivable",
    "is_on_wall",
    "has_early_discharge",
];
const ATTACK_COLUMNS: [&str; 9] = [
    "started",
    "ended",
    "attacker",
    "defender",
    "result",
    "respect_gain",
    "respect_loss",
    "chain",
    "is_stealthed",
];
const FF_STATS_COLUMNS: [&str; 7] = [
    "player_id",
    "fair_fight",
    "bs_estimate_human",
    "bs_estimate",
    "source",
    "last_updated",
    "premium_insights_available",
];
const FF_TARGET_COLUMNS: [&str; 8] = [
    "id",
    "player_id",
    "name",
    "level",
    "fair_fight",
    "bs_estimate_human",
    "status",
    "last_action",
];

fn columns_for_key(key: &str) -> &'static [&'static str] {
    match key {
        "members" => &MEMBER_COLUMNS,
        "attacks" => &ATTACK_COLUMNS,
        "revives" => &[
            "timestamp",
            "reviver",
            "target",
            "result",
            "chance",
            "success_chance",
            "fee",
        ],
        "events" | "messages" | "notifications" | "news" | "log" | "logs" => &[
            "timestamp",
            "time",
            "id",
            "type",
            "title",
            "event",
            "message",
            "text",
            "name",
        ],
        "items" | "inventory" | "itemmarket" => &[
            "id",
            "name",
            "type",
            "quantity",
            "price",
            "market_price",
            "value",
            "circulation",
        ],
        "properties" | "property" => &[
            "id",
            "name",
            "property_type",
            "happy",
            "upkeep",
            "market_price",
            "status",
        ],
        _ => &DEFAULT_COLUMNS,
    }
}

fn heading(label: &str, color: bool) -> String {
    if color {
        paint(label, Color::Green)
    } else {
        label.to_string()
    }
}

fn render_json(response: &ApiResponse, pretty: bool, color: bool) -> Result<String, AppError> {
    if let Some(json) = &response.body_json {
        if pretty {
            let rendered = serde_json::to_string_pretty(json).map_err(AppError::from)?;
            if color {
                Ok(colorize_pretty_json(&rendered))
            } else {
                Ok(rendered)
            }
        } else {
            serde_json::to_string(json).map_err(AppError::from)
        }
    } else {
        Ok(response.body_text.clone())
    }
}

fn render_table(response: &ApiResponse, color: bool) -> Result<String, AppError> {
    let Some(json) = &response.body_json else {
        return Err(AppError::Output(
            "table output requires a JSON response".to_string(),
        ));
    };

    match json {
        Value::Object(map) => {
            if let Some((key, payload)) = singleton_payload(map) {
                match payload {
                    Value::Object(inner) => return Ok(render_key_value_table(inner, color)),
                    Value::Array(items) => {
                        return Ok(render_object_array(key, items, color, columns_for_key(key)));
                    }
                    _ => {}
                }
            }
            Ok(render_key_value_table(map, color))
        }
        Value::Array(items) if items.iter().any(Value::is_object) => {
            Ok(render_object_array("items", items, color, &DEFAULT_COLUMNS))
        }
        Value::Array(items) => Ok(items
            .iter()
            .enumerate()
            .map(|(index, value)| {
                format!(
                    "{}\t{}",
                    colorize_key_for_table(&index.to_string(), color),
                    value_to_cell(value, color)
                )
            })
            .collect::<Vec<_>>()
            .join("\n")),
        other => Ok(value_to_cell(other, color)),
    }
}

fn singleton_payload(map: &Map<String, Value>) -> Option<(&str, &Value)> {
    let mut entries = map
        .iter()
        .filter(|(key, _)| key.as_str() != "_metadata")
        .collect::<Vec<_>>();
    if entries.len() == 1 {
        let (key, value) = entries.pop()?;
        Some((key.as_str(), value))
    } else {
        None
    }
}

fn render_key_value_table(map: &Map<String, Value>, color: bool) -> String {
    preferred_object_keys(map)
        .into_iter()
        .filter_map(|key| {
            map.get(key).map(|value| {
                format!(
                    "{}\t{}",
                    colorize_key_for_table(key, color),
                    value_to_cell_for_key(value, key, color)
                )
            })
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_csv(response: &ApiResponse) -> Result<String, AppError> {
    let Some(Value::Array(items)) = &response.body_json else {
        return Err(AppError::Output(
            "csv output currently requires a top-level JSON array".to_string(),
        ));
    };

    Ok(items
        .iter()
        .map(|value| value_to_cell(value, false))
        .collect::<Vec<_>>()
        .join("\n"))
}

fn value_to_cell_for_key(value: &Value, key: &str, color: bool) -> String {
    let cell = match value {
        Value::Number(number) if is_timestamp_key(key) => number
            .as_i64()
            .and_then(format_unix_timestamp)
            .unwrap_or_else(|| number.to_string()),
        Value::Number(number) if is_duration_key(key) => number
            .as_i64()
            .filter(|seconds| *seconds > 0)
            .map(format_seconds)
            .unwrap_or_else(|| number.to_string()),
        Value::Number(number) if is_money_key(key) => format_number_string(&number.to_string()),
        _ => value_to_cell(value, false),
    };
    if color {
        colorize_value_for_key(&cell, key)
    } else {
        cell
    }
}

fn value_to_cell(value: &Value, color: bool) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => value.clone(),
        Value::Array(_) => value.to_string(),
        Value::Object(map) => object_to_cell(map, color).unwrap_or_else(|| value.to_string()),
    }
}

fn object_to_cell(map: &Map<String, Value>, color: bool) -> Option<String> {
    status_object_to_cell(map, color)
        .or_else(|| last_action_object_to_cell(map, color))
        .or_else(|| bar_object_to_cell(map))
        .or_else(|| named_object_to_cell(map))
}

fn status_object_to_cell(map: &Map<String, Value>, color: bool) -> Option<String> {
    let statusish = ["description", "details", "state", "until", "color"]
        .iter()
        .any(|key| map.contains_key(*key));
    if !statusish {
        return None;
    }

    let mut parts = Vec::new();
    push_string_field(&mut parts, map, "state", color);
    push_string_field(&mut parts, map, "description", color);
    push_string_field(&mut parts, map, "details", color);
    if let Some(until) = map.get("until") {
        let until = match until {
            Value::Null => String::new(),
            Value::Number(number) => number
                .as_i64()
                .and_then(format_unix_timestamp)
                .unwrap_or_else(|| number.to_string()),
            Value::String(value) => value.clone(),
            other => other.to_string(),
        };
        if !until.is_empty() && until != "0" && until != "null" {
            let label = if color {
                paint("until", Color::DarkCyan)
            } else {
                "until".to_string()
            };
            parts.push(format!("{label} {until}"));
        }
    }

    parts.dedup();
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" | "))
    }
}

fn last_action_object_to_cell(map: &Map<String, Value>, color: bool) -> Option<String> {
    if !(map.contains_key("status")
        || map.contains_key("relative")
        || map.contains_key("timestamp"))
    {
        return None;
    }
    let mut parts = Vec::new();
    push_string_field(&mut parts, map, "status", color);
    push_string_field(&mut parts, map, "relative", false);
    if let Some(timestamp) = map.get("timestamp") {
        let timestamp = value_to_cell_for_key(timestamp, "timestamp", color);
        if !timestamp.is_empty() {
            parts.push(timestamp);
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" | "))
    }
}

fn bar_object_to_cell(map: &Map<String, Value>) -> Option<String> {
    let current = map.get("current")?;
    let maximum = map.get("maximum")?;
    Some(format!(
        "{}/{}",
        value_to_cell(current, false),
        value_to_cell(maximum, false)
    ))
}

fn named_object_to_cell(map: &Map<String, Value>) -> Option<String> {
    let name = map.get("name").and_then(Value::as_str)?;
    let id = map
        .get("id")
        .map(|value| value_to_cell(value, false))
        .unwrap_or_default();
    if id.is_empty() {
        Some(name.to_string())
    } else {
        Some(format!("{name} [{id}]"))
    }
}

fn push_string_field(parts: &mut Vec<String>, map: &Map<String, Value>, key: &str, color: bool) {
    let Some(Value::String(value)) = map.get(key) else {
        return;
    };
    if value.trim().is_empty() {
        return;
    }
    parts.push(if color {
        colorize_status_text(value)
    } else {
        value.clone()
    });
}

fn format_seconds(seconds: i64) -> String {
    if seconds < 60 {
        return format!("{seconds}s");
    }
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;
    if hours > 0 {
        format!("{hours}h {minutes}m")
    } else if minutes > 0 {
        format!("{minutes}m {secs}s")
    } else {
        format!("{seconds}s")
    }
}

fn format_unix_timestamp(value: i64) -> Option<String> {
    if !(1_000_000_000..=4_102_444_800).contains(&value) {
        return None;
    }
    DateTime::<Utc>::from_timestamp(value, 0)
        .map(|timestamp| timestamp.format("%Y-%m-%d %H:%M:%S UTC").to_string())
}

fn colorize_pretty_json(input: &str) -> String {
    input
        .lines()
        .map(colorize_pretty_json_line)
        .collect::<Vec<_>>()
        .join("\n")
}

fn colorize_pretty_json_line(line: &str) -> String {
    let indent_len = line.len() - line.trim_start().len();
    let (indent, trimmed) = line.split_at(indent_len);
    let Some(rest) = trimmed.strip_prefix('"') else {
        return format!("{indent}{}", paint(trimmed, Color::DarkGrey));
    };
    let Some(key_end) = rest.find("\":") else {
        return format!("{indent}{}", paint(trimmed, Color::Green));
    };
    let key = &rest[..key_end];
    let after_key = &rest[key_end + 2..];
    let key_text = format!("\"{key}\"");
    let key_color = color_for_key(key);
    format!(
        "{indent}{}:{}",
        paint(&key_text, key_color),
        colorize_json_value_fragment(after_key, key)
    )
}

fn colorize_json_value_fragment(fragment: &str, key: &str) -> String {
    let leading_len = fragment.len() - fragment.trim_start().len();
    let (leading, rest) = fragment.split_at(leading_len);
    if rest.is_empty() {
        return leading.to_string();
    }

    let (value, suffix) = split_json_suffix(rest);
    let color = if is_time_key(key) {
        Color::DarkCyan
    } else if is_status_key(key) {
        color_for_status_text(value.trim_matches('"'))
    } else if rest.starts_with('"') {
        Color::Green
    } else if rest.starts_with(|ch: char| ch.is_ascii_digit() || ch == '-') {
        Color::Yellow
    } else if rest.starts_with("true") || rest.starts_with("false") {
        Color::Magenta
    } else {
        Color::DarkGrey
    };
    format!("{leading}{}{}", paint(value, color), suffix)
}

fn split_json_suffix(rest: &str) -> (&str, &str) {
    let trimmed = rest.trim_end();
    if let Some(value) = trimmed.strip_suffix(',') {
        let suffix_start = value.len();
        (&rest[..suffix_start], &rest[suffix_start..])
    } else {
        (rest, "")
    }
}

fn colorize_key_for_table(key: &str, color: bool) -> String {
    if !color {
        return key.to_string();
    }
    paint(key, color_for_key(key))
}

fn colorize_value_for_key(value: &str, key: &str) -> String {
    if is_status_key(key) {
        colorize_status_text(value)
    } else if is_time_key(key) {
        paint(value, Color::DarkCyan)
    } else if key.eq_ignore_ascii_case("name") {
        paint(value, Color::Green)
    } else {
        value.to_string()
    }
}

fn colorize_status_text(value: &str) -> String {
    paint(value, color_for_status_text(value))
}

fn color_for_key(key: &str) -> Color {
    if key.eq_ignore_ascii_case("name") {
        Color::Green
    } else if is_status_key(key) {
        Color::Red
    } else if is_time_key(key) {
        Color::DarkCyan
    } else {
        Color::Cyan
    }
}

fn is_status_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    matches!(
        key.as_str(),
        "status" | "state" | "description" | "details" | "life" | "error"
    )
}

fn is_time_key(key: &str) -> bool {
    is_timestamp_key(key) || is_duration_key(key)
}

fn is_timestamp_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    matches!(
        key.as_str(),
        "time"
            | "timestamp"
            | "until"
            | "started"
            | "ended"
            | "last_updated"
            | "signed_up"
            | "departed_at"
            | "arrival_at"
    ) || key.ends_with("_at")
        || key.ends_with("_time")
}

fn is_duration_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    key == "time_left" || key == "duration" || key.ends_with("_cooldown")
}

fn is_money_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    key.contains("money")
        || key.contains("cash")
        || key.contains("price")
        || key.contains("cost")
        || key.contains("value")
        || key.contains("networth")
        || key == "fee"
}

fn format_number_string(input: &str) -> String {
    let Some((integer, fractional)) = input.split_once('.') else {
        return format_integer_with_commas(input);
    };
    format!("{}.{}", format_integer_with_commas(integer), fractional)
}

fn format_integer_with_commas(input: &str) -> String {
    let (sign, digits) = input
        .strip_prefix('-')
        .map_or(("", input), |rest| ("-", rest));
    if digits.len() <= 3 || !digits.chars().all(|ch| ch.is_ascii_digit()) {
        return input.to_string();
    }
    let mut out = String::new();
    for (idx, ch) in digits.chars().rev().enumerate() {
        if idx > 0 && idx % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    let grouped = out.chars().rev().collect::<String>();
    format!("{sign}{grouped}")
}

fn color_for_status_text(value: &str) -> Color {
    let lower = value.to_ascii_lowercase();
    if lower.contains("hospital") || lower.contains("jail") || lower.contains("federal") {
        Color::Red
    } else if lower.contains("okay") || lower.contains("ok") || lower.contains("online") {
        Color::Green
    } else if lower.contains("travel") || lower.contains("abroad") || lower.contains("away") {
        Color::Yellow
    } else if lower.contains("offline") || lower.contains("inactive") {
        Color::DarkGrey
    } else {
        Color::White
    }
}

fn paint(value: impl ToString, color: Color) -> String {
    format!("{}", style(value.to_string()).with(color))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::request::{ApiRequest, Service};

    fn response(body_json: Value) -> ApiResponse {
        ApiResponse {
            service: Service::Torn,
            status: 200,
            body_text: body_json.to_string(),
            body_json: Some(body_json),
            from_cache: false,
            elapsed_ms: 1,
        }
    }

    #[test]
    fn table_renders_status_objects_as_status_cells() {
        let response = response(serde_json::json!({
            "name": "Example",
            "status": {
                "state": "Hospital",
                "description": "Hospital",
                "details": "Recovering",
                "until": 1893456000
            }
        }));
        let rendered = render_response(&response, OutputMode::Table).unwrap();
        assert!(rendered.contains("status\tHospital | Recovering | until 2030-01-01 00:00:00 UTC"));
    }

    #[test]
    fn request_pretty_summarizes_user_profile_status() {
        let response = response(serde_json::json!({
            "profile": {
                "id": 123,
                "name": "Example",
                "level": 42,
                "status": {
                    "state": "Hospital",
                    "description": "Hospital",
                    "details": "Recovering",
                    "until": 1893456000
                },
                "last_action": {"status": "Online", "relative": "1 minute ago", "timestamp": 1893455900},
                "life": {"current": 100, "maximum": 500}
            }
        }));
        let request = ApiRequest::get(Service::Torn, "/user/123/basic").unwrap();
        let rendered =
            render_response_for_request(&request, &response, OutputMode::JsonPretty).unwrap();
        assert!(rendered.contains("Example [123]  level 42"));
        assert!(rendered.contains("status\tHospital | Recovering | until 2030-01-01 00:00:00 UTC"));
        assert!(rendered.contains("last_action\tOnline | 1 minute ago | 2029-12-31 23:58:20 UTC"));
    }

    #[test]
    fn request_pretty_summarizes_ffscouter_stats() {
        let response = response(serde_json::json!([
            {"player_id": 123, "fair_fight": 2.1, "bs_estimate_human": "8.2b", "source": "estimate"}
        ]));
        let request = ApiRequest::get(Service::Ffscouter, "/get-stats").unwrap();
        let rendered =
            render_response_for_request(&request, &response, OutputMode::JsonPretty).unwrap();
        assert!(rendered.contains("ffscouter stats (1)"));
        assert!(rendered.contains("player_id"));
        assert!(rendered.contains("8.2b"));
    }

    #[test]
    fn pretty_colored_output_keeps_json_text_and_adds_ansi() {
        let response = response(serde_json::json!({"name": "Example", "status": "Okay"}));
        let rendered = render_response_colored(&response, OutputMode::JsonPretty).unwrap();
        assert!(rendered.contains("\u{1b}["));
        assert!(rendered.contains("\"name\""));
        assert!(rendered.contains("\"Okay\""));
    }
}
