use std::{
    collections::{BTreeMap, HashSet},
    io::{IsTerminal, Read, Write},
    path::PathBuf,
    time::Duration,
};

use chrono::{Local, Utc};
use clap::{Args, Parser, Subcommand, ValueEnum};
use crossterm::style::{Color, Stylize, style};
use serde_json::{Map, Value, json};

const DEFAULT_API_PAGE_LIMIT: usize = 100;

use crate::{
    cache::CachePolicy,
    client::{ApiClient, ApiResponse, PaginationOptions},
    config::{
        Config, ConfigLoadOptions, ConfigOverrides, ConfigSecretKey, remove_log_preset,
        update_config_secret, upsert_log_preset,
    },
    endpoints::{EndpointIndex, EndpointRecord, EndpointServiceFilter, render_endpoint_records},
    error::AppError,
    log_presets::{
        LogPresetDefinition, LogPresetSummary, builtin_log_presets, combined_log_presets,
        resolve_log_preset, validate_preset_name,
    },
    logs::{
        LogGroupBy, LogsAnalyzeSpec, LogsCatalogSpec, LogsFetchSpec, LogsPresetAnalyzeSpec,
        analyze_user_logs, analyze_user_logs_with_preset, fetch_log_catalog, fetch_user_logs,
        parse_timestamp_arg, preset_fetch_specs, render_analysis, render_catalog,
        render_catalog_categories, render_catalog_types, render_log_entries,
    },
    output::{OutputMode, render_response_for_request, render_response_for_request_colored},
    permissions::{TornPermissionContext, capability_lines, should_preflight_request},
    request::{ApiRequest, HttpMethod, QueryParam, Service},
};

#[derive(Debug, Parser)]
#[command(
    name = "torn",
    version,
    about = "Privacy-conscious Torn API v2 CLI",
    long_about = "Privacy-conscious Torn API v2 and FFScouter CLI.\n\nTorn auth uses Authorization: ApiKey <TORN_API_KEY>; keys are never added to Torn URLs. FFScouter requires key= query auth, and displayed URLs/logs/cache keys/response bodies redact configured secrets.\n\nExamples:\n  torn config check\n  torn config set torn-api-key\n  torn config tui\n  torn endpoints --service torn\n  torn endpoints search attacks\n  torn api get /user/basic --pretty\n  torn api user basic --table\n  torn --watch 30s --pretty api user basic --id 1844049\n  torn api faction rankedwarreport --id 12345 --pretty\n  torn --all-pages api faction attacks --from 1781964000 --to 1782012665 --sort ASC --limit 100 --json\n  torn api market itemmarket --id 206 --param bonus=Any\n  torn api user futureSelection --param foo=bar --raw\n  torn logs analyze --since 7d --to now --group-by category\n  torn logs catalog --pretty\n  torn ff check-key --pretty\n  torn ff stats --target 3747263 --json\n  torn ff activity player --target 3747263 --since 24h --bucket 900"
)]
pub struct Cli {
    #[command(flatten)]
    pub global: GlobalOptions,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Clone, Args, Default)]
pub struct GlobalOptions {
    #[arg(
        short = 'c',
        long,
        global = true,
        value_name = "PATH",
        help = "Config file path"
    )]
    pub config: Option<PathBuf>,
    #[arg(
        long,
        global = true,
        value_name = "PATH",
        help = "Load variables from an explicit .env file"
    )]
    pub env_file: Option<PathBuf>,
    #[arg(
        long,
        global = true,
        help = "Do not load .env from the current directory"
    )]
    pub no_env: bool,
    #[arg(
        long,
        global = true,
        value_name = "DIR",
        help = "Override cache directory"
    )]
    pub cache_dir: Option<PathBuf>,
    #[arg(long, global = true, help = "Disable cache for this command")]
    pub no_cache: bool,
    #[arg(long, global = true, help = "Bypass cache and force a network request")]
    pub fresh: bool,
    #[arg(
        long,
        global = true,
        value_name = "DURATION",
        help = "Override cache TTL, e.g. 30s, 5m, 1h"
    )]
    pub cache_ttl: Option<String>,
    #[arg(long, global = true, conflicts_with_all = ["pretty", "raw", "table", "csv"], help = "Emit compact JSON")]
    pub json: bool,
    #[arg(long, global = true, conflicts_with_all = ["json", "raw", "table", "csv"], help = "Emit schema-aware human summary; colored on terminals")]
    pub pretty: bool,
    #[arg(long, global = true, conflicts_with_all = ["json", "pretty", "table", "csv"], help = "Emit raw API body")]
    pub raw: bool,
    #[arg(long, global = true, conflicts_with_all = ["json", "pretty", "raw", "csv"], help = "Emit table output when possible")]
    pub table: bool,
    #[arg(long, global = true, conflicts_with_all = ["json", "pretty", "raw", "table"], help = "Emit CSV output when possible")]
    pub csv: bool,
    #[arg(
        long,
        global = true,
        value_name = "DURATION",
        num_args = 0..=1,
        default_missing_value = "30s",
        help = "Repeat a GET request until interrupted; optional interval like 10s, 1m (default 30s). Watch bypasses cache."
    )]
    pub watch: Option<String>,
    #[arg(
        long,
        global = true,
        help = "Follow _metadata.links.next for GET requests and merge paginated JSON arrays before rendering"
    )]
    pub all_pages: bool,
    #[arg(
        long = "page-limit",
        global = true,
        value_name = "N",
        help = "Maximum pages to fetch with --all-pages (default 100). Passing --page-limit implies --all-pages."
    )]
    pub page_limit: Option<usize>,
    #[arg(long, global = true, help = "Suppress non-essential logs")]
    pub quiet: bool,
    #[arg(short = 'v', long, global = true, action = clap::ArgAction::Count, help = "Verbose logs without secrets")]
    pub verbose: u8,
    #[arg(
        long,
        global = true,
        value_name = "KEY",
        help = "Torn API key override (prefer TORN_API_KEY or --env-file to avoid shell history)"
    )]
    pub torn_api_key: Option<String>,
    #[arg(
        long,
        global = true,
        value_name = "KEY",
        help = "FFScouter API key override (prefer FFSCOUTER_API_KEY or --env-file)"
    )]
    pub ffscouter_api_key: Option<String>,
    #[arg(long, global = true, value_name = "URL")]
    pub torn_base_url: Option<String>,
    #[arg(long, global = true, value_name = "URL")]
    pub ffscouter_base_url: Option<String>,
    #[arg(long, global = true, value_name = "PATH")]
    pub endpoint_index_path: Option<PathBuf>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Config(ConfigCommand),
    Endpoints(EndpointsCommand),
    Api(ApiCommand),
    Ff(FfCommand),
    Logs(LogsCommand),
    Cache(CacheCommand),
    Saved(SavedCommand),
    Tui,
}

#[derive(Debug, Args)]
pub struct ConfigCommand {
    #[command(subcommand)]
    pub command: ConfigSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum ConfigSubcommand {
    Check {
        #[arg(long)]
        online: bool,
    },
    Path,
    Show {
        #[arg(long, default_value_t = true)]
        redacted: bool,
    },
    Set(ConfigSetArgs),
    Permissions,
    Tui,
}

#[derive(Debug, Args)]
pub struct ConfigSetArgs {
    #[arg(value_enum)]
    pub key: ConfigSetKey,
    #[arg(
        long,
        help = "Read the secret from stdin instead of an interactive hidden prompt"
    )]
    pub stdin: bool,
    #[arg(long, help = "Remove this secret from the private config file")]
    pub remove: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ConfigSetKey {
    TornApiKey,
    FfscouterApiKey,
}

impl ConfigSetKey {
    fn label(self) -> &'static str {
        match self {
            Self::TornApiKey => "Torn API key",
            Self::FfscouterApiKey => "FFScouter API key",
        }
    }

    fn config_secret_key(self) -> ConfigSecretKey {
        match self {
            Self::TornApiKey => ConfigSecretKey::TornApiKey,
            Self::FfscouterApiKey => ConfigSecretKey::FfscouterApiKey,
        }
    }
}

#[derive(Debug, Args)]
pub struct EndpointsCommand {
    #[arg(long, value_enum, default_value_t = EndpointServiceFilter::All)]
    pub service: EndpointServiceFilter,
    #[command(subcommand)]
    pub command: Option<EndpointsSubcommand>,
}

#[derive(Debug, Subcommand)]
pub enum EndpointsSubcommand {
    Search { query: String },
}

#[derive(Debug, Args)]
pub struct ApiCommand {
    #[command(subcommand)]
    pub command: ApiSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum ApiSubcommand {
    Get(GenericRequestArgs),
    Post(GenericPostArgs),
    User(SectionRequestArgs),
    Faction(SectionRequestArgs),
    Torn(SectionRequestArgs),
    Market(SectionRequestArgs),
    Company(SectionRequestArgs),
    Racing(SectionRequestArgs),
    Forum(SectionRequestArgs),
    Property(SectionRequestArgs),
    Key(SectionRequestArgs),
}

#[derive(Debug, Clone, Args)]
pub struct SectionRequestArgs {
    #[arg(
        value_name = "SELECTION",
        help = "Selection/endpoint. Unknown selections fall back to /<group>?selections=<selection> for future API coverage."
    )]
    pub selection: Option<String>,
    #[arg(
        long,
        value_name = "VALUE",
        help = "Value for first path parameter such as id, ids, raceId, stockId"
    )]
    pub id: Option<String>,
    #[arg(
        long = "path-param",
        value_name = "NAME=VALUE",
        help = "Bind a named path parameter"
    )]
    pub path_params: Vec<QueryParam>,
    #[arg(
        short = 'p',
        long = "param",
        value_name = "NAME=VALUE",
        help = "Add/override a query parameter; may be repeated"
    )]
    pub extra_params: Vec<QueryParam>,
    #[command(flatten)]
    pub params: CommonParams,
}

#[derive(Debug, Clone, Args)]
pub struct GenericRequestArgs {
    pub path: String,
    #[arg(short = 'p', long = "param", value_name = "NAME=VALUE")]
    pub params: Vec<QueryParam>,
    #[arg(long)]
    pub no_auth: bool,
    #[command(flatten)]
    pub common: CommonParams,
}

#[derive(Debug, Clone, Args)]
pub struct GenericPostArgs {
    #[command(flatten)]
    pub request: GenericRequestArgs,
    #[arg(long, conflicts_with = "body_file")]
    pub body: Option<String>,
    #[arg(long = "body-file", value_name = "PATH")]
    pub body_file: Option<PathBuf>,
}

#[derive(Debug, Clone, Args, Default)]
pub struct CommonParams {
    #[arg(long, value_name = "CSV")]
    pub selections: Option<String>,
    #[arg(long, value_name = "CSV")]
    pub legacy: Option<String>,
    #[arg(long, value_name = "N")]
    pub limit: Option<String>,
    #[arg(long, value_name = "N")]
    pub offset: Option<String>,
    #[arg(long, value_name = "TS")]
    pub from: Option<String>,
    #[arg(long, value_name = "TS")]
    pub to: Option<String>,
    #[arg(long, value_name = "ORDER")]
    pub sort: Option<String>,
    #[arg(long, value_name = "CAT")]
    pub cat: Option<String>,
    #[arg(long, value_name = "STAT")]
    pub stat: Option<String>,
    #[arg(long, value_name = "FILTERS")]
    pub filters: Option<String>,
    #[arg(long, value_name = "BOOL")]
    pub striptags: Option<String>,
    #[arg(long, value_name = "ID")]
    pub target: Option<String>,
    #[arg(long, value_name = "ID")]
    pub log: Option<String>,
    #[arg(long, value_name = "NAME")]
    pub name: Option<String>,
    #[arg(long, value_name = "BONUS")]
    pub bonus: Option<String>,
    #[arg(long, value_name = "TS")]
    pub timestamp: Option<String>,
    #[arg(long, value_name = "TEXT")]
    pub comment: Option<String>,
}

impl CommonParams {
    pub fn to_query_params(&self) -> Vec<QueryParam> {
        let mut params = Vec::new();
        push_opt(&mut params, "selections", &self.selections);
        push_opt(&mut params, "legacy", &self.legacy);
        push_opt(&mut params, "limit", &self.limit);
        push_opt(&mut params, "offset", &self.offset);
        push_opt(&mut params, "from", &self.from);
        push_opt(&mut params, "to", &self.to);
        push_opt(&mut params, "sort", &self.sort);
        push_opt(&mut params, "cat", &self.cat);
        push_opt(&mut params, "stat", &self.stat);
        push_opt(&mut params, "filters", &self.filters);
        push_opt(&mut params, "striptags", &self.striptags);
        push_opt(&mut params, "target", &self.target);
        push_opt(&mut params, "log", &self.log);
        push_opt(&mut params, "name", &self.name);
        push_opt(&mut params, "bonus", &self.bonus);
        push_opt(&mut params, "timestamp", &self.timestamp);
        push_opt(&mut params, "comment", &self.comment);
        params
    }
}

fn push_opt(params: &mut Vec<QueryParam>, name: &str, value: &Option<String>) {
    if let Some(value) = value {
        params.push(QueryParam::new(name, value));
    }
}

#[derive(Debug, Args)]
pub struct FfCommand {
    #[command(subcommand)]
    pub command: FfSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum FfSubcommand {
    Get(GenericRequestArgs),
    Post(GenericPostArgs),
    CheckKey(FfShortcutArgs),
    Status(FfShortcutArgs),
    Register(FfRegisterArgs),
    Stats(FfStatsArgs),
    StatsHistory(FfStatsHistoryArgs),
    Flights(FfTargetArgs),
    Activity(FfActivityCommand),
    Hits(FfHitCallingCommand),
    Losses(FfLossesCommand),
    Targets(FfTargetsArgs),
    Announcements(FfShortcutArgs),
}

#[derive(Debug, Clone, Args, Default)]
pub struct FfShortcutArgs {
    #[arg(long)]
    pub no_auth: bool,
    #[arg(
        short = 'p',
        long = "param",
        value_name = "NAME=VALUE",
        help = "Add/override a query parameter; may be repeated"
    )]
    pub extra_params: Vec<QueryParam>,
    #[command(flatten)]
    pub params: CommonParams,
}

#[derive(Debug, Clone, Args, Default)]
pub struct UserIdArgs {
    #[arg(long = "user", alias = "user-id", value_name = "ID")]
    pub user_id: Option<String>,
    #[arg(long)]
    pub no_auth: bool,
    #[arg(
        short = 'p',
        long = "param",
        value_name = "NAME=VALUE",
        help = "Add/override a query parameter; may be repeated"
    )]
    pub extra_params: Vec<QueryParam>,
    #[command(flatten)]
    pub params: CommonParams,
}

#[derive(Debug, Clone, Args, Default)]
pub struct FfStatsArgs {
    #[arg(
        long = "target",
        alias = "targets",
        alias = "user",
        alias = "user-id",
        value_name = "ID[,ID]",
        value_delimiter = ',',
        required = true,
        help = "One or more Torn player ids. FFScouter supports up to 205 per request."
    )]
    pub targets: Vec<String>,
    #[arg(long)]
    pub no_auth: bool,
    #[arg(short = 'p', long = "param", value_name = "NAME=VALUE")]
    pub extra_params: Vec<QueryParam>,
}

#[derive(Debug, Clone, Args, Default)]
pub struct FfStatsHistoryArgs {
    #[arg(
        long = "target",
        alias = "user",
        alias = "user-id",
        value_name = "ID",
        required = true
    )]
    pub target: String,
    #[arg(long)]
    pub limit: Option<u32>,
    #[arg(long = "since", alias = "from", value_name = "TS|DURATION")]
    pub since: Option<String>,
    #[arg(long, value_name = "TS|DURATION")]
    pub to: Option<String>,
    #[arg(long, value_enum)]
    pub sort: Option<FfHistorySort>,
    #[arg(long)]
    pub no_auth: bool,
    #[arg(short = 'p', long = "param", value_name = "NAME=VALUE")]
    pub extra_params: Vec<QueryParam>,
}

#[derive(Debug, Clone, Args, Default)]
pub struct FfTargetArgs {
    #[arg(
        long = "target",
        alias = "user",
        alias = "user-id",
        value_name = "ID",
        required = true
    )]
    pub target: String,
    #[arg(long)]
    pub no_auth: bool,
    #[arg(short = 'p', long = "param", value_name = "NAME=VALUE")]
    pub extra_params: Vec<QueryParam>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum FfHistorySort {
    Asc,
    Desc,
}

impl FfHistorySort {
    fn as_str(self) -> &'static str {
        match self {
            Self::Asc => "asc",
            Self::Desc => "desc",
        }
    }
}

#[derive(Debug, Clone, Args, Default)]
pub struct FfTargetsArgs {
    #[arg(long, value_enum)]
    pub preset: Option<FfTargetsPreset>,
    #[arg(long = "min-level")]
    pub min_level: Option<u32>,
    #[arg(long = "max-level")]
    pub max_level: Option<u32>,
    #[arg(long = "min-ff")]
    pub min_ff: Option<f64>,
    #[arg(long = "max-ff")]
    pub max_ff: Option<f64>,
    #[arg(long)]
    pub limit: Option<u32>,
    #[arg(
        long = "inactive-only",
        help = "Filter to players inactive for 14+ days"
    )]
    pub inactive_only: bool,
    #[arg(long = "include-active", help = "Send inactiveonly=0")]
    pub include_active: bool,
    #[arg(long = "factionless")]
    pub factionless: bool,
    #[arg(long)]
    pub no_auth: bool,
    #[arg(short = 'p', long = "param", value_name = "NAME=VALUE")]
    pub extra_params: Vec<QueryParam>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum FfTargetsPreset {
    Respect,
    Level,
}

impl FfTargetsPreset {
    fn as_str(self) -> &'static str {
        match self {
            Self::Respect => "respect",
            Self::Level => "level",
        }
    }
}

#[derive(Debug, Args)]
pub struct FfActivityCommand {
    #[command(subcommand)]
    pub command: FfActivitySubcommand,
}

#[derive(Debug, Subcommand)]
pub enum FfActivitySubcommand {
    Player(FfActivityPlayerArgs),
    Faction(FfActivityFactionArgs),
}

#[derive(Debug, Clone, Args)]
pub struct FfActivityPlayerArgs {
    #[arg(
        long = "target",
        alias = "user",
        alias = "user-id",
        value_name = "ID",
        required = true
    )]
    pub target: String,
    #[command(flatten)]
    pub window: FfActivityWindowArgs,
}

#[derive(Debug, Clone, Args)]
pub struct FfActivityFactionArgs {
    #[arg(
        long = "faction",
        alias = "faction-id",
        value_name = "ID",
        required = true
    )]
    pub faction_id: String,
    #[command(flatten)]
    pub window: FfActivityWindowArgs,
}

#[derive(Debug, Clone, Args)]
pub struct FfActivityWindowArgs {
    #[arg(
        long = "since",
        alias = "start",
        value_name = "TS|DURATION",
        default_value = "24h"
    )]
    pub since: String,
    #[arg(
        long = "to",
        alias = "end",
        value_name = "TS|DURATION",
        default_value = "now"
    )]
    pub to: String,
    #[arg(
        long,
        default_value_t = 900,
        help = "Bucket seconds: 300, 900, or 3600"
    )]
    pub bucket: u32,
    #[arg(long)]
    pub no_auth: bool,
    #[arg(short = 'p', long = "param", value_name = "NAME=VALUE")]
    pub extra_params: Vec<QueryParam>,
}

#[derive(Debug, Args)]
pub struct FfHitCallingCommand {
    #[command(subcommand)]
    pub command: FfHitCallingSubcommand,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Subcommand)]
pub enum FfHitCallingSubcommand {
    Claims(FfShortcutArgs),
    Claim(FfHitClaimArgs),
    Unclaim(FfHitUnclaimArgs),
    Wipe(FfHitWipeArgs),
}

#[derive(Debug, Clone, Args)]
pub struct FfHitClaimArgs {
    #[arg(
        long = "target",
        alias = "target-player-id",
        value_name = "ID",
        required = true
    )]
    pub target_player_id: String,
    #[arg(
        long,
        help = "Required: confirms placing a live faction hit-calling claim"
    )]
    pub yes: bool,
}

#[derive(Debug, Clone, Args)]
pub struct FfHitUnclaimArgs {
    #[arg(long = "target", alias = "target-player-id", value_name = "ID")]
    pub target_player_id: Option<String>,
    #[arg(long = "claim-id", value_name = "UUID")]
    pub claim_id: Option<String>,
    #[arg(
        long,
        help = "Required: confirms releasing live faction hit-calling claim(s)"
    )]
    pub yes: bool,
}

#[derive(Debug, Clone, Args)]
pub struct FfHitWipeArgs {
    #[arg(
        long,
        help = "Required: confirms releasing every hit-calling claim you placed"
    )]
    pub yes: bool,
}

#[derive(Debug, Args)]
pub struct FfLossesCommand {
    #[command(subcommand)]
    pub command: FfLossesSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum FfLossesSubcommand {
    Quote(FfLossesQuoteArgs),
    SellerContracts(FfShortcutArgs),
    SellerClaims(FfShortcutArgs),
    SellerOrder(FfLossesSellerOrderArgs),
    SellerClaim(FfLossesSellerClaimArgs),
    SellerComplete(FfLossesSellerCompleteArgs),
}

#[derive(Debug, Clone, Args)]
pub struct FfLossesQuoteArgs {
    #[arg(long)]
    pub quantity: u32,
    #[arg(long = "price-per-loss")]
    pub price_per_loss: u64,
    #[arg(short = 'p', long = "param", value_name = "NAME=VALUE")]
    pub extra_params: Vec<QueryParam>,
}

#[derive(Debug, Clone, Args)]
pub struct FfLossesSellerOrderArgs {
    #[arg(long = "order", alias = "order-number", value_name = "ORDER")]
    pub order_number: String,
    #[arg(long)]
    pub no_auth: bool,
    #[arg(short = 'p', long = "param", value_name = "NAME=VALUE")]
    pub extra_params: Vec<QueryParam>,
}

#[derive(Debug, Clone, Args)]
pub struct FfLossesSellerClaimArgs {
    #[arg(long = "order", alias = "order-number", value_name = "ORDER")]
    pub order_number: String,
    #[arg(long)]
    pub slots: Option<u32>,
    #[arg(long, help = "Required: confirms reserving live loss-selling slots")]
    pub yes: bool,
}

#[derive(Debug, Clone, Args)]
pub struct FfLossesSellerCompleteArgs {
    #[arg(long = "claim-id", value_name = "ID")]
    pub claim_id: String,
    #[arg(
        long,
        help = "Required: confirms marking a live loss-selling claim complete"
    )]
    pub yes: bool,
}

#[derive(Debug, Clone, Args)]
pub struct FfRegisterArgs {
    #[arg(
        long = "agree-to-data-policy",
        help = "Required. Confirms you read FFScouter's Data Policy and Terms at https://ffscouter.com/"
    )]
    pub agree_to_data_policy: bool,
    #[arg(long = "signup-source", default_value = "torncli")]
    pub signup_source: String,
}

#[derive(Debug, Args)]
pub struct LogsCommand {
    #[command(subcommand)]
    pub command: LogsSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum LogsSubcommand {
    Fetch(LogsFetchArgs),
    Analyze(LogsAnalyzeArgs),
    Presets(LogsPresetsCommand),
    Catalog(LogsCatalogArgs),
    Types(LogsCatalogArgs),
    Categories(LogsCatalogArgs),
}

#[derive(Debug, Clone, Args)]
pub struct LogsFetchArgs {
    #[arg(
        long = "since",
        alias = "from",
        value_name = "TS|DURATION",
        help = "Lower timestamp bound: unix seconds, RFC3339, YYYY-MM-DD, now, or relative duration like 7d"
    )]
    pub since: Option<String>,
    #[arg(
        long,
        value_name = "TS|DURATION",
        help = "Upper timestamp bound: unix seconds, RFC3339, YYYY-MM-DD, now, or relative duration"
    )]
    pub to: Option<String>,
    #[arg(
        long = "log",
        value_name = "ID[,ID]",
        value_delimiter = ',',
        help = "Filter by one or more Torn log type ids"
    )]
    pub log_ids: Vec<String>,
    #[arg(long = "cat", alias = "category", value_name = "ID")]
    pub category: Option<String>,
    #[arg(long, value_name = "USER_ID")]
    pub target: Option<String>,
    #[arg(long, default_value_t = 100)]
    pub limit: u32,
    #[arg(
        long = "max-pages",
        help = "Maximum log pages to fetch. Defaults to all pages for --since-bounded windows, or 1 page when no lower bound is provided."
    )]
    pub max_pages: Option<usize>,
    #[arg(short = 'p', long = "param", value_name = "NAME=VALUE")]
    pub extra_params: Vec<QueryParam>,
}

impl Default for LogsFetchArgs {
    fn default() -> Self {
        Self {
            since: None,
            to: None,
            log_ids: Vec::new(),
            category: None,
            target: None,
            limit: 100,
            max_pages: None,
            extra_params: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Args)]
pub struct LogsAnalyzeArgs {
    #[command(flatten)]
    pub fetch: LogsFetchArgs,
    #[arg(long = "group-by", value_enum, default_value_t = LogGroupBy::Category)]
    pub group_by: LogGroupBy,
    #[arg(
        long,
        value_name = "TEXT",
        help = "Client-side substring filter over title/category/data/params"
    )]
    pub contains: Vec<String>,
    #[arg(
        long = "data-key",
        value_name = "KEY",
        help = "Keep logs containing this data key; may repeat"
    )]
    pub data_keys: Vec<String>,
    #[arg(
        long = "param-key",
        value_name = "KEY",
        help = "Keep logs containing this params key; may repeat"
    )]
    pub param_keys: Vec<String>,
    #[arg(long, default_value_t = 20)]
    pub top: usize,
    #[arg(
        long = "include-raw",
        help = "Include filtered raw log entries in JSON output"
    )]
    pub include_raw: bool,
}

#[derive(Debug, Args)]
pub struct LogsPresetsCommand {
    #[command(subcommand)]
    pub command: LogsPresetsSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum LogsPresetsSubcommand {
    List {
        #[arg(long, help = "Only show user-created presets")]
        user: bool,
        #[arg(long, help = "Only show built-in presets")]
        built_in: bool,
    },
    Show {
        name: String,
    },
    Run(LogsPresetRunArgs),
    Add(LogsPresetAddArgs),
    Remove {
        name: String,
    },
}

#[derive(Debug, Clone, Args)]
pub struct LogsPresetRunArgs {
    pub name: String,
    #[arg(long = "since", alias = "from", value_name = "TS|DURATION")]
    pub since: Option<String>,
    #[arg(long, value_name = "TS|DURATION")]
    pub to: Option<String>,
    #[arg(long = "log", value_name = "ID[,ID]", value_delimiter = ',')]
    pub log_ids: Vec<String>,
    #[arg(
        long = "cat",
        alias = "category",
        value_name = "ID",
        value_delimiter = ','
    )]
    pub categories: Vec<String>,
    #[arg(long, value_name = "USER_ID")]
    pub target: Option<String>,
    #[arg(long)]
    pub limit: Option<u32>,
    #[arg(long = "max-pages")]
    pub max_pages: Option<usize>,
    #[arg(short = 'p', long = "param", value_name = "NAME=VALUE")]
    pub extra_params: Vec<QueryParam>,
    #[arg(long = "group-by", value_enum)]
    pub group_by: Option<LogGroupBy>,
    #[arg(long, value_name = "TEXT")]
    pub contains: Vec<String>,
    #[arg(long = "data-key", value_name = "KEY")]
    pub data_keys: Vec<String>,
    #[arg(long = "param-key", value_name = "KEY")]
    pub param_keys: Vec<String>,
    #[arg(long)]
    pub top: Option<usize>,
    #[arg(long = "include-raw")]
    pub include_raw: bool,
}

#[derive(Debug, Clone, Args)]
pub struct LogsPresetAddArgs {
    pub name: String,
    #[arg(long, value_name = "TEXT")]
    pub description: Option<String>,
    #[arg(
        long = "cat",
        alias = "category",
        value_name = "ID",
        value_delimiter = ','
    )]
    pub categories: Vec<String>,
    #[arg(long = "log", value_name = "ID[,ID]", value_delimiter = ',')]
    pub log_ids: Vec<String>,
    #[arg(long, value_name = "TEXT")]
    pub contains: Vec<String>,
    #[arg(long = "data-key", value_name = "KEY")]
    pub data_keys: Vec<String>,
    #[arg(long = "param-key", value_name = "KEY")]
    pub param_keys: Vec<String>,
    #[arg(long = "group-by", value_enum)]
    pub group_by: Option<LogGroupBy>,
    #[arg(long = "since", alias = "from", value_name = "TS|DURATION")]
    pub since: Option<String>,
    #[arg(long, value_name = "TS|DURATION")]
    pub to: Option<String>,
    #[arg(long, value_name = "USER_ID")]
    pub target: Option<String>,
    #[arg(long)]
    pub limit: Option<u32>,
    #[arg(long = "max-pages")]
    pub max_pages: Option<usize>,
    #[arg(
        long,
        help = "Replace an existing user preset or intentionally shadow a built-in preset"
    )]
    pub force: bool,
}

#[derive(Debug, Clone, Args)]
pub struct LogsCatalogArgs {
    #[arg(long = "cat", alias = "category", value_name = "ID_OR_TITLE")]
    pub category: Option<String>,
    #[arg(
        long = "no-expand",
        help = "Only fetch top-level categories/types; skip per-category logtype calls"
    )]
    pub no_expand: bool,
}

#[derive(Debug, Args)]
pub struct CacheCommand {
    #[command(subcommand)]
    pub command: CacheSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum CacheSubcommand {
    Status,
    Clear,
    Inspect { key: String },
}

#[derive(Debug, Args)]
pub struct SavedCommand {
    #[command(subcommand)]
    pub command: SavedSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum SavedSubcommand {
    List,
    Add { name: String, request: Vec<String> },
    Run { name: String },
    Remove { name: String },
}

pub async fn run(cli: Cli) -> Result<(), AppError> {
    let config = Config::load(&config_options_from_global(&cli.global)?)?;
    match cli.command {
        Command::Config(command) => handle_config(command, &config).await,
        Command::Endpoints(command) => handle_endpoints(command, &config),
        Command::Api(command) => handle_api(command, &cli.global, config).await,
        Command::Ff(command) => handle_ff(command, &cli.global, config).await,
        Command::Logs(command) => handle_logs(command, &cli.global, config).await,
        Command::Cache(command) => handle_cache(command, &config),
        Command::Saved(command) => handle_saved(command),
        Command::Tui => {
            println!("Use `torn config tui` for the interactive Ratatui config shell.");
            Ok(())
        }
    }
}

async fn handle_config(command: ConfigCommand, config: &Config) -> Result<(), AppError> {
    match command.command {
        ConfigSubcommand::Check { online } => {
            println!("torn_api_key: {}", presence(config.torn.api_key.is_some()));
            println!(
                "ffscouter_api_key: {}",
                presence(config.ffscouter.api_key.is_some())
            );
            println!("torn_base_url: ok ({})", config.torn.base_url);
            println!("ffscouter_base_url: ok ({})", config.ffscouter.base_url);
            println!("cache_dir: {}", config.cache.dir.display());
            println!(
                "endpoint_index: {}",
                config
                    .endpoint_index_path
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| "built-in".to_string())
            );
            if online {
                print_torn_permissions(config).await?;
            }
            Ok(())
        }
        ConfigSubcommand::Path => {
            println!("{}", config.config_path.display());
            Ok(())
        }
        ConfigSubcommand::Show { redacted: _ } => {
            println!("{}", config.redacted_summary());
            Ok(())
        }
        ConfigSubcommand::Set(args) => handle_config_set(args, config),
        ConfigSubcommand::Permissions => print_torn_permissions(config).await,
        ConfigSubcommand::Tui => crate::tui::run_config_tui(config).await,
    }
}

async fn print_torn_permissions(config: &Config) -> Result<(), AppError> {
    let client = ApiClient::new(config.clone())?;
    let index = EndpointIndex::load(config.endpoint_index_path.as_deref())?;
    let context = TornPermissionContext::fetch(&client).await?;
    println!("torn_permissions:");
    for line in capability_lines(&context.key_info, &index) {
        println!("  {line}");
    }
    Ok(())
}

fn handle_config_set(args: ConfigSetArgs, config: &Config) -> Result<(), AppError> {
    let value = if args.remove {
        None
    } else {
        Some(read_secret_value(args.key, args.stdin)?)
    };
    update_config_secret(&config.config_path, args.key.config_secret_key(), value)?;
    println!(
        "saved {} in private config: {}",
        args.key.label(),
        config.config_path.display()
    );
    Ok(())
}

fn read_secret_value(key: ConfigSetKey, from_stdin: bool) -> Result<String, AppError> {
    if from_stdin {
        let mut value = String::new();
        std::io::stdin().read_to_string(&mut value)?;
        let value = value.trim().to_string();
        if value.is_empty() {
            return Err(AppError::InvalidRequest(format!(
                "{} read from stdin was empty",
                key.label()
            )));
        }
        return Ok(value);
    }

    let prompt = format!("{}: ", key.label());
    let value = rpassword::prompt_password(prompt)
        .map_err(|err| AppError::Io(format!("could not read secret from terminal: {err}")))?;
    let value = value.trim().to_string();
    if value.is_empty() {
        return Err(AppError::InvalidRequest(format!(
            "{} cannot be empty; use --remove to clear it",
            key.label()
        )));
    }
    Ok(value)
}

fn handle_endpoints(command: EndpointsCommand, config: &Config) -> Result<(), AppError> {
    let index = EndpointIndex::load(config.endpoint_index_path.as_deref())?;
    let records = match command.command {
        Some(EndpointsSubcommand::Search { query }) => index.search(&query, command.service),
        None => index.list(command.service),
    };
    println!("{}", render_endpoint_records(&records));
    Ok(())
}

async fn handle_api(
    command: ApiCommand,
    global: &GlobalOptions,
    config: Config,
) -> Result<(), AppError> {
    let index = EndpointIndex::load(config.endpoint_index_path.as_deref())?;
    let request = match command.command {
        ApiSubcommand::Get(args) => generic_request(Service::Torn, HttpMethod::Get, args, None)?,
        ApiSubcommand::Post(args) => generic_request(
            Service::Torn,
            HttpMethod::Post,
            args.request,
            request_body(args.body, args.body_file)?,
        )?,
        ApiSubcommand::User(args) => section_request(&index, "user", args)?,
        ApiSubcommand::Faction(args) => section_request(&index, "faction", args)?,
        ApiSubcommand::Torn(args) => section_request(&index, "torn", args)?,
        ApiSubcommand::Market(args) => section_request(&index, "market", args)?,
        ApiSubcommand::Company(args) => section_request(&index, "company", args)?,
        ApiSubcommand::Racing(args) => section_request(&index, "racing", args)?,
        ApiSubcommand::Forum(args) => section_request(&index, "forum", args)?,
        ApiSubcommand::Property(args) => section_request(&index, "property", args)?,
        ApiSubcommand::Key(args) => section_request(&index, "key", args)?,
    };
    execute_and_print(request, global, config).await
}

async fn handle_ff(
    command: FfCommand,
    global: &GlobalOptions,
    config: Config,
) -> Result<(), AppError> {
    let request = match command.command {
        FfSubcommand::Get(args) => {
            generic_request(Service::Ffscouter, HttpMethod::Get, args, None)?
        }
        FfSubcommand::Post(args) => generic_request(
            Service::Ffscouter,
            HttpMethod::Post,
            args.request,
            request_body(args.body, args.body_file)?,
        )?,
        FfSubcommand::CheckKey(args) | FfSubcommand::Status(args) => ff_shortcut_request(
            "/check-key",
            args.params,
            args.extra_params,
            None,
            args.no_auth,
        )?,
        FfSubcommand::Register(args) => ff_register_request(args, &config)?,
        FfSubcommand::Stats(args) => ff_stats_request(args)?,
        FfSubcommand::StatsHistory(args) => ff_stats_history_request(args)?,
        FfSubcommand::Flights(args) => ff_target_get_request("/player-flights", args)?,
        FfSubcommand::Activity(command) => ff_activity_request(command)?,
        FfSubcommand::Hits(command) => ff_hit_calling_request(command)?,
        FfSubcommand::Losses(command) => ff_losses_request(command)?,
        FfSubcommand::Targets(args) => ff_targets_request(args)?,
        FfSubcommand::Announcements(args) => ff_shortcut_request(
            "/announcements",
            args.params,
            args.extra_params,
            None,
            args.no_auth,
        )?,
    };
    execute_and_print(request, global, config).await
}

fn ff_register_request(args: FfRegisterArgs, config: &Config) -> Result<ApiRequest, AppError> {
    if !args.agree_to_data_policy {
        return Err(AppError::InvalidRequest(
            "FFScouter registration requires --agree-to-data-policy after reading https://ffscouter.com/".to_string(),
        ));
    }
    if !args
        .signup_source
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric())
        || !(3..=30).contains(&args.signup_source.len())
    {
        return Err(AppError::InvalidRequest(
            "--signup-source must be 3-30 alphanumeric characters with no spaces".to_string(),
        ));
    }
    let key = config
        .ffscouter
        .api_key
        .as_ref()
        .ok_or(AppError::MissingApiKey {
            service: Service::Ffscouter,
        })?;
    Ok(ApiRequest::post(Service::Ffscouter, "/register")?
        .with_body(json!({
            "key": key.expose_secret(),
            "agree_to_data_policy": true,
            "signup_source": args.signup_source,
        }))
        .without_auth())
}

fn ff_stats_request(args: FfStatsArgs) -> Result<ApiRequest, AppError> {
    let targets = join_required_ids(&args.targets, "--target")?;
    if args.targets.len() > 205 {
        return Err(AppError::InvalidRequest(
            "FFScouter get-stats accepts at most 205 targets per request".to_string(),
        ));
    }
    let mut request = ApiRequest::get(Service::Ffscouter, "/get-stats")?
        .with_param("targets", targets)
        .with_params(args.extra_params);
    if args.no_auth {
        request = request.without_auth();
    }
    Ok(request)
}

fn ff_stats_history_request(args: FfStatsHistoryArgs) -> Result<ApiRequest, AppError> {
    let mut params = vec![QueryParam::new("target", args.target)];
    if let Some(limit) = args.limit {
        params.push(QueryParam::new("limit", limit.to_string()));
    }
    if let Some(since) = args.since {
        params.push(QueryParam::new(
            "from",
            parse_timestamp_arg(&since, Utc::now())?.to_string(),
        ));
    }
    if let Some(to) = args.to {
        params.push(QueryParam::new(
            "to",
            parse_timestamp_arg(&to, Utc::now())?.to_string(),
        ));
    }
    if let Some(sort) = args.sort {
        params.push(QueryParam::new("sort", sort.as_str()));
    }
    params.extend(args.extra_params);
    let mut request =
        ApiRequest::get(Service::Ffscouter, "/get-stats-history")?.with_params(params);
    if args.no_auth {
        request = request.without_auth();
    }
    Ok(request)
}

fn ff_target_get_request(path: &str, args: FfTargetArgs) -> Result<ApiRequest, AppError> {
    let mut request = ApiRequest::get(Service::Ffscouter, path)?
        .with_param("target", args.target)
        .with_params(args.extra_params);
    if args.no_auth {
        request = request.without_auth();
    }
    Ok(request)
}

fn ff_targets_request(args: FfTargetsArgs) -> Result<ApiRequest, AppError> {
    if args.inactive_only && args.include_active {
        return Err(AppError::InvalidRequest(
            "use only one of --inactive-only or --include-active".to_string(),
        ));
    }
    if args.preset.is_some()
        && (args.min_level.is_some()
            || args.max_level.is_some()
            || args.min_ff.is_some()
            || args.max_ff.is_some()
            || args.inactive_only
            || args.include_active
            || args.factionless)
    {
        return Err(AppError::InvalidRequest(
            "FFScouter presets only allow --preset and --limit; remove custom filters".to_string(),
        ));
    }

    let mut params = Vec::new();
    if let Some(preset) = args.preset {
        params.push(QueryParam::new("preset", preset.as_str()));
    }
    if let Some(value) = args.min_level {
        params.push(QueryParam::new("minlevel", value.to_string()));
    }
    if let Some(value) = args.max_level {
        params.push(QueryParam::new("maxlevel", value.to_string()));
    }
    if let Some(value) = args.min_ff {
        params.push(QueryParam::new("minff", value.to_string()));
    }
    if let Some(value) = args.max_ff {
        params.push(QueryParam::new("maxff", value.to_string()));
    }
    if let Some(value) = args.limit {
        params.push(QueryParam::new("limit", value.to_string()));
    }
    if args.inactive_only {
        params.push(QueryParam::new("inactiveonly", "1"));
    }
    if args.include_active {
        params.push(QueryParam::new("inactiveonly", "0"));
    }
    if args.factionless {
        params.push(QueryParam::new("factionless", "1"));
    }
    params.extend(args.extra_params);
    let mut request = ApiRequest::get(Service::Ffscouter, "/get-targets")?.with_params(params);
    if args.no_auth {
        request = request.without_auth();
    }
    Ok(request)
}

fn ff_activity_request(command: FfActivityCommand) -> Result<ApiRequest, AppError> {
    match command.command {
        FfActivitySubcommand::Player(args) => {
            ff_activity_get_request("/activity/player", "target", args.target, args.window)
        }
        FfActivitySubcommand::Faction(args) => ff_activity_get_request(
            "/activity/faction",
            "faction_id",
            args.faction_id,
            args.window,
        ),
    }
}

fn ff_activity_get_request(
    path: &str,
    subject_param: &str,
    subject_id: String,
    args: FfActivityWindowArgs,
) -> Result<ApiRequest, AppError> {
    if !matches!(args.bucket, 300 | 900 | 3600) {
        return Err(AppError::InvalidRequest(
            "FFScouter activity --bucket must be 300, 900, or 3600 seconds".to_string(),
        ));
    }
    let start = parse_timestamp_arg(&args.since, Utc::now())?;
    let end = parse_timestamp_arg(&args.to, Utc::now())?;
    if start >= end {
        return Err(AppError::InvalidRequest(
            "FFScouter activity --since/start must be before --to/end".to_string(),
        ));
    }
    let mut params = vec![
        QueryParam::new(subject_param, subject_id),
        QueryParam::new("start", start.to_string()),
        QueryParam::new("end", end.to_string()),
        QueryParam::new("bucket", args.bucket.to_string()),
    ];
    params.extend(args.extra_params);
    let mut request = ApiRequest::get(Service::Ffscouter, path)?.with_params(params);
    if args.no_auth {
        request = request.without_auth();
    }
    Ok(request)
}

fn ff_hit_calling_request(command: FfHitCallingCommand) -> Result<ApiRequest, AppError> {
    match command.command {
        FfHitCallingSubcommand::Claims(args) => ff_shortcut_request(
            "/hit-calling/claims",
            args.params,
            args.extra_params,
            None,
            args.no_auth,
        ),
        FfHitCallingSubcommand::Claim(args) => {
            if !args.yes {
                return Err(AppError::InvalidRequest(
                    "hit-calling claim places a live faction claim; pass --yes to confirm"
                        .to_string(),
                ));
            }
            Ok(
                ApiRequest::post(Service::Ffscouter, "/hit-calling/claim")?.with_body(json!({
                    "target_player_id": parse_positive_u64(&args.target_player_id, "--target")?
                })),
            )
        }
        FfHitCallingSubcommand::Unclaim(args) => {
            if !args.yes {
                return Err(AppError::InvalidRequest(
                    "hit-calling unclaim releases live faction claim(s); pass --yes to confirm"
                        .to_string(),
                ));
            }
            if args.target_player_id.is_none() && args.claim_id.is_none() {
                return Err(AppError::InvalidRequest(
                    "pass --target <id>, --claim-id <uuid>, or both".to_string(),
                ));
            }
            let mut body = serde_json::Map::new();
            if let Some(target) = args.target_player_id {
                body.insert(
                    "target_player_id".to_string(),
                    json!(parse_positive_u64(&target, "--target")?),
                );
            }
            if let Some(claim_id) = args.claim_id {
                body.insert("claim_id".to_string(), json!(claim_id));
            }
            Ok(
                ApiRequest::post(Service::Ffscouter, "/hit-calling/unclaim")?
                    .with_body(Value::Object(body)),
            )
        }
        FfHitCallingSubcommand::Wipe(args) => {
            if !args.yes {
                return Err(AppError::InvalidRequest(
                    "hit-calling wipe releases every claim you placed; pass --yes to confirm"
                        .to_string(),
                ));
            }
            Ok(ApiRequest::post(Service::Ffscouter, "/hit-calling/wipe")?)
        }
    }
}

fn ff_losses_request(command: FfLossesCommand) -> Result<ApiRequest, AppError> {
    match command.command {
        FfLossesSubcommand::Quote(args) => {
            let mut params = vec![
                QueryParam::new("quantity", args.quantity.to_string()),
                QueryParam::new("price_per_loss", args.price_per_loss.to_string()),
            ];
            params.extend(args.extra_params);
            Ok(ApiRequest::get(Service::Ffscouter, "/losses/orders/quote")?
                .with_params(params)
                .without_auth())
        }
        FfLossesSubcommand::SellerContracts(args) => ff_shortcut_request(
            "/losses/seller/contracts",
            args.params,
            args.extra_params,
            None,
            args.no_auth,
        ),
        FfLossesSubcommand::SellerClaims(args) => ff_shortcut_request(
            "/losses/seller/claims",
            args.params,
            args.extra_params,
            None,
            args.no_auth,
        ),
        FfLossesSubcommand::SellerOrder(args) => {
            let mut request = ApiRequest::get(
                Service::Ffscouter,
                format!("/losses/seller/orders/{}", args.order_number),
            )?
            .with_params(args.extra_params);
            if args.no_auth {
                request = request.without_auth();
            }
            Ok(request)
        }
        FfLossesSubcommand::SellerClaim(args) => {
            if !args.yes {
                return Err(AppError::InvalidRequest(
                    "losses seller-claim reserves live selling slots; pass --yes to confirm"
                        .to_string(),
                ));
            }
            let mut body = serde_json::Map::new();
            body.insert("order_number".to_string(), json!(args.order_number));
            if let Some(slots) = args.slots {
                body.insert("slots".to_string(), json!(slots));
            }
            Ok(
                ApiRequest::post(Service::Ffscouter, "/losses/seller/claim")?
                    .with_body(Value::Object(body)),
            )
        }
        FfLossesSubcommand::SellerComplete(args) => {
            if !args.yes {
                return Err(AppError::InvalidRequest(
                    "losses seller-complete marks a live claim complete; pass --yes to confirm"
                        .to_string(),
                ));
            }
            Ok(ApiRequest::post(
                Service::Ffscouter,
                format!("/losses/seller/claims/{}/complete", args.claim_id),
            )?)
        }
    }
}

fn parse_positive_u64(value: &str, label: &str) -> Result<u64, AppError> {
    let parsed = value
        .trim()
        .parse::<u64>()
        .map_err(|_| AppError::InvalidRequest(format!("{label} must be a positive integer id")))?;
    if parsed == 0 {
        return Err(AppError::InvalidRequest(format!(
            "{label} must be a positive integer id"
        )));
    }
    Ok(parsed)
}

fn join_required_ids(values: &[String], label: &str) -> Result<String, AppError> {
    let ids = values
        .iter()
        .flat_map(|value| value.split(','))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    if ids.is_empty() {
        return Err(AppError::InvalidRequest(format!(
            "at least one {label} value is required"
        )));
    }
    if ids
        .iter()
        .any(|value| value.parse::<u64>().map_or(true, |id| id == 0))
    {
        return Err(AppError::InvalidRequest(format!(
            "{label} values must be positive integer Torn player ids"
        )));
    }
    Ok(ids.join(","))
}

async fn handle_logs(
    command: LogsCommand,
    global: &GlobalOptions,
    config: Config,
) -> Result<(), AppError> {
    let index = EndpointIndex::load(config.endpoint_index_path.as_deref())?;
    let client = ApiClient::new(config.clone())?;
    let mode = output_mode_from_global(global)?;
    match command.command {
        LogsSubcommand::Fetch(args) => {
            let spec = logs_fetch_spec(args);
            preflight_torn_request(&client, &index, &crate::logs::user_logs_request(&spec)?)
                .await?;
            let entries = fetch_user_logs(&client, &spec).await?;
            println!("{}", render_log_entries(&entries, mode)?);
            Ok(())
        }
        LogsSubcommand::Analyze(args) => {
            let spec = logs_analyze_spec(args);
            preflight_torn_request(
                &client,
                &index,
                &crate::logs::user_logs_request(&spec.fetch)?,
            )
            .await?;
            let analysis = analyze_user_logs(&client, &spec).await?;
            println!("{}", render_analysis(&analysis, mode)?);
            Ok(())
        }
        LogsSubcommand::Presets(command) => {
            handle_log_presets(command, mode, &client, &index, &config).await
        }
        LogsSubcommand::Catalog(args) => {
            let spec = logs_catalog_spec(args, true);
            preflight_logs_catalog(&client, &index).await?;
            let catalog = fetch_log_catalog(&client, &spec).await?;
            println!("{}", render_catalog(&catalog, mode)?);
            Ok(())
        }
        LogsSubcommand::Types(args) => {
            let spec = logs_catalog_spec(args, false);
            preflight_logs_catalog(&client, &index).await?;
            let catalog = fetch_log_catalog(&client, &spec).await?;
            println!("{}", render_catalog_types(&catalog, mode)?);
            Ok(())
        }
        LogsSubcommand::Categories(args) => {
            let spec = logs_catalog_spec(args, false);
            preflight_logs_catalog(&client, &index).await?;
            let catalog = fetch_log_catalog(&client, &spec).await?;
            println!("{}", render_catalog_categories(&catalog, mode)?);
            Ok(())
        }
    }
}

async fn handle_log_presets(
    command: LogsPresetsCommand,
    mode: OutputMode,
    client: &ApiClient,
    index: &EndpointIndex,
    config: &Config,
) -> Result<(), AppError> {
    match command.command {
        LogsPresetsSubcommand::List { user, built_in } => {
            let mut presets = combined_log_presets(&config.logs.presets);
            if user {
                presets.retain(|preset| preset.source == "user");
            }
            if built_in {
                presets.retain(|preset| preset.source == "built-in");
            }
            let summaries = presets
                .into_iter()
                .map(|preset| preset.summary())
                .collect::<Vec<_>>();
            println!("{}", render_preset_summaries(&summaries, mode)?);
            Ok(())
        }
        LogsPresetsSubcommand::Show { name } => {
            let preset = resolve_log_preset(&config.logs.presets, &name).ok_or_else(|| {
                AppError::InvalidRequest(format!(
                    "unknown log preset `{name}`; run `torn logs presets list`"
                ))
            })?;
            println!("{}", render_named_preset(&preset, mode)?);
            Ok(())
        }
        LogsPresetsSubcommand::Run(args) => {
            let preset = resolve_log_preset(&config.logs.presets, &args.name).ok_or_else(|| {
                AppError::InvalidRequest(format!(
                    "unknown log preset `{}`; run `torn logs presets list`",
                    args.name
                ))
            })?;
            let spec = logs_preset_analyze_spec(preset, args);
            let requests = preset_fetch_specs(&spec)
                .into_iter()
                .map(|fetch| crate::logs::user_logs_request(&fetch))
                .collect::<Result<Vec<_>, _>>()?;
            preflight_torn_requests(client, index, &requests).await?;
            let analysis = analyze_user_logs_with_preset(client, &spec).await?;
            println!("{}", render_analysis(&analysis, mode)?);
            Ok(())
        }
        LogsPresetsSubcommand::Add(args) => {
            let name = validate_preset_name(&args.name)?;
            if !args.force && config.logs.presets.contains_key(&name) {
                return Err(AppError::InvalidRequest(format!(
                    "user preset `{name}` already exists; pass --force to replace it"
                )));
            }
            if !args.force && builtin_log_presets().contains_key(&name) {
                return Err(AppError::InvalidRequest(format!(
                    "`{name}` is a built-in preset; pass --force to shadow it with a user preset"
                )));
            }
            let preset = log_preset_definition_from_add_args(args).normalized();
            upsert_log_preset(&config.config_path, &name, preset)?;
            println!(
                "saved log preset `{name}` in private config: {}",
                config.config_path.display()
            );
            Ok(())
        }
        LogsPresetsSubcommand::Remove { name } => {
            let name = validate_preset_name(&name)?;
            if remove_log_preset(&config.config_path, &name)? {
                println!("removed user log preset `{name}`");
                Ok(())
            } else if builtin_log_presets().contains_key(&name) {
                Err(AppError::InvalidRequest(format!(
                    "`{name}` is built in and cannot be removed; create a user preset with the same name to shadow it"
                )))
            } else {
                Err(AppError::InvalidRequest(format!(
                    "user log preset `{name}` does not exist"
                )))
            }
        }
    }
}

fn logs_preset_analyze_spec(
    preset: crate::log_presets::NamedLogPreset,
    args: LogsPresetRunArgs,
) -> LogsPresetAnalyzeSpec {
    let categories = merge_vecs(preset.definition.categories.clone(), args.categories);
    let log_ids = merge_vecs(preset.definition.log_ids.clone(), args.log_ids);
    let contains = merge_vecs(preset.definition.contains.clone(), args.contains);
    let data_keys = merge_vecs(preset.definition.data_keys.clone(), args.data_keys);
    let param_keys = merge_vecs(preset.definition.param_keys.clone(), args.param_keys);
    LogsPresetAnalyzeSpec {
        name: preset.name,
        source: preset.source,
        fetch: LogsFetchSpec {
            since: args.since.or(preset.definition.since.clone()),
            to: args.to.or(preset.definition.to.clone()),
            log_ids,
            category: None,
            target: args.target.or(preset.definition.target.clone()),
            limit: args.limit.or(preset.definition.limit).unwrap_or(100),
            max_pages: args.max_pages.or(preset.definition.max_pages),
            extra_params: args.extra_params,
        },
        categories,
        group_by: args
            .group_by
            .or(preset.definition.group_by)
            .unwrap_or(LogGroupBy::Category),
        contains,
        data_keys,
        param_keys,
        top: args.top.unwrap_or(20),
        include_raw: args.include_raw,
        preset: preset.definition,
    }
}

fn log_preset_definition_from_add_args(args: LogsPresetAddArgs) -> LogPresetDefinition {
    LogPresetDefinition {
        description: args.description,
        categories: args.categories,
        log_ids: args.log_ids,
        contains: args.contains,
        data_keys: args.data_keys,
        param_keys: args.param_keys,
        group_by: args.group_by,
        since: args.since,
        to: args.to,
        target: args.target,
        limit: args.limit,
        max_pages: args.max_pages,
    }
}

fn merge_vecs(mut base: Vec<String>, extra: Vec<String>) -> Vec<String> {
    base.extend(extra);
    base.retain(|value| !value.trim().is_empty());
    base.iter_mut()
        .for_each(|value| *value = value.trim().to_string());
    base.sort_by(
        |left, right| match (left.parse::<u64>(), right.parse::<u64>()) {
            (Ok(left), Ok(right)) => left.cmp(&right),
            _ => left.cmp(right),
        },
    );
    base.dedup();
    base
}

fn render_preset_summaries(
    summaries: &[LogPresetSummary],
    mode: OutputMode,
) -> Result<String, AppError> {
    match mode {
        OutputMode::JsonPretty | OutputMode::Raw => {
            serde_json::to_string_pretty(summaries).map_err(AppError::from)
        }
        OutputMode::JsonCompact => serde_json::to_string(summaries).map_err(AppError::from),
        OutputMode::Csv => Ok(render_preset_summaries_csv(summaries)),
        OutputMode::Auto | OutputMode::Table => Ok(render_preset_summaries_table(summaries)),
    }
}

fn render_named_preset(
    preset: &crate::log_presets::NamedLogPreset,
    mode: OutputMode,
) -> Result<String, AppError> {
    match mode {
        OutputMode::JsonPretty | OutputMode::Raw => {
            serde_json::to_string_pretty(preset).map_err(AppError::from)
        }
        OutputMode::JsonCompact => serde_json::to_string(preset).map_err(AppError::from),
        OutputMode::Csv => Ok(render_preset_summaries_csv(&[preset.summary()])),
        OutputMode::Auto | OutputMode::Table => Ok(render_named_preset_table(preset)),
    }
}

fn render_preset_summaries_table(summaries: &[LogPresetSummary]) -> String {
    let mut lines = vec![
        "name\tsource\tcategories\tlog_ids\tgroup_by\tsince\tlimit\tmax_pages\tdescription"
            .to_string(),
    ];
    lines.extend(summaries.iter().map(|preset| {
        format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            preset.name,
            preset.source,
            preset.categories,
            preset.log_ids,
            preset
                .group_by
                .map(|value| value.label().to_string())
                .unwrap_or_default(),
            preset.since.clone().unwrap_or_default(),
            preset
                .limit
                .map(|value| value.to_string())
                .unwrap_or_default(),
            preset
                .max_pages
                .map(|value| value.to_string())
                .unwrap_or_default(),
            preset.description
        )
    }));
    lines.join("\n")
}

fn render_named_preset_table(preset: &crate::log_presets::NamedLogPreset) -> String {
    let definition = &preset.definition;
    [
        format!("name\t{}", preset.name),
        format!("source\t{}", preset.source),
        format!("description\t{}", definition.short_description()),
        format!("categories\t{}", definition.categories.join(",")),
        format!("log_ids\t{}", definition.log_ids.join(",")),
        format!("contains\t{}", definition.contains.join("|")),
        format!("data_keys\t{}", definition.data_keys.join(",")),
        format!("param_keys\t{}", definition.param_keys.join(",")),
        format!(
            "group_by\t{}",
            definition
                .group_by
                .map(|value| value.label().to_string())
                .unwrap_or_default()
        ),
        format!("since\t{}", definition.since.clone().unwrap_or_default()),
        format!("to\t{}", definition.to.clone().unwrap_or_default()),
        format!("target\t{}", definition.target.clone().unwrap_or_default()),
        format!(
            "limit\t{}",
            definition
                .limit
                .map(|value| value.to_string())
                .unwrap_or_default()
        ),
        format!(
            "max_pages\t{}",
            definition
                .max_pages
                .map(|value| value.to_string())
                .unwrap_or_default()
        ),
    ]
    .join("\n")
}

fn render_preset_summaries_csv(summaries: &[LogPresetSummary]) -> String {
    let mut lines = vec![
        "name,source,categories,log_ids,group_by,since,limit,max_pages,description".to_string(),
    ];
    lines.extend(summaries.iter().map(|preset| {
        [
            preset.name.clone(),
            preset.source.clone(),
            preset.categories.to_string(),
            preset.log_ids.to_string(),
            preset
                .group_by
                .map(|value| value.label().to_string())
                .unwrap_or_default(),
            preset.since.clone().unwrap_or_default(),
            preset
                .limit
                .map(|value| value.to_string())
                .unwrap_or_default(),
            preset
                .max_pages
                .map(|value| value.to_string())
                .unwrap_or_default(),
            preset.description.clone(),
        ]
        .into_iter()
        .map(csv_escape_cli)
        .collect::<Vec<_>>()
        .join(",")
    }));
    lines.join("\n")
}

fn csv_escape_cli(value: String) -> String {
    if value
        .chars()
        .any(|ch| matches!(ch, ',' | '"' | '\n' | '\r'))
    {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value
    }
}

async fn preflight_logs_catalog(client: &ApiClient, index: &EndpointIndex) -> Result<(), AppError> {
    let requests = vec![
        ApiRequest::get(Service::Torn, "/torn/logcategories")?,
        ApiRequest::get(Service::Torn, "/torn/logtypes")?,
    ];
    preflight_torn_requests(client, index, &requests).await
}

fn logs_fetch_spec(args: LogsFetchArgs) -> LogsFetchSpec {
    LogsFetchSpec {
        since: args.since,
        to: args.to,
        log_ids: args.log_ids,
        category: args.category,
        target: args.target,
        limit: args.limit,
        max_pages: args.max_pages,
        extra_params: args.extra_params,
    }
}

fn logs_analyze_spec(args: LogsAnalyzeArgs) -> LogsAnalyzeSpec {
    LogsAnalyzeSpec {
        fetch: logs_fetch_spec(args.fetch),
        group_by: args.group_by,
        contains: args.contains,
        data_keys: args.data_keys,
        param_keys: args.param_keys,
        top: args.top,
        include_raw: args.include_raw,
    }
}

fn logs_catalog_spec(args: LogsCatalogArgs, default_expand: bool) -> LogsCatalogSpec {
    LogsCatalogSpec {
        category: args.category,
        expand_categories: default_expand && !args.no_expand,
    }
}

fn section_request(
    index: &EndpointIndex,
    group: &str,
    args: SectionRequestArgs,
) -> Result<ApiRequest, AppError> {
    let mut query = args.params.to_query_params();
    query.extend(args.extra_params.clone());
    let prefer_path_param = args.id.is_some() || !args.path_params.is_empty();
    let path = match args.selection.as_deref() {
        None => format!("/{group}"),
        Some(selection) => match index.find_torn(group, Some(selection), prefer_path_param) {
            Some(endpoint) => materialize_endpoint_path(endpoint, &args)?,
            None => {
                if !contains_query_param(&query, "selections") {
                    query.insert(0, QueryParam::new("selections", selection));
                }
                format!("/{group}")
            }
        },
    };
    Ok(ApiRequest::get(Service::Torn, path)?.with_params(query))
}

fn materialize_endpoint_path(
    endpoint: &EndpointRecord,
    args: &SectionRequestArgs,
) -> Result<String, AppError> {
    if endpoint.path_params.is_empty() {
        return Ok(endpoint.path.clone());
    }
    let named = args
        .path_params
        .iter()
        .map(|param| (param.name.as_str(), param.value.as_str()))
        .collect::<BTreeMap<_, _>>();
    let mut path = endpoint.path.clone();
    for (idx, name) in endpoint.path_params.iter().enumerate() {
        let value = named
            .get(name.as_str())
            .copied()
            .or(if idx == 0 { args.id.as_deref() } else { None })
            .ok_or_else(|| {
                AppError::InvalidRequest(format!(
                    "{} requires path parameter {{{}}}; pass --id <value> or --path-param {}=<value>",
                    endpoint.command, name, name
                ))
            })?;
        path = path.replace(&format!("{{{name}}}"), value);
    }
    Ok(path)
}

fn ff_shortcut_request(
    path: &str,
    params: CommonParams,
    extra_params: Vec<QueryParam>,
    user_id: Option<String>,
    no_auth: bool,
) -> Result<ApiRequest, AppError> {
    let mut query = params.to_query_params();
    query.extend(extra_params);
    let mut request = ApiRequest::get(Service::Ffscouter, path)?.with_params(query);
    if let Some(user_id) = user_id {
        if !request.query.iter().any(|param| param.name == "user_id") {
            request = request.with_param("user_id", user_id);
        }
    }
    if no_auth {
        request = request.without_auth();
    }
    Ok(request)
}

async fn execute_and_print(
    request: ApiRequest,
    global: &GlobalOptions,
    config: Config,
) -> Result<(), AppError> {
    let watch_interval = watch_interval_from_global(global)?;
    let paginate = pagination_options_from_global(global)?;
    if watch_interval.is_some() && paginate.is_some() {
        return Err(AppError::InvalidRequest(
            "use either --watch or --all-pages, not both".to_string(),
        ));
    }
    let cache_policy = if watch_interval.is_some() {
        watch_cache_policy_from_global(global)?
    } else {
        cache_policy_from_global(global)?
    };
    let request = request.with_cache_policy(cache_policy);
    let index = EndpointIndex::load(config.endpoint_index_path.as_deref())?;
    let client = ApiClient::new(config)?;
    preflight_torn_request(&client, &index, &request).await?;
    let mut mode = output_mode_from_global(global)?;
    if watch_interval.is_some() && matches!(mode, OutputMode::Auto) {
        mode = OutputMode::Table;
    }

    if let Some(interval) = watch_interval {
        execute_watch_and_print(&client, request, mode, interval).await
    } else {
        let response = if let Some(options) = paginate {
            let responses = client.execute_pages(request.clone(), options).await?;
            merge_paginated_responses(responses)?
        } else {
            client.execute(request.clone()).await?
        };
        let rendered = if global.pretty && std::io::stdout().is_terminal() {
            render_response_for_request_colored(&request, &response, mode)?
        } else {
            render_response_for_request(&request, &response, mode)?
        };
        println!("{rendered}");
        Ok(())
    }
}

fn pagination_options_from_global(
    global: &GlobalOptions,
) -> Result<Option<PaginationOptions>, AppError> {
    if let Some(limit) = global.page_limit {
        if limit == 0 {
            return Err(AppError::InvalidRequest(
                "--page-limit must be greater than zero".to_string(),
            ));
        }
    }
    if global.all_pages || global.page_limit.is_some() {
        Ok(Some(PaginationOptions {
            max_pages: Some(global.page_limit.unwrap_or(DEFAULT_API_PAGE_LIMIT)),
        }))
    } else {
        Ok(None)
    }
}

fn merge_paginated_responses(responses: Vec<ApiResponse>) -> Result<ApiResponse, AppError> {
    let mut iter = responses.into_iter();
    let Some(first) = iter.next() else {
        return Err(AppError::Output(
            "paginated request returned no responses".to_string(),
        ));
    };
    let mut merged = first;
    let mut pages_fetched = 1usize;
    let mut json_pages = Vec::new();
    let Some(first_json) = merged.body_json.take() else {
        return Ok(merged);
    };
    json_pages.push(first_json);
    for response in iter {
        pages_fetched += 1;
        if let Some(json) = response.body_json {
            json_pages.push(json);
        }
        merged.elapsed_ms += response.elapsed_ms;
        merged.from_cache &= response.from_cache;
    }

    let body_json = merge_json_pages(json_pages, pages_fetched);
    merged.body_text = serde_json::to_string(&body_json)?;
    merged.body_json = Some(body_json);
    Ok(merged)
}

fn merge_json_pages(pages: Vec<Value>, pages_fetched: usize) -> Value {
    if pages.len() == 1 {
        let mut only = pages.into_iter().next().unwrap_or(Value::Null);
        annotate_pagination_metadata(&mut only, pages_fetched);
        return only;
    }

    if pages.iter().all(Value::is_array) {
        let mut merged = Vec::new();
        for page in pages {
            if let Some(items) = page.as_array() {
                append_json_array_dedup_by_id(&mut merged, items);
            }
        }
        let mut object = Map::new();
        object.insert("items".to_string(), Value::Array(merged));
        object.insert(
            "_metadata".to_string(),
            pagination_metadata_value(pages_fetched),
        );
        return Value::Object(object);
    }

    if let Some(merged_object) = try_merge_object_pages(&pages, pages_fetched) {
        return Value::Object(merged_object);
    }

    let mut object = Map::new();
    object.insert("pages".to_string(), Value::Array(pages));
    object.insert(
        "_metadata".to_string(),
        pagination_metadata_value(pages_fetched),
    );
    Value::Object(object)
}

fn try_merge_object_pages(pages: &[Value], pages_fetched: usize) -> Option<Map<String, Value>> {
    let mut merged = Map::new();
    let mut saw_mergeable_array = false;
    for page in pages {
        let object = page.as_object()?;
        for (key, value) in object {
            if key == "_metadata" {
                continue;
            }
            match (merged.get_mut(key), value) {
                (Some(Value::Array(existing)), Value::Array(items)) => {
                    append_json_array_dedup_by_id(existing, items);
                    saw_mergeable_array = true;
                }
                (None, Value::Array(items)) => {
                    let mut cloned = Vec::new();
                    append_json_array_dedup_by_id(&mut cloned, items);
                    merged.insert(key.clone(), Value::Array(cloned));
                    saw_mergeable_array = true;
                }
                (None, other) => {
                    merged.insert(key.clone(), other.clone());
                }
                (Some(existing), other) if existing == other => {}
                _ => return None,
            }
        }
    }
    if !saw_mergeable_array {
        return None;
    }
    merged.insert(
        "_metadata".to_string(),
        pagination_metadata_value(pages_fetched),
    );
    Some(merged)
}

fn append_json_array_dedup_by_id(existing: &mut Vec<Value>, items: &[Value]) {
    let mut seen_ids = existing
        .iter()
        .filter_map(json_item_id)
        .collect::<HashSet<_>>();
    for item in items {
        if let Some(id) = json_item_id(item) {
            if seen_ids.insert(id) {
                existing.push(item.clone());
            }
        } else {
            existing.push(item.clone());
        }
    }
}

fn json_item_id(value: &Value) -> Option<String> {
    let id = value.as_object()?.get("id")?;
    match id {
        Value::String(id) => Some(id.clone()),
        Value::Number(id) => Some(id.to_string()),
        other => Some(other.to_string()),
    }
}

fn annotate_pagination_metadata(value: &mut Value, pages_fetched: usize) {
    if let Value::Object(object) = value {
        let metadata = object
            .entry("_metadata".to_string())
            .or_insert_with(|| Value::Object(Map::new()));
        if let Value::Object(metadata_object) = metadata {
            metadata_object.insert(
                "pagination".to_string(),
                json!({ "pages_fetched": pages_fetched }),
            );
        }
    }
}

fn pagination_metadata_value(pages_fetched: usize) -> Value {
    json!({ "pagination": { "pages_fetched": pages_fetched } })
}

async fn execute_watch_and_print(
    client: &ApiClient,
    request: ApiRequest,
    mode: OutputMode,
    interval: Duration,
) -> Result<(), AppError> {
    if !matches!(request.method, HttpMethod::Get) {
        return Err(AppError::InvalidRequest(
            "--watch is only supported for GET requests".to_string(),
        ));
    }

    loop {
        let response = client.execute(request.clone()).await?;
        let rendered = render_response_for_request_colored(&request, &response, mode)?;
        println!("{}", prefix_watch_output(&rendered));
        std::io::stdout().flush()?;
        tokio::time::sleep(interval).await;
    }
}

fn prefix_watch_output(rendered: &str) -> String {
    let prefix = watch_prefix();
    rendered
        .lines()
        .map(|line| format!("{prefix} {line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn watch_prefix() -> String {
    let timestamp = Local::now().format("%H:%M:%S");
    format!("{}", style(format!("[{timestamp}]")).with(Color::DarkCyan))
}

async fn preflight_torn_request(
    client: &ApiClient,
    index: &EndpointIndex,
    request: &ApiRequest,
) -> Result<(), AppError> {
    preflight_torn_requests(client, index, std::slice::from_ref(request)).await
}

async fn preflight_torn_requests(
    client: &ApiClient,
    index: &EndpointIndex,
    requests: &[ApiRequest],
) -> Result<(), AppError> {
    let mut context = None;
    for request in requests {
        if should_preflight_request(request) {
            if context.is_none() {
                context = Some(TornPermissionContext::fetch(client).await?);
            }
            if let Some(context) = &context {
                context.ensure_request_allowed(index, request)?;
            }
        }
    }
    Ok(())
}

fn generic_request(
    service: Service,
    method: HttpMethod,
    args: GenericRequestArgs,
    body: Option<Value>,
) -> Result<ApiRequest, AppError> {
    let mut explicit = args.common.to_query_params();
    explicit.extend(args.params);
    let mut request = ApiRequest::new(service, method, args.path)?.with_params(explicit);
    if args.no_auth {
        request = request.without_auth();
    }
    if let Some(body) = body {
        request = request.with_body(body);
    }
    Ok(request)
}

fn request_body(
    body: Option<String>,
    body_file: Option<PathBuf>,
) -> Result<Option<Value>, AppError> {
    if let Some(body) = body {
        return serde_json::from_str(&body)
            .map(Some)
            .map_err(AppError::from);
    }
    if let Some(body_file) = body_file {
        let text = std::fs::read_to_string(body_file)?;
        return serde_json::from_str(&text)
            .map(Some)
            .map_err(AppError::from);
    }
    Ok(None)
}

fn handle_cache(command: CacheCommand, config: &Config) -> Result<(), AppError> {
    match command.command {
        CacheSubcommand::Status => {
            println!("cache.enabled = {}", config.cache.enabled);
            println!(
                "cache.default_ttl_seconds = {}",
                config.cache.default_ttl.as_secs()
            );
            println!("cache.dir = {}", config.cache.dir.display());
            Ok(())
        }
        CacheSubcommand::Clear => Err(AppError::InvalidRequest(
            "persistent cache store is not implemented yet".to_string(),
        )),
        CacheSubcommand::Inspect { key } => Err(AppError::InvalidRequest(format!(
            "persistent cache inspect is not implemented yet for key {key}"
        ))),
    }
}

fn handle_saved(command: SavedCommand) -> Result<(), AppError> {
    match command.command {
        SavedSubcommand::List => {
            println!("saved request storage is not implemented yet");
            Ok(())
        }
        SavedSubcommand::Add { .. }
        | SavedSubcommand::Run { .. }
        | SavedSubcommand::Remove { .. } => Err(AppError::InvalidRequest(
            "saved request storage is not implemented yet".to_string(),
        )),
    }
}

fn config_options_from_global(global: &GlobalOptions) -> Result<ConfigLoadOptions, AppError> {
    Ok(ConfigLoadOptions {
        config_path: global.config.clone(),
        env_file: global.env_file.clone(),
        no_env: global.no_env,
        current_dir: std::env::current_dir()?,
        overrides: ConfigOverrides {
            torn_api_key: global.torn_api_key.clone(),
            ffscouter_api_key: global.ffscouter_api_key.clone(),
            torn_base_url: global.torn_base_url.clone(),
            ffscouter_base_url: global.ffscouter_base_url.clone(),
            cache_dir: global.cache_dir.clone(),
            endpoint_index_path: global.endpoint_index_path.clone(),
        },
        process_env: None,
    })
}

fn cache_policy_from_global(global: &GlobalOptions) -> Result<CachePolicy, AppError> {
    if global.no_cache && global.fresh {
        return Err(AppError::InvalidRequest(
            "--no-cache and --fresh cannot be used together".to_string(),
        ));
    }
    if global.no_cache {
        return Ok(CachePolicy::Disabled);
    }
    if global.fresh {
        return Ok(CachePolicy::Fresh);
    }
    if let Some(ttl) = &global.cache_ttl {
        return Ok(CachePolicy::Ttl(parse_duration(ttl)?));
    }
    Ok(CachePolicy::Default)
}

fn watch_cache_policy_from_global(global: &GlobalOptions) -> Result<CachePolicy, AppError> {
    if global.no_cache && global.fresh {
        return Err(AppError::InvalidRequest(
            "--no-cache and --fresh cannot be used together".to_string(),
        ));
    }
    if global.no_cache {
        Ok(CachePolicy::Disabled)
    } else {
        Ok(CachePolicy::Fresh)
    }
}

fn watch_interval_from_global(global: &GlobalOptions) -> Result<Option<Duration>, AppError> {
    let Some(interval) = &global.watch else {
        return Ok(None);
    };
    let interval = parse_duration(interval)?;
    if interval.is_zero() {
        return Err(AppError::InvalidRequest(
            "--watch interval must be greater than zero".to_string(),
        ));
    }
    Ok(Some(interval))
}

fn output_mode_from_global(global: &GlobalOptions) -> Result<OutputMode, AppError> {
    let selected = [
        global.json,
        global.pretty,
        global.raw,
        global.table,
        global.csv,
    ]
    .into_iter()
    .filter(|selected| *selected)
    .count();
    if selected > 1 {
        return Err(AppError::InvalidRequest(
            "choose only one output mode among --json, --pretty, --raw, --table, --csv".to_string(),
        ));
    }
    if global.pretty {
        Ok(OutputMode::JsonPretty)
    } else if global.raw {
        Ok(OutputMode::Raw)
    } else if global.table {
        Ok(OutputMode::Table)
    } else if global.csv {
        Ok(OutputMode::Csv)
    } else if global.json {
        Ok(OutputMode::JsonCompact)
    } else {
        Ok(OutputMode::Auto)
    }
}

fn parse_duration(input: &str) -> Result<Duration, AppError> {
    humantime::parse_duration(input)
        .map_err(|err| AppError::InvalidRequest(format!("invalid duration '{input}': {err}")))
}

fn presence(present: bool) -> &'static str {
    if present { "present" } else { "missing" }
}

fn contains_query_param(params: &[QueryParam], name: &str) -> bool {
    params
        .iter()
        .any(|param| param.name.eq_ignore_ascii_case(name))
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    #[test]
    fn parses_dynamic_torn_shortcut() {
        let cli = Cli::try_parse_from([
            "torn",
            "api",
            "faction",
            "rankedwarreport",
            "--id",
            "1",
            "--param",
            "striptags=false",
        ])
        .unwrap();
        match cli.command {
            Command::Api(ApiCommand {
                command: ApiSubcommand::Faction(args),
            }) => {
                assert_eq!(args.selection.as_deref(), Some("rankedwarreport"));
                assert_eq!(args.id.as_deref(), Some("1"));
                assert_eq!(
                    args.extra_params,
                    vec![QueryParam::new("striptags", "false")]
                );
            }
            _ => panic!("wrong command"),
        }
    }

    #[test]
    fn rejects_multiple_output_modes_at_parse_time() {
        let err = Cli::try_parse_from(["torn", "api", "get", "/user/basic", "--json", "--pretty"])
            .unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
    }

    #[test]
    fn parses_watch_with_optional_interval() {
        let cli = Cli::try_parse_from([
            "torn", "--watch", "5s", "--pretty", "api", "user", "basic", "--id", "123",
        ])
        .unwrap();
        assert_eq!(cli.global.watch.as_deref(), Some("5s"));
        assert!(cli.global.pretty);
    }

    #[test]
    fn parses_all_pages_with_page_limit() {
        let cli = Cli::try_parse_from([
            "torn",
            "--all-pages",
            "--page-limit",
            "25",
            "api",
            "faction",
            "attacks",
        ])
        .unwrap();
        assert!(cli.global.all_pages);
        assert_eq!(cli.global.page_limit, Some(25));
        assert_eq!(
            pagination_options_from_global(&cli.global)
                .unwrap()
                .unwrap()
                .max_pages,
            Some(25)
        );
    }

    #[test]
    fn page_limit_implies_all_pages() {
        let cli =
            Cli::try_parse_from(["torn", "--page-limit", "3", "api", "user", "events"]).unwrap();
        assert!(!cli.global.all_pages);
        assert_eq!(
            pagination_options_from_global(&cli.global)
                .unwrap()
                .unwrap()
                .max_pages,
            Some(3)
        );
    }

    #[test]
    fn merges_paginated_top_level_arrays() {
        let first = ApiResponse {
            service: Service::Torn,
            status: 200,
            body_text: String::new(),
            body_json: Some(json!({
                "attacks": [{"id": 1}],
                "_metadata": {"links": {"next": "https://api.torn.com/v2/faction/attacks?from=2"}}
            })),
            from_cache: true,
            elapsed_ms: 10,
        };
        let second = ApiResponse {
            service: Service::Torn,
            status: 200,
            body_text: String::new(),
            body_json: Some(json!({
                "attacks": [{"id": 1}, {"id": 2}],
                "_metadata": {"links": {"next": null}}
            })),
            from_cache: true,
            elapsed_ms: 20,
        };

        let merged = merge_paginated_responses(vec![first, second]).unwrap();
        let body = merged.body_json.unwrap();
        assert_eq!(body.pointer("/attacks/0/id"), Some(&json!(1)));
        assert_eq!(body.pointer("/attacks/1/id"), Some(&json!(2)));
        assert_eq!(
            body.get("attacks").and_then(Value::as_array).unwrap().len(),
            2
        );
        assert_eq!(
            body.pointer("/_metadata/pagination/pages_fetched"),
            Some(&json!(2))
        );
        assert_eq!(merged.elapsed_ms, 30);
        assert!(merged.from_cache);
    }

    #[test]
    fn unknown_selection_falls_back_to_generic_selections() {
        let index = EndpointIndex::load(None).unwrap();
        let req = section_request(
            &index,
            "user",
            SectionRequestArgs {
                selection: Some("future".to_string()),
                id: None,
                path_params: Vec::new(),
                extra_params: Vec::new(),
                params: CommonParams::default(),
            },
        )
        .unwrap();
        assert_eq!(req.path, "/user");
        assert_eq!(req.query, vec![QueryParam::new("selections", "future")]);
    }

    #[test]
    fn id_shortcut_materializes_path_param() {
        let index = EndpointIndex::load(None).unwrap();
        let req = section_request(
            &index,
            "faction",
            SectionRequestArgs {
                selection: Some("basic".to_string()),
                id: Some("123".to_string()),
                path_params: Vec::new(),
                extra_params: Vec::new(),
                params: CommonParams::default(),
            },
        )
        .unwrap();
        assert_eq!(req.path, "/faction/123/basic");
    }

    #[test]
    fn parses_config_set_key() {
        let cli =
            Cli::try_parse_from(["torn", "config", "set", "torn-api-key", "--stdin"]).unwrap();
        match cli.command {
            Command::Config(ConfigCommand {
                command: ConfigSubcommand::Set(args),
            }) => {
                assert_eq!(args.key, ConfigSetKey::TornApiKey);
                assert!(args.stdin);
            }
            _ => panic!("wrong command"),
        }
    }

    #[test]
    fn parses_config_permissions() {
        let cli = Cli::try_parse_from(["torn", "config", "permissions"]).unwrap();
        match cli.command {
            Command::Config(ConfigCommand {
                command: ConfigSubcommand::Permissions,
            }) => {}
            _ => panic!("wrong command"),
        }
    }

    #[test]
    fn ff_stats_shortcut_uses_targets_parameter() {
        let request = ff_stats_request(FfStatsArgs {
            targets: vec!["123".to_string(), "456".to_string()],
            no_auth: false,
            extra_params: Vec::new(),
        })
        .unwrap();
        assert_eq!(request.path, "/get-stats");
        assert_eq!(request.query, vec![QueryParam::new("targets", "123,456")]);
    }

    #[test]
    fn ff_activity_rejects_invalid_bucket() {
        let error = ff_activity_get_request(
            "/activity/player",
            "target",
            "123".to_string(),
            FfActivityWindowArgs {
                since: "24h".to_string(),
                to: "now".to_string(),
                bucket: 600,
                no_auth: false,
                extra_params: Vec::new(),
            },
        )
        .unwrap_err();
        assert!(error.to_string().contains("300, 900, or 3600"));
    }

    #[test]
    fn ff_mutating_shortcuts_require_confirmation() {
        let error = ff_hit_calling_request(FfHitCallingCommand {
            command: FfHitCallingSubcommand::Claim(FfHitClaimArgs {
                target_player_id: "123".to_string(),
                yes: false,
            }),
        })
        .unwrap_err();
        assert!(error.to_string().contains("pass --yes"));
    }

    #[test]
    fn parses_ff_activity_player() {
        let cli = Cli::try_parse_from([
            "torn", "ff", "activity", "player", "--target", "123", "--since", "12h", "--bucket",
            "3600",
        ])
        .unwrap();
        match cli.command {
            Command::Ff(FfCommand {
                command:
                    FfSubcommand::Activity(FfActivityCommand {
                        command: FfActivitySubcommand::Player(args),
                    }),
            }) => {
                assert_eq!(args.target, "123");
                assert_eq!(args.window.since, "12h");
                assert_eq!(args.window.bucket, 3600);
            }
            _ => panic!("wrong command"),
        }
    }

    #[test]
    fn parses_logs_preset_run() {
        let cli = Cli::try_parse_from([
            "torn",
            "logs",
            "presets",
            "run",
            "security",
            "--since",
            "30d",
            "--group-by",
            "type",
            "--cat",
            "2,221",
        ])
        .unwrap();
        match cli.command {
            Command::Logs(LogsCommand {
                command:
                    LogsSubcommand::Presets(LogsPresetsCommand {
                        command: LogsPresetsSubcommand::Run(args),
                    }),
            }) => {
                assert_eq!(args.name, "security");
                assert_eq!(args.since.as_deref(), Some("30d"));
                assert_eq!(args.group_by, Some(LogGroupBy::Type));
                assert_eq!(args.categories, vec!["2", "221"]);
            }
            _ => panic!("wrong command"),
        }
    }

    #[test]
    fn parses_logs_analyze_since_grouping() {
        let cli = Cli::try_parse_from([
            "torn",
            "logs",
            "analyze",
            "--since",
            "7d",
            "--to",
            "now",
            "--group-by",
            "type",
            "--log",
            "105,4900",
            "--data-key",
            "item",
        ])
        .unwrap();
        match cli.command {
            Command::Logs(LogsCommand {
                command: LogsSubcommand::Analyze(args),
            }) => {
                assert_eq!(args.fetch.since.as_deref(), Some("7d"));
                assert_eq!(args.group_by, LogGroupBy::Type);
                assert_eq!(args.fetch.log_ids, vec!["105", "4900"]);
                assert_eq!(args.data_keys, vec!["item"]);
            }
            _ => panic!("wrong command"),
        }
    }
}
