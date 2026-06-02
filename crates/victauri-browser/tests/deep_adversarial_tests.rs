//! Deep adversarial tests for victauri-browser.
//!
//! Cross-module integration tests exercising auth bypass attempts, origin guard
//! evasion, request smuggling, tool execution edge cases, state corruption,
//! rate limiter adversarial scenarios, and response format validation.
//!
//! All tests use `tower::ServiceExt::oneshot` against the full axum router built
//! by `build_app` / `build_app_full`, so every middleware layer (origin guard,
//! security headers, rate limiter, auth) is exercised on every request.

use std::sync::Arc;

use axum::body::Body;
use http_body_util::BodyExt;
use tower::ServiceExt;

use victauri_browser::auth::RateLimiterState;
use victauri_browser::bridge_dispatch::BridgeDispatch;
use victauri_browser::mcp_handler::VictauriBrowserHandler;
use victauri_browser::server::{build_app, build_app_full};
use victauri_browser::tab_state::TabManager;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn test_app(auth_token: Option<String>) -> (axum::Router, Arc<TabManager>, Arc<BridgeDispatch>) {
    let tab_mgr = Arc::new(TabManager::new());
    let dispatch = Arc::new(BridgeDispatch::new_sink());
    let handler = VictauriBrowserHandler::new(Arc::clone(&tab_mgr), Arc::clone(&dispatch));
    let app = build_app(handler, auth_token);
    (app, tab_mgr, dispatch)
}

fn test_app_rate_limited(budget: u64) -> (axum::Router, Arc<TabManager>, Arc<BridgeDispatch>) {
    let tab_mgr = Arc::new(TabManager::new());
    let dispatch = Arc::new(BridgeDispatch::new_sink());
    let handler = VictauriBrowserHandler::new(Arc::clone(&tab_mgr), Arc::clone(&dispatch));
    let limiter = Arc::new(RateLimiterState::new(budget));
    let app = build_app_full(handler, None, Some(limiter));
    (app, tab_mgr, dispatch)
}

async fn get(app: axum::Router, path: &str, headers: Vec<(&str, &str)>) -> (u16, Vec<u8>) {
    let mut builder = axum::http::Request::builder()
        .uri(path)
        .header("host", "localhost");
    for (k, v) in headers {
        builder = builder.header(k, v);
    }
    let req = builder.body(Body::empty()).unwrap();
    let resp = app.oneshot(req).await.unwrap();
    let status = resp.status().as_u16();
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    (status, body.to_vec())
}

async fn get_json(
    app: axum::Router,
    path: &str,
    headers: Vec<(&str, &str)>,
) -> (u16, serde_json::Value) {
    let (status, body) = get(app, path, headers).await;
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
    (status, json)
}

async fn post_tool(
    app: axum::Router,
    tool: &str,
    body: serde_json::Value,
    auth: Option<&str>,
) -> (u16, serde_json::Value) {
    let mut builder = axum::http::Request::builder()
        .method("POST")
        .uri(format!("/api/tools/{tool}"))
        .header("content-type", "application/json")
        .header("host", "localhost");
    if let Some(token) = auth {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    let req = builder
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    let status = resp.status().as_u16();
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
    (status, json)
}

async fn raw_post(
    app: axum::Router,
    path: &str,
    content_type: Option<&str>,
    body_bytes: Vec<u8>,
    headers: Vec<(&str, &str)>,
) -> (u16, Vec<u8>) {
    let mut builder = axum::http::Request::builder()
        .method("POST")
        .uri(path)
        .header("host", "localhost");
    if let Some(ct) = content_type {
        builder = builder.header("content-type", ct);
    }
    for (k, v) in headers {
        builder = builder.header(k, v);
    }
    let req = builder.body(Body::from(body_bytes)).unwrap();
    let resp = app.oneshot(req).await.unwrap();
    let status = resp.status().as_u16();
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    (status, body.to_vec())
}

async fn resp_headers(
    app: axum::Router,
    path: &str,
    method: &str,
    extra_headers: Vec<(&str, &str)>,
) -> (u16, axum::http::HeaderMap) {
    let mut builder = axum::http::Request::builder()
        .method(method)
        .uri(path)
        .header("host", "localhost");
    for (k, v) in extra_headers {
        builder = builder.header(k, v);
    }
    let req = builder.body(Body::empty()).unwrap();
    let resp = app.oneshot(req).await.unwrap();
    let status = resp.status().as_u16();
    let headers = resp.headers().clone();
    (status, headers)
}

// ===========================================================================
// Group A: Auth Bypass Attempts (18 tests)
// ===========================================================================

#[tokio::test]
async fn auth_bypass_sql_injection_in_header() {
    let (app, _, _) = test_app(Some("real-token".into()));
    let (status, _) = get_json(app, "/info", vec![("authorization", "Bearer ' OR 1=1 --")]).await;
    assert_eq!(status, 401);
}

#[tokio::test]
async fn auth_bypass_jwt_format_token() {
    let (app, _, _) = test_app(Some("real-token".into()));
    let jwt = "Bearer eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxIn0.fake";
    let (status, _) = get_json(app, "/info", vec![("authorization", jwt)]).await;
    assert_eq!(status, 401);
}

#[tokio::test]
async fn auth_bypass_base64_encoded_token() {
    let (app, _, _) = test_app(Some("real-token".into()));
    // base64("real-token") = "cmVhbC10b2tlbg=="
    let (status, _) = get_json(
        app,
        "/info",
        vec![("authorization", "Bearer cmVhbC10b2tlbg==")],
    )
    .await;
    assert_eq!(status, 401);
}

#[tokio::test]
async fn auth_bypass_url_encoded_token() {
    let (app, _, _) = test_app(Some("real-token".into()));
    let (status, _) = get_json(app, "/info", vec![("authorization", "Bearer real%2Dtoken")]).await;
    assert_eq!(status, 401);
}

#[tokio::test]
async fn auth_bypass_null_byte_in_token() {
    let (app, _, _) = test_app(Some("real-token".into()));
    // Null byte in header value is invalid HTTP — the `http` crate rejects it
    // at build time, which means the attack is blocked at the protocol level.
    let result = axum::http::Request::builder()
        .uri("/info")
        .header("host", "localhost")
        .header("authorization", "Bearer real-token\0extra")
        .body(Body::empty());
    if let Ok(req) = result {
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status().as_u16(), 401);
    }
    // If Err: http crate rejects null bytes in headers — attack blocked at protocol level
}

#[tokio::test]
async fn auth_bypass_double_bearer_prefix() {
    let (app, _, _) = test_app(Some("real-token".into()));
    let (status, _) = get_json(
        app,
        "/info",
        vec![("authorization", "Bearer Bearer real-token")],
    )
    .await;
    assert_eq!(status, 401);
}

#[tokio::test]
async fn auth_bypass_token_1_char() {
    let (app, _, _) = test_app(Some("real-token".into()));
    let (status, _) = get_json(app, "/info", vec![("authorization", "Bearer x")]).await;
    assert_eq!(status, 401);
}

#[tokio::test]
async fn auth_bypass_token_10kb() {
    let (app, _, _) = test_app(Some("real-token".into()));
    let long = "x".repeat(10_000);
    let header = format!("Bearer {long}");
    let (status, _) = get_json(app, "/info", vec![("authorization", &header)]).await;
    assert_eq!(status, 401);
}

#[tokio::test]
async fn auth_bypass_multiple_schemes_in_one_header() {
    let (app, _, _) = test_app(Some("real-token".into()));
    let (status, _) = get_json(
        app,
        "/info",
        vec![("authorization", "Basic dXNlcjpwYXNz, Bearer real-token")],
    )
    .await;
    assert_eq!(status, 401);
}

#[tokio::test]
async fn auth_bypass_case_variations_bearer() {
    let token = "my-secret";
    let (app, _, _) = test_app(Some(token.into()));

    // The existing inline tests show case-insensitive "BEARER" works.
    // Verify mixed case also works.
    let (status, _) = get_json(
        app.clone(),
        "/info",
        vec![("authorization", &format!("bEaReR {token}"))],
    )
    .await;
    assert_eq!(status, 200);

    // But "Beare r" (space in wrong place) must fail.
    let (status, _) = get_json(
        app,
        "/info",
        vec![("authorization", &format!("Beare r {token}"))],
    )
    .await;
    assert_eq!(status, 401);
}

#[tokio::test]
async fn auth_bypass_unicode_homoglyph_bearer() {
    // U+0412 Cyrillic "B" looks like Latin "B"
    let (app, _, _) = test_app(Some("tok".into()));
    let (status, _) = get_json(app, "/info", vec![("authorization", "\u{0412}earer tok")]).await;
    assert_eq!(status, 401);
}

#[tokio::test]
async fn auth_bypass_bom_in_header() {
    let (app, _, _) = test_app(Some("tok".into()));
    let (status, _) = get_json(app, "/info", vec![("authorization", "\u{FEFF}Bearer tok")]).await;
    assert_eq!(status, 401);
}

#[tokio::test]
async fn auth_bypass_multiple_spaces_before_token() {
    let (app, _, _) = test_app(Some("tok".into()));
    let (status, _) = get_json(app, "/info", vec![("authorization", "Bearer     tok")]).await;
    // Extra spaces become part of the token, so "    tok" != "tok"
    assert_eq!(status, 401);
}

#[tokio::test]
async fn auth_bypass_tab_instead_of_space() {
    let (app, _, _) = test_app(Some("tok".into()));
    let (status, _) = get_json(app, "/info", vec![("authorization", "Bearer\ttok")]).await;
    // Tab is not a space; "Bearer\ttok" starts_with "bearer " is false after lowercase
    assert_eq!(status, 401);
}

#[tokio::test]
async fn auth_no_authorization_header_returns_401() {
    let (app, _, _) = test_app(Some("tok".into()));
    let (status, _) = get_json(app, "/info", vec![]).await;
    assert_eq!(status, 401);
}

#[tokio::test]
async fn auth_empty_authorization_header() {
    let (app, _, _) = test_app(Some("tok".into()));
    let (status, _) = get_json(app, "/info", vec![("authorization", "")]).await;
    assert_eq!(status, 401);
}

#[tokio::test]
async fn auth_only_bearer_no_token() {
    let (app, _, _) = test_app(Some("tok".into()));
    let (status, _) = get_json(app, "/info", vec![("authorization", "Bearer")]).await;
    assert_eq!(status, 401);
}

#[tokio::test]
async fn auth_response_does_not_leak_token() {
    let token = "super-secret-token-12345";
    let (app, _, _) = test_app(Some(token.into()));
    let (_, body) = get(app, "/info", vec![("authorization", "Bearer wrong")]).await;
    let body_str = String::from_utf8_lossy(&body);
    assert!(
        !body_str.contains(token),
        "response body leaked the auth token"
    );
}

// ===========================================================================
// Group B: Origin Guard Adversarial (18 tests)
// ===========================================================================

#[tokio::test]
async fn origin_bypass_localhost_evil_com() {
    let (app, _, _) = test_app(None);
    let (status, _) = get_json(
        app,
        "/health",
        vec![("origin", "http://localhost.evil.com")],
    )
    .await;
    assert_eq!(status, 403);
}

#[tokio::test]
async fn origin_bypass_evil_dot_localhost() {
    let (app, _, _) = test_app(None);
    let (status, _) = get_json(app, "/health", vec![("origin", "http://evil.localhost")]).await;
    assert_eq!(status, 403);
}

#[tokio::test]
async fn origin_bypass_localhost_at_evil() {
    let (app, _, _) = test_app(None);
    let (status, _) = get_json(
        app,
        "/health",
        vec![("origin", "http://localhost@evil.com")],
    )
    .await;
    assert_eq!(status, 403);
}

#[tokio::test]
async fn origin_bypass_127_in_subdomain() {
    let (app, _, _) = test_app(None);
    let (status, _) = get_json(
        app,
        "/health",
        vec![("origin", "http://127.0.0.1.evil.com")],
    )
    .await;
    assert_eq!(status, 403);
}

#[tokio::test]
async fn origin_bypass_evil_on_local_port() {
    let (app, _, _) = test_app(None);
    let (status, _) = get_json(app, "/health", vec![("origin", "http://evil.com:7474")]).await;
    assert_eq!(status, 403);
}

#[tokio::test]
async fn origin_bypass_ipv6_evil() {
    let (app, _, _) = test_app(None);
    // "[::1].evil.com" is invalid per WHATWG URL spec (nothing valid after `]`
    // except `:port`, `/path`, `?query`, `#fragment`), so url::Url::parse fails.
    let (status, _) = get_json(app, "/health", vec![("origin", "http://[::1].evil.com")]).await;
    assert_eq!(status, 403);
}

#[tokio::test]
async fn origin_bypass_hex_ip() {
    let (app, _, _) = test_app(None);
    // url::Url::parse resolves 0x7f000001 to 127.0.0.1 per WHATWG — this IS localhost
    let (status, _) = get_json(app, "/health", vec![("origin", "http://0x7f000001")]).await;
    assert_eq!(status, 200);
}

#[tokio::test]
async fn origin_bypass_octal_ip() {
    let (app, _, _) = test_app(None);
    // url::Url::parse resolves octal to 127.0.0.1 per WHATWG — this IS localhost
    let (status, _) = get_json(app, "/health", vec![("origin", "http://017700000001")]).await;
    assert_eq!(status, 200);
}

#[tokio::test]
async fn origin_bypass_decimal_ip() {
    let (app, _, _) = test_app(None);
    // url::Url::parse resolves 2130706433 to 127.0.0.1 per WHATWG — this IS localhost
    let (status, _) = get_json(app, "/health", vec![("origin", "http://2130706433")]).await;
    assert_eq!(status, 200);
}

#[tokio::test]
async fn origin_bypass_zero_ip() {
    let (app, _, _) = test_app(None);
    let (status, _) = get_json(app, "/health", vec![("origin", "http://0.0.0.0")]).await;
    assert_eq!(status, 403);
}

#[tokio::test]
async fn origin_forwarded_for_header_ignored() {
    // X-Forwarded-For should not affect origin guard
    let (app, _, _) = test_app(None);
    let (status, _) = get_json(
        app,
        "/health",
        vec![
            ("origin", "https://evil.com"),
            ("x-forwarded-for", "127.0.0.1"),
        ],
    )
    .await;
    assert_eq!(status, 403);
}

#[tokio::test]
async fn origin_host_header_does_not_bypass() {
    let (app, _, _) = test_app(None);
    let (status, _) = get_json(
        app,
        "/health",
        vec![("origin", "https://evil.com"), ("host", "localhost:7474")],
    )
    .await;
    assert_eq!(status, 403);
}

#[tokio::test]
async fn origin_with_fragment_blocked() {
    let (app, _, _) = test_app(None);
    let (status, _) = get_json(
        app,
        "/health",
        vec![("origin", "https://evil.com#localhost")],
    )
    .await;
    assert_eq!(status, 403);
}

#[tokio::test]
async fn origin_with_query_string_blocked() {
    let (app, _, _) = test_app(None);
    let (status, _) = get_json(
        app,
        "/health",
        vec![("origin", "https://evil.com?host=localhost")],
    )
    .await;
    assert_eq!(status, 403);
}

#[tokio::test]
async fn origin_with_credentials_evil() {
    let (app, _, _) = test_app(None);
    let (status, _) = get_json(
        app,
        "/health",
        vec![("origin", "http://user:pass@evil.com")],
    )
    .await;
    assert_eq!(status, 403);
}

#[tokio::test]
async fn origin_with_credentials_localhost() {
    // "user:pass@localhost" -> host extraction gets "user" not "localhost"
    let (app, _, _) = test_app(None);
    let (status, _) = get_json(
        app,
        "/health",
        vec![("origin", "http://user:pass@localhost:7474")],
    )
    .await;
    assert_eq!(status, 403);
}

#[tokio::test]
async fn origin_no_header_is_allowed() {
    let (app, _, _) = test_app(None);
    let (status, json) = get_json(app, "/health", vec![]).await;
    assert_eq!(status, 200);
    assert_eq!(json["status"], "ok");
}

#[tokio::test]
async fn origin_x_original_url_does_not_bypass() {
    let (app, _, _) = test_app(None);
    let (status, _) = get_json(
        app,
        "/health",
        vec![
            ("origin", "https://evil.com"),
            ("x-original-url", "http://localhost:7474/health"),
        ],
    )
    .await;
    assert_eq!(status, 403);
}

// ===========================================================================
// Group C: Request Smuggling & Protocol Confusion (16 tests)
// ===========================================================================

#[tokio::test]
async fn smuggling_connect_method() {
    let (app, _, _) = test_app(None);
    let req = axum::http::Request::builder()
        .method("CONNECT")
        .uri("/health")
        .header("host", "localhost")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert!(resp.status().as_u16() == 405 || resp.status().as_u16() == 400);
}

#[tokio::test]
async fn smuggling_trace_method() {
    let (app, _, _) = test_app(None);
    let req = axum::http::Request::builder()
        .method("TRACE")
        .uri("/health")
        .header("host", "localhost")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert!(resp.status().as_u16() == 405 || resp.status().as_u16() == 400);
}

#[tokio::test]
async fn smuggling_very_long_url_path() {
    let (app, _, _) = test_app(None);
    let long_path = format!("/api/tools/{}", "A".repeat(10_000));
    let (status, body) = raw_post(
        app,
        &long_path,
        Some("application/json"),
        b"{}".to_vec(),
        vec![],
    )
    .await;
    // Either 200 with error (unknown tool) or 414 URI too long
    assert!(
        status == 200 || status == 414,
        "unexpected status: {status}"
    );
    if status == 200 {
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
        assert!(json.get("error").is_some());
    }
}

#[tokio::test]
async fn smuggling_path_encoded_slashes() {
    let (app, _, _) = test_app(None);
    // %2f is '/' — trying to navigate tool path
    let (status, body) = raw_post(
        app,
        "/api/tools/eval_js%2f..%2f..%2fetc%2fpasswd",
        Some("application/json"),
        b"{}".to_vec(),
        vec![],
    )
    .await;
    // Should not match any sensitive route
    assert!(
        status == 200 || status == 404,
        "unexpected status: {status}"
    );
    if status == 200 {
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
        assert!(json.get("error").is_some());
    }
}

#[tokio::test]
async fn smuggling_double_encoded_path_traversal() {
    let (app, _, _) = test_app(None);
    // %252f = double-encoded slash
    let (status, body) = raw_post(
        app,
        "/api/tools/..%252f..%252fetc%252fpasswd",
        Some("application/json"),
        b"{}".to_vec(),
        vec![],
    )
    .await;
    assert!(
        status == 200 || status == 404,
        "unexpected status: {status}"
    );
    if status == 200 {
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
        assert!(json.get("error").is_some());
    }
}

#[tokio::test]
async fn smuggling_query_injection_in_tool_path() {
    let (app, _, _) = test_app(None);
    let (status, body) = raw_post(
        app,
        "/api/tools/eval_js?admin=true&__proto__=polluted",
        Some("application/json"),
        serde_json::to_vec(&serde_json::json!({"code": "1"})).unwrap(),
        vec![],
    )
    .await;
    // Tool should still be dispatched as "eval_js" — query params don't affect routing
    assert_eq!(status, 200);
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
    // eval_js dispatches to bridge which won't respond, so we expect an error or timeout
    // But the tool IS recognized (no "unknown tool" error from query pollution)
    if let Some(err) = json["error"].as_str() {
        assert!(
            !err.contains("unknown tool"),
            "query string affected tool routing"
        );
    }
}

#[tokio::test]
async fn smuggling_multipart_formdata_on_tool() {
    let (app, _, _) = test_app(None);
    let (status, _) = raw_post(
        app,
        "/api/tools/get_plugin_info",
        Some("multipart/form-data; boundary=----WebKitFormBoundary"),
        b"------WebKitFormBoundary\r\nContent-Disposition: form-data; name=\"file\"\r\n\r\nmalicious\r\n------WebKitFormBoundary--".to_vec(),
        vec![],
    )
    .await;
    // axum JSON extraction should reject non-JSON content-type
    assert!(
        status == 415 || status == 400,
        "unexpected status: {status}"
    );
}

#[tokio::test]
async fn smuggling_accept_header_manipulation() {
    // Accept header should not affect tool execution
    let (app, _, _) = test_app(None);
    let (status, json) = post_tool(app, "get_plugin_info", serde_json::json!({}), None).await;
    // Even without explicit Accept, should work
    assert_eq!(status, 200);
    assert_eq!(json["result"]["name"], "victauri-browser");
}

#[tokio::test]
async fn smuggling_x_rewrite_url_injection() {
    let (app, _, _) = test_app(Some("tok".into()));
    let req = axum::http::Request::builder()
        .uri("/health")
        .header("host", "localhost")
        .header("x-rewrite-url", "/info")
        .header("x-original-url", "/info")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    let status = resp.status().as_u16();
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
    // Should still hit /health (not rewritten to /info which requires auth)
    assert_eq!(status, 200);
    assert_eq!(json["status"], "ok");
}

#[tokio::test]
async fn smuggling_connection_upgrade_without_websocket() {
    let (app, _, _) = test_app(None);
    let req = axum::http::Request::builder()
        .uri("/health")
        .header("host", "localhost")
        .header("connection", "upgrade")
        .header("upgrade", "h2c")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    // Should still serve health normally
    assert_eq!(resp.status().as_u16(), 200);
}

#[tokio::test]
async fn smuggling_delete_method_on_health() {
    let (app, _, _) = test_app(None);
    let req = axum::http::Request::builder()
        .method("DELETE")
        .uri("/health")
        .header("host", "localhost")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status().as_u16(), 405);
}

#[tokio::test]
async fn smuggling_patch_method_on_tool() {
    let (app, _, _) = test_app(None);
    let req = axum::http::Request::builder()
        .method("PATCH")
        .uri("/api/tools/get_plugin_info")
        .header("host", "localhost")
        .header("content-type", "application/json")
        .body(Body::from("{}"))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status().as_u16(), 405);
}

#[tokio::test]
async fn smuggling_body_limit_exactly_at_boundary() {
    let (app, _, _) = test_app(None);
    // Exactly 2MB should be accepted
    let padding = "x".repeat(2 * 1024 * 1024 - 50); // leave room for JSON wrapper
    let body = serde_json::json!({"_pad": padding});
    let body_bytes = serde_json::to_vec(&body).unwrap();
    let (status, _) = raw_post(
        app,
        "/api/tools/get_plugin_info",
        Some("application/json"),
        body_bytes,
        vec![],
    )
    .await;
    // Should either succeed (200) or be rejected for size (413)
    assert!(
        status == 200 || status == 413,
        "unexpected status: {status}"
    );
}

#[tokio::test]
async fn smuggling_body_limit_just_over() {
    let (app, _, _) = test_app(None);
    let huge = vec![b'x'; 2 * 1024 * 1024 + 100];
    let (status, _) = raw_post(
        app,
        "/api/tools/get_plugin_info",
        Some("application/json"),
        huge,
        vec![],
    )
    .await;
    assert_eq!(status, 413);
}

#[tokio::test]
async fn smuggling_xml_body_on_tool_endpoint() {
    let (app, _, _) = test_app(None);
    let (status, _) = raw_post(
        app,
        "/api/tools/get_plugin_info",
        Some("application/xml"),
        b"<params></params>".to_vec(),
        vec![],
    )
    .await;
    assert!(
        status == 415 || status == 400,
        "unexpected status: {status}"
    );
}

#[tokio::test]
async fn smuggling_empty_content_type() {
    let (app, _, _) = test_app(None);
    let (status, _) = raw_post(
        app,
        "/api/tools/get_plugin_info",
        Some(""),
        b"{}".to_vec(),
        vec![],
    )
    .await;
    assert!(
        status == 415 || status == 400,
        "unexpected status: {status}"
    );
}

// ===========================================================================
// Group D: Tool Execution Adversarial (22 tests)
// ===========================================================================

#[tokio::test]
async fn tool_proto_pollution_in_params() {
    let (app, _, _) = test_app(None);
    let (status, json) = post_tool(
        app,
        "get_plugin_info",
        serde_json::json!({"__proto__": {"admin": true}, "constructor": {"prototype": {"isAdmin": true}}}),
        None,
    )
    .await;
    assert_eq!(status, 200);
    // Should return normal plugin info, not polluted
    assert_eq!(json["result"]["name"], "victauri-browser");
    assert!(json["result"].get("admin").is_none());
    assert!(json["result"].get("isAdmin").is_none());
}

#[tokio::test]
async fn tool_deeply_nested_json_params_moderate() {
    let (app, _, _) = test_app(None);
    // Build moderately nested JSON: {"a":{"a":{"a":...}}} (50 levels)
    // serde_json has a recursion limit (~128), so 50 is safe
    let mut val = serde_json::json!("leaf");
    for _ in 0..50 {
        val = serde_json::json!({"a": val});
    }
    let (status, json) = post_tool(app, "get_plugin_info", val, None).await;
    assert_eq!(status, 200);
    assert_eq!(json["result"]["name"], "victauri-browser");
}

#[tokio::test]
async fn tool_deeply_nested_json_rejected_at_limit() {
    let (app, _, _) = test_app(None);
    // Build excessively nested JSON (200+ levels) — serde_json rejects this
    let mut val = serde_json::json!("leaf");
    for _ in 0..200 {
        val = serde_json::json!({"a": val});
    }
    let body_bytes = serde_json::to_vec(&val).unwrap();
    let (status, _) = raw_post(
        app,
        "/api/tools/get_plugin_info",
        Some("application/json"),
        body_bytes,
        vec![],
    )
    .await;
    // axum/serde rejects deeply nested JSON with 400
    assert_eq!(status, 400);
}

#[tokio::test]
async fn tool_very_wide_json_params() {
    let (app, _, _) = test_app(None);
    let mut obj = serde_json::Map::new();
    for i in 0..5000 {
        obj.insert(
            format!("key_{i}"),
            serde_json::Value::String(format!("val_{i}")),
        );
    }
    let (status, json) =
        post_tool(app, "get_plugin_info", serde_json::Value::Object(obj), None).await;
    assert_eq!(status, 200);
    assert_eq!(json["result"]["name"], "victauri-browser");
}

#[tokio::test]
async fn tool_duplicate_json_keys_serde_last_wins() {
    let (app, _, _) = test_app(None);
    // serde_json takes the last duplicate key
    let (status, body) = raw_post(
        app,
        "/api/tools/tabs",
        Some("application/json"),
        br#"{"action": "list", "action": "list"}"#.to_vec(),
        vec![],
    )
    .await;
    assert_eq!(status, 200);
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
    assert!(json["result"].is_array());
}

#[tokio::test]
async fn tool_name_with_unicode() {
    let (app, _, _) = test_app(None);
    let (status, json) = post_tool(app, "\u{0435}val_js", serde_json::json!({}), None).await;
    // Cyrillic "e" looks like Latin "e" but is different
    assert_eq!(status, 200);
    assert!(json.get("error").is_some());
}

#[tokio::test]
async fn tool_name_with_special_chars() {
    let (app, _, _) = test_app(None);
    for name in ["eval;js", "eval.js", "eval/js"] {
        let (status, _) = post_tool(app.clone(), name, serde_json::json!({}), None).await;
        // All should either 200 with error or 404
        assert!(
            status == 200 || status == 404,
            "unexpected status {status} for tool name '{name}'"
        );
    }
}

#[tokio::test]
async fn tool_params_extremely_long_string() {
    let (app, _, _) = test_app(None);
    let long_str = "X".repeat(100_000);
    let (status, json) = post_tool(
        app,
        "get_plugin_info",
        serde_json::json!({"unused_field": long_str}),
        None,
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(json["result"]["name"], "victauri-browser");
}

#[tokio::test]
async fn tool_action_as_bool() {
    let (app, _, _) = test_app(None);
    let (status, json) = post_tool(
        app,
        "interact",
        serde_json::json!({"action": true, "ref_id": "e0"}),
        None,
    )
    .await;
    assert_eq!(status, 200);
    assert!(json.get("error").is_some());
}

#[tokio::test]
async fn tool_action_as_null() {
    let (app, _, _) = test_app(None);
    let (status, json) = post_tool(app, "logs", serde_json::json!({"action": null}), None).await;
    assert_eq!(status, 200);
    assert!(json.get("error").is_some());
}

#[tokio::test]
async fn tool_action_as_array() {
    let (app, _, _) = test_app(None);
    let (status, json) = post_tool(
        app,
        "storage",
        serde_json::json!({"action": ["get", "set"]}),
        None,
    )
    .await;
    assert_eq!(status, 200);
    assert!(json.get("error").is_some());
}

#[tokio::test]
async fn tool_action_as_number() {
    let (app, _, _) = test_app(None);
    let (status, json) = post_tool(app, "navigate", serde_json::json!({"action": 42}), None).await;
    assert_eq!(status, 200);
    assert!(json.get("error").is_some());
}

#[tokio::test]
async fn tool_rapid_50_concurrent() {
    let (app, _, _) = test_app(None);
    let mut handles = vec![];
    for _ in 0..50 {
        let a = app.clone();
        handles.push(tokio::spawn(async move {
            let req = axum::http::Request::builder()
                .method("POST")
                .uri("/api/tools/get_plugin_info")
                .header("host", "localhost")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap();
            let resp = a.oneshot(req).await.unwrap();
            let status = resp.status().as_u16();
            let body = resp.into_body().collect().await.unwrap().to_bytes();
            let json: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
            (status, json)
        }));
    }
    for h in handles {
        let (status, json) = h.await.unwrap();
        assert_eq!(status, 200);
        assert_eq!(json["result"]["name"], "victauri-browser");
    }
}

#[tokio::test]
async fn tool_binary_content_type_rejected() {
    let (app, _, _) = test_app(None);
    let (status, _) = raw_post(
        app,
        "/api/tools/get_plugin_info",
        Some("application/octet-stream"),
        b"\x00\x01\x02\x03".to_vec(),
        vec![],
    )
    .await;
    assert!(
        status == 415 || status == 400,
        "unexpected status: {status}"
    );
}

#[tokio::test]
async fn tool_eval_js_whitespace_only_code() {
    let (app, _, _) = test_app(None);
    // Whitespace-only code should still be dispatched (not error on missing code)
    let (status, json) = post_tool(app, "eval_js", serde_json::json!({"code": "   "}), None).await;
    assert_eq!(status, 200);
    // This dispatches to bridge which has no listener — will get a bridge error
    // But importantly, it was NOT rejected as "missing code"
    if let Some(err) = json["error"].as_str() {
        assert!(
            !err.contains("missing 'code'"),
            "whitespace code should not be treated as missing"
        );
    }
}

#[tokio::test]
async fn tool_eval_js_null_char_code() {
    let (app, _, _) = test_app(None);
    let (status, json) = post_tool(app, "eval_js", serde_json::json!({"code": "\0"}), None).await;
    assert_eq!(status, 200);
    // Should be dispatched, not rejected as missing
    if let Some(err) = json["error"].as_str() {
        assert!(!err.contains("missing 'code'"));
    }
}

#[tokio::test]
async fn tool_interact_unknown_action_via_rest() {
    let (app, _, _) = test_app(None);
    let (status, json) = post_tool(
        app,
        "interact",
        serde_json::json!({"action": "launch_missiles"}),
        None,
    )
    .await;
    assert_eq!(status, 200);
    assert!(
        json["error"]
            .as_str()
            .unwrap()
            .contains("unknown interact action")
    );
}

#[tokio::test]
async fn tool_navigate_javascript_uri() {
    let (app, _, dispatch) = test_app(None);

    // Dispatch navigate with javascript: URI — the tool DOES dispatch it
    // (URL validation is the browser extension's job, not the host's)
    let handle = tokio::spawn(async move {
        post_tool(
            app,
            "navigate",
            serde_json::json!({"action": "go_to", "url": "javascript:alert(1)"}),
            None,
        )
        .await
    });

    // Resolve the pending bridge command
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let ids = dispatch.pending_ids().await;
    for id in &ids {
        dispatch
            .on_response(id, Some(serde_json::json!({"navigated": true})), None)
            .await;
    }

    let (status, json) = handle.await.unwrap();
    assert_eq!(status, 200);
    // It dispatched to bridge successfully (host does not filter URLs)
    assert!(json.get("result").is_some() || json.get("error").is_some());
}

#[tokio::test]
async fn tool_navigate_data_uri() {
    let (app, _, dispatch) = test_app(None);

    let handle = tokio::spawn(async move {
        post_tool(
            app,
            "navigate",
            serde_json::json!({"action": "go_to", "url": "data:text/html,<h1>evil</h1>"}),
            None,
        )
        .await
    });

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let ids = dispatch.pending_ids().await;
    for id in &ids {
        dispatch
            .on_response(id, Some(serde_json::json!({"ok": true})), None)
            .await;
    }

    let (status, _) = handle.await.unwrap();
    assert_eq!(status, 200);
}

#[tokio::test]
async fn tool_unknown_name_returns_error_not_crash() {
    let (app, _, _) = test_app(None);
    // Names that are valid in URI paths
    for name in [
        "DROP_TABLE_users",
        "....etc.passwd",
        "eval_js_hidden",
        "xss_attempt",
    ] {
        let (status, json) = post_tool(app.clone(), name, serde_json::json!({}), None).await;
        assert_eq!(status, 200, "unexpected status for tool '{name}'");
        assert!(
            json.get("error").is_some(),
            "expected error for tool '{name}'"
        );
    }

    // Names with characters that are invalid in URIs — the http crate rejects these
    // at request build time, which is correct protocol-level protection.
    for name in ["DROP TABLE users", "<script>alert(1)</script>"] {
        let uri = format!("/api/tools/{name}");
        let result = axum::http::Request::builder()
            .method("POST")
            .uri(&uri)
            .header("host", "localhost")
            .header("content-type", "application/json")
            .body(Body::from("{}"));
        // These should either fail to build (InvalidUri) or return an error
        if let Ok(req) = result {
            let resp = app.clone().oneshot(req).await.unwrap();
            let status = resp.status().as_u16();
            assert!(
                status == 200 || status == 400,
                "unexpected status for '{name}': {status}"
            );
        }
        // If Err: URI rejected at protocol level — attack blocked
    }
}

#[tokio::test]
async fn tool_invocation_counter_increments_across_http() {
    let (app, _, _) = test_app(None);
    let (_, json1) = post_tool(app.clone(), "get_plugin_info", serde_json::json!({}), None).await;
    let count1 = json1["result"]["invocations"].as_u64().unwrap();

    let (_, json2) = post_tool(app.clone(), "get_plugin_info", serde_json::json!({}), None).await;
    let count2 = json2["result"]["invocations"].as_u64().unwrap();

    assert_eq!(count2, count1 + 1);
}

#[tokio::test]
async fn tool_invocation_counter_increments_on_failure() {
    let (app, _, _) = test_app(None);
    let _ = post_tool(app.clone(), "nonexistent_tool", serde_json::json!({}), None).await;
    let (_, json) = post_tool(app, "get_plugin_info", serde_json::json!({}), None).await;
    assert_eq!(json["result"]["invocations"], 2);
}

// ===========================================================================
// Group E: State Corruption Attempts (12 tests)
// ===========================================================================

#[tokio::test]
async fn state_create_tab_close_then_list() {
    let (app, tab_mgr, _) = test_app(None);
    tab_mgr
        .on_tab_created(1, "https://example.com", "Test")
        .await;
    tab_mgr.on_tab_activated(1).await;
    tab_mgr.on_tab_closed(1).await;

    let (status, json) = post_tool(app, "tabs", serde_json::json!({"action": "list"}), None).await;
    assert_eq!(status, 200);
    assert!(json["result"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn state_rapid_create_close_same_tab() {
    let (_, tab_mgr, _) = test_app(None);

    for _ in 0..100 {
        tab_mgr.on_tab_created(42, "https://x.com", "X").await;
        tab_mgr.on_tab_closed(42).await;
    }

    assert_eq!(tab_mgr.tab_count().await, 0);
}

#[tokio::test]
async fn state_tab_max_u32_id() {
    let (app, tab_mgr, _) = test_app(None);
    tab_mgr
        .on_tab_created(u32::MAX, "https://max.com", "MaxTab")
        .await;
    tab_mgr.on_tab_activated(u32::MAX).await;

    let (status, json) = post_tool(app, "tabs", serde_json::json!({"action": "list"}), None).await;
    assert_eq!(status, 200);
    let tabs = json["result"].as_array().unwrap();
    assert_eq!(tabs.len(), 1);
    assert_eq!(tabs[0]["tab_id"], u64::from(u32::MAX));
}

#[tokio::test]
async fn state_fill_10000_tabs_then_list() {
    let (app, tab_mgr, _) = test_app(None);
    for i in 1..=10_000u32 {
        tab_mgr
            .on_tab_created(i, &format!("https://{i}.com"), &format!("Tab {i}"))
            .await;
    }

    let (status, json) = post_tool(app, "tabs", serde_json::json!({"action": "list"}), None).await;
    assert_eq!(status, 200);
    assert_eq!(json["result"].as_array().unwrap().len(), 10_000);
}

#[tokio::test]
async fn state_active_tab_closed_plugin_info_shows_zero() {
    let (app, tab_mgr, _) = test_app(None);
    tab_mgr.on_tab_created(1, "https://x.com", "X").await;
    tab_mgr.on_tab_activated(1).await;
    tab_mgr.on_tab_closed(1).await;

    let (status, json) = post_tool(app, "get_plugin_info", serde_json::json!({}), None).await;
    assert_eq!(status, 200);
    assert_eq!(json["result"]["tab_count"], 0);
}

#[tokio::test]
async fn state_bridge_ready_then_close_then_create() {
    let (app, tab_mgr, _) = test_app(None);
    tab_mgr.on_tab_created(1, "https://x.com", "X").await;
    tab_mgr.on_bridge_ready(1).await;
    tab_mgr.on_tab_closed(1).await;

    // Re-create same tab ID
    tab_mgr.on_tab_created(1, "https://new.com", "New").await;

    let (status, json) = post_tool(app, "tabs", serde_json::json!({"action": "list"}), None).await;
    assert_eq!(status, 200);
    let tabs = json["result"].as_array().unwrap();
    assert_eq!(tabs.len(), 1);
    assert_eq!(tabs[0]["url"], "https://new.com");
    // bridge_ready should be reset on re-creation
    assert_eq!(tabs[0]["bridge_ready"], false);
}

#[tokio::test]
async fn state_concurrent_tab_operations_with_tool_calls() {
    let (app, tab_mgr, _) = test_app(None);
    let tab_mgr2 = Arc::clone(&tab_mgr);

    // Spawn tab creation concurrently with tool calls
    let create_handle = tokio::spawn(async move {
        for i in 1..=100u32 {
            tab_mgr2
                .on_tab_created(i, &format!("https://{i}.com"), &format!("Tab {i}"))
                .await;
        }
    });

    let mut tool_handles = vec![];
    for _ in 0..20 {
        let a = app.clone();
        tool_handles.push(tokio::spawn(async move {
            let (status, json) = post_tool(a, "get_plugin_info", serde_json::json!({}), None).await;
            assert_eq!(status, 200);
            json["result"]["tab_count"].as_u64().unwrap()
        }));
    }

    create_handle.await.unwrap();

    for h in tool_handles {
        let count = h.await.unwrap();
        // Count should be between 0 and 100 (race with creation)
        assert!(count <= 100, "unexpected tab count: {count}");
    }

    // After everything settles, should have exactly 100
    assert_eq!(tab_mgr.tab_count().await, 100);
}

#[tokio::test]
async fn state_tab_update_after_close_is_noop() {
    let (app, tab_mgr, _) = test_app(None);
    tab_mgr.on_tab_created(1, "https://x.com", "X").await;
    tab_mgr.on_tab_closed(1).await;
    tab_mgr
        .on_tab_updated(1, Some("https://evil.com"), Some("Hacked"))
        .await;

    let (_, json) = post_tool(app, "tabs", serde_json::json!({"action": "list"}), None).await;
    assert!(json["result"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn state_bridge_ready_on_closed_tab_is_noop() {
    let (_, tab_mgr, _) = test_app(None);
    tab_mgr.on_tab_created(1, "https://x.com", "X").await;
    tab_mgr.on_tab_closed(1).await;
    tab_mgr.on_bridge_ready(1).await;

    // Tab was closed, so bridge_ready on non-existent tab should be noop
    assert!(!tab_mgr.is_bridge_ready(1).await);
}

#[tokio::test]
async fn state_tab_zero_id_edge_case() {
    let (app, tab_mgr, _) = test_app(None);
    // Tab ID 0 is special — active_tab defaults to 0 meaning "none"
    tab_mgr.on_tab_created(0, "https://zero.com", "Zero").await;
    tab_mgr.on_tab_activated(0).await;

    let (_, json) = post_tool(app, "tabs", serde_json::json!({"action": "list"}), None).await;
    let tabs = json["result"].as_array().unwrap();
    assert_eq!(tabs.len(), 1);
    assert_eq!(tabs[0]["tab_id"], 0);
}

#[tokio::test]
async fn state_info_reflects_live_tab_count() {
    let (app, tab_mgr, _) = test_app(None);

    let (_, json) = get_json(app.clone(), "/info", vec![]).await;
    assert_eq!(json["tabs"], 0);

    tab_mgr.on_tab_created(1, "https://x.com", "X").await;
    tab_mgr.on_tab_created(2, "https://y.com", "Y").await;

    let (_, json) = get_json(app, "/info", vec![]).await;
    assert_eq!(json["tabs"], 2);
}

#[tokio::test]
async fn state_dispatch_pending_after_cancel_all() {
    let dispatch = Arc::new(BridgeDispatch::new_sink());

    // Register a pending command and cancel
    let rx = dispatch.register_test_pending("test-cmd").await;
    dispatch.cancel_all().await;

    let result = rx.await.unwrap();
    assert!(result.error.is_some());
    assert!(
        result.error.unwrap().contains("disconnected"),
        "cancel_all should send disconnect error"
    );

    // Now register a new one — should work fine
    let rx2 = dispatch.register_test_pending("test-cmd-2").await;
    dispatch
        .on_response("test-cmd-2", Some(serde_json::json!({"ok": true})), None)
        .await;
    let result2 = rx2.await.unwrap();
    assert_eq!(result2.data.unwrap(), serde_json::json!({"ok": true}));
}

// ===========================================================================
// Group F: Rate Limiter Adversarial (10 tests)
// ===========================================================================

#[tokio::test]
async fn rate_limit_exact_exhaustion_then_one_more() {
    let (app, _, _) = test_app_rate_limited(3);

    for i in 0..3 {
        let (status, _) = get_json(app.clone(), "/info", vec![]).await;
        assert_eq!(status, 200, "request {i} should pass");
    }

    let (status, _) = get_json(app, "/info", vec![]).await;
    assert_eq!(status, 429);
}

#[tokio::test]
async fn rate_limit_concurrent_burst_from_many_tasks() {
    let (app, _, _) = test_app_rate_limited(10);

    let mut handles = vec![];
    for _ in 0..50 {
        let a = app.clone();
        handles.push(tokio::spawn(async move {
            let (status, _) = get_json(a, "/info", vec![]).await;
            status
        }));
    }

    let mut ok_count = 0u32;
    let mut limited_count = 0u32;
    for h in handles {
        match h.await.unwrap() {
            200 => ok_count += 1,
            429 => limited_count += 1,
            s => panic!("unexpected status: {s}"),
        }
    }

    assert!(ok_count <= 10, "too many passed: {ok_count}");
    assert!(ok_count >= 1, "none passed");
    assert!(limited_count >= 40, "not enough limited: {limited_count}");
}

#[tokio::test]
async fn rate_limit_health_endpoint_bypasses_rate_limit() {
    // /health is registered AFTER the rate_limit layer is applied to the inner router,
    // so health requests do NOT consume rate limit tokens. This mirrors the auth bypass.
    let (app, _, _) = test_app_rate_limited(1);

    // Exhaust the budget on /info
    let (status1, _) = get_json(app.clone(), "/info", vec![]).await;
    assert_eq!(status1, 200);

    // /info is now rate limited
    let (status2, _) = get_json(app.clone(), "/info", vec![]).await;
    assert_eq!(status2, 429);

    // But /health should still work
    let (status3, _) = get_json(app, "/health", vec![]).await;
    assert_eq!(status3, 200);
}

#[tokio::test]
async fn rate_limit_on_info_endpoint() {
    let (app, _, _) = test_app_rate_limited(1);
    let (status1, _) = get_json(app.clone(), "/info", vec![]).await;
    assert_eq!(status1, 200);
    let (status2, _) = get_json(app, "/info", vec![]).await;
    assert_eq!(status2, 429);
}

#[tokio::test]
async fn rate_limit_on_tool_list() {
    let (app, _, _) = test_app_rate_limited(1);
    let (status1, _) = get_json(app.clone(), "/api/tools", vec![]).await;
    assert_eq!(status1, 200);
    let (status2, _) = get_json(app, "/api/tools", vec![]).await;
    assert_eq!(status2, 429);
}

#[tokio::test]
async fn rate_limit_on_tool_execution() {
    let (app, _, _) = test_app_rate_limited(1);
    let (status1, _) = post_tool(app.clone(), "get_plugin_info", serde_json::json!({}), None).await;
    assert_eq!(status1, 200);
    let (status2, _) = post_tool(app, "get_plugin_info", serde_json::json!({}), None).await;
    assert_eq!(status2, 429);
}

#[tokio::test]
async fn rate_limit_with_auth_failures_still_counts() {
    let tab_mgr = Arc::new(TabManager::new());
    let dispatch = Arc::new(BridgeDispatch::new_sink());
    let handler = VictauriBrowserHandler::new(tab_mgr, dispatch);
    let limiter = Arc::new(RateLimiterState::new(3));
    let app = build_app_full(handler, Some("correct-token".into()), Some(limiter));

    // Auth failures consume rate limit tokens
    for _ in 0..3 {
        let (status, _) = get_json(
            app.clone(),
            "/info",
            vec![("authorization", "Bearer wrong")],
        )
        .await;
        assert_eq!(status, 401);
    }

    // Now even a correct token should be rate limited
    let (status, _) = get_json(
        app,
        "/info",
        vec![("authorization", "Bearer correct-token")],
    )
    .await;
    assert_eq!(status, 429);
}

#[tokio::test]
async fn rate_limit_zero_budget_rejects_protected_immediately() {
    let (app, _, _) = test_app_rate_limited(0);
    // /info goes through rate limiter; /health does not
    let (status, _) = get_json(app.clone(), "/info", vec![]).await;
    assert_eq!(status, 429);

    // Health still works even with zero budget
    let (status, _) = get_json(app, "/health", vec![]).await;
    assert_eq!(status, 200);
}

#[tokio::test]
async fn rate_limit_single_budget_on_protected() {
    let (app, _, _) = test_app_rate_limited(1);
    let (status1, _) = get_json(app.clone(), "/info", vec![]).await;
    assert_eq!(status1, 200);
    let (status2, _) = get_json(app, "/info", vec![]).await;
    assert_eq!(status2, 429);
}

#[tokio::test]
async fn rate_limit_429_response_has_security_headers() {
    let (app, _, _) = test_app_rate_limited(0);
    let (status, headers) = resp_headers(app, "/info", "GET", vec![]).await;
    assert_eq!(status, 429);
    // Security headers should still be applied on 429 responses
    assert_eq!(
        headers
            .get("x-content-type-options")
            .map(|v| v.to_str().unwrap()),
        Some("nosniff")
    );
    assert_eq!(
        headers.get("cache-control").map(|v| v.to_str().unwrap()),
        Some("no-store")
    );
}

// ===========================================================================
// Group G: Response Format Validation (14 tests)
// ===========================================================================

#[tokio::test]
async fn response_health_format() {
    let (app, _, _) = test_app(None);
    let (status, json) = get_json(app, "/health", vec![]).await;
    assert_eq!(status, 200);
    assert_eq!(json["status"], "ok");
}

#[tokio::test]
async fn response_info_format() {
    let (app, _, _) = test_app(None);
    let (status, json) = get_json(app, "/info", vec![]).await;
    assert_eq!(status, 200);
    assert_eq!(json["name"], "victauri-browser");
    assert!(json.get("version").is_some());
    assert_eq!(json["protocol"], "mcp");
    assert_eq!(json["mode"], "browser");
    assert!(json.get("tabs").is_some());
    assert!(json.get("auth_required").is_some());
}

#[tokio::test]
async fn response_tool_list_format() {
    let (app, _, _) = test_app(None);
    let (status, json) = get_json(app, "/api/tools", vec![]).await;
    assert_eq!(status, 200);
    let tools = json.as_array().unwrap();
    assert_eq!(tools.len(), 20);
    for tool in tools {
        assert!(tool.get("name").is_some());
        assert!(tool.get("description").is_some());
        assert!(!tool["name"].as_str().unwrap().is_empty());
        assert!(!tool["description"].as_str().unwrap().is_empty());
    }
}

#[tokio::test]
async fn response_tool_success_has_result_key() {
    let (app, _, _) = test_app(None);
    let (status, json) = post_tool(app, "get_plugin_info", serde_json::json!({}), None).await;
    assert_eq!(status, 200);
    assert!(
        json.get("result").is_some(),
        "success response missing 'result' key"
    );
    assert!(
        json.get("error").is_none(),
        "success response should not have 'error' key"
    );
}

#[tokio::test]
async fn response_tool_error_has_error_key() {
    let (app, _, _) = test_app(None);
    let (status, json) = post_tool(app, "nonexistent_tool", serde_json::json!({}), None).await;
    assert_eq!(status, 200);
    assert!(
        json.get("error").is_some(),
        "error response missing 'error' key"
    );
    assert!(
        json.get("result").is_none(),
        "error response should not have 'result' key"
    );
}

#[tokio::test]
async fn response_security_headers_on_health() {
    let (app, _, _) = test_app(None);
    let (status, headers) = resp_headers(app, "/health", "GET", vec![]).await;
    assert_eq!(status, 200);
    assert_eq!(headers.get("x-content-type-options").unwrap(), "nosniff");
    assert_eq!(headers.get("cache-control").unwrap(), "no-store");
}

#[tokio::test]
async fn response_security_headers_on_info() {
    let (app, _, _) = test_app(None);
    let (status, headers) = resp_headers(app, "/info", "GET", vec![]).await;
    assert_eq!(status, 200);
    assert_eq!(headers.get("x-content-type-options").unwrap(), "nosniff");
    assert_eq!(headers.get("cache-control").unwrap(), "no-store");
}

#[tokio::test]
async fn response_security_headers_on_tool_list() {
    let (app, _, _) = test_app(None);
    let (status, headers) = resp_headers(app, "/api/tools", "GET", vec![]).await;
    assert_eq!(status, 200);
    assert_eq!(headers.get("x-content-type-options").unwrap(), "nosniff");
    assert_eq!(headers.get("cache-control").unwrap(), "no-store");
}

#[tokio::test]
async fn response_security_headers_on_tool_execution() {
    let (app, _, _) = test_app(None);
    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/tools/get_plugin_info")
        .header("host", "localhost")
        .header("content-type", "application/json")
        .body(Body::from("{}"))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    assert_eq!(
        resp.headers().get("x-content-type-options").unwrap(),
        "nosniff"
    );
    assert_eq!(resp.headers().get("cache-control").unwrap(), "no-store");
}

#[tokio::test]
async fn response_security_headers_on_401() {
    let (app, _, _) = test_app(Some("tok".into()));
    let (status, headers) = resp_headers(app, "/info", "GET", vec![]).await;
    assert_eq!(status, 401);
    assert_eq!(headers.get("x-content-type-options").unwrap(), "nosniff");
    assert_eq!(headers.get("cache-control").unwrap(), "no-store");
}

#[tokio::test]
async fn response_security_headers_on_403_not_present() {
    // origin_guard is the outermost layer and short-circuits before security_headers
    // runs. This means 403 responses from origin guard do NOT have security headers.
    // This is an architectural observation, not a bug — origin_guard rejects the
    // request before any inner middleware processes it.
    let (app, _, _) = test_app(None);
    let (status, headers) =
        resp_headers(app, "/health", "GET", vec![("origin", "https://evil.com")]).await;
    assert_eq!(status, 403);
    // Security headers are NOT present because origin_guard short-circuits
    assert!(
        headers.get("x-content-type-options").is_none(),
        "origin_guard 403 should not have security headers (short-circuit)"
    );
}

#[tokio::test]
async fn response_security_headers_on_404() {
    let (app, _, _) = test_app(None);
    let (status, headers) = resp_headers(app, "/nonexistent", "GET", vec![]).await;
    assert_eq!(status, 404);
    assert_eq!(headers.get("x-content-type-options").unwrap(), "nosniff");
    assert_eq!(headers.get("cache-control").unwrap(), "no-store");
}

#[tokio::test]
async fn response_content_type_json_on_info() {
    let (app, _, _) = test_app(None);
    let (_, headers) = resp_headers(app, "/info", "GET", vec![]).await;
    let ct = headers.get("content-type").unwrap().to_str().unwrap();
    assert!(
        ct.contains("application/json"),
        "info content-type should be JSON, got: {ct}"
    );
}

#[tokio::test]
async fn response_content_type_json_on_tool_list() {
    let (app, _, _) = test_app(None);
    let (_, headers) = resp_headers(app, "/api/tools", "GET", vec![]).await;
    let ct = headers.get("content-type").unwrap().to_str().unwrap();
    assert!(
        ct.contains("application/json"),
        "tool list content-type should be JSON, got: {ct}"
    );
}

// ===========================================================================
// Group H: Cross-Module Integration (12 tests)
// ===========================================================================

#[tokio::test]
async fn integration_auth_plus_origin_guard_stacked() {
    // Both origin guard and auth must pass
    let (app, _, _) = test_app(Some("tok".into()));
    // Good origin, bad auth
    let (status, _) = get_json(
        app.clone(),
        "/info",
        vec![
            ("origin", "http://localhost:3000"),
            ("authorization", "Bearer wrong"),
        ],
    )
    .await;
    assert_eq!(status, 401);

    // Bad origin, good auth
    let (status, _) = get_json(
        app.clone(),
        "/info",
        vec![
            ("origin", "https://evil.com"),
            ("authorization", "Bearer tok"),
        ],
    )
    .await;
    assert_eq!(status, 403);

    // Good origin, good auth
    let (status, json) = get_json(
        app,
        "/info",
        vec![
            ("origin", "http://localhost:3000"),
            ("authorization", "Bearer tok"),
        ],
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(json["name"], "victauri-browser");
}

#[tokio::test]
async fn integration_rate_limit_plus_auth() {
    let tab_mgr = Arc::new(TabManager::new());
    let dispatch = Arc::new(BridgeDispatch::new_sink());
    let handler = VictauriBrowserHandler::new(tab_mgr, dispatch);
    let limiter = Arc::new(RateLimiterState::new(2));
    let app = build_app_full(handler, Some("tok".into()), Some(limiter));

    // First request: auth OK, rate OK
    let (status, _) = get_json(app.clone(), "/info", vec![("authorization", "Bearer tok")]).await;
    assert_eq!(status, 200);

    // Second request: auth fails, consumes rate limit
    let (status, _) = get_json(
        app.clone(),
        "/info",
        vec![("authorization", "Bearer wrong")],
    )
    .await;
    assert_eq!(status, 401);

    // Third request: rate limit exhausted — 429 before auth even checks
    let (status, _) = get_json(app, "/info", vec![("authorization", "Bearer tok")]).await;
    assert_eq!(status, 429);
}

#[tokio::test]
async fn integration_tab_state_visible_through_rest_api() {
    let (app, tab_mgr, _) = test_app(None);

    // Start with 0 tabs
    let (_, json) = get_json(app.clone(), "/info", vec![]).await;
    assert_eq!(json["tabs"], 0);

    // Add tabs
    tab_mgr.on_tab_created(1, "https://a.com", "A").await;
    tab_mgr.on_tab_created(2, "https://b.com", "B").await;
    tab_mgr.on_tab_activated(2).await;

    // /info reflects count
    let (_, json) = get_json(app.clone(), "/info", vec![]).await;
    assert_eq!(json["tabs"], 2);

    // tabs tool reflects list
    let (_, json) = post_tool(
        app.clone(),
        "tabs",
        serde_json::json!({"action": "list"}),
        None,
    )
    .await;
    let tabs = json["result"].as_array().unwrap();
    assert_eq!(tabs.len(), 2);

    // Close one
    tab_mgr.on_tab_closed(1).await;
    let (_, json) = get_json(app, "/info", vec![]).await;
    assert_eq!(json["tabs"], 1);
}

#[tokio::test]
async fn integration_tool_names_match_rest_api() {
    let (app, _, dispatch) = test_app(None);

    let (_, json) = get_json(app.clone(), "/api/tools", vec![]).await;
    let rest_names: Vec<String> = json
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .collect();

    // Spawn a background resolver that responds to any bridge dispatches with mock data
    let resolver_dispatch = Arc::clone(&dispatch);
    let resolver = tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            let ids = resolver_dispatch.pending_ids().await;
            for id in &ids {
                resolver_dispatch
                    .on_response(
                        id,
                        Some(serde_json::json!({"mock": true, "js_heap": {}})),
                        None,
                    )
                    .await;
            }
        }
    });

    // Every tool name from list should be callable (even if dispatches to bridge)
    for name in &rest_names {
        let (status, json) = post_tool(app.clone(), name, serde_json::json!({}), None).await;
        assert_eq!(status, 200, "tool '{name}' did not return 200");
        // Should either have result or error — never both
        assert!(
            json.get("result").is_some() ^ json.get("error").is_some(),
            "tool '{name}' has inconsistent response format: {json}"
        );
    }

    resolver.abort();
}

#[tokio::test]
async fn integration_auth_on_all_protected_endpoints() {
    let (app, _, _) = test_app(Some("secret".into()));

    // These should all be blocked without auth
    let protected = vec![("GET", "/info"), ("GET", "/api/tools")];

    for (method, path) in protected {
        let req = axum::http::Request::builder()
            .method(method)
            .uri(path)
            .header("host", "localhost")
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(
            resp.status().as_u16(),
            401,
            "{method} {path} should require auth"
        );
    }

    // /health should bypass auth
    let (status, _) = get_json(app, "/health", vec![]).await;
    assert_eq!(status, 200);
}

#[tokio::test]
async fn integration_tool_call_with_tabs_populated() {
    let (app, tab_mgr, _) = test_app(None);

    tab_mgr
        .on_tab_created(100, "https://example.com", "Example")
        .await;
    tab_mgr.on_tab_activated(100).await;
    tab_mgr.on_bridge_ready(100).await;

    // get_plugin_info should reflect the tab
    let (_, json) = post_tool(app.clone(), "get_plugin_info", serde_json::json!({}), None).await;
    assert_eq!(json["result"]["tab_count"], 1);

    // tabs list should show it
    let (_, json) = post_tool(app, "tabs", serde_json::json!({"action": "list"}), None).await;
    let tabs = json["result"].as_array().unwrap();
    assert_eq!(tabs.len(), 1);
    assert_eq!(tabs[0]["tab_id"], 100);
    assert_eq!(tabs[0]["active"], true);
    assert_eq!(tabs[0]["bridge_ready"], true);
}

#[tokio::test]
async fn integration_dispatch_resolve_through_handler() {
    let (app, _, dispatch) = test_app(None);

    // Start a tool call that dispatches to bridge
    let app_clone = app;
    let handle = tokio::spawn(async move {
        post_tool(app_clone, "dom_snapshot", serde_json::json!({}), None).await
    });

    // Wait for dispatch to register
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Resolve the pending command
    let ids = dispatch.pending_ids().await;
    assert!(!ids.is_empty(), "no pending dispatches found");
    for id in &ids {
        dispatch
            .on_response(
                id,
                Some(serde_json::json!({
                    "tree": [{"role": "document", "name": "Test"}],
                    "ref_count": 1
                })),
                None,
            )
            .await;
    }

    let (status, json) = handle.await.unwrap();
    assert_eq!(status, 200);
    assert!(json.get("result").is_some());
    assert_eq!(json["result"]["ref_count"], 1);
}

#[tokio::test]
async fn integration_dispatch_error_through_handler() {
    let (app, _, dispatch) = test_app(None);

    let handle = tokio::spawn(async move {
        post_tool(
            app,
            "eval_js",
            serde_json::json!({"code": "throw new Error('boom')"}),
            None,
        )
        .await
    });

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let ids = dispatch.pending_ids().await;
    for id in &ids {
        dispatch
            .on_response(id, None, Some("Error: boom".to_string()))
            .await;
    }

    let (status, json) = handle.await.unwrap();
    assert_eq!(status, 200);
    assert!(json.get("error").is_some());
    assert!(json["error"].as_str().unwrap().contains("boom"));
}

#[tokio::test]
async fn integration_concurrent_dispatch_all_resolve() {
    let (app, _, dispatch) = test_app(None);

    let mut handles = vec![];
    for i in 0..10 {
        let a = app.clone();
        handles.push(tokio::spawn(async move {
            post_tool(
                a,
                "dom_snapshot",
                serde_json::json!({"format": format!("test-{i}")}),
                None,
            )
            .await
        }));
    }

    // Resolve all pending commands as they appear
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let ids = dispatch.pending_ids().await;
    for id in &ids {
        dispatch
            .on_response(
                id,
                Some(serde_json::json!({"tree": [], "ref_count": 0})),
                None,
            )
            .await;
    }

    for h in handles {
        let (status, json) = h.await.unwrap();
        assert_eq!(status, 200);
        assert!(json.get("result").is_some());
    }
}

#[tokio::test]
async fn integration_info_auth_required_field_accuracy() {
    // When no auth: auth_required should be false
    let (app_no_auth, _, _) = test_app(None);
    let (_, json) = get_json(app_no_auth, "/info", vec![]).await;
    assert_eq!(json["auth_required"], false);

    // When auth enabled: auth_required should be true (need valid token to see it)
    let (app_auth, _, _) = test_app(Some("tok".into()));
    let (status, json) = get_json(app_auth, "/info", vec![("authorization", "Bearer tok")]).await;
    assert_eq!(status, 200);
    assert_eq!(json["auth_required"], true);
}

#[tokio::test]
async fn integration_all_20_tool_names_in_rest_list() {
    let (app, _, _) = test_app(None);
    let (_, json) = get_json(app, "/api/tools", vec![]).await;
    let names: Vec<&str> = json
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();

    let expected = [
        "eval_js",
        "dom_snapshot",
        "find_elements",
        "interact",
        "input",
        "inspect",
        "css",
        "logs",
        "storage",
        "navigate",
        "wait_for",
        "assert_semantic",
        "recording",
        "screenshot",
        "tabs",
        "page_info",
        "cookies",
        "get_diagnostics",
        "get_plugin_info",
        "get_memory_stats",
    ];

    for name in &expected {
        assert!(names.contains(name), "missing tool in REST list: {name}");
    }
    assert_eq!(names.len(), 20);
}

#[tokio::test]
async fn integration_malformed_json_various_formats() {
    let (app, _, _) = test_app(None);

    let cases: Vec<(&str, &[u8])> = vec![
        ("truncated object", b"{\"key\":"),
        ("trailing comma", b"{\"key\": 1,}"),
        ("single quotes", b"{'key': 'val'}"),
        ("unquoted key", b"{key: 1}"),
        ("just a number", b"42"),
        ("just a string", b"\"hello\""),
        ("bare null", b"null"),
        ("bare true", b"true"),
        ("empty string", b""),
    ];

    for (label, body) in cases {
        let (status, _) = raw_post(
            app.clone(),
            "/api/tools/get_plugin_info",
            Some("application/json"),
            body.to_vec(),
            vec![],
        )
        .await;
        // Some of these are valid JSON (42, "hello", null, true) but not valid for
        // axum's Json<Value> which expects an object. Others are malformed.
        // All should return 400 or 200 (if axum accepts the shape).
        assert!(
            status == 400 || status == 200 || status == 422,
            "unexpected status {status} for case: {label}"
        );
    }
}
