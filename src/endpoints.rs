use std::{fmt, path::Path};

use clap::ValueEnum;
use serde::Deserialize;

use crate::error::AppError;

const BUILT_IN_ENDPOINT_INDEX: &str = include_str!("../assets/endpoint-index.json");

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum EndpointServiceFilter {
    All,
    Torn,
    Ff,
}

impl fmt::Display for EndpointServiceFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::All => write!(f, "all"),
            Self::Torn => write!(f, "torn"),
            Self::Ff => write!(f, "ff"),
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawIndex {
    services: RawServices,
}

#[derive(Debug, Deserialize)]
struct RawServices {
    torn: RawService,
    ffscouter: RawService,
}

#[derive(Debug, Deserialize)]
struct RawService {
    #[allow(dead_code)]
    endpoint_count: Option<usize>,
    groups: Vec<RawGroup>,
}

#[derive(Debug, Deserialize)]
struct RawGroup {
    name: String,
    #[serde(default)]
    endpoints: Vec<RawEndpoint>,
}

#[derive(Debug, Deserialize)]
struct RawEndpoint {
    method: String,
    path: String,
    group: Option<String>,
    selection: Option<String>,
    #[serde(default)]
    path_params: Vec<String>,
    summary: Option<String>,
    description: Option<String>,
    auth_level: Option<String>,
    #[serde(default)]
    parameters: Vec<RawParameter>,
}

#[derive(Debug, Deserialize)]
struct RawParameter {
    name: String,
    #[serde(default)]
    secret: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndpointRecord {
    pub service: String,
    pub method: String,
    pub path: String,
    pub group: String,
    pub selection: Option<String>,
    pub path_params: Vec<String>,
    pub description: String,
    pub auth_level: Option<String>,
    pub params: Vec<String>,
    pub command: String,
}

#[derive(Debug, Clone)]
pub struct EndpointIndex {
    records: Vec<EndpointRecord>,
}

impl EndpointIndex {
    pub fn load(path: Option<&Path>) -> Result<Self, AppError> {
        let text = if let Some(path) = path {
            std::fs::read_to_string(path)?
        } else {
            BUILT_IN_ENDPOINT_INDEX.to_string()
        };
        Self::from_json(&text)
    }

    pub fn from_json(text: &str) -> Result<Self, AppError> {
        let raw = serde_json::from_str::<RawIndex>(text)?;
        let mut records = Vec::new();
        extend_service_records(&mut records, "torn", raw.services.torn.groups);
        extend_service_records(&mut records, "ffscouter", raw.services.ffscouter.groups);
        records.sort_by(|left, right| {
            left.service
                .cmp(&right.service)
                .then(left.group.cmp(&right.group))
                .then(left.selection.cmp(&right.selection))
                .then(left.path_params.len().cmp(&right.path_params.len()))
                .then(left.path.cmp(&right.path))
        });
        Ok(Self { records })
    }

    pub fn list(&self, filter: EndpointServiceFilter) -> Vec<&EndpointRecord> {
        self.records
            .iter()
            .filter(|record| service_matches(&record.service, filter))
            .collect()
    }

    pub fn search(&self, query: &str, filter: EndpointServiceFilter) -> Vec<&EndpointRecord> {
        let query = query.to_lowercase();
        self.list(filter)
            .into_iter()
            .filter(|record| {
                [
                    record.service.as_str(),
                    record.method.as_str(),
                    record.path.as_str(),
                    record.group.as_str(),
                    record.selection.as_deref().unwrap_or_default(),
                    record.description.as_str(),
                    record.command.as_str(),
                ]
                .iter()
                .any(|field| field.to_lowercase().contains(&query))
            })
            .collect()
    }

    pub fn find_torn(
        &self,
        group: &str,
        selection: Option<&str>,
        prefer_path_param: bool,
    ) -> Option<&EndpointRecord> {
        let mut matches = self
            .records
            .iter()
            .filter(|record| record.service == "torn" && record.group.eq_ignore_ascii_case(group))
            .filter(|record| match (selection, record.selection.as_deref()) {
                (None, None) => true,
                (Some(left), Some(right)) => left.eq_ignore_ascii_case(right),
                _ => false,
            })
            .collect::<Vec<_>>();
        matches.sort_by(|left, right| {
            left.path_params
                .len()
                .cmp(&right.path_params.len())
                .then(left.path.cmp(&right.path))
        });
        if prefer_path_param {
            matches
                .iter()
                .rev()
                .find(|record| !record.path_params.is_empty())
                .copied()
                .or_else(|| matches.first().copied())
        } else {
            matches
                .iter()
                .find(|record| record.path_params.is_empty())
                .copied()
                .or_else(|| matches.first().copied())
        }
    }

    pub fn torn_endpoint_count(&self) -> usize {
        self.records
            .iter()
            .filter(|record| record.service == "torn")
            .count()
    }
}

fn extend_service_records(records: &mut Vec<EndpointRecord>, service: &str, groups: Vec<RawGroup>) {
    for group in groups {
        for endpoint in group.endpoints {
            let group_name = endpoint.group.unwrap_or_else(|| group.name.clone());
            let description = endpoint
                .summary
                .or(endpoint.description)
                .unwrap_or_else(|| endpoint.path.clone());
            let params = endpoint
                .parameters
                .into_iter()
                .filter(|param| !param.secret)
                .map(|param| param.name)
                .collect::<Vec<_>>();
            let command = command_for(
                service,
                &group_name,
                endpoint.selection.as_deref(),
                &endpoint.path_params,
                &endpoint.path,
            );
            records.push(EndpointRecord {
                service: service.to_string(),
                method: endpoint.method,
                path: endpoint.path,
                group: group_name,
                selection: endpoint.selection,
                path_params: endpoint.path_params,
                description,
                auth_level: endpoint.auth_level,
                params,
                command,
            });
        }
    }
}

fn command_for(
    service: &str,
    group: &str,
    selection: Option<&str>,
    path_params: &[String],
    path: &str,
) -> String {
    if service == "torn" {
        match selection {
            Some(selection) => {
                let id_hint = path_params
                    .first()
                    .map(|name| format!(" --id <{name}>"))
                    .unwrap_or_default();
                format!("torn api {group} {selection}{id_hint}")
            }
            None => format!("torn api {group} --selections <csv>"),
        }
    } else {
        match selection {
            Some("check-key") => "torn ff check-key".to_string(),
            Some("register") => "torn ff register --agree-to-data-policy".to_string(),
            Some("stats") => "torn ff stats --target <id[,id]>".to_string(),
            Some("stats-history") => "torn ff stats-history --target <id>".to_string(),
            Some("flights") => "torn ff flights --target <id>".to_string(),
            Some("player") if group == "activity" => {
                "torn ff activity player --target <id>".to_string()
            }
            Some("faction") if group == "activity" => {
                "torn ff activity faction --faction <id>".to_string()
            }
            Some("claims") if group == "hit-calling" => "torn ff hits claims".to_string(),
            Some("claim") if group == "hit-calling" => {
                "torn ff hits claim --target <id> --yes".to_string()
            }
            Some("unclaim") if group == "hit-calling" => {
                "torn ff hits unclaim --target <id> --yes".to_string()
            }
            Some("wipe") if group == "hit-calling" => "torn ff hits wipe --yes".to_string(),
            Some("quote") if group == "losses" => {
                "torn ff losses quote --quantity <n> --price-per-loss <n>".to_string()
            }
            Some("seller-contracts") => "torn ff losses seller-contracts".to_string(),
            Some("seller-claims") => "torn ff losses seller-claims".to_string(),
            Some("seller-order") => "torn ff losses seller-order --order <order>".to_string(),
            Some("seller-claim") => "torn ff losses seller-claim --order <order> --yes".to_string(),
            Some("seller-complete") => {
                "torn ff losses seller-complete --claim-id <id> --yes".to_string()
            }
            Some("targets") => "torn ff targets".to_string(),
            Some("announcements") => "torn ff announcements".to_string(),
            _ => format!("torn ff get {path}"),
        }
    }
}

fn service_matches(service: &str, filter: EndpointServiceFilter) -> bool {
    match filter {
        EndpointServiceFilter::All => true,
        EndpointServiceFilter::Torn => service == "torn",
        EndpointServiceFilter::Ff => service == "ffscouter" || service == "ff",
    }
}

pub fn render_endpoint_records(records: &[&EndpointRecord]) -> String {
    records
        .iter()
        .map(|record| {
            let params = if record.params.is_empty() {
                String::new()
            } else {
                format!(" params:{}", record.params.join(","))
            };
            format!(
                "{:<10} {:<4} {:<36} {:<42} {}{}",
                record.service,
                record.method,
                record.path,
                record.command,
                record.description,
                params
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_index_covers_current_torn_openapi_paths() {
        let index = EndpointIndex::load(None).unwrap();
        assert_eq!(index.torn_endpoint_count(), 205);
        assert!(
            index
                .list(EndpointServiceFilter::Torn)
                .iter()
                .any(|record| record.path == "/user/basic")
        );
        assert!(
            index
                .list(EndpointServiceFilter::Ff)
                .iter()
                .any(|record| record.path == "/check-key")
        );
    }

    #[test]
    fn resolves_id_aware_shortcuts() {
        let index = EndpointIndex::load(None).unwrap();
        assert_eq!(
            index
                .find_torn("faction", Some("basic"), false)
                .unwrap()
                .path,
            "/faction/basic"
        );
        assert_eq!(
            index
                .find_torn("faction", Some("basic"), true)
                .unwrap()
                .path,
            "/faction/{id}/basic"
        );
    }
}
