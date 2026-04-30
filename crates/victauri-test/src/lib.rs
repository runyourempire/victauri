#![deny(missing_docs)]
//! Test assertion helpers for AI-agent and CI testing of Tauri apps via Victauri.
//!
//! This crate provides a typed HTTP client for the Victauri MCP server,
//! plus assertion helpers for common test patterns: DOM checks, IPC verification,
//! state comparison, accessibility audits, and performance budgets.
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use victauri_test::VictauriClient;
//!
//! #[tokio::test]
//! async fn app_loads_correctly() {
//!     let client = VictauriClient::connect(7373).await.unwrap();
//!
//!     // Check the page title
//!     let title = client.eval_js("document.title").await.unwrap();
//!     assert_eq!(title.as_str(), Some("My App"));
//!
//!     // Verify no accessibility violations
//!     let audit = client.audit_accessibility().await.unwrap();
//!     assert_eq!(audit["summary"]["violations"], 0);
//!
//!     // Check IPC health
//!     let integrity = client.check_ipc_integrity().await.unwrap();
//!     assert_eq!(integrity["healthy"], true);
//! }
//! ```

mod client;
mod error;

pub use client::{
    VictauriClient, assert_ipc_healthy, assert_json_eq, assert_json_truthy,
    assert_no_a11y_violations, assert_performance_budget, assert_state_matches,
};
pub use error::TestError;
