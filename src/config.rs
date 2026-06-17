use std::{
    collections::{BTreeMap, HashMap},
    fmt,
    path::Path,
    path::PathBuf,
    time::Duration,
};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{error::AppError, log_presets::LogPresetDefinition, redaction::redact_secret};

pub const DEFAULT_TORN_BASE_URL: &str = "https://api.torn.com/v2";
pub const DEFAULT_FFSCOUTER_BASE_URL: &str = "https://ffscouter.com/api/v1";
pub const DEFAULT_CACHE_TTL_SECONDS: u64 = 30;

#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    pub fn new(value: impl Into<String>) -> Option<Self> {
        let value = value.into();
        if value.trim().is_empty() {
            None
        } else {
            Some(Self(value))
        }
    }

    pub fn expose_secret(&self) -> &str {
        &self.0
    }

    pub fn redacted(&self) -> String {
        redact_secret(&self.0)
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Secret").field(&"<redacted>").finish()
    }
}

#[derive(Debug, Clone)]
pub struct ServiceConfig {
    pub base_url: Url,
    pub api_key: Option<Secret>,
}

#[derive(Debug, Clone)]
pub struct CacheConfig {
    pub enabled: bool,
    pub default_ttl: Duration,
    pub dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct LogsConfig {
    pub presets: BTreeMap<String, LogPresetDefinition>,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub torn: ServiceConfig,
    pub ffscouter: ServiceConfig,
    pub cache: CacheConfig,
    pub logs: LogsConfig,
    pub config_dir: PathBuf,
    pub config_path: PathBuf,
    pub endpoint_index_path: Option<PathBuf>,
    pub user_agent: String,
}

#[derive(Debug, Clone, Default)]
pub struct ConfigOverrides {
    pub torn_api_key: Option<String>,
    pub ffscouter_api_key: Option<String>,
    pub torn_base_url: Option<String>,
    pub ffscouter_base_url: Option<String>,
    pub cache_dir: Option<PathBuf>,
    pub endpoint_index_path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct ConfigLoadOptions {
    pub config_path: Option<PathBuf>,
    pub env_file: Option<PathBuf>,
    pub no_env: bool,
    pub current_dir: PathBuf,
    pub overrides: ConfigOverrides,
    pub process_env: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigSecretKey {
    TornApiKey,
    FfscouterApiKey,
}

impl Default for ConfigLoadOptions {
    fn default() -> Self {
        Self {
            config_path: None,
            env_file: None,
            no_env: false,
            current_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            overrides: ConfigOverrides::default(),
            process_env: None,
        }
    }
}

impl Config {
    pub fn load(options: &ConfigLoadOptions) -> Result<Self, AppError> {
        let project_dirs = ProjectDirs::from("dev", "torn-cli", "torn-cli");
        let default_config_dir = project_dirs
            .as_ref()
            .map(|dirs| dirs.config_dir().to_path_buf())
            .unwrap_or_else(|| options.current_dir.join(".torn-cli"));
        let default_cache_dir = project_dirs
            .as_ref()
            .map(|dirs| dirs.cache_dir().to_path_buf())
            .unwrap_or_else(|| options.current_dir.join(".torn-cli/cache"));
        let config_path = options
            .config_path
            .clone()
            .unwrap_or_else(|| default_config_dir.join("config.toml"));

        let mut config = Self {
            torn: ServiceConfig {
                base_url: Url::parse(DEFAULT_TORN_BASE_URL)?,
                api_key: None,
            },
            ffscouter: ServiceConfig {
                base_url: Url::parse(DEFAULT_FFSCOUTER_BASE_URL)?,
                api_key: None,
            },
            cache: CacheConfig {
                enabled: true,
                default_ttl: Duration::from_secs(DEFAULT_CACHE_TTL_SECONDS),
                dir: default_cache_dir,
            },
            logs: LogsConfig {
                presets: BTreeMap::new(),
            },
            config_dir: default_config_dir,
            config_path,
            endpoint_index_path: None,
            user_agent: default_user_agent(),
        };

        if config.config_path.exists() {
            let file_config = std::fs::read_to_string(&config.config_path)?;
            let file_config = toml::from_str::<FileConfig>(&file_config)
                .map_err(|err| AppError::Config(format!("invalid config file: {err}")))?;
            config.apply_file_config(file_config)?;
        }

        if !options.no_env {
            let dotenv_path = options.current_dir.join(".env");
            if dotenv_path.exists() {
                config.apply_env_map(&read_env_file(&dotenv_path)?)?;
            }
        }

        if let Some(env_file) = &options.env_file {
            config.apply_env_map(&read_env_file(env_file)?)?;
        }

        let process_env = options
            .process_env
            .clone()
            .unwrap_or_else(|| std::env::vars().collect());
        config.apply_env_map(&process_env)?;
        config.apply_overrides(&options.overrides)?;

        Ok(config)
    }

    pub fn secret_values(&self) -> Vec<String> {
        [self.torn.api_key.as_ref(), self.ffscouter.api_key.as_ref()]
            .into_iter()
            .flatten()
            .map(|secret| secret.expose_secret().to_string())
            .collect()
    }

    pub fn redacted_summary(&self) -> String {
        format!(
            "torn.base_url = {}\ntorn.api_key = {}\nffscouter.base_url = {}\nffscouter.api_key = {}\ncache.enabled = {}\ncache.default_ttl_seconds = {}\ncache.dir = {}\nlogs.user_presets = {}\nendpoint_index_path = {}",
            self.torn.base_url,
            self.torn
                .api_key
                .as_ref()
                .map(|_| "<redacted>".to_string())
                .unwrap_or_else(|| "<missing>".to_string()),
            self.ffscouter.base_url,
            self.ffscouter
                .api_key
                .as_ref()
                .map(|_| "<redacted>".to_string())
                .unwrap_or_else(|| "<missing>".to_string()),
            self.cache.enabled,
            self.cache.default_ttl.as_secs(),
            self.cache.dir.display(),
            self.logs.presets.len(),
            self.endpoint_index_path
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "<built-in>".to_string()),
        )
    }

    fn apply_file_config(&mut self, file_config: FileConfig) -> Result<(), AppError> {
        if let Some(torn) = file_config.torn {
            if let Some(base_url) = torn.base_url {
                self.torn.base_url = Url::parse(&base_url)?;
            }
            if let Some(api_key) = torn.api_key {
                self.torn.api_key = Secret::new(api_key);
            }
        }
        if let Some(ffscouter) = file_config.ffscouter {
            if let Some(base_url) = ffscouter.base_url {
                self.ffscouter.base_url = Url::parse(&base_url)?;
            }
            if let Some(api_key) = ffscouter.api_key {
                self.ffscouter.api_key = Secret::new(api_key);
            }
        }
        if let Some(cache) = file_config.cache {
            if let Some(enabled) = cache.enabled {
                self.cache.enabled = enabled;
            }
            if let Some(default_ttl_seconds) = cache.default_ttl_seconds {
                self.cache.default_ttl = Duration::from_secs(default_ttl_seconds);
            }
            if let Some(dir) = cache.dir {
                self.cache.dir = dir;
            }
        }
        if let Some(logs) = file_config.logs {
            self.logs.presets = logs
                .presets
                .into_iter()
                .map(|(name, preset)| (name.to_ascii_lowercase(), preset.normalized()))
                .collect();
        }
        if let Some(endpoint_index_path) = file_config.endpoint_index_path {
            self.endpoint_index_path = Some(endpoint_index_path);
        }
        Ok(())
    }

    fn apply_env_map(&mut self, env: &HashMap<String, String>) -> Result<(), AppError> {
        if let Some(value) = env.get("TORN_API_KEY") {
            self.torn.api_key = Secret::new(value.clone());
        }
        if let Some(value) = env.get("FFSCOUTER_API_KEY") {
            self.ffscouter.api_key = Secret::new(value.clone());
        }
        if let Some(value) = env
            .get("TORN_BASE_URL")
            .filter(|value| !value.trim().is_empty())
        {
            self.torn.base_url = Url::parse(value)?;
        }
        if let Some(value) = env
            .get("FFSCOUTER_BASE_URL")
            .filter(|value| !value.trim().is_empty())
        {
            self.ffscouter.base_url = Url::parse(value)?;
        }
        if let Some(value) = env
            .get("TORN_CACHE_DIR")
            .filter(|value| !value.trim().is_empty())
        {
            self.cache.dir = PathBuf::from(value);
        }
        if let Some(value) = env
            .get("TORN_CONFIG_DIR")
            .filter(|value| !value.trim().is_empty())
        {
            self.config_dir = PathBuf::from(value);
            if self
                .config_path
                .file_name()
                .is_some_and(|name| name == "config.toml")
            {
                self.config_path = self.config_dir.join("config.toml");
            }
        }
        if let Some(value) = env
            .get("TORN_API_INDEX_PATH")
            .filter(|value| !value.trim().is_empty())
        {
            self.endpoint_index_path = Some(PathBuf::from(value));
        }
        Ok(())
    }

    fn apply_overrides(&mut self, overrides: &ConfigOverrides) -> Result<(), AppError> {
        if let Some(value) = &overrides.torn_api_key {
            self.torn.api_key = Secret::new(value.clone());
        }
        if let Some(value) = &overrides.ffscouter_api_key {
            self.ffscouter.api_key = Secret::new(value.clone());
        }
        if let Some(value) = &overrides.torn_base_url {
            self.torn.base_url = Url::parse(value)?;
        }
        if let Some(value) = &overrides.ffscouter_base_url {
            self.ffscouter.base_url = Url::parse(value)?;
        }
        if let Some(value) = &overrides.cache_dir {
            self.cache.dir = value.clone();
        }
        if let Some(value) = &overrides.endpoint_index_path {
            self.endpoint_index_path = Some(value.clone());
        }
        Ok(())
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct FileConfig {
    torn: Option<FileServiceConfig>,
    ffscouter: Option<FileServiceConfig>,
    cache: Option<FileCacheConfig>,
    logs: Option<FileLogsConfig>,
    endpoint_index_path: Option<PathBuf>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct FileServiceConfig {
    base_url: Option<String>,
    api_key: Option<String>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct FileCacheConfig {
    enabled: Option<bool>,
    default_ttl_seconds: Option<u64>,
    dir: Option<PathBuf>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct FileLogsConfig {
    #[serde(default)]
    presets: BTreeMap<String, LogPresetDefinition>,
}

pub fn upsert_log_preset(
    path: &Path,
    name: &str,
    preset: LogPresetDefinition,
) -> Result<(), AppError> {
    let mut file_config = read_file_config(path)?;
    file_config
        .logs
        .get_or_insert_with(Default::default)
        .presets
        .insert(name.to_ascii_lowercase(), preset.normalized());
    write_file_config_private(path, &file_config)
}

pub fn remove_log_preset(path: &Path, name: &str) -> Result<bool, AppError> {
    let mut file_config = read_file_config(path)?;
    let removed = file_config
        .logs
        .as_mut()
        .and_then(|logs| logs.presets.remove(&name.to_ascii_lowercase()))
        .is_some();
    write_file_config_private(path, &file_config)?;
    Ok(removed)
}

pub fn update_config_secret(
    path: &Path,
    key: ConfigSecretKey,
    value: Option<String>,
) -> Result<(), AppError> {
    let mut file_config = read_file_config(path)?;
    match key {
        ConfigSecretKey::TornApiKey => {
            file_config
                .torn
                .get_or_insert_with(Default::default)
                .api_key = value;
        }
        ConfigSecretKey::FfscouterApiKey => {
            file_config
                .ffscouter
                .get_or_insert_with(Default::default)
                .api_key = value;
        }
    }
    write_file_config_private(path, &file_config)
}

fn read_file_config(path: &Path) -> Result<FileConfig, AppError> {
    if !path.exists() {
        return Ok(FileConfig::default());
    }
    let text = std::fs::read_to_string(path)?;
    toml::from_str::<FileConfig>(&text)
        .map_err(|err| AppError::Config(format!("invalid config file {}: {err}", path.display())))
}

fn write_file_config_private(path: &Path, file_config: &FileConfig) -> Result<(), AppError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
        set_private_dir_permissions(parent)?;
    }
    let text = toml::to_string_pretty(file_config)
        .map_err(|err| AppError::Config(format!("could not encode config TOML: {err}")))?;
    std::fs::write(path, text)?;
    set_private_file_permissions(path)?;
    Ok(())
}

#[cfg(unix)]
fn set_private_dir_permissions(path: &Path) -> Result<(), AppError> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = std::fs::metadata(path)?.permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(path, permissions)?;
    Ok(())
}

#[cfg(not(unix))]
fn set_private_dir_permissions(_path: &Path) -> Result<(), AppError> {
    Ok(())
}

#[cfg(unix)]
fn set_private_file_permissions(path: &Path) -> Result<(), AppError> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = std::fs::metadata(path)?.permissions();
    permissions.set_mode(0o600);
    std::fs::set_permissions(path, permissions)?;
    Ok(())
}

#[cfg(not(unix))]
fn set_private_file_permissions(_path: &Path) -> Result<(), AppError> {
    Ok(())
}

fn read_env_file(path: &std::path::Path) -> Result<HashMap<String, String>, AppError> {
    let mut values = HashMap::new();
    let iter = dotenvy::from_path_iter(path).map_err(|err| {
        AppError::Config(format!("could not read env file {}: {err}", path.display()))
    })?;
    for item in iter {
        let (key, value) = item.map_err(|err| {
            AppError::Config(format!(
                "could not parse env file {}: {err}",
                path.display()
            ))
        })?;
        values.insert(key, value);
    }
    Ok(values)
}

fn default_user_agent() -> String {
    format!(
        "torn-cli/{} (+{})",
        env!("CARGO_PKG_VERSION"),
        env!("CARGO_PKG_REPOSITORY")
    )
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn config_precedence_is_file_dotenv_envfile_process_cli() {
        let temp = tempdir().unwrap();
        let config_path = temp.path().join("config.toml");
        fs::write(
            &config_path,
            r#"
[torn]
api_key = "file-torn-key"
base_url = "https://file.example/v2"
"#,
        )
        .unwrap();
        fs::write(temp.path().join(".env"), "TORN_API_KEY=dotenv-torn-key\n").unwrap();
        let env_file = temp.path().join("custom.env");
        fs::write(&env_file, "TORN_API_KEY=envfile-torn-key\n").unwrap();
        let mut process_env = HashMap::new();
        process_env.insert("TORN_API_KEY".to_string(), "process-torn-key".to_string());

        let options = ConfigLoadOptions {
            config_path: Some(config_path),
            env_file: Some(env_file),
            no_env: false,
            current_dir: temp.path().to_path_buf(),
            overrides: ConfigOverrides {
                torn_api_key: Some("cli-torn-key".to_string()),
                ..ConfigOverrides::default()
            },
            process_env: Some(process_env),
        };

        let config = Config::load(&options).unwrap();
        assert_eq!(
            config.torn.api_key.as_ref().unwrap().expose_secret(),
            "cli-torn-key"
        );
        assert_eq!(config.torn.base_url.as_str(), "https://file.example/v2");
    }

    #[test]
    fn config_debug_and_summary_redact_secrets() {
        let mut process_env = HashMap::new();
        process_env.insert("TORN_API_KEY".to_string(), "abc123456789".to_string());
        let options = ConfigLoadOptions {
            process_env: Some(process_env),
            no_env: true,
            ..ConfigLoadOptions::default()
        };
        let config = Config::load(&options).unwrap();
        assert!(!format!("{config:?}").contains("abc123456789"));
        assert!(!config.redacted_summary().contains("abc123456789"));
        assert!(config.redacted_summary().contains("<redacted>"));
    }

    #[test]
    fn update_log_preset_round_trips() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("config.toml");
        upsert_log_preset(
            &path,
            "money-watch",
            LogPresetDefinition {
                description: Some("cash".to_string()),
                categories: vec!["13".to_string()],
                since: Some("30d".to_string()),
                ..LogPresetDefinition::default()
            },
        )
        .unwrap();

        let options = ConfigLoadOptions {
            config_path: Some(path.clone()),
            no_env: true,
            current_dir: temp.path().to_path_buf(),
            process_env: Some(HashMap::new()),
            ..ConfigLoadOptions::default()
        };
        let config = Config::load(&options).unwrap();
        assert_eq!(config.logs.presets["money-watch"].categories, vec!["13"]);
        assert!(remove_log_preset(&path, "money-watch").unwrap());
    }

    #[test]
    fn update_config_secret_writes_private_config_file() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("config.toml");
        update_config_secret(
            &path,
            ConfigSecretKey::TornApiKey,
            Some("secret-value".to_string()),
        )
        .unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("[torn]"));
        assert!(text.contains("api_key = \"secret-value\""));

        let options = ConfigLoadOptions {
            config_path: Some(path.clone()),
            no_env: true,
            current_dir: temp.path().to_path_buf(),
            process_env: Some(HashMap::new()),
            ..ConfigLoadOptions::default()
        };
        let config = Config::load(&options).unwrap();
        assert_eq!(
            config.torn.api_key.as_ref().unwrap().expose_secret(),
            "secret-value"
        );

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }
}
