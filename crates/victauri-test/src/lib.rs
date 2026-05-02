#![deny(missing_docs)]
//! Integration testing for Tauri apps via the Victauri MCP server.
//!
//! Provides [`TestApp`] for managed app lifecycle and [`VictauriClient`] with
//! Playwright-style interactions (`click_by_text`, `fill_by_id`, `expect_text`)
//! plus assertion helpers for accessibility, performance, and state verification.
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use victauri_test::TestApp;
//!
//! #[tokio::test]
//! async fn greet_flow() {
//!     let app = TestApp::spawn("cargo run -p my-app").await.unwrap();
//!     let mut client = app.client().await.unwrap();
//!
//!     client.fill_by_id("name-input", "World").await.unwrap();
//!     client.click_by_id("greet-btn").await.unwrap();
//!     client.expect_text("Hello, World!").await.unwrap();
//! }
//! ```
//!
//! # Without `TestApp` (connect to existing server)
//!
//! ```rust,ignore
//! use victauri_test::VictauriClient;
//!
//! #[tokio::test]
//! async fn check_health() {
//!     let mut client = VictauriClient::discover().await.unwrap();
//!     let audit = client.audit_accessibility().await.unwrap();
//!     assert_eq!(audit["summary"]["violations"], 0);
//! }
//! ```

mod app;
mod client;
mod error;

pub use app::TestApp;
pub use client::{
    VictauriClient, assert_ipc_healthy, assert_json_eq, assert_json_truthy,
    assert_no_a11y_violations, assert_performance_budget, assert_state_matches,
};
pub use error::TestError;
