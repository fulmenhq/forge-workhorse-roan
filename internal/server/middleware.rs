//! Default HTTP middleware. Request-ID follows Groningen
//! `internal/server/middleware/requestid.go`.

use axum::extract::{FromRequestParts, Request};
use axum::http::request::Parts;
use axum::http::{HeaderName, HeaderValue};
use axum::middleware::Next;
use axum::response::Response;
use std::convert::Infallible;

/// Incoming/outgoing request correlation header (`X-Request-ID`).
pub const REQUEST_ID_HEADER: HeaderName = HeaderName::from_static("x-request-id");

/// Request correlation identifier (extension + extractor).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestId(pub String);

impl RequestId {
    /// Borrow the identifier.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<S> FromRequestParts<S> for RequestId
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(request_id_from_parts(parts))
    }
}

/// Honor `X-Request-ID` when present; otherwise generate a UUID.
pub fn resolve_request_id(incoming: Option<&HeaderValue>) -> String {
    if let Some(value) = incoming.and_then(|value| value.to_str().ok()) {
        let trimmed = value.trim();
        if !trimmed.is_empty() && HeaderValue::from_str(trimmed).is_ok() {
            return trimmed.to_string();
        }
    }
    uuid::Uuid::new_v4().to_string()
}

/// Read a request ID from extensions, or return empty when middleware did not run.
pub fn request_id_from_parts(parts: &Parts) -> RequestId {
    parts
        .extensions
        .get::<RequestId>()
        .cloned()
        .unwrap_or_else(|| RequestId(String::new()))
}

/// Axum layer: honor/set `X-Request-ID` and attach it for log correlation.
pub async fn request_id(mut req: Request, next: Next) -> Response {
    let id = resolve_request_id(req.headers().get(&REQUEST_ID_HEADER));
    req.extensions_mut().insert(RequestId(id.clone()));

    let mut res = next.run(req).await;
    if let Ok(value) = HeaderValue::from_str(&id) {
        res.headers_mut().insert(REQUEST_ID_HEADER, value);
    }
    res
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn honors_incoming_request_id() {
        let value = HeaderValue::from_static("client-req-1");
        assert_eq!(resolve_request_id(Some(&value)), "client-req-1");
    }

    #[test]
    fn generates_uuid_when_missing() {
        let generated = resolve_request_id(None);
        assert!(uuid::Uuid::parse_str(&generated).is_ok());
    }

    #[test]
    fn blank_header_generates_uuid() {
        let value = HeaderValue::from_static("   ");
        let generated = resolve_request_id(Some(&value));
        assert!(uuid::Uuid::parse_str(&generated).is_ok());
    }
}
