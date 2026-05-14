use std::sync::Arc;

use axum::extract::DefaultBodyLimit;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};

use crate::auth;
use crate::mcp_handler::VictauriBrowserHandler;

/// Build the axum router for the browser MCP server.
///
/// Mirrors `victauri-plugin`'s server pattern: `/mcp` for MCP Streamable HTTP,
/// `/api/tools` for REST, `/health` and `/info` for diagnostics.
pub fn build_app(handler: VictauriBrowserHandler, auth_token: Option<String>) -> axum::Router {
    build_app_full(handler, auth_token, None)
}

/// Build the axum router with full control over auth and rate limiting.
pub fn build_app_full(
    handler: VictauriBrowserHandler,
    auth_token: Option<String>,
    rate_limiter: Option<Arc<auth::RateLimiterState>>,
) -> axum::Router {
    let rest = rest_routes(handler.clone());

    let mcp_handler = handler.clone();
    let mcp_service = StreamableHttpService::new(
        move || Ok(mcp_handler.clone()),
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default(),
    );

    let info_handler_ref = handler.clone();
    let info_auth = auth_token.is_some();

    let auth_state = Arc::new(auth::AuthState {
        token: auth_token.clone(),
    });

    let mut router = axum::Router::new()
        .route_service("/mcp", mcp_service)
        .nest("/api/tools", rest)
        .route(
            "/info",
            axum::routing::get(move || {
                let h = info_handler_ref.clone();
                async move {
                    axum::Json(serde_json::json!({
                        "name": "victauri-browser",
                        "version": env!("CARGO_PKG_VERSION"),
                        "protocol": "mcp",
                        "mode": "browser",
                        "tabs": h.tab_count().await,
                        "auth_required": info_auth,
                    }))
                }
            }),
        );

    if auth_token.is_some() {
        router = router.layer(axum::middleware::from_fn_with_state(
            auth_state,
            auth::require_auth,
        ));
    }

    let limiter = rate_limiter.unwrap_or_else(auth::default_rate_limiter);
    router = router.layer(axum::middleware::from_fn_with_state(limiter, auth::rate_limit));

    router
        .route(
            "/health",
            axum::routing::get(|| async { axum::Json(serde_json::json!({"status": "ok"})) }),
        )
        .layer(DefaultBodyLimit::max(2 * 1024 * 1024))
        .layer(axum::middleware::from_fn(auth::security_headers))
        .layer(axum::middleware::from_fn(auth::origin_guard))
}

fn rest_routes(handler: VictauriBrowserHandler) -> axum::Router {
    let list_handler = handler.clone();

    axum::Router::new()
        .route(
            "/",
            axum::routing::get(move || {
                let h = list_handler.clone();
                async move { axum::Json(h.list_tools()) }
            }),
        )
        .route(
            "/{name}",
            axum::routing::post(move |path, body| execute_tool(handler, path, body)),
        )
}

async fn execute_tool(
    handler: VictauriBrowserHandler,
    axum::extract::Path(name): axum::extract::Path<String>,
    axum::Json(args): axum::Json<serde_json::Value>,
) -> axum::Json<serde_json::Value> {
    match handler.execute_tool(&name, args).await {
        Ok(result) => axum::Json(serde_json::json!({"result": result})),
        Err(e) => axum::Json(serde_json::json!({"error": e})),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge_dispatch::BridgeDispatch;
    use crate::tab_state::TabManager;
    use axum::body::Body;
    use http_body_util::BodyExt;
    use std::sync::Arc;
    use tower::ServiceExt;

    fn make_app(auth: Option<String>) -> axum::Router {
        let tab_mgr = Arc::new(TabManager::new());
        let dispatch = Arc::new(BridgeDispatch::new(tokio::io::stdout()));
        let handler = VictauriBrowserHandler::new(tab_mgr, dispatch);
        build_app(handler, auth)
    }

    async fn get_json(
        app: axum::Router,
        path: &str,
    ) -> (axum::http::StatusCode, serde_json::Value) {
        let req = axum::http::Request::builder()
            .uri(path)
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        let status = resp.status();
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
        (status, json)
    }

    async fn post_json(
        app: axum::Router,
        path: &str,
        body: serde_json::Value,
        auth: Option<&str>,
    ) -> (axum::http::StatusCode, serde_json::Value) {
        let mut req = axum::http::Request::builder()
            .method("POST")
            .uri(path)
            .header("content-type", "application/json");
        if let Some(token) = auth {
            req = req.header("authorization", format!("Bearer {token}"));
        }
        let req = req
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        let status = resp.status();
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
        (status, json)
    }

    #[test]
    fn router_builds_without_auth() {
        let _router = make_app(None);
    }

    #[test]
    fn router_builds_with_auth() {
        let _router = make_app(Some("test-token".to_string()));
    }

    #[tokio::test]
    async fn health_returns_ok() {
        let (status, json) = get_json(make_app(None), "/health").await;
        assert_eq!(status, 200);
        assert_eq!(json["status"], "ok");
    }

    #[tokio::test]
    async fn info_returns_metadata() {
        let (status, json) = get_json(make_app(None), "/info").await;
        assert_eq!(status, 200);
        assert_eq!(json["name"], "victauri-browser");
        assert_eq!(json["protocol"], "mcp");
        assert_eq!(json["mode"], "browser");
        assert_eq!(json["tabs"], 0);
    }

    #[tokio::test]
    async fn tool_list_returns_20() {
        let (status, json) = get_json(make_app(None), "/api/tools").await;
        assert_eq!(status, 200);
        assert_eq!(json.as_array().unwrap().len(), 20);
    }

    #[tokio::test]
    async fn plugin_info_via_rest() {
        let (status, json) =
            post_json(make_app(None), "/api/tools/get_plugin_info", serde_json::json!({}), None)
                .await;
        assert_eq!(status, 200);
        assert_eq!(json["result"]["name"], "victauri-browser");
        assert_eq!(json["result"]["tool_count"], 20);
    }

    #[tokio::test]
    async fn tabs_list_empty_via_rest() {
        let (status, json) = post_json(
            make_app(None),
            "/api/tools/tabs",
            serde_json::json!({"action": "list"}),
            None,
        )
        .await;
        assert_eq!(status, 200);
        assert!(json["result"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn unknown_tool_returns_error() {
        let (status, json) = post_json(
            make_app(None),
            "/api/tools/nonexistent",
            serde_json::json!({}),
            None,
        )
        .await;
        assert_eq!(status, 200);
        assert!(json["error"].as_str().unwrap().contains("unknown tool"));
    }

    #[tokio::test]
    async fn auth_blocks_without_token() {
        let app = make_app(Some("secret-token".to_string()));
        let (status, _) = get_json(app, "/info").await;
        assert_eq!(status, 401);
    }

    #[tokio::test]
    async fn auth_passes_with_correct_token() {
        let token = "secret-token";
        let app = make_app(Some(token.to_string()));
        let req = axum::http::Request::builder()
            .uri("/info")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test]
    async fn health_bypasses_auth() {
        let app = make_app(Some("secret-token".to_string()));
        let (status, json) = get_json(app, "/health").await;
        assert_eq!(status, 200);
        assert_eq!(json["status"], "ok");
    }

    #[tokio::test]
    async fn security_headers_present() {
        let app = make_app(None);
        let req = axum::http::Request::builder()
            .uri("/health")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(
            resp.headers().get("x-content-type-options").unwrap(),
            "nosniff"
        );
        assert_eq!(resp.headers().get("cache-control").unwrap(), "no-store");
    }

    #[tokio::test]
    async fn non_local_origin_blocked() {
        let app = make_app(None);
        let req = axum::http::Request::builder()
            .uri("/health")
            .header("origin", "https://evil.com")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), 403);
    }

    #[tokio::test]
    async fn local_origin_allowed() {
        let app = make_app(None);
        let req = axum::http::Request::builder()
            .uri("/health")
            .header("origin", "http://127.0.0.1:7474")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), 200);
    }

    fn make_app_with_rate_limit(budget: u64) -> axum::Router {
        let tab_mgr = Arc::new(TabManager::new());
        let dispatch = Arc::new(BridgeDispatch::new(tokio::io::stdout()));
        let handler = VictauriBrowserHandler::new(tab_mgr, dispatch);
        let limiter = Arc::new(crate::auth::RateLimiterState::new(budget));
        build_app_full(handler, None, Some(limiter))
    }

    #[tokio::test]
    async fn rate_limit_exhaustion_returns_429() {
        let app = make_app_with_rate_limit(1);

        let req1 = axum::http::Request::builder()
            .uri("/info")
            .body(Body::empty())
            .unwrap();
        let resp1 = app.clone().oneshot(req1).await.unwrap();
        assert_eq!(resp1.status(), 200);

        let req2 = axum::http::Request::builder()
            .uri("/info")
            .body(Body::empty())
            .unwrap();
        let resp2 = app.oneshot(req2).await.unwrap();
        assert_eq!(resp2.status(), 429);
    }

    #[tokio::test]
    async fn auth_wrong_token_returns_401() {
        let app = make_app(Some("correct-token".to_string()));
        let req = axum::http::Request::builder()
            .uri("/info")
            .header("authorization", "Bearer wrong-token")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), 401);
    }

    #[tokio::test]
    async fn auth_no_bearer_prefix_returns_401() {
        let app = make_app(Some("my-token".to_string()));
        let req = axum::http::Request::builder()
            .uri("/info")
            .header("authorization", "my-token")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), 401);
    }

    #[tokio::test]
    async fn auth_case_insensitive_bearer() {
        let token = "my-secret-token";
        let app = make_app(Some(token.to_string()));
        let req = axum::http::Request::builder()
            .uri("/info")
            .header("authorization", format!("BEARER {token}"))
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test]
    async fn rest_tool_with_auth() {
        let token = "secret";
        let (status, json) = post_json(
            make_app(Some(token.to_string())),
            "/api/tools/get_plugin_info",
            serde_json::json!({}),
            Some(token),
        )
        .await;
        assert_eq!(status, 200);
        assert_eq!(json["result"]["name"], "victauri-browser");
    }

    #[tokio::test]
    async fn rest_tool_without_auth_when_required() {
        let (status, _) = post_json(
            make_app(Some("secret".to_string())),
            "/api/tools/get_plugin_info",
            serde_json::json!({}),
            None,
        )
        .await;
        assert_eq!(status, 401);
    }

    #[tokio::test]
    async fn localhost_origin_allowed() {
        let app = make_app(None);
        let req = axum::http::Request::builder()
            .uri("/health")
            .header("origin", "http://localhost:3000")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test]
    async fn ipv6_localhost_origin_allowed() {
        let app = make_app(None);
        let req = axum::http::Request::builder()
            .uri("/health")
            .header("origin", "http://[::1]:7474")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test]
    async fn no_origin_header_allowed() {
        let app = make_app(None);
        let (status, json) = get_json(app, "/health").await;
        assert_eq!(status, 200);
        assert_eq!(json["status"], "ok");
    }

    #[tokio::test]
    async fn info_shows_auth_required() {
        let (_, json_no_auth) = get_json(make_app(None), "/info").await;
        assert_eq!(json_no_auth["auth_required"], false);

        let app = make_app(Some("tok".to_string()));
        let req = axum::http::Request::builder()
            .uri("/info")
            .header("authorization", "Bearer tok")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let json_auth: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json_auth["auth_required"], true);
    }

    #[tokio::test]
    async fn tool_list_via_rest_has_names() {
        let (_, json) = get_json(make_app(None), "/api/tools").await;
        let tools = json.as_array().unwrap();
        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"eval_js"));
        assert!(names.contains(&"dom_snapshot"));
        assert!(names.contains(&"screenshot"));
        assert!(names.contains(&"tabs"));
    }

    #[tokio::test]
    async fn rest_error_format() {
        let (status, json) = post_json(
            make_app(None),
            "/api/tools/nonexistent",
            serde_json::json!({}),
            None,
        )
        .await;
        assert_eq!(status, 200);
        assert!(json.get("error").is_some());
        assert!(json.get("result").is_none());
    }

    #[tokio::test]
    async fn rest_success_format() {
        let (status, json) = post_json(
            make_app(None),
            "/api/tools/get_plugin_info",
            serde_json::json!({}),
            None,
        )
        .await;
        assert_eq!(status, 200);
        assert!(json.get("result").is_some());
        assert!(json.get("error").is_none());
    }
}
