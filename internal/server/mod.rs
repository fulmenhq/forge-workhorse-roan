//! Placeholder HTTP server: health, version, metrics, and echo.

use crate::appid::Identity;
use crate::config::{parse_duration, Config};
use crate::core;
use crate::observability::Observability;
use crate::{BUILD_COMMIT, BUILD_DATE, BUILD_VERSION};
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use rsfulmen::crucible;
use rsfulmen::docscribe;
use rsfulmen::error_handling::ErrorResponse;
use rsfulmen::signals::{self, SignalEndpointRequest};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::watch;

/// Shared HTTP state.
#[derive(Clone)]
pub struct AppState {
    /// App identity.
    pub identity: Arc<Identity>,
    /// Effective config.
    pub config: Arc<Config>,
    /// Observability handles.
    pub observability: Observability,
    /// Shutdown trigger for `/admin/signal`.
    pub shutdown: watch::Sender<bool>,
}

/// Bind and serve until shutdown.
pub async fn serve(
    identity: Identity,
    config: Config,
    observability: Observability,
) -> Result<(), ServerError> {
    let addr: SocketAddr = format!("{}:{}", config.server.host, config.server.port)
        .parse::<SocketAddr>()
        .map_err(|e: std::net::AddrParseError| ServerError::Bind(e.to_string()))?;

    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
    let state = AppState {
        identity: Arc::new(identity),
        config: Arc::new(config.clone()),
        observability: observability.clone(),
        shutdown: shutdown_tx.clone(),
    };

    let app = router(state.clone());
    let listener = TcpListener::bind(addr)
        .await
        .map_err(|e| ServerError::Bind(e.to_string()))?;
    let bound = listener
        .local_addr()
        .map_err(|e| ServerError::Bind(e.to_string()))?;

    observability
        .logger
        .info("server listening", &[("addr", bound.to_string().as_str())]);

    let shutdown_timeout = parse_duration(&config.server.shutdown_timeout)
        .unwrap_or(std::time::Duration::from_secs(10));
    let double_tap = signals::DoubleTapConfig::from_catalog();
    let _ = double_tap.window;

    let server = axum::serve(listener, app).with_graceful_shutdown(async move {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = async {
                loop {
                    if *shutdown_rx.borrow() {
                        break;
                    }
                    if shutdown_rx.changed().await.is_err() {
                        break;
                    }
                }
            } => {}
        }
    });

    if let Err(err) = server.await {
        return Err(ServerError::Io(err.to_string()));
    }

    observability.logger.info(
        "server shutdown complete",
        &[("timeout", format!("{shutdown_timeout:?}").as_str())],
    );
    Ok(())
}

/// Build the HTTP router (also used by tests).
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/version", get(version))
        .route("/metrics", get(metrics))
        .route("/echo", get(echo_get).post(echo_post))
        .route("/docs", get(docs))
        .route("/admin/signal", post(admin_signal))
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
    if !state.config.health.enabled {
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
    if !state.config.metrics.enabled {
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
) -> Json<EchoResponse> {
    state.observability.record_http_request();
    Json(EchoResponse {
        message: core::echo(&query.message),
    })
}

async fn echo_post(
    State(state): State<AppState>,
    Json(body): Json<EchoBody>,
) -> Json<EchoResponse> {
    state.observability.record_http_request();
    Json(EchoResponse {
        message: core::echo(&body.message),
    })
}

async fn docs(State(state): State<AppState>) -> impl IntoResponse {
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
        ),
    }
}

async fn admin_signal(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<SignalEndpointRequest>,
) -> impl IntoResponse {
    state.observability.record_http_request();
    let _ = headers;
    let token = request.signal.to_ascii_uppercase();
    match token.as_str() {
        "TERM" | "INT" | "QUIT" => {
            let _ = state.shutdown.send(true);
            Json(serde_json::json!({
                "accepted": true,
                "signal": token,
                "reason": request.reason,
            }))
            .into_response()
        }
        "HUP" | "USR1" | "USR2" => Json(serde_json::json!({
            "accepted": true,
            "signal": token,
            "action": "reload-noop",
            "reason": request.reason,
        }))
        .into_response(),
        other => json_error(
            &state,
            StatusCode::BAD_REQUEST,
            "INVALID_SIGNAL",
            &format!("unsupported signal token: {other}"),
        ),
    }
}

fn json_error(state: &AppState, status: StatusCode, code: &str, message: &str) -> Response {
    let err: ErrorResponse = state.observability.wrap_error(code, message);
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
        let identity = appid::load().unwrap();
        let config = Config::default();
        let observability = Observability::new(&identity, &config.logging, false);
        let (shutdown, _) = watch::channel(false);
        AppState {
            identity: Arc::new(identity),
            config: Arc::new(config),
            observability,
            shutdown,
        }
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
}
