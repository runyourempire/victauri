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

pub struct RateLimiterState {
    tokens: AtomicU64,
    max_tokens: u64,
    last_refill: AtomicU64,
    refill_rate_per_sec: u64,
}

impl RateLimiterState {
    pub fn new(max_requests_per_sec: u64) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self {
            tokens: AtomicU64::new(max_requests_per_sec),
            max_tokens: max_requests_per_sec,
            last_refill: AtomicU64::new(now),
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
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let last = self.last_refill.load(Ordering::Relaxed);
        let elapsed = now.saturating_sub(last);
        if elapsed == 0 {
            return;
        }
        if self
            .last_refill
            .compare_exchange(last, now, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
        {
            let add = elapsed * self.refill_rate_per_sec;
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
