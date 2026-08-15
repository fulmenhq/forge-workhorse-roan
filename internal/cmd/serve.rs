use super::CliError;
use crate::appid::Identity;
use crate::config::{Config, LoadOptions};
use crate::observability::Observability;
use crate::server;
use clap::Args;
use serde_yaml::{Mapping, Value};

/// Flags for `serve`.
#[derive(Debug, Args)]
#[command(after_help = "Examples:\n  \
    serve\n  \
    serve --port 8080 --log-level info\n  \
    serve --host 127.0.0.1 --port 9090 --log-level debug\n\n\
    Signals:\n  \
    SIGTERM/SIGINT  graceful shutdown\n  \
    Ctrl+C twice    force quit (2s window from the signal catalog)\n  \
    SIGHUP          reload config (validate, apply in-process)\n")]
pub struct ServeArgs {
    /// Bind host (overrides config / env).
    #[arg(long)]
    pub host: Option<String>,

    /// Bind port (overrides config / env).
    #[arg(long)]
    pub port: Option<u16>,

    /// Log level: trace, debug, info, warn, error.
    #[arg(long)]
    pub log_level: Option<String>,

    /// Dedicated metrics port (optional).
    #[arg(long)]
    pub metrics_port: Option<u16>,

    /// Dedicated health port (accepted for flag parity; served on the main port).
    #[arg(long)]
    pub health_port: Option<u16>,

    /// Environment prefix override (informational; identity remains authoritative).
    #[arg(long)]
    pub env_prefix: Option<String>,
}

impl ServeArgs {
    /// Layer 3 YAML overlay from CLI flags.
    pub fn as_runtime_overrides(&self) -> Result<Value, CliError> {
        let mut root = Mapping::new();
        let mut server = Mapping::new();
        let mut logging = Mapping::new();
        let mut metrics = Mapping::new();

        if let Some(host) = &self.host {
            server.insert(Value::String("host".into()), Value::String(host.clone()));
        }
        if let Some(port) = self.port {
            server.insert(Value::String("port".into()), Value::Number(port.into()));
        }
        if let Some(level) = &self.log_level {
            logging.insert(Value::String("level".into()), Value::String(level.clone()));
        }
        if let Some(port) = self.metrics_port {
            metrics.insert(Value::String("port".into()), Value::Number(port.into()));
        }
        if self.health_port.is_some() {
            // Health shares the main listener in this template.
        }
        if let Some(prefix) = &self.env_prefix {
            if !prefix.ends_with('_') {
                return Err(CliError::Usage(
                    "env-prefix must end with '_' (example: MYAPI_)".to_string(),
                ));
            }
        }

        if !server.is_empty() {
            root.insert(Value::String("server".into()), Value::Mapping(server));
        }
        if !logging.is_empty() {
            root.insert(Value::String("logging".into()), Value::Mapping(logging));
        }
        if !metrics.is_empty() {
            root.insert(Value::String("metrics".into()), Value::Mapping(metrics));
        }
        Ok(Value::Mapping(root))
    }
}

pub fn run(
    identity: &Identity,
    config: &Config,
    observability: &Observability,
    _args: ServeArgs,
    load_options: LoadOptions,
) -> Result<(), CliError> {
    observability.logger.info(
        "starting server",
        &[
            ("binary", identity.binary_name.as_str()),
            ("host", config.server.host.as_str()),
            ("port", config.server.port.to_string().as_str()),
        ],
    );

    let runtime = tokio::runtime::Runtime::new().map_err(|e| CliError::Server(e.to_string()))?;
    runtime
        .block_on(server::serve(
            identity.clone(),
            config.clone(),
            observability.clone(),
            load_options,
        ))
        .map_err(|e| CliError::Server(e.to_string()))
}
