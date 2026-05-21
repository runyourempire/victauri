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
///
/// # Panics
/// Panics if header values cannot be parsed (hardcoded valid values).
pub async fn security_headers(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert("x-content-type-options", "nosniff".parse().unwrap());
    headers.insert("cache-control", "no-store".parse().unwrap());
    response
}

/// Localhost origin guard: rejects requests with non-localhost Origin header.
///
/// Parses the origin as a URL and checks the host component directly,
/// preventing bypass via subdomains like "localhost.evil.com".
///
/// # Errors
/// Returns `403 Forbidden` if the Origin header contains a non-localhost host.
pub async fn origin_guard(request: Request, next: Next) -> Result<Response, StatusCode> {
    if let Some(origin) = request
        .headers()
        .get("origin")
        .and_then(|v| v.to_str().ok())
    {
        let is_local = is_localhost_origin(origin);
        if !is_local {
            tracing::warn!("rejected non-local origin: {origin}");
            return Err(StatusCode::FORBIDDEN);
        }
    }
    Ok(next.run(request).await)
}

fn is_localhost_origin(origin: &str) -> bool {
    // Extract the host from scheme://host[:port]
    let after_scheme = match origin.find("://") {
        Some(i) => &origin[i + 3..],
        None => origin,
    };
    // Strip port if present
    let host = if after_scheme.starts_with('[') {
        // IPv6: [::1]:port
        match after_scheme.find(']') {
            Some(i) => &after_scheme[..=i],
            None => after_scheme,
        }
    } else {
        after_scheme.split(':').next().unwrap_or(after_scheme)
    };
    // Strip trailing path if any
    let host = host.split('/').next().unwrap_or(host);

    host == "127.0.0.1" || host == "localhost" || host == "[::1]"
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

    #[test]
    fn constant_time_eq_empty_strings() {
        assert!(constant_time_eq(b"", b""));
        assert!(!constant_time_eq(b"", b"x"));
    }

    #[test]
    fn constant_time_eq_single_bit_diff() {
        assert!(!constant_time_eq(b"\x00", b"\x01"));
        assert!(!constant_time_eq(b"\xff", b"\xfe"));
    }

    #[test]
    fn rate_limiter_single_token() {
        let limiter = RateLimiterState::new(1);
        assert!(limiter.try_acquire());
        assert!(!limiter.try_acquire());
    }

    #[test]
    fn token_format_is_uuid() {
        let token = generate_token();
        assert_eq!(token.len(), 36);
        assert_eq!(token.chars().filter(|c| *c == '-').count(), 4);
    }

    #[test]
    fn default_rate_limiter_has_budget() {
        let limiter = default_rate_limiter();
        assert!(limiter.try_acquire());
    }

    // --- Adversarial stress tests ---

    #[test]
    fn rate_limiter_exact_boundary() {
        let limiter = RateLimiterState::new(100);
        for i in 0..100 {
            assert!(limiter.try_acquire(), "failed at iteration {i}");
        }
        assert!(!limiter.try_acquire());
        assert!(!limiter.try_acquire());
        assert!(!limiter.try_acquire());
    }

    #[test]
    fn rate_limiter_concurrent_contention() {
        use std::sync::Arc;
        use std::thread;

        let limiter = Arc::new(RateLimiterState::new(50));
        let mut handles = vec![];

        for _ in 0..10 {
            let l = Arc::clone(&limiter);
            handles.push(thread::spawn(move || {
                let mut acquired = 0u32;
                for _ in 0..20 {
                    if l.try_acquire() {
                        acquired += 1;
                    }
                }
                acquired
            }));
        }

        let total: u32 = handles.into_iter().map(|h| h.join().unwrap()).sum();
        // With 50 tokens and 10 threads each trying 20 times, at most 50 succeed
        assert!(total <= 50, "acquired {total} but budget was 50");
        assert!(total >= 45, "should acquire most tokens, got {total}");
    }

    #[test]
    fn constant_time_eq_long_strings() {
        let a = "a".repeat(10_000);
        let b = "a".repeat(10_000);
        assert!(constant_time_eq(a.as_bytes(), b.as_bytes()));

        let mut c = "a".repeat(10_000);
        c.push('b');
        assert!(!constant_time_eq(a.as_bytes(), c.as_bytes()));
    }

    #[test]
    fn constant_time_eq_timing_consistency() {
        let token = "8f14e45f-ceea-367f-a27f-c790e5a0fdc4";
        let wrong1 = "0000000f-ceea-367f-a27f-c790e5a0fdc4";
        let wrong2 = "8f14e45f-ceea-367f-a27f-c790e5a0fd00";

        // Both should fail regardless of where the mismatch is
        assert!(!constant_time_eq(token.as_bytes(), wrong1.as_bytes()));
        assert!(!constant_time_eq(token.as_bytes(), wrong2.as_bytes()));
    }

    #[test]
    fn token_uniqueness_over_1000_generations() {
        let mut tokens = std::collections::HashSet::new();
        for _ in 0..1000 {
            let t = generate_token();
            assert!(tokens.insert(t), "duplicate token generated");
        }
    }

    #[test]
    fn rate_limiter_zero_budget() {
        let limiter = RateLimiterState::new(0);
        assert!(!limiter.try_acquire());
    }

    #[test]
    fn constant_time_eq_all_byte_values() {
        for b in 0..=255u8 {
            let a = [b];
            assert!(constant_time_eq(&a, &a));
            if b < 255 {
                let c = [b + 1];
                assert!(!constant_time_eq(&a, &c));
            }
        }
    }

    // --- Origin guard tests ---

    #[test]
    fn localhost_origin_accepted() {
        assert!(is_localhost_origin("http://localhost:3000"));
        assert!(is_localhost_origin("http://localhost"));
        assert!(is_localhost_origin("https://localhost:7474"));
    }

    #[test]
    fn ipv4_loopback_accepted() {
        assert!(is_localhost_origin("http://127.0.0.1:7474"));
        assert!(is_localhost_origin("http://127.0.0.1"));
        assert!(is_localhost_origin("https://127.0.0.1:443"));
    }

    #[test]
    fn ipv6_loopback_accepted() {
        assert!(is_localhost_origin("http://[::1]:7474"));
        assert!(is_localhost_origin("http://[::1]"));
    }

    #[test]
    fn subdomain_bypass_rejected() {
        assert!(!is_localhost_origin("https://localhost.evil.com"));
        assert!(!is_localhost_origin("https://127.0.0.1.evil.com"));
        assert!(!is_localhost_origin("https://evil-localhost.com"));
    }

    #[test]
    fn path_bypass_rejected() {
        assert!(!is_localhost_origin("https://evil.com/localhost"));
        assert!(!is_localhost_origin("https://evil.com/127.0.0.1"));
    }

    #[test]
    fn external_origins_rejected() {
        assert!(!is_localhost_origin("https://google.com"));
        assert!(!is_localhost_origin("https://example.com:443"));
        assert!(!is_localhost_origin("http://attacker.com"));
    }
}
