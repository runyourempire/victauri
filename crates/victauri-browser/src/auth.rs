pub use victauri_core::middleware::{
    AuthState, default_rate_limiter, dns_rebinding_guard, origin_guard, rate_limit, require_auth,
    security_headers,
};
pub use victauri_core::security::{
    self, RateLimiter as RateLimiterState, constant_time_eq, generate_token, is_allowed_origin,
    is_localhost_host,
};

#[cfg(test)]
mod tests {
    use std::sync::Arc;

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
        let limiter = Arc::new(RateLimiterState::new(1000));
        let mut handles = vec![];

        for _ in 0..10 {
            let l = Arc::clone(&limiter);
            handles.push(std::thread::spawn(move || {
                let mut acquired = 0u64;
                for _ in 0..200 {
                    if l.try_acquire() {
                        acquired += 1;
                    }
                }
                acquired
            }));
        }

        let total: u64 = handles.into_iter().map(|h| h.join().unwrap()).sum();
        assert!(
            total >= 1000,
            "should dispense at least the initial budget, got {total}"
        );
        assert!(total <= 1200, "refill overshoot too high, got {total}");
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

    // --- DNS rebinding guard tests ---

    #[test]
    fn dns_rebinding_guard_allows_localhost() {
        assert!(is_localhost_host("localhost"));
        assert!(is_localhost_host("localhost:7474"));
    }

    #[test]
    fn dns_rebinding_guard_allows_127() {
        assert!(is_localhost_host("127.0.0.1"));
        assert!(is_localhost_host("127.0.0.1:7474"));
    }

    #[test]
    fn dns_rebinding_guard_allows_ipv6() {
        assert!(is_localhost_host("[::1]"));
        assert!(is_localhost_host("[::1]:7474"));
        assert!(is_localhost_host("::1"));
    }

    #[test]
    fn dns_rebinding_guard_blocks_evil() {
        assert!(!is_localhost_host("evil.com"));
    }

    #[test]
    fn dns_rebinding_guard_blocks_localhost_subdomain() {
        assert!(!is_localhost_host("localhost.evil.com"));
    }

    #[test]
    fn dns_rebinding_guard_blocks_empty() {
        assert!(!is_localhost_host(""));
    }

    // --- Origin guard tests ---

    #[test]
    fn localhost_origin_accepted() {
        assert!(is_allowed_origin("http://localhost:3000"));
        assert!(is_allowed_origin("http://localhost"));
        assert!(is_allowed_origin("https://localhost:7474"));
    }

    #[test]
    fn ipv4_loopback_accepted() {
        assert!(is_allowed_origin("http://127.0.0.1:7474"));
        assert!(is_allowed_origin("http://127.0.0.1"));
        assert!(is_allowed_origin("https://127.0.0.1:443"));
    }

    #[test]
    fn ipv6_loopback_accepted() {
        assert!(is_allowed_origin("http://[::1]:7474"));
        assert!(is_allowed_origin("http://[::1]"));
    }

    #[test]
    fn subdomain_bypass_rejected() {
        assert!(!is_allowed_origin("https://localhost.evil.com"));
        assert!(!is_allowed_origin("https://127.0.0.1.evil.com"));
        assert!(!is_allowed_origin("https://evil-localhost.com"));
    }

    #[test]
    fn path_bypass_rejected() {
        assert!(!is_allowed_origin("https://evil.com/localhost"));
        assert!(!is_allowed_origin("https://evil.com/127.0.0.1"));
    }

    #[test]
    fn external_origins_rejected() {
        assert!(!is_allowed_origin("https://google.com"));
        assert!(!is_allowed_origin("https://example.com:443"));
        assert!(!is_allowed_origin("http://attacker.com"));
    }
}
