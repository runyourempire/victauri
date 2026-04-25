#![allow(dead_code)]

use std::sync::atomic::{AtomicI64, Ordering};

static ALLOCATED_BYTES: AtomicI64 = AtomicI64::new(0);
static ALLOCATION_COUNT: AtomicI64 = AtomicI64::new(0);
static DEALLOCATION_COUNT: AtomicI64 = AtomicI64::new(0);

pub fn record_alloc(size: usize) {
    ALLOCATED_BYTES.fetch_add(size as i64, Ordering::Relaxed);
    ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
}

pub fn record_dealloc(size: usize) {
    ALLOCATED_BYTES.fetch_sub(size as i64, Ordering::Relaxed);
    DEALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
}

pub fn current_stats() -> serde_json::Value {
    serde_json::json!({
        "allocated_bytes": ALLOCATED_BYTES.load(Ordering::Relaxed),
        "allocation_count": ALLOCATION_COUNT.load(Ordering::Relaxed),
        "deallocation_count": DEALLOCATION_COUNT.load(Ordering::Relaxed),
    })
}

pub fn snapshot_bytes() -> i64 {
    ALLOCATED_BYTES.load(Ordering::Relaxed)
}
