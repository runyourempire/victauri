//! Tests for the Playwright-style high-level API.
//!
//! Requires a running demo app:
//!   `VICTAURI_PORT=7374` cargo run -p demo-app
//!
//! Then:
//!   `VICTAURI_E2E=1` `VICTAURI_PORT=7374` cargo test -p victauri-test --test `high_level_api`

use victauri_test::VictauriClient;

fn skip_unless_e2e() -> bool {
    std::env::var("VICTAURI_E2E").is_ok()
}

fn port() -> u16 {
    std::env::var("VICTAURI_PORT")
        .unwrap_or_else(|_| "7373".to_string())
        .parse()
        .unwrap()
}

#[tokio::test]
async fn click_by_id_greet_btn() {
    if !skip_unless_e2e() {
        return;
    }
    let mut client = VictauriClient::connect(port()).await.unwrap();
    client.fill_by_id("name-input", "TestUser").await.unwrap();
    client.click_by_id("greet-btn").await.unwrap();
    client.expect_text("Hello, TestUser!").await.unwrap();
}

#[tokio::test]
async fn click_by_text_increment() {
    if !skip_unless_e2e() {
        return;
    }
    let mut client = VictauriClient::connect(port()).await.unwrap();
    client.click_by_text("+").await.unwrap();
    let text = client.text_by_id("counter-value").await.unwrap();
    assert!(!text.is_empty());
}

#[tokio::test]
async fn fill_by_id_and_expect_text() {
    if !skip_unless_e2e() {
        return;
    }
    let mut client = VictauriClient::connect(port()).await.unwrap();
    client
        .fill_by_id("todo-input", "Write tests")
        .await
        .unwrap();
    client.click_by_id("add-todo-btn").await.unwrap();
    client.expect_text("Write tests").await.unwrap();
}

#[tokio::test]
async fn select_by_id_language() {
    if !skip_unless_e2e() {
        return;
    }
    let mut client = VictauriClient::connect(port()).await.unwrap();
    client.select_by_id("lang-select", "ja").await.unwrap();
}

#[tokio::test]
async fn expect_no_text_works() {
    if !skip_unless_e2e() {
        return;
    }
    let mut client = VictauriClient::connect(port()).await.unwrap();
    client.expect_no_text("NONEXISTENT_xyz_42").await.unwrap();
}

#[tokio::test]
async fn element_not_found_gives_clear_error() {
    if !skip_unless_e2e() {
        return;
    }
    let mut client = VictauriClient::connect(port()).await.unwrap();
    let err = client.click_by_id("nonexistent-id-xyz").await;
    assert!(err.is_err());
    let msg = err.unwrap_err().to_string();
    assert!(msg.contains("element not found"), "got: {msg}");
}
