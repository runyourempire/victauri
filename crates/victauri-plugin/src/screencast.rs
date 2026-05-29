//! Screencast state for the `trace` tool — a ring buffer of timestamped PNG
//! frames captured by a background task at a fixed interval. Pairs with the
//! `EventRecorder` (events) and `logs` (network/console) to form a trace bundle.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

/// A single captured frame: milliseconds since trace start + base64 PNG.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TraceFrame {
    /// Milliseconds since the trace started.
    pub t_ms: u64,
    /// Base64-encoded PNG image data.
    pub data_b64: String,
}

/// Shared screencast state. Thread-safe; mutex locks are short-lived and
/// recover from poisoning.
#[derive(Debug)]
pub struct Screencast {
    active: AtomicBool,
    interval_ms: AtomicU64,
    max_frames: AtomicUsize,
    generation: AtomicU64,
    frames: Mutex<Vec<TraceFrame>>,
    label: Mutex<Option<String>>,
}

impl Default for Screencast {
    fn default() -> Self {
        Self {
            active: AtomicBool::new(false),
            interval_ms: AtomicU64::new(500),
            max_frames: AtomicUsize::new(60),
            generation: AtomicU64::new(0),
            frames: Mutex::new(Vec::new()),
            label: Mutex::new(None),
        }
    }
}

impl Screencast {
    /// Begin a new trace: clears frames, records settings, returns the
    /// generation token the capture task must check to know it is current.
    pub fn start(&self, interval_ms: u64, max_frames: usize, label: Option<String>) -> u64 {
        self.interval_ms
            .store(interval_ms.max(50), Ordering::Relaxed);
        self.max_frames
            .store(max_frames.clamp(1, 600), Ordering::Relaxed);
        {
            let mut f = self
                .frames
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            f.clear();
        }
        {
            let mut l = self
                .label
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            *l = label;
        }
        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        self.active.store(true, Ordering::SeqCst);
        generation
    }

    /// Stop the current trace. Returns the captured frame count.
    pub fn stop(&self) -> usize {
        self.active.store(false, Ordering::SeqCst);
        // Invalidate any running task.
        self.generation.fetch_add(1, Ordering::SeqCst);
        self.frame_count()
    }

    /// Whether a trace is currently active.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::SeqCst)
    }

    /// The current generation token (a capture task is stale if it differs).
    #[must_use]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }

    /// Configured capture interval in milliseconds.
    #[must_use]
    pub fn interval_ms(&self) -> u64 {
        self.interval_ms.load(Ordering::Relaxed)
    }

    /// Target webview label for capture, if set.
    #[must_use]
    pub fn label(&self) -> Option<String> {
        self.label
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// Append a frame, enforcing the `max_frames` ring-buffer cap.
    pub fn push_frame(&self, t_ms: u64, data_b64: String) {
        let max = self.max_frames.load(Ordering::Relaxed);
        let mut f = self
            .frames
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        f.push(TraceFrame { t_ms, data_b64 });
        let len = f.len();
        if len > max {
            f.drain(0..len - max);
        }
    }

    /// Number of frames currently buffered.
    #[must_use]
    pub fn frame_count(&self) -> usize {
        self.frames
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len()
    }

    /// Return up to `limit` of the most recent frames (or all if `limit` is 0).
    #[must_use]
    pub fn frames(&self, limit: usize) -> Vec<TraceFrame> {
        let f = self
            .frames
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if limit == 0 || limit >= f.len() {
            f.clone()
        } else {
            f[f.len() - limit..].to_vec()
        }
    }

    /// Frame timestamps (ms since start) without the image payloads.
    #[must_use]
    pub fn frame_timestamps(&self) -> Vec<u64> {
        self.frames
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .map(|fr| fr.t_ms)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_buffer_caps_frames() {
        let sc = Screencast::default();
        sc.start(100, 3, None);
        for i in 0..5 {
            sc.push_frame(i * 100, format!("frame{i}"));
        }
        assert_eq!(sc.frame_count(), 3, "should cap at max_frames");
        let frames = sc.frames(0);
        // Oldest dropped: keeps frame2, frame3, frame4.
        assert_eq!(frames[0].data_b64, "frame2");
        assert_eq!(frames[2].data_b64, "frame4");
    }

    #[test]
    fn start_clears_and_bumps_generation() {
        let sc = Screencast::default();
        let g1 = sc.start(200, 10, Some("main".into()));
        sc.push_frame(0, "x".into());
        assert_eq!(sc.frame_count(), 1);
        let g2 = sc.start(200, 10, None);
        assert!(g2 > g1, "generation must increase");
        assert_eq!(sc.frame_count(), 0, "start clears frames");
        assert!(sc.is_active());
    }

    #[test]
    fn stop_deactivates_and_invalidates() {
        let sc = Screencast::default();
        let g = sc.start(200, 10, None);
        sc.stop();
        assert!(!sc.is_active());
        assert!(sc.generation() > g, "stop invalidates the task generation");
    }

    #[test]
    fn frames_limit_returns_most_recent() {
        let sc = Screencast::default();
        sc.start(100, 100, None);
        for i in 0..5 {
            sc.push_frame(i, format!("f{i}"));
        }
        let last2 = sc.frames(2);
        assert_eq!(last2.len(), 2);
        assert_eq!(last2[0].data_b64, "f3");
        assert_eq!(last2[1].data_b64, "f4");
    }
}
