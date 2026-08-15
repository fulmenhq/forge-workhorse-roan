//! Workhorse App Identity loader.
//!
//! `.fulmen/app.yaml` uses the Fulmen workhorse identity schema (`app.binary_name`,
//! `app.vendor`, `app.env_prefix`, `app.config_name`) documented by gofulmen and
//! the live Go workhorse. rsfulmen 0.1.5 ships a flatter `name` schema, so this
//! module parses the workhorse document and uses the same discovery overrides
//! (`FULMEN_APP_IDENTITY_FILE`, plus gofulmen's `FULMEN_APP_IDENTITY_PATH`).
//!
//! Standalone binaries fall back to the embedded identity mirror.

use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const IDENTITY_RELATIVE_PATH: &str = ".fulmen/app.yaml";
const RS_IDENTITY_FILE_ENV: &str = "FULMEN_APP_IDENTITY_FILE";
const GO_IDENTITY_PATH_ENV: &str = "FULMEN_APP_IDENTITY_PATH";
const EMBEDDED_IDENTITY_YAML: &str = include_str!("../assets/appidentity/app.yaml");

static IDENTITY: OnceCell<Identity> = OnceCell::new();

/// Application identity used for binary name, env prefix, and config paths.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Identity {
    /// Executable and telemetry service name.
    pub binary_name: String,
    /// Vendor / organization used in XDG paths.
    pub vendor: String,
    /// Environment variable prefix. Must end with `_`.
    pub env_prefix: String,
    /// Config directory / file stem.
    pub config_name: String,
    /// One-line description shown in CLI help.
    #[serde(default)]
    pub description: String,
    /// Optional identity-declared version.
    #[serde(default)]
    pub version: Option<String>,
    /// Optional metadata block.
    #[serde(default)]
    pub metadata: Metadata,
}

/// Optional identity metadata.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Metadata {
    /// Taxonomy category (`workhorse`, `cli`, …).
    #[serde(default)]
    pub repository_category: Option<String>,
    /// Metrics namespace override. Defaults to `binary_name`.
    #[serde(default)]
    pub telemetry_namespace: Option<String>,
    /// Project URL.
    #[serde(default)]
    pub project_url: Option<String>,
    /// SPDX license identifier.
    #[serde(default)]
    pub license: Option<String>,
}

#[derive(Debug, Deserialize)]
struct IdentityFile {
    app: IdentityFields,
    #[serde(default)]
    metadata: Metadata,
}

#[derive(Debug, Deserialize)]
struct IdentityFields {
    binary_name: String,
    vendor: String,
    env_prefix: String,
    config_name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    version: Option<String>,
}

/// Errors loading or validating app identity.
#[derive(Debug, thiserror::Error)]
pub enum AppIdError {
    /// No identity file and no usable embedded fallback.
    #[error("app identity not found (searched from {search_root})")]
    NotFound {
        /// Directory where discovery started.
        search_root: PathBuf,
    },
    /// YAML could not be parsed.
    #[error("malformed app identity at {path}: {source}")]
    Malformed {
        /// Path that failed to parse.
        path: String,
        /// Parser error.
        #[source]
        source: serde_yaml::Error,
    },
    /// Required fields failed validation.
    #[error("invalid app identity at {path}: {reason}")]
    Invalid {
        /// Path that failed validation.
        path: String,
        /// Human-readable reason.
        reason: String,
    },
    /// Filesystem error.
    #[error("failed to read app identity at {path}: {source}")]
    Io {
        /// Path that failed to read.
        path: String,
        /// IO error.
        #[source]
        source: std::io::Error,
    },
}

/// Load identity with process-level caching.
pub fn get() -> Result<&'static Identity, AppIdError> {
    if let Some(existing) = IDENTITY.get() {
        return Ok(existing);
    }
    let loaded = load()?;
    Ok(IDENTITY.get_or_init(|| loaded))
}

/// Load identity without using the process cache.
pub fn load() -> Result<Identity, AppIdError> {
    if let Some(path) = env_override_path() {
        return load_file(&path);
    }

    let cwd = std::env::current_dir().map_err(|source| AppIdError::Io {
        path: ".".to_string(),
        source,
    })?;

    if let Some(path) = discover_upwards(&cwd) {
        return load_file(&path);
    }

    parse_yaml(EMBEDDED_IDENTITY_YAML, "embedded:app.yaml")
}

/// Load identity from an explicit file.
pub fn load_file(path: &Path) -> Result<Identity, AppIdError> {
    let content = fs::read_to_string(path).map_err(|source| AppIdError::Io {
        path: path.display().to_string(),
        source,
    })?;
    parse_yaml(&content, &path.display().to_string())
}

/// Embedded identity YAML used by standalone binaries.
pub fn embedded_yaml() -> &'static str {
    EMBEDDED_IDENTITY_YAML
}

impl Identity {
    /// Construct `{env_prefix}{key}`.
    pub fn env_var(&self, key: &str) -> String {
        format!("{}{key}", self.env_prefix)
    }

    /// Telemetry / metrics namespace.
    pub fn telemetry_namespace(&self) -> &str {
        self.metadata
            .telemetry_namespace
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or(self.binary_name.as_str())
    }

    /// Service name for logs.
    pub fn service_name(&self) -> &str {
        &self.binary_name
    }

    /// Vendor and config-name pair for config-path helpers.
    pub fn config_params(&self) -> (&str, &str) {
        (&self.vendor, &self.config_name)
    }
}

fn parse_yaml(content: &str, path: &str) -> Result<Identity, AppIdError> {
    let file: IdentityFile =
        serde_yaml::from_str(content).map_err(|source| AppIdError::Malformed {
            path: path.to_string(),
            source,
        })?;

    let identity = Identity {
        binary_name: file.app.binary_name,
        vendor: file.app.vendor,
        env_prefix: file.app.env_prefix,
        config_name: file.app.config_name,
        description: file.app.description,
        version: file.app.version,
        metadata: file.metadata,
    };
    validate(&identity, path)?;
    Ok(identity)
}

fn validate(identity: &Identity, path: &str) -> Result<(), AppIdError> {
    if !is_name(&identity.binary_name) {
        return Err(AppIdError::Invalid {
            path: path.to_string(),
            reason: "app.binary_name must match [a-z][a-z0-9-]*".to_string(),
        });
    }
    if !is_vendor(&identity.vendor) {
        return Err(AppIdError::Invalid {
            path: path.to_string(),
            reason: "app.vendor must match [a-z][a-z0-9_]*".to_string(),
        });
    }
    if !is_name(&identity.config_name) {
        return Err(AppIdError::Invalid {
            path: path.to_string(),
            reason: "app.config_name must match [a-z][a-z0-9-]*".to_string(),
        });
    }
    if !is_env_prefix(&identity.env_prefix) {
        return Err(AppIdError::Invalid {
            path: path.to_string(),
            reason: "app.env_prefix must be uppercase and end with '_'".to_string(),
        });
    }
    Ok(())
}

fn is_name(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first.is_ascii_lowercase()
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn is_vendor(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first.is_ascii_lowercase()
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

fn is_env_prefix(value: &str) -> bool {
    value.ends_with('_')
        && value.len() > 1
        && value
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

fn env_override_path() -> Option<PathBuf> {
    for key in [RS_IDENTITY_FILE_ENV, GO_IDENTITY_PATH_ENV] {
        if let Some(raw) = std::env::var_os(key) {
            if !raw.is_empty() {
                return Some(PathBuf::from(raw));
            }
        }
    }
    None
}

fn discover_upwards(start: &Path) -> Option<PathBuf> {
    let mut current = start.to_path_buf();
    loop {
        let candidate = current.join(IDENTITY_RELATIVE_PATH);
        if candidate.is_file() {
            return Some(candidate);
        }
        if !current.pop() {
            return None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_identity_loads() {
        let identity = parse_yaml(EMBEDDED_IDENTITY_YAML, "embedded").expect("embedded yaml");
        assert!(!identity.binary_name.is_empty());
        assert!(identity.env_prefix.ends_with('_'));
        assert_eq!(
            identity.metadata.repository_category.as_deref(),
            Some("workhorse")
        );
    }

    #[test]
    fn env_var_joins_prefix() {
        let identity = parse_yaml(EMBEDDED_IDENTITY_YAML, "embedded").unwrap();
        assert_eq!(
            identity.env_var("PORT"),
            format!("{}PORT", identity.env_prefix)
        );
    }

    #[test]
    fn rejects_missing_trailing_underscore() {
        let yaml = r#"
app:
  binary_name: myapp
  vendor: fulmen
  env_prefix: MYAPP
  config_name: myapp
"#;
        let err = parse_yaml(yaml, "mem").unwrap_err();
        assert!(matches!(err, AppIdError::Invalid { .. }));
    }
}
