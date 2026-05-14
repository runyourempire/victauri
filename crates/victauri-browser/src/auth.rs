use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::Response;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

const BEARER_PREFIX_LEN: usize = "Bearer ".len();

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

/// Generate a random UUID v4 token for Bearer authentication.
#[must_use]
pub fn generate_token() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[derive(Clone)]
pub struct AuthState {
    pub token: Option<String>,
}

/// Axum middleware that validates Bearer token authentication.
///
/// # Errors
///
/// Returns `401 Unauthorized` if the token is missing or invalid.
pub async fn require_auth(
    axum::extract::State(auth): axum::extract::State<Arc<AuthState>>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let Some(expected) = &auth.token else {
        return Ok(next.run(request).await);
    };

    let provided = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| {
            let lower = v.to_lowercase();
            if lower.starts_with("bearer ") {
                Some(v[BEARER_PREFIX_LEN..].to_string())
            } else {
                None
            }
        });

    match provided {
        Some(ref token) if constant_time_eq(token.as_bytes(), expected.as_bytes()) => {
            Ok(next.run(request).await)
        }
        _ => {
            tracing::warn!("victauri-browser: rejected request — invalid or missing auth token");
            Err(StatusCode::UNAUTHORIZED)
        }
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub struct RateLimiterState {
    tokens: AtomicU64,
    max_tokens: u64,
    last_refill_ms: AtomicU64,
    refill_rate_per_sec: u64,
}

impl RateLimiterState {
    #[must_use]
    pub fn new(max_requests_per_sec: u64) -> Self {
        Self {
            tokens: AtomicU64::new(max_requests_per_sec),
            max_tokens: max_requests_per_sec,
            last_refill_ms: AtomicU64::new(now_ms()),
            refill_rate_per_sec: max_requests_per_sec,
        }
    }

    /// Try to consume one token. Returns `true` if allowed.
    pub fn try_acquire(&self) -> bool {
        self.refill();
        loop {
            let current = self.tokens.load(Ordering::Relaxed);
            if current == 0 {
                return false;
            }
            if self
                .tokens
                .compare_exchange_weak(current, current - 1, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                return true;
            }
        }
    }

    fn refill(&self) {
        let now = now_ms();
        let last = self.last_refill_ms.load(Ordering::Relaxed);
        let elapsed_ms = now.saturating_sub(last);
        if elapsed_ms < 10 {
            return;
        }
        let new_tokens = (elapsed_ms * self.refill_rate_per_sec) / 1000;
        if new_tokens == 0 {
            return;
        }
        if self
            .last_refill_ms
            .compare_exchange(last, now, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
        {
            let current = self.tokens.load(Ordering::Relaxed);
            let capped = (current + new_tokens).min(self.max_tokens);
            self.tokens.store(capped, Ordering::Relaxed);
        }
    }
}

/// Default rate limiter: 1000 requests per second.
#[must_use]
pub fn default_rate_limiter() -> Arc<RateLimiterState> {
    Arc::new(RateLimiterState::new(1000))
}

/// Axum middleware for rate limiting.
///
/// # Errors
///
/// Returns `429 Too Many Requests` when the rate limit is exceeded.
pub async fn rate_limit(
    axum::extract::State(limiter): axum::extract::State<Arc<RateLimiterState>>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    if limiter.try_acquire() {
        Ok(next.run(request).await)
    } else {
        Err(StatusCode::TOO_MANY_REQUESTS)
    }
}

/// Security headers middleware: X-Content-Type-Options, Cache-Control.
pub async fn security_headers(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert("x-content-type-options", "nosniff".parse().unwrap());
    headers.insert("cache-control", "no-store".parse().unwrap());
    response
}

/// Localhost origin guard: rejects requests with non-localhost Origin header.
pub async fn origin_guard(request: Request, next: Next) -> Result<Response, StatusCode> {
    if let Some(origin) = request.headers().get("origin").and_then(|v| v.to_str().ok()) {
        let is_local = origin.contains("127.0.0.1")
            || origin.contains("localhost")
            || origin.contains("[::1]");
        if !is_local {
            tracing::warn!("rejected non-local origin: {origin}");
            return Err(StatusCode::FORBIDDEN);
        }
    }
    Ok(next.run(request).await)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_generation() {
        let t1 = generate_token();
        let t2 = generate_token();
        assert_ne!(t1, t2);
        assert_eq!(t1.len(), 36);
    }

    #[test]
    fn rate_limiter_allows_within_budget() {
        let limiter = RateLimiterState::new(10);
        for _ in 0..10 {
            assert!(limiter.try_acquire());
        }
        assert!(!limiter.try_acquire());
    }

    #[test]
    fn constant_time_eq_works() {
        assert!(constant_time_eq(b"hello", b"hello"));
        assert!(!constant_time_eq(b"hello", b"world"));
        assert!(!constant_time_eq(b"hello", b"hell"));
    }
}
