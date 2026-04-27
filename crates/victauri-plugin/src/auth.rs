use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::Response;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

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
        .and_then(|v| {
            let lower = v.to_lowercase();
            if lower.starts_with("bearer ") {
                Some(v[7..].to_string())
            } else {
                None
            }
        });

    match provided {
        Some(ref token) if token == expected => Ok(next.run(request).await),
        _ => Err(StatusCode::UNAUTHORIZED),
    }
}

// ── Rate Limiter ───────────────────────────────────────────────────────────

/// Uses millisecond-precision timestamps for smooth token refill.
pub struct RateLimiterState {
    tokens: AtomicU64,
    max_tokens: u64,
    last_refill_ms: AtomicU64,
    refill_rate_per_sec: u64,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

impl RateLimiterState {
    pub fn new(max_requests_per_sec: u64) -> Self {
        Self {
            tokens: AtomicU64::new(max_requests_per_sec),
            max_tokens: max_requests_per_sec,
            last_refill_ms: AtomicU64::new(now_ms()),
            refill_rate_per_sec: max_requests_per_sec,
        }
    }

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
        if elapsed_ms == 0 {
            return;
        }
        let add = elapsed_ms * self.refill_rate_per_sec / 1000;
        if add == 0 {
            return;
        }
        if self
            .last_refill_ms
            .compare_exchange(last, now, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
        {
            loop {
                let current = self.tokens.load(Ordering::Relaxed);
                let new_val = (current + add).min(self.max_tokens);
                if self
                    .tokens
                    .compare_exchange_weak(current, new_val, Ordering::Relaxed, Ordering::Relaxed)
                    .is_ok()
                {
                    break;
                }
            }
        }
    }
}

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

const DEFAULT_RATE_LIMIT: u64 = 100;

pub fn default_rate_limiter() -> Arc<RateLimiterState> {
    Arc::new(RateLimiterState::new(DEFAULT_RATE_LIMIT))
}

// ── Security Middlewares ──────────────────────────────────────────────────

/// Axum middleware that blocks DNS rebinding attacks.
///
/// Rejects any request where the Host header is not a localhost address.
pub async fn dns_rebinding_guard(request: Request, next: Next) -> Result<Response, StatusCode> {
    let host = request
        .headers()
        .get("host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let host_name = host.split(':').next().unwrap_or(host);
    let is_allowed = matches!(host_name, "localhost" | "127.0.0.1" | "::1" | "[::1]" | "");
    if !is_allowed {
        tracing::warn!("DNS rebinding attempt blocked: Host={host}");
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(next.run(request).await)
}

/// Axum middleware that blocks cross-origin requests from browsers.
pub async fn origin_guard(request: Request, next: Next) -> Result<Response, StatusCode> {
    if let Some(origin) = request
        .headers()
        .get("origin")
        .and_then(|v| v.to_str().ok())
    {
        let allowed = origin.starts_with("http://localhost")
            || origin.starts_with("https://localhost")
            || origin.starts_with("http://127.0.0.1")
            || origin.starts_with("https://127.0.0.1")
            || origin.starts_with("http://[::1]")
            || origin.starts_with("https://[::1]")
            || origin.starts_with("tauri://")
            || origin == "null";
        if !allowed {
            tracing::warn!("Cross-origin request blocked: Origin={origin}");
            return Err(StatusCode::FORBIDDEN);
        }
    }
    Ok(next.run(request).await)
}

/// Axum middleware that sets security-hardening response headers on every response.
pub async fn security_headers(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert(
        axum::http::header::X_CONTENT_TYPE_OPTIONS,
        axum::http::HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        axum::http::header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    headers.insert(
        axum::http::header::HeaderName::from_static("x-frame-options"),
        axum::http::HeaderValue::from_static("DENY"),
    );
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_generation_is_unique() {
        let t1 = generate_token();
        let t2 = generate_token();
        assert_ne!(t1, t2);
        assert_eq!(t1.len(), 36); // UUID v4 format
    }

    #[test]
    fn token_is_valid_uuid() {
        let token = generate_token();
        assert!(uuid::Uuid::parse_str(&token).is_ok());
    }

    #[test]
    fn rate_limiter_allows_within_budget() {
        let limiter = RateLimiterState::new(10);
        for _ in 0..10 {
            assert!(limiter.try_acquire());
        }
    }

    #[test]
    fn rate_limiter_denies_when_exhausted() {
        let limiter = RateLimiterState::new(5);
        for _ in 0..5 {
            assert!(limiter.try_acquire());
        }
        assert!(!limiter.try_acquire());
    }

    #[test]
    fn rate_limiter_initial_tokens_match_max() {
        let limiter = RateLimiterState::new(42);
        assert_eq!(limiter.tokens.load(Ordering::Relaxed), 42);
        assert_eq!(limiter.max_tokens, 42);
    }

    #[test]
    fn rate_limiter_concurrent_acquire() {
        // Use a large bucket so time-based refills (1 per second) are negligible
        let limiter = Arc::new(RateLimiterState::new(1000));
        let mut handles = vec![];
        for _ in 0..10 {
            let l = limiter.clone();
            handles.push(std::thread::spawn(move || {
                let mut acquired = 0;
                for _ in 0..200 {
                    if l.try_acquire() {
                        acquired += 1;
                    }
                }
                acquired
            }));
        }
        let total: u64 = handles.into_iter().map(|h| h.join().unwrap()).sum();
        // All 1000 tokens should be dispensed; a time-based refill may add a few
        assert!((1000..=1010).contains(&total));
    }

    #[test]
    fn default_rate_limiter_has_100_tokens() {
        let limiter = default_rate_limiter();
        assert_eq!(limiter.max_tokens, 100);
    }

    #[test]
    fn rate_limiter_zero_capacity() {
        let limiter = RateLimiterState::new(0);
        assert!(!limiter.try_acquire());
    }
}
