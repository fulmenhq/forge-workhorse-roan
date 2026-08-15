//! Logging, metrics, and error helpers wired through rsfulmen.

use crate::appid::Identity;
use crate::config::LoggingConfig;
use rsfulmen::error_handling::{ErrorResponse, WrapOptions};
use rsfulmen::logging::{self, Logger, LoggerConfig, Profile, Severity};
use rsfulmen::telemetry_metrics::{self, Metrics};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Process-wide observability handles.
#[derive(Clone)]
pub struct Observability {
    /// Structured logger.
    pub logger: Logger,
    /// rsfulmen metrics registry (taxonomy-backed).
    pub registry: Arc<Metrics>,
    /// Identity-prefixed HTTP request counter for Prometheus text.
    http_requests: Arc<AtomicU64>,
    /// Telemetry namespace / metric prefix.
    namespace: String,
}

impl Observability {
    /// Build logging + metrics from identity and config.
    pub fn new(identity: &Identity, logging: &LoggingConfig, verbose: bool) -> Self {
        let mut level = parse_severity(&logging.level);
        if verbose {
            level = Severity::Debug;
        }
        let profile = parse_profile(&logging.profile);
        let logger = logging::new(LoggerConfig {
            name: identity.service_name().to_string(),
            level,
            profile,
        });
        Self {
            logger,
            registry: Arc::new(Metrics::new()),
            http_requests: Arc::new(AtomicU64::new(0)),
            namespace: identity.telemetry_namespace().to_string(),
        }
    }

    /// Record one HTTP request and increment taxonomy exporter counters when present.
    pub fn record_http_request(&self) {
        self.http_requests.fetch_add(1, Ordering::Relaxed);
        if let Ok(counter) = self
            .registry
            .counter("prometheus_exporter_http_requests_total")
        {
            let _ = counter.inc(Some(1));
        }
    }

    /// Prometheus text exposition.
    pub fn prometheus_text(&self) -> String {
        let count = self.http_requests.load(Ordering::Relaxed);
        let prefix = sanitize_metric_prefix(&self.namespace);
        format!(
            "# HELP {prefix}_http_requests_total HTTP requests handled by the workhorse placeholder server\n\
             # TYPE {prefix}_http_requests_total counter\n\
             {prefix}_http_requests_total {count}\n\
             # HELP prometheus_exporter_http_requests_total HTTP requests observed by the Prometheus exporter\n\
             # TYPE prometheus_exporter_http_requests_total counter\n\
             prometheus_exporter_http_requests_total {count}\n"
        )
    }

    /// Wrap an error using rsfulmen's error envelope.
    pub fn wrap_error(&self, code: &str, message: &str) -> ErrorResponse {
        self.wrap_error_correlated(code, message, None)
    }

    /// Wrap an error and attach a request correlation identifier.
    pub fn wrap_error_correlated(
        &self,
        code: &str,
        message: &str,
        correlation_id: Option<&str>,
    ) -> ErrorResponse {
        let correlation_id = correlation_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);
        let mut err = ErrorResponse::wrap(
            ErrorResponse::new(code, message),
            WrapOptions {
                correlation_id,
                ..WrapOptions::default()
            },
        )
        .unwrap_or_else(|_| ErrorResponse::new(code, message));
        let _ = err.set_timestamp_now_utc();
        err
    }
}

/// Parse a log level string.
pub fn parse_severity(value: &str) -> Severity {
    match value.trim().to_ascii_lowercase().as_str() {
        "trace" => Severity::Trace,
        "debug" => Severity::Debug,
        "warn" | "warning" => Severity::Warn,
        "error" => Severity::Error,
        "fatal" => Severity::Fatal,
        _ => Severity::Info,
    }
}

fn parse_profile(value: &str) -> Profile {
    match value.trim().to_ascii_uppercase().as_str() {
        "SIMPLE" => Profile::Simple,
        _ => Profile::Structured,
    }
}

fn sanitize_metric_prefix(namespace: &str) -> String {
    namespace
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect()
}

/// Demonstrate rsfulmen telemetry types are linked.
pub fn telemetry_module_available() -> bool {
    let registry = Metrics::new();
    telemetry_metrics::METRICS_EVENT_SCHEMA_PATH.contains("metrics") && registry.export().is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::appid;

    #[test]
    fn prometheus_text_includes_namespace() {
        let identity = appid::load().unwrap();
        let obs = Observability::new(&identity, &crate::config::LoggingConfig::default(), false);
        obs.record_http_request();
        let text = obs.prometheus_text();
        assert!(text.contains("_http_requests_total"));
        assert!(text.contains("prometheus_exporter_http_requests_total"));
    }
}
