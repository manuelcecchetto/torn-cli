use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{error::AppError, logs::LogGroupBy};

pub const BUILTIN_PRESET_SOURCE: &str = "built-in";
pub const USER_PRESET_SOURCE: &str = "user";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct LogPresetDefinition {
    pub description: Option<String>,
    pub categories: Vec<String>,
    pub log_ids: Vec<String>,
    pub contains: Vec<String>,
    pub data_keys: Vec<String>,
    pub param_keys: Vec<String>,
    pub group_by: Option<LogGroupBy>,
    pub since: Option<String>,
    pub to: Option<String>,
    pub target: Option<String>,
    pub limit: Option<u32>,
    pub max_pages: Option<usize>,
}

impl LogPresetDefinition {
    pub fn normalized(mut self) -> Self {
        normalize_vec(&mut self.categories);
        normalize_vec(&mut self.log_ids);
        normalize_vec(&mut self.contains);
        normalize_vec(&mut self.data_keys);
        normalize_vec(&mut self.param_keys);
        if self
            .description
            .as_ref()
            .is_some_and(|value| value.trim().is_empty())
        {
            self.description = None;
        }
        self
    }

    pub fn short_description(&self) -> &str {
        self.description.as_deref().unwrap_or("")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NamedLogPreset {
    pub name: String,
    pub source: String,
    pub definition: LogPresetDefinition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogPresetSummary {
    pub name: String,
    pub source: String,
    pub description: String,
    pub categories: usize,
    pub log_ids: usize,
    pub contains: Vec<String>,
    pub data_keys: Vec<String>,
    pub param_keys: Vec<String>,
    pub group_by: Option<LogGroupBy>,
    pub since: Option<String>,
    pub limit: Option<u32>,
    pub max_pages: Option<usize>,
}

impl NamedLogPreset {
    pub fn summary(&self) -> LogPresetSummary {
        LogPresetSummary {
            name: self.name.clone(),
            source: self.source.clone(),
            description: self.definition.short_description().to_string(),
            categories: self.definition.categories.len(),
            log_ids: self.definition.log_ids.len(),
            contains: self.definition.contains.clone(),
            data_keys: self.definition.data_keys.clone(),
            param_keys: self.definition.param_keys.clone(),
            group_by: self.definition.group_by,
            since: self.definition.since.clone(),
            limit: self.definition.limit,
            max_pages: self.definition.max_pages,
        }
    }
}

pub fn validate_preset_name(name: &str) -> Result<String, AppError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(AppError::InvalidRequest(
            "preset name cannot be empty".to_string(),
        ));
    }
    if !trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
    {
        return Err(AppError::InvalidRequest(format!(
            "invalid preset name `{trimmed}`; use letters, numbers, dash, underscore, or dot"
        )));
    }
    Ok(trimmed.to_ascii_lowercase())
}

pub fn builtin_log_presets() -> BTreeMap<String, LogPresetDefinition> {
    [
        preset(
            "all",
            "Fetch recent logs without category filtering; useful for discovery before making a narrower preset.",
            &[],
            LogGroupBy::Category,
            "24h",
            100,
            1,
        ),
        preset(
            "security",
            "Account access, API keys, preferences, captcha, staff/reporting, and recovery/closure events.",
            &["1", "2", "5", "10", "165", "178", "179", "200", "221"],
            LogGroupBy::Type,
            "30d",
            100,
            1,
        ),
        preset(
            "api-keys",
            "API key creation, editing, deletion, and related audit events.",
            &["221"],
            LogGroupBy::Type,
            "90d",
            100,
            1,
        ),
        preset(
            "account-lifecycle",
            "Account creation, closure, recovery, newsletters, donator status, referrals, preferences, and profile-adjacent changes.",
            &[
                "1", "5", "8", "9", "72", "73", "120", "121", "122", "123", "124",
                "143", "144", "166", "172", "178", "179", "200", "211", "213", "221",
                "231",
            ],
            LogGroupBy::Type,
            "30d",
            100,
            1,
        ),
        preset(
            "communications-social",
            "Friends, enemies, ignores, messages, events, forums, personals, lists, newspaper/social content.",
            &[
                "8", "9", "114", "115", "124", "143", "144", "166", "167", "168",
                "169", "170", "171", "172", "204", "205", "206", "208", "211", "231",
                "233",
            ],
            LogGroupBy::Type,
            "14d",
            100,
            1,
        ),
        preset(
            "money",
            "Cash, banking, checks, sending, loans, vaults, piggy bank, offshore bank, and faction payouts.",
            &[
                "13", "14", "17", "59", "60", "76", "91", "92", "112", "138", "145",
                "146", "228",
            ],
            LogGroupBy::Type,
            "30d",
            100,
            1,
        ),
        preset(
            "points-credits-tokens",
            "Points, refills, credits, donator, token shop/tokens, bunker bucks, and related currency movements.",
            &[
                "3", "4", "6", "7", "72", "73", "74", "113", "119", "173", "174",
                "176", "197", "198", "199", "203", "213", "215", "216",
            ],
            LogGroupBy::Type,
            "30d",
            100,
            1,
        ),
        preset(
            "market-trading",
            "Item market, bazaars, parcels, sending, auctions, trades, stocks, points market, property rental, and token shop.",
            &[
                "11", "18", "84", "85", "88", "94", "95", "119", "132", "133",
                "134", "140", "141", "197",
            ],
            LogGroupBy::Type,
            "14d",
            100,
            1,
        ),
        preset(
            "items",
            "Items, item movement, item use families, ammo/mods/equipment, shops, dump, museum, city finds, relics, keepsakes.",
            &[
                "11", "12", "15", "16", "20", "23", "24", "27", "28", "31", "33",
                "34", "35", "44", "47", "61", "62", "69", "70", "75", "77", "83",
                "84", "85", "86", "88", "107", "108", "109", "110", "111", "135",
                "162", "163", "177", "197", "204", "223", "224", "225", "226",
            ],
            LogGroupBy::Type,
            "14d",
            100,
            1,
        ),
        preset(
            "combat",
            "Attacks, hospital/jail/life/radiation, bounties, revives, ammo/mods/equipping, targets, and territory conflict.",
            &[
                "36", "37", "38", "39", "40", "41", "42", "43", "45", "46", "63",
                "70", "71", "81", "82", "107", "108", "109", "110", "111", "127",
                "128", "129", "130", "157", "180", "181", "182", "220", "227",
            ],
            LogGroupBy::Type,
            "7d",
            100,
            1,
        ),
        preset(
            "faction-war",
            "Faction activity, respect, dirty bombs, NAPs/treaties, incoming/outgoing faction events, OCs, territory war, and payouts.",
            &[
                "80", "81", "82", "98", "99", "100", "101", "102", "103", "161",
                "220", "228", "229", "230",
            ],
            LogGroupBy::Type,
            "14d",
            100,
            1,
        ),
        preset(
            "travel-property",
            "Travel, property, display case, rentals, estate agents, upkeep, marriage/property-adjacent events, offshore bank, bunker.",
            &[
                "19", "87", "89", "90", "93", "139", "140", "141", "144", "145",
                "214",
            ],
            LogGroupBy::Type,
            "30d",
            100,
            1,
        ),
        preset(
            "company-job",
            "Company logs, company specials, jobs, job points in/out, applications, and working stats.",
            &[
                "104", "105", "106", "142", "147", "148", "149", "150", "151", "153",
                "154",
            ],
            LogGroupBy::Type,
            "14d",
            100,
            1,
        ),
        preset(
            "progression-training",
            "Energy/nerve/happy/life/stat changes, education, merits, gym, addiction, skills, hunting, and training progression.",
            &[
                "21", "22", "25", "26", "29", "30", "32", "36", "37", "40", "42",
                "43", "48", "49", "50", "51", "52", "53", "54", "55", "56", "57",
                "58", "63", "64", "65", "66", "67", "68", "117", "118", "123",
                "125", "126", "142", "222", "232",
            ],
            LogGroupBy::Type,
            "7d",
            100,
            1,
        ),
        preset(
            "crimes",
            "Crimes, viruses, organized crimes, and crime success/failure/critical-failure outcomes.",
            &["61", "136", "137", "161", "217", "218", "219", "229", "230"],
            LogGroupBy::Type,
            "14d",
            100,
            1,
        ),
        preset(
            "casino-gambling",
            "Casino tokens, casino root category, slots, roulette, high-low, keno, craps, lottery, blackjack, wheel, RR, poker, bookie.",
            &[
                "78", "79", "183", "184", "185", "186", "187", "188", "189", "190",
                "191", "192", "193", "194", "195",
            ],
            LogGroupBy::Type,
            "14d",
            100,
            1,
        ),
        preset(
            "racing",
            "Racing activity plus racing points in/out.",
            &["116", "155", "156", "196"],
            LogGroupBy::Type,
            "30d",
            100,
            1,
        ),
        preset(
            "competitions-seasonal",
            "Awards, honors, medals, referrals, missions, seasonal events, competitions, comics/articles/headlines, relics/keepsakes.",
            &[
                "83", "120", "121", "122", "124", "135", "158", "160", "164", "175",
                "201", "202", "204", "207", "208", "209", "210", "212", "214", "215",
                "216", "223", "225",
            ],
            LogGroupBy::Type,
            "90d",
            100,
            1,
        ),
        preset(
            "staff-moderation",
            "Staff, reporting, account closure/recovery, and moderation-adjacent audit events.",
            &["165", "178", "179", "200"],
            LogGroupBy::Type,
            "90d",
            100,
            1,
        ),
    ]
    .into_iter()
    .map(|item| (item.0, item.1))
    .collect()
}

pub fn combined_log_presets(
    user_presets: &BTreeMap<String, LogPresetDefinition>,
) -> Vec<NamedLogPreset> {
    let builtins = builtin_log_presets();
    let names = builtins
        .keys()
        .chain(user_presets.keys())
        .collect::<BTreeSet<_>>();
    names
        .into_iter()
        .filter_map(|name| resolve_log_preset(user_presets, name))
        .collect()
}

pub fn resolve_log_preset(
    user_presets: &BTreeMap<String, LogPresetDefinition>,
    name: &str,
) -> Option<NamedLogPreset> {
    let normalized = validate_preset_name(name).ok()?;
    if let Some(definition) = user_presets.get(&normalized) {
        return Some(NamedLogPreset {
            name: normalized,
            source: USER_PRESET_SOURCE.to_string(),
            definition: definition.clone().normalized(),
        });
    }
    builtin_log_presets()
        .remove(&normalized)
        .map(|definition| NamedLogPreset {
            name: normalized,
            source: BUILTIN_PRESET_SOURCE.to_string(),
            definition,
        })
}

pub fn builtin_category_coverage() -> BTreeSet<String> {
    builtin_log_presets()
        .into_values()
        .flat_map(|preset| preset.categories)
        .collect()
}

fn preset(
    name: &str,
    description: &str,
    categories: &[&str],
    group_by: LogGroupBy,
    since: &str,
    limit: u32,
    max_pages: usize,
) -> (String, LogPresetDefinition) {
    (
        name.to_string(),
        LogPresetDefinition {
            description: Some(description.to_string()),
            categories: strings(categories),
            log_ids: Vec::new(),
            contains: Vec::new(),
            data_keys: Vec::new(),
            param_keys: Vec::new(),
            group_by: Some(group_by),
            since: Some(since.to_string()),
            to: None,
            target: None,
            limit: Some(limit),
            max_pages: Some(max_pages),
        },
    )
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

fn normalize_vec(values: &mut Vec<String>) {
    for value in values.iter_mut() {
        *value = value.trim().to_string();
    }
    values.retain(|value| !value.is_empty());
    values.sort_by(
        |left, right| match (left.parse::<u64>(), right.parse::<u64>()) {
            (Ok(left), Ok(right)) => left.cmp(&right),
            _ => left.cmp(right),
        },
    );
    values.dedup();
}

#[cfg(test)]
mod tests {
    use super::*;

    const OBSERVED_CATEGORY_IDS: &[&str] = &[
        "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12", "13", "14", "15", "16",
        "17", "18", "19", "20", "21", "22", "23", "24", "25", "26", "27", "28", "29", "30", "31",
        "32", "33", "34", "35", "36", "37", "38", "39", "40", "41", "42", "43", "44", "45", "46",
        "47", "48", "49", "50", "51", "52", "53", "54", "55", "56", "57", "58", "59", "60", "61",
        "62", "63", "64", "65", "66", "67", "68", "69", "70", "71", "72", "73", "74", "75", "76",
        "77", "78", "79", "80", "81", "82", "83", "84", "85", "86", "87", "88", "89", "90", "91",
        "92", "93", "94", "95", "98", "99", "100", "101", "102", "103", "104", "105", "106", "107",
        "108", "109", "110", "111", "112", "113", "114", "115", "116", "117", "118", "119", "120",
        "121", "122", "123", "124", "125", "126", "127", "128", "129", "130", "132", "133", "134",
        "135", "136", "137", "138", "139", "140", "141", "142", "143", "144", "145", "146", "147",
        "148", "149", "150", "151", "153", "154", "155", "156", "157", "158", "160", "161", "162",
        "163", "164", "165", "166", "167", "168", "169", "170", "171", "172", "173", "174", "175",
        "176", "177", "178", "179", "180", "181", "182", "183", "184", "185", "186", "187", "188",
        "189", "190", "191", "192", "193", "194", "195", "196", "197", "198", "199", "200", "201",
        "202", "203", "204", "205", "206", "207", "208", "209", "210", "211", "212", "213", "214",
        "215", "216", "217", "218", "219", "220", "221", "222", "223", "224", "225", "226", "227",
        "228", "229", "230", "231", "232", "233",
    ];

    #[test]
    fn builtin_presets_cover_all_observed_categories() {
        let covered = builtin_category_coverage();
        let observed = OBSERVED_CATEGORY_IDS
            .iter()
            .map(|value| (*value).to_string())
            .collect::<BTreeSet<_>>();
        assert_eq!(covered, observed);
    }

    #[test]
    fn user_presets_shadow_builtins() {
        let mut user = BTreeMap::new();
        user.insert(
            "security".to_string(),
            LogPresetDefinition {
                description: Some("mine".to_string()),
                categories: vec!["2".to_string()],
                ..LogPresetDefinition::default()
            },
        );
        let preset = resolve_log_preset(&user, "security").unwrap();
        assert_eq!(preset.source, USER_PRESET_SOURCE);
        assert_eq!(preset.definition.short_description(), "mine");
    }
}
