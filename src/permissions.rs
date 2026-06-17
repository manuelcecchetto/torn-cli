use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{
    client::ApiClient,
    endpoints::{EndpointIndex, EndpointRecord, EndpointServiceFilter},
    error::AppError,
    request::{ApiRequest, QueryParam, Service},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AccessLevel {
    Public,
    Minimal,
    Limited,
    Full,
}

impl AccessLevel {
    pub fn parse(input: &str) -> Option<Self> {
        match input.trim().to_ascii_lowercase().as_str() {
            "public" | "public only" => Some(Self::Public),
            "minimal" | "minimal access" => Some(Self::Minimal),
            "limited" | "limited access" => Some(Self::Limited),
            "full" | "full access" => Some(Self::Full),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Public => "Public",
            Self::Minimal => "Minimal Access",
            Self::Limited => "Limited Access",
            Self::Full => "Full Access",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyAccessKind {
    Custom,
    PublicOnly,
    MinimalAccess,
    LimitedAccess,
    FullAccess,
    Unknown,
}

impl KeyAccessKind {
    pub fn parse(input: &str) -> Self {
        match input.trim().to_ascii_lowercase().as_str() {
            "custom" => Self::Custom,
            "public" | "public only" => Self::PublicOnly,
            "minimal" | "minimal access" => Self::MinimalAccess,
            "limited" | "limited access" => Self::LimitedAccess,
            "full" | "full access" => Self::FullAccess,
            _ => Self::Unknown,
        }
    }

    pub fn max_level(self) -> Option<AccessLevel> {
        match self {
            Self::PublicOnly => Some(AccessLevel::Public),
            Self::MinimalAccess => Some(AccessLevel::Minimal),
            Self::LimitedAccess => Some(AccessLevel::Limited),
            Self::FullAccess => Some(AccessLevel::Full),
            Self::Custom | Self::Unknown => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Custom => "Custom",
            Self::PublicOnly => "Public Only",
            Self::MinimalAccess => "Minimal Access",
            Self::LimitedAccess => "Limited Access",
            Self::FullAccess => "Full Access",
            Self::Unknown => "Unknown",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyInfoResponse {
    pub info: KeyInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyInfo {
    pub selections: BTreeMap<String, Vec<String>>,
    pub access: KeyAccess,
    pub user: KeyInfoUser,
}

impl KeyInfo {
    pub fn access_kind(&self) -> KeyAccessKind {
        KeyAccessKind::parse(&self.access.kind)
    }

    pub fn has_selection(&self, group: &str, selection: &str) -> bool {
        self.selections
            .get(&group.to_ascii_lowercase())
            .or_else(|| self.selections.get(group))
            .map(|selections| {
                selections
                    .iter()
                    .any(|candidate| candidate.eq_ignore_ascii_case(selection))
            })
            .unwrap_or(false)
    }

    pub fn selection_count(&self) -> usize {
        self.selections.values().map(Vec::len).sum()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyAccess {
    pub level: i64,
    #[serde(rename = "type")]
    pub kind: String,
    pub faction: bool,
    pub company: bool,
    pub log: KeyLogAccess,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyLogAccess {
    pub custom_permissions: bool,
    pub available: Vec<KeyInfoAvailableLog>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyInfoAvailableLog {
    pub category_id: i64,
    pub log_ids: Vec<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyInfoUser {
    pub id: i64,
    pub faction_id: Option<i64>,
    pub company_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionRequirement {
    pub group: String,
    pub selection: Option<String>,
    pub min_access: Option<AccessLevel>,
    pub path: String,
    pub command: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionDecision {
    Allowed,
    Denied(String),
    Unknown(String),
}

impl PermissionDecision {
    fn into_result(self) -> Result<(), AppError> {
        match self {
            Self::Allowed | Self::Unknown(_) => Ok(()),
            Self::Denied(reason) => Err(AppError::PermissionDenied(reason)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TornPermissionContext {
    pub key_info: KeyInfo,
}

impl TornPermissionContext {
    pub async fn fetch(client: &ApiClient) -> Result<Self, AppError> {
        let response = client
            .execute(ApiRequest::get(Service::Torn, "/key/info")?)
            .await?;
        let json = response
            .body_json
            .ok_or_else(|| AppError::Json("/key/info did not return JSON".to_string()))?;
        let key_info = serde_json::from_value::<KeyInfoResponse>(json)?.info;
        Ok(Self { key_info })
    }

    pub fn ensure_request_allowed(
        &self,
        index: &EndpointIndex,
        request: &ApiRequest,
    ) -> Result<(), AppError> {
        for requirement in request_requirements(index, request) {
            self.evaluate_requirement(&requirement, &request.query)
                .into_result()?;
        }
        Ok(())
    }

    pub fn evaluate_request(
        &self,
        index: &EndpointIndex,
        request: &ApiRequest,
    ) -> Vec<(PermissionRequirement, PermissionDecision)> {
        request_requirements(index, request)
            .into_iter()
            .map(|requirement| {
                let decision = self.evaluate_requirement(&requirement, &request.query);
                (requirement, decision)
            })
            .collect()
    }

    pub fn evaluate_requirement(
        &self,
        requirement: &PermissionRequirement,
        query: &[QueryParam],
    ) -> PermissionDecision {
        let kind = self.key_info.access_kind();
        if matches!(kind, KeyAccessKind::FullAccess) {
            return PermissionDecision::Allowed;
        }

        if matches!(kind, KeyAccessKind::Custom) {
            return self.evaluate_custom_requirement(requirement, query);
        }

        let Some(required) = requirement.min_access else {
            return PermissionDecision::Unknown(format!(
                "no access-level metadata is available for {}",
                requirement_label(requirement)
            ));
        };
        let Some(max_level) = kind.max_level() else {
            return PermissionDecision::Unknown(format!(
                "key type {} cannot be ordered against {}",
                kind.label(),
                required.label()
            ));
        };
        if max_level >= required {
            PermissionDecision::Allowed
        } else {
            PermissionDecision::Denied(format!(
                "{} requires {}, but the configured Torn key is {}. Use `torn config set torn-api-key` or a custom key containing this selection.",
                requirement_label(requirement),
                required.label(),
                kind.label()
            ))
        }
    }

    fn evaluate_custom_requirement(
        &self,
        requirement: &PermissionRequirement,
        query: &[QueryParam],
    ) -> PermissionDecision {
        let Some(selection) = &requirement.selection else {
            return PermissionDecision::Unknown(format!(
                "custom key preflight could not infer a selection for {}",
                requirement.path
            ));
        };

        if self
            .key_info
            .has_selection(&requirement.group, selection.as_str())
        {
            return PermissionDecision::Allowed;
        }

        if requirement.group.eq_ignore_ascii_case("user")
            && selection.eq_ignore_ascii_case("log")
            && self.key_info.access.log.custom_permissions
        {
            return self.evaluate_custom_log_requirement(query);
        }

        PermissionDecision::Denied(format!(
            "{} is not present in this custom Torn key. Rebuild the custom key with `{}` -> `{}` or use a key with a sufficient predefined access level.",
            requirement_label(requirement),
            requirement.group,
            selection
        ))
    }

    fn evaluate_custom_log_requirement(&self, query: &[QueryParam]) -> PermissionDecision {
        let available = &self.key_info.access.log.available;
        if available.is_empty() {
            return PermissionDecision::Denied(
                "this custom Torn key has custom log permissions enabled, but /key/info reports no available log ids".to_string(),
            );
        }

        let requested_logs = comma_query_values(query, "log")
            .into_iter()
            .filter_map(|value| value.parse::<i64>().ok())
            .collect::<Vec<_>>();
        if !requested_logs.is_empty() {
            let allowed = available
                .iter()
                .flat_map(|item| item.log_ids.iter().copied())
                .collect::<BTreeSet<_>>();
            if requested_logs.iter().all(|id| allowed.contains(id)) {
                return PermissionDecision::Allowed;
            }
            return PermissionDecision::Denied(format!(
                "this custom Torn key can only read log ids {}; requested log ids were {}",
                join_i64_set(&allowed),
                requested_logs
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(",")
            ));
        }

        let requested_categories = comma_query_values(query, "cat")
            .into_iter()
            .filter_map(|value| value.parse::<i64>().ok())
            .collect::<Vec<_>>();
        if !requested_categories.is_empty() {
            let allowed = available
                .iter()
                .map(|item| item.category_id)
                .collect::<BTreeSet<_>>();
            if requested_categories.iter().all(|id| allowed.contains(id)) {
                return PermissionDecision::Allowed;
            }
            return PermissionDecision::Denied(format!(
                "this custom Torn key can only read log categories {}; requested categories were {}",
                join_i64_set(&allowed),
                requested_categories
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(",")
            ));
        }

        PermissionDecision::Denied(
            "this custom Torn key has limited log permissions; pass --log <id[,id]> or --cat <id> so torn-cli can verify the request before calling /user/log".to_string(),
        )
    }
}

pub fn should_preflight_request(request: &ApiRequest) -> bool {
    request.service == Service::Torn && request.use_auth && !is_key_info_request(request)
}

pub fn request_requirements(
    index: &EndpointIndex,
    request: &ApiRequest,
) -> Vec<PermissionRequirement> {
    if request.service != Service::Torn {
        return Vec::new();
    }

    let group = request
        .path
        .trim_start_matches('/')
        .split('/')
        .next()
        .filter(|group| !group.is_empty())
        .map(|group| group.to_ascii_lowercase());

    if let Some(group) = &group {
        let selections = selections_from_query(&request.query);
        if !selections.is_empty() {
            return selections
                .into_iter()
                .map(|selection| requirement_for_selection(index, group, &selection, &request.path))
                .collect();
        }
    }

    if let Some(record) = find_record_for_path(index, &request.path) {
        return vec![requirement_from_record(record)];
    }

    if let Some(group) = group {
        return vec![PermissionRequirement {
            group,
            selection: None,
            min_access: None,
            path: request.path.clone(),
            command: format!("torn api get {}", request.path),
        }];
    }

    Vec::new()
}

pub fn capability_lines(key_info: &KeyInfo, index: &EndpointIndex) -> Vec<String> {
    let kind = key_info.access_kind();
    let mut lines = vec![
        format!("access: {} (level {})", kind.label(), key_info.access.level),
        format!(
            "owner: user {}{}{}",
            key_info.user.id,
            key_info
                .user
                .faction_id
                .map(|id| format!(", faction {id}"))
                .unwrap_or_default(),
            key_info
                .user
                .company_id
                .map(|id| format!(", company {id}"))
                .unwrap_or_default()
        ),
        format!(
            "scopes: faction={} company={} total_selections={}",
            key_info.access.faction,
            key_info.access.company,
            key_info.selection_count()
        ),
    ];

    match kind {
        KeyAccessKind::PublicOnly
        | KeyAccessKind::MinimalAccess
        | KeyAccessKind::LimitedAccess
        | KeyAccessKind::FullAccess => {
            if let Some(level) = kind.max_level() {
                let counts = indexed_access_counts(index, level);
                lines.push(format!(
                    "predefined access: can call indexed selections up to {} (public {}, minimal {}, limited {}, full {})",
                    level.label(),
                    counts.get(&AccessLevel::Public).copied().unwrap_or_default(),
                    counts.get(&AccessLevel::Minimal).copied().unwrap_or_default(),
                    counts.get(&AccessLevel::Limited).copied().unwrap_or_default(),
                    counts.get(&AccessLevel::Full).copied().unwrap_or_default()
                ));
            }
        }
        KeyAccessKind::Custom => {
            lines.push("custom access: exact selections reported by /key/info".to_string());
        }
        KeyAccessKind::Unknown => {
            lines.push(
                "unknown access type: torn-cli will allow unknown preflight cases through to Torn"
                    .to_string(),
            );
        }
    }

    for (group, selections) in &key_info.selections {
        if selections.is_empty() {
            continue;
        }
        lines.push(format!(
            "{group}: {}",
            abbreviated_list(selections.iter().map(String::as_str), 12)
        ));
    }

    if key_info.access.log.custom_permissions {
        lines.push(format!(
            "custom log permissions: {} categories, {} log ids",
            key_info.access.log.available.len(),
            key_info
                .access
                .log
                .available
                .iter()
                .map(|item| item.log_ids.len())
                .sum::<usize>()
        ));
    }

    lines
}

fn indexed_access_counts(
    index: &EndpointIndex,
    level: AccessLevel,
) -> BTreeMap<AccessLevel, usize> {
    let mut counts = BTreeMap::new();
    for record in index.list(EndpointServiceFilter::Torn) {
        if let Some(access) = record.auth_level.as_deref().and_then(AccessLevel::parse) {
            if access <= level {
                *counts.entry(access).or_insert(0) += 1;
            }
        }
    }
    counts
}

fn requirement_for_selection(
    index: &EndpointIndex,
    group: &str,
    selection: &str,
    path: &str,
) -> PermissionRequirement {
    if let Some(record) = index.find_torn(group, Some(selection), false) {
        requirement_from_record(record)
    } else {
        PermissionRequirement {
            group: group.to_string(),
            selection: Some(selection.to_string()),
            min_access: None,
            path: path.to_string(),
            command: format!("torn api {group} {selection}"),
        }
    }
}

fn requirement_from_record(record: &EndpointRecord) -> PermissionRequirement {
    PermissionRequirement {
        group: record.group.to_ascii_lowercase(),
        selection: record.selection.clone(),
        min_access: record.auth_level.as_deref().and_then(AccessLevel::parse),
        path: record.path.clone(),
        command: record.command.clone(),
    }
}

fn find_record_for_path<'a>(index: &'a EndpointIndex, path: &str) -> Option<&'a EndpointRecord> {
    let mut candidates = index
        .list(EndpointServiceFilter::Torn)
        .into_iter()
        .filter(|record| template_matches_path(&record.path, path))
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        template_score(&right.path)
            .cmp(&template_score(&left.path))
            .then(left.path.cmp(&right.path))
    });
    candidates.into_iter().next()
}

fn template_matches_path(template: &str, path: &str) -> bool {
    let template_parts = path_parts(template);
    let path_parts = path_parts(path);
    template_parts.len() == path_parts.len()
        && template_parts
            .iter()
            .zip(path_parts.iter())
            .all(|(a, b)| (a.starts_with('{') && a.ends_with('}')) || a.eq_ignore_ascii_case(b))
}

fn template_score(template: &str) -> usize {
    path_parts(template)
        .into_iter()
        .filter(|part| !(part.starts_with('{') && part.ends_with('}')))
        .count()
}

fn path_parts(path: &str) -> Vec<&str> {
    path.trim_start_matches('/')
        .split('/')
        .filter(|part| !part.is_empty())
        .collect()
}

fn selections_from_query(query: &[QueryParam]) -> Vec<String> {
    comma_query_values(query, "selections")
}

fn comma_query_values(query: &[QueryParam], name: &str) -> Vec<String> {
    query
        .iter()
        .filter(|param| param.name.eq_ignore_ascii_case(name))
        .flat_map(|param| param.value.split(','))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
}

fn requirement_label(requirement: &PermissionRequirement) -> String {
    match &requirement.selection {
        Some(selection) => format!(
            "`{}` -> `{}` ({})",
            requirement.group, selection, requirement.command
        ),
        None => format!("`{}` ({})", requirement.path, requirement.command),
    }
}

fn join_i64_set(values: &BTreeSet<i64>) -> String {
    values
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

fn abbreviated_list<'a>(values: impl Iterator<Item = &'a str>, max: usize) -> String {
    let values = values.collect::<Vec<_>>();
    let shown = values
        .iter()
        .take(max)
        .copied()
        .collect::<Vec<_>>()
        .join(", ");
    if values.len() > max {
        format!("{shown}, … (+{} more)", values.len() - max)
    } else {
        shown
    }
}

fn is_key_info_request(request: &ApiRequest) -> bool {
    let path = request.path.trim_end_matches('/');
    if path.eq_ignore_ascii_case("/key/info") {
        return true;
    }
    path.eq_ignore_ascii_case("/key")
        && selections_from_query(&request.query)
            .iter()
            .any(|selection| selection.eq_ignore_ascii_case("info"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{endpoints::EndpointIndex, request::ApiRequest};

    fn custom_key_info() -> KeyInfo {
        KeyInfo {
            selections: BTreeMap::from([
                ("user".to_string(), vec!["basic".to_string()]),
                (
                    "key".to_string(),
                    vec!["info".to_string(), "log".to_string()],
                ),
            ]),
            access: KeyAccess {
                level: 0,
                kind: "Custom".to_string(),
                faction: false,
                company: false,
                log: KeyLogAccess {
                    custom_permissions: true,
                    available: vec![KeyInfoAvailableLog {
                        category_id: 1,
                        log_ids: vec![105, 106],
                    }],
                },
            },
            user: KeyInfoUser {
                id: 1,
                faction_id: None,
                company_id: None,
            },
        }
    }

    #[test]
    fn limited_key_cannot_preflight_full_user_log() {
        let ctx = TornPermissionContext {
            key_info: KeyInfo {
                access: KeyAccess {
                    kind: "Limited Access".to_string(),
                    level: 3,
                    faction: false,
                    company: false,
                    log: KeyLogAccess {
                        custom_permissions: false,
                        available: Vec::new(),
                    },
                },
                selections: BTreeMap::new(),
                user: KeyInfoUser {
                    id: 1,
                    faction_id: None,
                    company_id: None,
                },
            },
        };
        let index = EndpointIndex::load(None).unwrap();
        let request = ApiRequest::get(Service::Torn, "/user/log").unwrap();
        let decisions = ctx.evaluate_request(&index, &request);
        assert!(matches!(decisions[0].1, PermissionDecision::Denied(_)));
    }

    #[test]
    fn custom_key_allows_listed_selection_and_denies_missing_one() {
        let ctx = TornPermissionContext {
            key_info: custom_key_info(),
        };
        let index = EndpointIndex::load(None).unwrap();
        let basic = ApiRequest::get(Service::Torn, "/user/basic").unwrap();
        assert!(matches!(
            ctx.evaluate_request(&index, &basic)[0].1,
            PermissionDecision::Allowed
        ));
        let bars = ApiRequest::get(Service::Torn, "/user/bars").unwrap();
        assert!(matches!(
            ctx.evaluate_request(&index, &bars)[0].1,
            PermissionDecision::Denied(_)
        ));
    }

    #[test]
    fn custom_log_permissions_require_matching_log_or_category() {
        let ctx = TornPermissionContext {
            key_info: custom_key_info(),
        };
        let index = EndpointIndex::load(None).unwrap();
        let allowed = ApiRequest::get(Service::Torn, "/user/log?log=105").unwrap();
        assert!(matches!(
            ctx.evaluate_request(&index, &allowed)[0].1,
            PermissionDecision::Allowed
        ));
        let denied = ApiRequest::get(Service::Torn, "/user/log?log=999").unwrap();
        assert!(matches!(
            ctx.evaluate_request(&index, &denied)[0].1,
            PermissionDecision::Denied(_)
        ));
    }

    #[test]
    fn requirements_expand_query_selections() {
        let index = EndpointIndex::load(None).unwrap();
        let request = ApiRequest::get(Service::Torn, "/user?selections=basic,bars").unwrap();
        let requirements = request_requirements(&index, &request);
        assert_eq!(requirements.len(), 2);
        assert_eq!(requirements[0].selection.as_deref(), Some("basic"));
        assert_eq!(requirements[1].selection.as_deref(), Some("bars"));
    }
}
