//! Placeholder HTTP server: health, version, metrics, and echo.

mod middleware;

use crate::appid::Identity;
use crate::config::{parse_duration, Config, LoadOptions, ServerConfig};
use crate::core;
use crate::observability::Observability;
use crate::{BUILD_COMMIT, BUILD_DATE, BUILD_VERSION};
use axum::extract::{DefaultBodyLimit, Query, Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::middleware as axum_middleware;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use middleware::{request_id, RequestId};
use rsfulmen::crucible;
use rsfulmen::docscribe;
use rsfulmen::error_handling::ErrorResponse;
use rsfulmen::signals::{self, SignalEndpointRequest, SignalManager};
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::watch;

pub use middleware::{resolve_request_id, REQUEST_ID_HEADER};

/// Maximum `/echo` message size (bytes).
const ECHO_MAX_BYTES: usize = 8192;
/// Maximum HTTP body size accepted by the placeholder server.
const BODY_MAX_BYTES: usize = 16 * 1024;

/// Shared HTTP state.
#[derive(Clone)]
pub struct AppState {
    /// App identity.
    pub identity: Arc<Identity>,
    /// Effective config (replaced on a successful SIGHUP reload).
    pub config: Arc<RwLock<Config>>,
    /// Observability handles.
    pub observability: Observability,
    /// Shutdown trigger for `/admin/signal`.
    pub shutdown: watch::Sender<bool>,
    /// Shared secret for `/admin/signal`. Never logged.
    admin_token: Option<Arc<str>>,
    /// Whether `/admin/signal` is mounted.
    serve_admin: bool,
    /// Options used to reload three-layer config on SIGHUP.
    load_options: LoadOptions,
}

impl AppState {
    /// Snapshot the current effective configuration.
    pub fn current_config(&self) -> Config {
        self.config
            .read()
            .unwrap_or_else(|err| err.into_inner())
            .clone()
    }
}

/// Bind policy for the admin route.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminGate {
    /// Whether `/admin/signal` may be mounted (loopback + token present).
    pub serve_admin: bool,
}

/// Bind and serve until shutdown.
pub async fn serve(
    identity: Identity,
    config: Config,
    observability: Observability,
    load_options: LoadOptions,
) -> Result<(), ServerError> {
    let _ = request_deadline(&config.server)?;

    let admin_token = std::env::var(identity.env_var("ADMIN_TOKEN"))
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let gate = admin_gate(&config.server.host, admin_token.is_some())?;

    let addr: SocketAddr = format!("{}:{}", config.server.host, config.server.port)
        .parse::<SocketAddr>()
        .map_err(|e: std::net::AddrParseError| ServerError::Bind(e.to_string()))?;

    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
    let state = AppState {
        identity: Arc::new(identity),
        config: Arc::new(RwLock::new(config.clone())),
        observability: observability.clone(),
        shutdown: shutdown_tx.clone(),
        admin_token: admin_token.map(Arc::<str>::from),
        serve_admin: gate.serve_admin,
        load_options,
    };

    let signals = install_signal_manager(&state, shutdown_tx)?;

    let app = router(state.clone());
    let listener = TcpListener::bind(addr)
        .await
        .map_err(|e| ServerError::Bind(e.to_string()))?;
    let bound = listener
        .local_addr()
        .map_err(|e| ServerError::Bind(e.to_string()))?;

    observability.logger.info(
        "server listening",
        &[
            ("addr", bound.to_string().as_str()),
            (
                "admin_signal",
                if gate.serve_admin {
                    "enabled"
                } else {
                    "disabled"
                },
            ),
        ],
    );

    let shutdown_timeout = parse_duration(&config.server.shutdown_timeout)
        .unwrap_or(std::time::Duration::from_secs(10));

    let server = axum::serve(listener, app).with_graceful_shutdown(async move {
        loop {
            if *shutdown_rx.borrow() {
                break;
            }
            if shutdown_rx.changed().await.is_err() {
                break;
            }
        }
    });

    let result = server.await.map_err(|err| ServerError::Io(err.to_string()));
    signals.stop();
    result?;

    observability.logger.info(
        "server shutdown complete",
        &[("timeout", format!("{shutdown_timeout:?}").as_str())],
    );
    Ok(())
}

/// Wire `rsfulmen::signals` for SIGTERM/SIGINT, catalog double-tap, and SIGHUP reload.
fn install_signal_manager(
    state: &AppState,
    shutdown: watch::Sender<bool>,
) -> Result<SignalManager, ServerError> {
    let manager = SignalManager::new();
    manager.enable_double_tap(signals::DoubleTapConfig::from_catalog());

    let reload_state = state.clone();
    manager.on_reload(move || {
        if let Err(err) = reload_runtime_config(&reload_state) {
            reload_state
                .observability
                .logger
                .error("config reload failed", &[("error", err.as_str())]);
            return Err(err.into());
        }
        Ok(())
    });

    manager.on_shutdown(move || {
        let _ = shutdown.send(true);
        Ok(())
    });

    let listener = manager.clone();
    std::thread::Builder::new()
        .name("signals".to_string())
        .spawn(move || {
            if let Err(err) = listener.listen() {
                eprintln!("signal listener stopped: {err}");
            }
        })
        .map_err(|err| ServerError::Io(err.to_string()))?;

    Ok(manager)
}

/// Re-read three-layer config, validate, and apply in-process (Groningen SIGHUP).
pub fn reload_runtime_config(state: &AppState) -> Result<(), String> {
    state
        .observability
        .logger
        .info("received SIGHUP: attempting config reload", &[]);

    let loaded = crate::config::load(&state.identity, state.load_options.clone())
        .map_err(|err| err.to_string())?;

    let previous = state.current_config();
    let mut next = loaded.config;
    if next.server.host != previous.server.host || next.server.port != previous.server.port {
        state.observability.logger.info(
            "reload ignored listen address change; restart required",
            &[
                ("host", next.server.host.as_str()),
                ("port", next.server.port.to_string().as_str()),
            ],
        );
        next.server.host = previous.server.host;
        next.server.port = previous.server.port;
    }
    if next.logging != previous.logging {
        state.observability.logger.info(
            "reload noted logging change; process logger is unchanged until restart",
            &[
                ("level", next.logging.level.as_str()),
                ("profile", next.logging.profile.as_str()),
            ],
        );
    }

    {
        let mut guard = state.config.write().unwrap_or_else(|err| err.into_inner());
        *guard = next;
    }

    state
        .observability
        .logger
        .info("configuration reloaded successfully", &[]);
    Ok(())
}

/// Decide whether `/admin/signal` may be served, and refuse non-loopback binds
/// that have no shared secret.
pub fn admin_gate(host: &str, has_token: bool) -> Result<AdminGate, ServerError> {
    if !is_loopback_bind(host) && !has_token {
        return Err(ServerError::Bind(format!(
            "refusing to bind {host}: admin token is required for a non-loopback address"
        )));
    }
    // Wildcard / unspecified listeners stay public-facing; keep admin off them
    // even when a token is present.
    Ok(AdminGate {
        serve_admin: has_token && is_loopback_bind(host) && !is_wildcard_bind(host),
    })
}

/// True for loopback hosts (`127.0.0.1`, `::1`, `localhost`).
pub fn is_loopback_bind(host: &str) -> bool {
    let host = strip_brackets(host);
    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }
    host.parse::<IpAddr>()
        .map(|ip| ip.is_loopback())
        .unwrap_or(false)
}

/// True for unspecified / wildcard binds (`0.0.0.0`, `::`).
pub fn is_wildcard_bind(host: &str) -> bool {
    strip_brackets(host)
        .parse::<IpAddr>()
        .map(|ip| ip.is_unspecified())
        .unwrap_or(false)
}

fn strip_brackets(host: &str) -> &str {
    host.trim().trim_start_matches('[').trim_end_matches(']')
}

/// Build the HTTP router (also used by tests).
pub fn router(state: AppState) -> Router {
    let mut app = Router::new()
        .route("/health", get(health))
        .route("/version", get(version))
        .route("/metrics", get(metrics))
        .route("/echo", get(echo_get).post(echo_post))
        .route("/docs", get(docs));
    if state.serve_admin {
        app = app.route("/admin/signal", post(admin_signal));
    }
    app.layer(DefaultBodyLimit::max(BODY_MAX_BYTES))
        .layer(axum_middleware::from_fn_with_state(
            state.clone(),
            io_timeout,
        ))
        .layer(axum_middleware::from_fn(request_id))
        .with_state(state)
}

#[derive(Debug, Serialize)]
struct HealthBody {
    status: &'static str,
    version: String,
}

#[derive(Debug, Serialize)]
struct VersionBody {
    name: String,
    version: String,
    commit: String,
    built: String,
    rustc: String,
    rsfulmen: String,
    crucible: String,
}

#[derive(Debug, Deserialize)]
struct EchoQuery {
    #[serde(default)]
    message: String,
}

#[derive(Debug, Deserialize)]
struct EchoBody {
    #[serde(default)]
    message: String,
}

#[derive(Debug, Serialize)]
struct EchoResponse {
    message: String,
}

async fn health(State(state): State<AppState>) -> impl IntoResponse {
    state.observability.record_http_request();
    if !state.current_config().health.enabled {
        return StatusCode::NOT_FOUND.into_response();
    }
    Json(HealthBody {
        status: "healthy",
        version: BUILD_VERSION.to_string(),
    })
    .into_response()
}

async fn version(State(state): State<AppState>) -> Json<VersionBody> {
    state.observability.record_http_request();
    Json(VersionBody {
        name: state.identity.binary_name.clone(),
        version: BUILD_VERSION.to_string(),
        commit: BUILD_COMMIT.to_string(),
        built: BUILD_DATE.to_string(),
        rustc: rustc_version(),
        rsfulmen: rsfulmen::VERSION.to_string(),
        crucible: crucible::version().to_string(),
    })
}

async fn metrics(State(state): State<AppState>) -> impl IntoResponse {
    state.observability.record_http_request();
    if !state.current_config().metrics.enabled {
        return StatusCode::NOT_FOUND.into_response();
    }
    (
        [(
            http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        state.observability.prometheus_text(),
    )
        .into_response()
}

async fn echo_get(
    State(state): State<AppState>,
    Query(query): Query<EchoQuery>,
) -> impl IntoResponse {
    state.observability.record_http_request();
    match bounded_echo(&query.message) {
        Ok(message) => Json(EchoResponse { message }).into_response(),
        Err(status) => status.into_response(),
    }
}

async fn echo_post(State(state): State<AppState>, Json(body): Json<EchoBody>) -> impl IntoResponse {
    state.observability.record_http_request();
    match bounded_echo(&body.message) {
        Ok(message) => Json(EchoResponse { message }).into_response(),
        Err(status) => status.into_response(),
    }
}

fn bounded_echo(message: &str) -> Result<String, StatusCode> {
    if message.len() > ECHO_MAX_BYTES {
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }
    Ok(core::echo(message))
}

async fn docs(State(state): State<AppState>, request_id: RequestId) -> impl IntoResponse {
    state.observability.record_http_request();
    match docscribe::read_parsed_doc("architecture/fulmen-forge-workhorse-standard.md") {
        Ok(doc) => Json(serde_json::json!({
            "path": doc.path,
            "title": doc.frontmatter.title,
            "status": doc.frontmatter.status,
        }))
        .into_response(),
        Err(err) => json_error(
            &state,
            StatusCode::NOT_FOUND,
            "DOC_NOT_FOUND",
            &err.to_string(),
            request_id.as_str(),
        ),
    }
}

async fn admin_signal(
    State(state): State<AppState>,
    request_id: RequestId,
    headers: HeaderMap,
    Json(request): Json<SignalEndpointRequest>,
) -> impl IntoResponse {
    state.observability.record_http_request();
    let request_log = state
        .observability
        .logger
        .with_fields(&[("request_id", request_id.as_str())]);
    let Some(expected) = state.admin_token.as_deref() else {
        request_log.warn("admin signal rejected", &[("result", "disabled")]);
        return StatusCode::NOT_FOUND.into_response();
    };
    if !authorized(&headers, expected) {
        request_log.warn("admin signal rejected", &[("result", "unauthorized")]);
        return json_error(
            &state,
            StatusCode::UNAUTHORIZED,
            "ADMIN_UNAUTHORIZED",
            "admin token required",
            request_id.as_str(),
        );
    }

    let token = request.signal.to_ascii_uppercase();
    match token.as_str() {
        "TERM" | "INT" | "QUIT" => {
            request_log.info(
                "admin signal accepted",
                &[("result", "shutdown"), ("signal", token.as_str())],
            );
            let _ = state.shutdown.send(true);
            Json(serde_json::json!({
                "accepted": true,
                "signal": token,
            }))
            .into_response()
        }
        "HUP" => match reload_runtime_config(&state) {
            Ok(()) => {
                request_log.info(
                    "admin signal accepted",
                    &[("result", "reload"), ("signal", token.as_str())],
                );
                Json(serde_json::json!({
                    "accepted": true,
                    "signal": token,
                    "action": "reload",
                }))
                .into_response()
            }
            Err(err) => {
                request_log.warn(
                    "admin signal accepted",
                    &[
                        ("result", "reload-rejected"),
                        ("signal", token.as_str()),
                        ("error", err.as_str()),
                    ],
                );
                Json(serde_json::json!({
                    "accepted": true,
                    "signal": token,
                    "action": "reload-rejected",
                    "error": err,
                }))
                .into_response()
            }
        },
        "USR1" | "USR2" => {
            request_log.info(
                "admin signal accepted",
                &[("result", "reload-noop"), ("signal", token.as_str())],
            );
            Json(serde_json::json!({
                "accepted": true,
                "signal": token,
                "action": "reload-noop",
            }))
            .into_response()
        }
        other => {
            request_log.warn(
                "admin signal rejected",
                &[("result", "invalid"), ("signal", other)],
            );
            json_error(
                &state,
                StatusCode::BAD_REQUEST,
                "INVALID_SIGNAL",
                &format!("unsupported signal token: {other}"),
                request_id.as_str(),
            )
        }
    }
}

fn authorized(headers: &HeaderMap, expected: &str) -> bool {
    if let Some(value) = header_str(headers, "x-admin-token") {
        if token_eq(expected, value) {
            return true;
        }
    }
    if let Some(value) = headers
        .get(http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
    {
        if let Some(bearer) = value
            .strip_prefix("Bearer ")
            .or_else(|| value.strip_prefix("bearer "))
        {
            if token_eq(expected, bearer.trim()) {
                return true;
            }
        }
    }
    false
}

fn header_str<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get(name)
        .and_then(|value: &HeaderValue| value.to_str().ok())
}

fn token_eq(expected: &str, provided: &str) -> bool {
    let left = expected.as_bytes();
    let right = provided.as_bytes();
    if left.len() != right.len() {
        return false;
    }
    let mut diff = 0u8;
    for (a, b) in left.iter().zip(right.iter()) {
        diff |= a ^ b;
    }
    diff == 0
}

async fn io_timeout(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let deadline =
        request_deadline(&state.current_config().server).unwrap_or(Duration::from_secs(30));
    match tokio::time::timeout(deadline, next.run(req)).await {
        Ok(res) => res,
        Err(_) => StatusCode::REQUEST_TIMEOUT.into_response(),
    }
}

fn request_deadline(server: &ServerConfig) -> Result<Duration, ServerError> {
    let read =
        parse_duration(&server.read_timeout).map_err(|e| ServerError::Bind(e.to_string()))?;
    let write =
        parse_duration(&server.write_timeout).map_err(|e| ServerError::Bind(e.to_string()))?;
    let idle =
        parse_duration(&server.idle_timeout).map_err(|e| ServerError::Bind(e.to_string()))?;
    Ok(read.saturating_add(write).min(idle))
}

fn json_error(
    state: &AppState,
    status: StatusCode,
    code: &str,
    message: &str,
    request_id: &str,
) -> Response {
    let err: ErrorResponse =
        state
            .observability
            .wrap_error_correlated(code, message, Some(request_id));
    let body = err
        .to_json_value()
        .unwrap_or_else(|_| serde_json::json!({"code": code, "message": message}));
    (status, Json(serde_json::json!({"error": body}))).into_response()
}

fn rustc_version() -> String {
    option_env!("APP_RUSTC_VERSION")
        .unwrap_or(env!("CARGO_PKG_RUST_VERSION"))
        .to_string()
}

/// Server errors.
#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    /// Bind failure.
    #[error("failed to bind: {0}")]
    Bind(String),
    /// IO failure.
    #[error("server io error: {0}")]
    Io(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::appid;
    use crate::config::Config;
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn test_state() -> AppState {
        test_state_with_admin(None)
    }

    fn test_state_with_admin(token: Option<&str>) -> AppState {
        let identity = appid::load().unwrap();
        let config = Config::default();
        let observability = Observability::new(&identity, &config.logging, false);
        let (shutdown, _) = watch::channel(false);
        let serve_admin = token.is_some();
        AppState {
            identity: Arc::new(identity),
            config: Arc::new(RwLock::new(config)),
            observability,
            shutdown,
            admin_token: token.map(Arc::<str>::from),
            serve_admin,
            load_options: LoadOptions::default(),
        }
    }

    #[test]
    fn default_host_is_loopback() {
        assert_eq!(Config::default().server.host, "127.0.0.1");
    }

    #[test]
    fn admin_gate_requires_token_off_loopback() {
        let err = admin_gate("0.0.0.0", false).expect_err("wildcard without token");
        assert!(err.to_string().contains("admin token"));
        let gate = admin_gate("0.0.0.0", true).expect("wildcard with token may bind");
        assert!(!gate.serve_admin);
        let loopback = admin_gate("127.0.0.1", false).expect("loopback without token");
        assert!(!loopback.serve_admin);
        let armed = admin_gate("127.0.0.1", true).expect("loopback with token");
        assert!(armed.serve_admin);
    }

    #[tokio::test]
    async fn health_endpoint_reports_healthy() {
        let app = router(test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body["status"], "healthy");
    }

    #[tokio::test]
    async fn version_includes_crucible() {
        let app = router(test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/version")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(!body["name"].as_str().unwrap().is_empty());
        assert!(body["crucible"].as_str().unwrap().starts_with('v'));
        assert_eq!(body["rsfulmen"], rsfulmen::VERSION);
    }

    #[tokio::test]
    async fn metrics_is_prometheus_text() {
        let app = router(test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/metrics")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let text = String::from_utf8(bytes.to_vec()).unwrap();
        assert!(text.contains("http_requests_total"));
    }

    #[tokio::test]
    async fn admin_signal_absent_without_token() {
        let app = router(test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/admin/signal")
                    .header(http::header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"signal":"TERM"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn admin_signal_rejects_missing_secret() {
        let app = router(test_state_with_admin(Some("test-secret")));
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/admin/signal")
                    .header(http::header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"signal":"TERM"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn admin_signal_accepts_bearer_token() {
        let app = router(test_state_with_admin(Some("test-secret")));
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/admin/signal")
                    .header(http::header::CONTENT_TYPE, "application/json")
                    .header(http::header::AUTHORIZATION, "Bearer test-secret")
                    .body(Body::from(r#"{"signal":"HUP"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.headers().get(&REQUEST_ID_HEADER).is_some());
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body["accepted"], true);
        assert_eq!(body["signal"], "HUP");
        assert_eq!(body["action"], "reload");
    }

    #[tokio::test]
    async fn request_id_honors_incoming_header() {
        let app = router(test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .header(&REQUEST_ID_HEADER, "client-trace-1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            response
                .headers()
                .get(&REQUEST_ID_HEADER)
                .and_then(|value| value.to_str().ok()),
            Some("client-trace-1")
        );
    }

    #[tokio::test]
    async fn request_id_is_generated_when_absent() {
        let app = router(test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let value = response
            .headers()
            .get(&REQUEST_ID_HEADER)
            .and_then(|value| value.to_str().ok())
            .unwrap();
        assert!(uuid::Uuid::parse_str(value).is_ok());
    }

    #[test]
    fn double_tap_window_comes_from_catalog() {
        let cfg = signals::DoubleTapConfig::from_catalog();
        assert_eq!(cfg.window, Duration::from_secs(2));
    }

    #[test]
    fn reload_applies_runtime_overrides() {
        let state = test_state();
        let mut logging = serde_yaml::Mapping::new();
        logging.insert(
            serde_yaml::Value::String("level".into()),
            serde_yaml::Value::String("debug".into()),
        );
        let mut root = serde_yaml::Mapping::new();
        root.insert(
            serde_yaml::Value::String("logging".into()),
            serde_yaml::Value::Mapping(logging),
        );
        let mut state = state;
        state.load_options = LoadOptions {
            runtime_overrides: Some(serde_yaml::Value::Mapping(root)),
            ..LoadOptions::default()
        };
        reload_runtime_config(&state).expect("reload");
        assert_eq!(state.current_config().logging.level, "debug");
    }

    #[test]
    fn reload_rejects_invalid_config() {
        let state = test_state();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.yaml");
        std::fs::write(&path, "server:\n  port: not-a-port\n").unwrap();
        let mut state = state;
        state.load_options = LoadOptions {
            config_path: Some(path),
            ..LoadOptions::default()
        };
        let previous = state.current_config();
        assert!(reload_runtime_config(&state).is_err());
        assert_eq!(state.current_config(), previous);
    }

    #[tokio::test]
    async fn echo_rejects_oversized_message() {
        let app = router(test_state());
        let oversized = "x".repeat(ECHO_MAX_BYTES + 1);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/echo")
                    .header(http::header::CONTENT_TYPE, "application/json")
                    .body(Body::from(format!(r#"{{"message":"{oversized}"}}"#)))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }
}
