use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::Response;
use std::sync::Arc;

pub fn generate_token() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[derive(Clone)]
pub struct AuthState {
    pub token: Option<String>,
}

pub async fn require_auth(
    axum::extract::State(auth): axum::extract::State<Arc<AuthState>>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let expected = match &auth.token {
        Some(t) => t,
        None => return Ok(next.run(request).await),
    };

    let provided = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    match provided {
        Some(token) if token == expected => Ok(next.run(request).await),
        _ => Err(StatusCode::UNAUTHORIZED),
    }
}
