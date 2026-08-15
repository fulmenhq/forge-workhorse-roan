//! Three-layer configuration for the workhorse.
//!
//! Layer 1: embedded defaults (and optional Crucible defaults via rsfulmen).
//! Layer 2: user file from the Config Path API / `--config`.
//! Layer 3: environment variables and CLI flags.

use crate::appid::Identity;
use rsfulmen::config::{get_app_config_dir, three_layer};
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::path::{Path, PathBuf};
use std::time::Duration;

const EMBEDDED_DEFAULTS: &str = include_str!("../assets/config/defaults.yaml");
const EMBEDDED_SCHEMA: &str = include_str!("../assets/schemas/config.schema.json");

/// Loaded runtime configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Config {
    /// HTTP bind settings.
    #[serde(default)]
    pub server: ServerConfig,
    /// Logging settings.
    #[serde(default)]
    pub logging: LoggingConfig,
    /// Metrics settings.
    #[serde(default)]
    pub metrics: MetricsConfig,
    /// Health endpoint toggle.
    #[serde(default)]
    pub health: HealthConfig,
    /// Debug toggle.
    #[serde(default)]
    pub debug: DebugConfig,
    /// Placeholder worker pool size.
    #[serde(default = "default_workers")]
    pub workers: u32,
    /// Optional `/ui` placeholder.
    #[serde(default)]
    pub enable_ui: bool,
}

/// HTTP server settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Bind host.
    #[serde(default = "default_host")]
    pub host: String,
    /// Bind port.
    #[serde(default = "default_port")]
    pub port: u16,
    /// Read timeout.
    #[serde(default = "default_read_timeout")]
    pub read_timeout: String,
    /// Write timeout.
    #[serde(default = "default_write_timeout")]
    pub write_timeout: String,
    /// Idle timeout.
    #[serde(default = "default_idle_timeout")]
    pub idle_timeout: String,
    /// Graceful shutdown timeout.
    #[serde(default = "default_shutdown_timeout")]
    pub shutdown_timeout: String,
}

/// Logging settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// `trace|debug|info|warn|error`.
    #[serde(default = "default_log_level")]
    pub level: String,
    /// `SIMPLE` or `STRUCTURED`.
    #[serde(default = "default_log_profile")]
    pub profile: String,
}

/// Metrics settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetricsConfig {
    /// Whether `/metrics` is served.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Optional dedicated metrics port. `0` means same as the server port.
    #[serde(default)]
    pub port: u16,
}

/// Health settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HealthConfig {
    /// Whether `/health` is served.
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// Debug settings.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DebugConfig {
    /// Debug mode flag.
    #[serde(default)]
    pub enabled: bool,
}

impl Default for Config {
    fn default() -> Self {
        embedded_defaults()
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            read_timeout: default_read_timeout(),
            write_timeout: default_write_timeout(),
            idle_timeout: default_idle_timeout(),
            shutdown_timeout: default_shutdown_timeout(),
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            profile: default_log_profile(),
        }
    }
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            port: 0,
        }
    }
}

impl Default for HealthConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// Options for loading configuration.
#[derive(Debug, Clone, Default)]
pub struct LoadOptions {
    /// Explicit `--config` path.
    pub config_path: Option<PathBuf>,
    /// CLI / runtime YAML overlay (Layer 3).
    pub runtime_overrides: Option<Value>,
}

/// Result of a layered load, including provenance.
#[derive(Debug, Clone)]
pub struct LoadedConfig {
    /// Merged configuration.
    pub config: Config,
    /// Provenance from rsfulmen's merger.
    pub provenance: three_layer::Provenance,
    /// User file that was read, if any.
    pub user_path: Option<PathBuf>,
}

/// Load three-layer configuration using rsfulmen merge helpers.
pub fn load(identity: &Identity, options: LoadOptions) -> Result<LoadedConfig, ConfigError> {
    let defaults_value: Value =
        serde_yaml::from_str(EMBEDDED_DEFAULTS).map_err(|source| ConfigError::InvalidYaml {
            path: "embedded:defaults.yaml".to_string(),
            message: source.to_string(),
        })?;

    let user_path = resolve_user_path(identity, options.config_path.as_deref());
    let user_value = if let Some(path) = user_path.as_ref() {
        three_layer::load_yaml_file_optional(path).map_err(ConfigError::from_three_layer)?
    } else {
        None
    };

    let mut provenance = three_layer::Provenance::default();
    let mut merged = defaults_value;

    if let Some(user) = user_value {
        merged = overlay(
            merged,
            user,
            three_layer::ConfigLayer::User,
            &mut provenance,
            "",
        );
    }

    let env_overlay = env_overrides(identity);
    if !is_empty_mapping(&env_overlay) {
        merged = overlay(
            merged,
            env_overlay,
            three_layer::ConfigLayer::Runtime,
            &mut provenance,
            "",
        );
    }

    if let Some(runtime) = options.runtime_overrides {
        merged = overlay(
            merged,
            runtime,
            three_layer::ConfigLayer::Runtime,
            &mut provenance,
            "",
        );
    }

    validate_against_embedded_schema(&merged)?;

    let config: Config =
        serde_yaml::from_value(merged).map_err(|source| ConfigError::InvalidYaml {
            path: "merged".to_string(),
            message: source.to_string(),
        })?;

    Ok(LoadedConfig {
        config,
        provenance,
        user_path,
    })
}

/// Standard env var specs derived from App Identity.
pub fn env_var_names(identity: &Identity) -> Vec<(String, &'static str)> {
    vec![
        (identity.env_var("HOST"), "server.host"),
        (identity.env_var("PORT"), "server.port"),
        (identity.env_var("LOG_LEVEL"), "logging.level"),
        (identity.env_var("LOG_PROFILE"), "logging.profile"),
        (identity.env_var("CONFIG_PATH"), "config_path"),
        (identity.env_var("METRICS_PORT"), "metrics.port"),
        (identity.env_var("METRICS_ENABLED"), "metrics.enabled"),
        (identity.env_var("HEALTH_PORT"), "health.port"),
        (identity.env_var("HEALTH_ENABLED"), "health.enabled"),
        (identity.env_var("SERVER_HOST"), "server.host"),
        (identity.env_var("SERVER_PORT"), "server.port"),
        (identity.env_var("LOGGING_LEVEL"), "logging.level"),
        (identity.env_var("LOGGING_PROFILE"), "logging.profile"),
        (identity.env_var("READ_TIMEOUT"), "server.read_timeout"),
        (identity.env_var("WRITE_TIMEOUT"), "server.write_timeout"),
        (identity.env_var("IDLE_TIMEOUT"), "server.idle_timeout"),
        (
            identity.env_var("SHUTDOWN_TIMEOUT"),
            "server.shutdown_timeout",
        ),
        (identity.env_var("ADMIN_TOKEN"), "admin.token"),
    ]
}

/// Parse a duration string such as `30s` or `120s`.
pub fn parse_duration(value: &str) -> Result<Duration, ConfigError> {
    let value = value.trim();
    if let Some(ms) = value.strip_suffix("ms") {
        let n: u64 = ms.parse().map_err(|_| ConfigError::InvalidDuration {
            value: value.to_string(),
        })?;
        return Ok(Duration::from_millis(n));
    }
    if let Some(secs) = value.strip_suffix('s') {
        let n: u64 = secs.parse().map_err(|_| ConfigError::InvalidDuration {
            value: value.to_string(),
        })?;
        return Ok(Duration::from_secs(n));
    }
    if let Some(mins) = value.strip_suffix('m') {
        let n: u64 = mins.parse().map_err(|_| ConfigError::InvalidDuration {
            value: value.to_string(),
        })?;
        return Ok(Duration::from_secs(n.saturating_mul(60)));
    }
    Err(ConfigError::InvalidDuration {
        value: value.to_string(),
    })
}

/// Configuration errors.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// YAML parse failure.
    #[error("invalid yaml at {path}: {message}")]
    InvalidYaml {
        /// Path or layer name.
        path: String,
        /// Parser message.
        message: String,
    },
    /// Schema validation failed.
    #[error("config failed schema validation: {0}")]
    Schema(String),
    /// Duration string was not parseable.
    #[error("invalid duration: {value}")]
    InvalidDuration {
        /// Original value.
        value: String,
    },
}

impl ConfigError {
    fn from_three_layer(err: three_layer::ThreeLayerError) -> Self {
        ConfigError::InvalidYaml {
            path: "user-config".to_string(),
            message: err.to_string(),
        }
    }
}

fn embedded_defaults() -> Config {
    serde_yaml::from_str(EMBEDDED_DEFAULTS).expect("embedded defaults must parse")
}

fn resolve_user_path(identity: &Identity, explicit: Option<&Path>) -> Option<PathBuf> {
    if let Some(path) = explicit {
        return Some(path.to_path_buf());
    }
    if let Ok(path) = std::env::var(identity.env_var("CONFIG_PATH")) {
        if !path.trim().is_empty() {
            return Some(PathBuf::from(path));
        }
    }

    let (_vendor, config_name) = identity.config_params();
    // rsfulmen Config Path API: ~/.config/{app_name}/...
    // Workhorse standard also documents ~/.config/{vendor}/{config_name}/config.yaml.
    let primary = get_app_config_dir(config_name).join("config.yaml");
    if primary.is_file() {
        return Some(primary);
    }
    let vendor_path =
        get_app_config_dir(&format!("{}/{}", identity.vendor, config_name)).join("config.yaml");
    if vendor_path.is_file() {
        return Some(vendor_path);
    }
    Some(primary)
}

fn env_overrides(identity: &Identity) -> Value {
    let mut map = serde_yaml::Mapping::new();
    let mut server = serde_yaml::Mapping::new();
    let mut logging = serde_yaml::Mapping::new();
    let mut metrics = serde_yaml::Mapping::new();
    let mut health = serde_yaml::Mapping::new();

    insert_string(
        &mut server,
        "host",
        &first_env(identity, &["HOST", "SERVER_HOST"]),
    );
    insert_u16(
        &mut server,
        "port",
        &first_env(identity, &["PORT", "SERVER_PORT"]),
    );
    insert_string(
        &mut logging,
        "level",
        &first_env(identity, &["LOG_LEVEL", "LOGGING_LEVEL"]),
    );
    insert_string(
        &mut logging,
        "profile",
        &first_env(identity, &["LOG_PROFILE", "LOGGING_PROFILE"]),
    );
    insert_u16(
        &mut metrics,
        "port",
        &first_env(identity, &["METRICS_PORT"]),
    );
    insert_bool(
        &mut metrics,
        "enabled",
        &first_env(identity, &["METRICS_ENABLED"]),
    );
    insert_string(
        &mut server,
        "read_timeout",
        &first_env(identity, &["READ_TIMEOUT"]),
    );
    insert_string(
        &mut server,
        "write_timeout",
        &first_env(identity, &["WRITE_TIMEOUT"]),
    );
    insert_string(
        &mut server,
        "idle_timeout",
        &first_env(identity, &["IDLE_TIMEOUT"]),
    );
    insert_string(
        &mut server,
        "shutdown_timeout",
        &first_env(identity, &["SHUTDOWN_TIMEOUT"]),
    );
    insert_bool(
        &mut health,
        "enabled",
        &first_env(identity, &["HEALTH_ENABLED"]),
    );

    if !server.is_empty() {
        map.insert(Value::String("server".into()), Value::Mapping(server));
    }
    if !logging.is_empty() {
        map.insert(Value::String("logging".into()), Value::Mapping(logging));
    }
    if !metrics.is_empty() {
        map.insert(Value::String("metrics".into()), Value::Mapping(metrics));
    }
    if !health.is_empty() {
        map.insert(Value::String("health".into()), Value::Mapping(health));
    }
    Value::Mapping(map)
}

fn first_env(identity: &Identity, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Ok(value) = std::env::var(identity.env_var(key)) {
            if !value.is_empty() {
                return Some(value);
            }
        }
    }
    None
}

fn insert_string(map: &mut serde_yaml::Mapping, key: &str, value: &Option<String>) {
    if let Some(value) = value {
        map.insert(Value::String(key.to_string()), Value::String(value.clone()));
    }
}

fn insert_u16(map: &mut serde_yaml::Mapping, key: &str, value: &Option<String>) {
    if let Some(value) = value {
        if let Ok(parsed) = value.parse::<i64>() {
            map.insert(Value::String(key.to_string()), Value::Number(parsed.into()));
        }
    }
}

fn insert_bool(map: &mut serde_yaml::Mapping, key: &str, value: &Option<String>) {
    if let Some(value) = value {
        let parsed = matches!(
            value.to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        );
        map.insert(Value::String(key.to_string()), Value::Bool(parsed));
    }
}

fn overlay(
    base: Value,
    overlay_value: Value,
    layer: three_layer::ConfigLayer,
    provenance: &mut three_layer::Provenance,
    path: &str,
) -> Value {
    // rsfulmen's merge is crate-private; re-apply the same mapping overlay rules.
    match (base, overlay_value) {
        (Value::Mapping(mut base_map), Value::Mapping(overlay_map)) => {
            for (k, v) in overlay_map {
                let child = match k.as_str() {
                    Some(key) if path.is_empty() => format!("/{key}"),
                    Some(key) => format!("{path}/{key}"),
                    None => path.to_string(),
                };
                if v.is_null() {
                    base_map.remove(&k);
                    provenance.paths.insert(child, layer);
                    continue;
                }
                if let Some(existing) = base_map.remove(&k) {
                    let merged = overlay(existing, v, layer, provenance, &child);
                    base_map.insert(k, merged);
                } else {
                    provenance.paths.insert(child, layer);
                    base_map.insert(k, v);
                }
            }
            Value::Mapping(base_map)
        }
        (_, overlay_value) => {
            provenance.paths.insert(path.to_string(), layer);
            overlay_value
        }
    }
}

fn is_empty_mapping(value: &Value) -> bool {
    matches!(value, Value::Mapping(map) if map.is_empty())
}

fn validate_against_embedded_schema(merged: &Value) -> Result<(), ConfigError> {
    let schema: serde_json::Value =
        serde_json::from_str(EMBEDDED_SCHEMA).map_err(|e| ConfigError::Schema(e.to_string()))?;
    let instance = serde_json::to_value(merged).map_err(|e| ConfigError::Schema(e.to_string()))?;
    let compiled = jsonschema::JSONSchema::compile(&schema)
        .map_err(|e| ConfigError::Schema(format!("compile embedded schema: {e}")))?;
    if let Err(errors) = compiled.validate(&instance) {
        let messages: Vec<String> = errors.map(|e| e.to_string()).collect();
        return Err(ConfigError::Schema(messages.join("; ")));
    }
    Ok(())
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}
fn default_port() -> u16 {
    8080
}
fn default_read_timeout() -> String {
    "30s".to_string()
}
fn default_write_timeout() -> String {
    "30s".to_string()
}
fn default_idle_timeout() -> String {
    "120s".to_string()
}
fn default_shutdown_timeout() -> String {
    "10s".to_string()
}
fn default_log_level() -> String {
    "info".to_string()
}
fn default_log_profile() -> String {
    "STRUCTURED".to_string()
}
fn default_true() -> bool {
    true
}
fn default_workers() -> u32 {
    4
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::appid;

    #[test]
    fn embedded_defaults_match_schema_shape() {
        let identity = appid::load().expect("identity");
        let loaded = load(&identity, LoadOptions::default()).expect("load");
        assert_eq!(loaded.config.server.port, 8080);
        assert_eq!(loaded.config.server.host, "127.0.0.1");
        assert!(loaded.config.health.enabled);
    }

    #[test]
    fn parse_duration_seconds() {
        assert_eq!(parse_duration("10s").unwrap(), Duration::from_secs(10));
        assert_eq!(parse_duration("250ms").unwrap(), Duration::from_millis(250));
    }

    #[test]
    fn schema_rejects_invalid_workers() {
        let identity = appid::load().expect("identity");
        let overlay = serde_yaml::from_str("workers: 0").expect("overlay");
        let err = load(
            &identity,
            LoadOptions {
                runtime_overrides: Some(overlay),
                ..LoadOptions::default()
            },
        )
        .expect_err("workers: 0 must fail schema validation");
        assert!(matches!(err, ConfigError::Schema(_)));
    }
}
