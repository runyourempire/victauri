//! Concurrent multi-client stress test: spawns multiple `VictauriClient`
//! connections exercising different tools simultaneously to detect deadlocks,
//! dropped responses, and data races.
//!
//! Requires a running app with Victauri (e.g. `demo-app`):
//!   `cargo run -p demo-app`
//!
//! Then run with:
//!   `VICTAURI_STRESS=1 cargo test -p victauri-test --test concurrent_stress_test -- --nocapture`
//!
//! Optional env vars:
//!   `VICTAURI_STRESS_CLIENTS`     — number of concurrent clients (default: 10)
//!   `VICTAURI_STRESS_DURATION`    — seconds to run (default: 60)

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use victauri_test::VictauriClient;

fn skip_unless_stress() -> bool {
    std::env::var("VICTAURI_STRESS").is_ok()
}

fn client_count() -> usize {
    std::env::var("VICTAURI_STRESS_CLIENTS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(10)
}

fn stress_duration() -> Duration {
    let secs: u64 = std::env::var("VICTAURI_STRESS_DURATION")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(60);
    Duration::from_secs(secs)
}

#[tokio::test]
async fn concurrent_multi_client_stress() {
    if !skip_unless_stress() {
        eprintln!("skipping stress test (set VICTAURI_STRESS=1 to run)");
        return;
    }

    let num_clients = client_count();
    let duration = stress_duration();

    eprintln!(
        "stress test: {num_clients} clients for {}s",
        duration.as_secs()
    );

    let total_calls = Arc::new(AtomicU64::new(0));
    let total_errors = Arc::new(AtomicU64::new(0));
    let total_timeouts = Arc::new(AtomicU64::new(0));
    let start = Instant::now();

    let mut handles = Vec::new();

    for client_id in 0..num_clients {
        let calls = total_calls.clone();
        let errors = total_errors.clone();
        let timeouts = total_timeouts.clone();
        let dur = duration;

        let handle = tokio::spawn(async move {
            let mut client = match VictauriClient::discover().await {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("client {client_id}: failed to connect: {e}");
                    errors.fetch_add(1, Ordering::Relaxed);
                    return;
                }
            };

            let mut iteration = 0u64;
            let client_start = Instant::now();

            while client_start.elapsed() < dur {
                iteration += 1;
                let tool = iteration % 5;

                let result = match tool {
                    0 => client.eval_js("document.title").await.map(|_| ()),
                    1 => client.dom_snapshot().await.map(|_| ()),
                    2 => client
                        .find_elements(serde_json::json!({"selector": "button"}))
                        .await
                        .map(|_| ()),
                    3 => client.get_memory_stats().await.map(|_| ()),
                    4 => client
                        .eval_js("document.querySelectorAll('*').length")
                        .await
                        .map(|_| ()),
                    _ => unreachable!(),
                };

                calls.fetch_add(1, Ordering::Relaxed);

                if let Err(e) = result {
                    let msg = e.to_string();
                    if msg.contains("timed out") {
                        timeouts.fetch_add(1, Ordering::Relaxed);
                    } else {
                        errors.fetch_add(1, Ordering::Relaxed);
                    }
                }

                tokio::time::sleep(Duration::from_millis(20 + (client_id as u64 % 10) * 5)).await;
            }

            eprintln!(
                "  client {client_id}: {iteration} iterations in {:.0}s",
                client_start.elapsed().as_secs_f64()
            );
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.await.unwrap();
    }

    let total = total_calls.load(Ordering::Relaxed);
    let errs = total_errors.load(Ordering::Relaxed);
    let tos = total_timeouts.load(Ordering::Relaxed);
    let elapsed = start.elapsed();

    eprintln!("\n=== STRESS TEST REPORT ===");
    eprintln!("Duration: {:.0}s", elapsed.as_secs_f64());
    eprintln!("Clients: {num_clients}");
    eprintln!("Total calls: {total}");
    eprintln!(
        "Throughput: {:.0} calls/sec",
        total as f64 / elapsed.as_secs_f64()
    );
    eprintln!("Errors: {errs}");
    eprintln!("Timeouts: {tos}");
    eprintln!(
        "Error rate: {:.2}%",
        (errs + tos) as f64 / total.max(1) as f64 * 100.0
    );

    let error_rate = (errs + tos) as f64 / total.max(1) as f64;
    assert!(
        error_rate < 0.05,
        "error rate {:.1}% exceeds 5% threshold",
        error_rate * 100.0
    );

    let expected_min = num_clients as u64 * 10;
    assert!(
        total >= expected_min,
        "only {total} total calls — expected at least {expected_min}"
    );
}
