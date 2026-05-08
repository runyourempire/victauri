//! Visual regression testing — compare screenshots against baselines.
//!
//! Decodes PNG images, computes per-pixel RGBA diffs, and generates diff
//! images highlighting changes. Baselines are stored in a `snapshots/`
//! directory alongside test files.

use std::path::{Path, PathBuf};

use base64::Engine;

use crate::error::TestError;

/// Result of comparing two screenshots pixel-by-pixel.
#[derive(Debug)]
pub struct VisualDiff {
    /// Percentage of pixels that matched (0.0 to 100.0).
    pub match_percentage: f64,
    /// Total number of pixels that differed beyond tolerance.
    pub diff_pixel_count: usize,
    /// Total pixels compared.
    pub total_pixels: usize,
    /// Path to the diff image, if one was generated.
    pub diff_image_path: Option<PathBuf>,
}

impl VisualDiff {
    /// Returns true if the images match within the given threshold.
    #[must_use]
    pub fn is_match(&self, threshold_percent: f64) -> bool {
        self.match_percentage >= (100.0 - threshold_percent)
    }
}

/// Options for visual regression comparison.
#[derive(Debug, Clone)]
pub struct VisualOptions {
    /// Directory where baseline snapshots are stored.
    pub snapshot_dir: PathBuf,
    /// Per-channel tolerance (0-255). Pixels differing by less than this
    /// in all channels are considered matching.
    pub channel_tolerance: u8,
    /// Maximum allowed diff percentage before comparison fails.
    pub threshold_percent: f64,
    /// Whether to generate a diff image on mismatch.
    pub generate_diff_image: bool,
    /// Whether to update baselines instead of comparing.
    pub update_baselines: bool,
}

impl Default for VisualOptions {
    fn default() -> Self {
        Self {
            snapshot_dir: PathBuf::from("tests/snapshots"),
            channel_tolerance: 2,
            threshold_percent: 0.1,
            generate_diff_image: true,
            update_baselines: false,
        }
    }
}

/// Compares a screenshot (base64 PNG) against a stored baseline.
///
/// On first run (no baseline exists), saves the screenshot as the new baseline
/// and returns a perfect match. On subsequent runs, decodes both PNGs and
/// compares pixel-by-pixel.
///
/// # Errors
///
/// Returns [`TestError::VisualRegression`] if the diff exceeds the threshold,
/// or [`TestError::Other`] for IO/decode failures.
pub fn compare_screenshot(
    name: &str,
    screenshot_base64: &str,
    options: &VisualOptions,
) -> Result<VisualDiff, TestError> {
    let screenshot_bytes = base64::engine::general_purpose::STANDARD
        .decode(screenshot_base64)
        .map_err(|e| TestError::Other(format!("failed to decode base64 screenshot: {e}")))?;

    std::fs::create_dir_all(&options.snapshot_dir)
        .map_err(|e| TestError::Other(format!("failed to create snapshot dir: {e}")))?;

    let baseline_path = options.snapshot_dir.join(format!("{name}.png"));

    if options.update_baselines || !baseline_path.exists() {
        std::fs::write(&baseline_path, &screenshot_bytes)
            .map_err(|e| TestError::Other(format!("failed to write baseline: {e}")))?;

        return Ok(VisualDiff {
            match_percentage: 100.0,
            diff_pixel_count: 0,
            total_pixels: 0,
            diff_image_path: None,
        });
    }

    let baseline_bytes = std::fs::read(&baseline_path)
        .map_err(|e| TestError::Other(format!("failed to read baseline: {e}")))?;

    let current = decode_png(&screenshot_bytes)?;
    let baseline = decode_png(&baseline_bytes)?;

    if current.width != baseline.width || current.height != baseline.height {
        return Err(TestError::Other(format!(
            "screenshot size {}x{} doesn't match baseline {}x{}",
            current.width, current.height, baseline.width, baseline.height
        )));
    }

    let diff = compute_diff(&current, &baseline, options.channel_tolerance);
    let total_pixels = (current.width * current.height) as usize;
    let match_percentage = if total_pixels == 0 {
        100.0
    } else {
        (1.0 - diff.len() as f64 / total_pixels as f64) * 100.0
    };

    let diff_image_path = if !diff.is_empty() && options.generate_diff_image {
        let diff_path = options.snapshot_dir.join(format!("{name}.diff.png"));
        write_diff_image(&diff_path, &current, &diff)?;
        Some(diff_path)
    } else {
        None
    };

    let result = VisualDiff {
        match_percentage,
        diff_pixel_count: diff.len(),
        total_pixels,
        diff_image_path,
    };

    if !result.is_match(options.threshold_percent) {
        return Err(TestError::VisualRegression(format!(
            "visual regression: {:.2}% pixels differ (threshold: {:.2}%)",
            100.0 - match_percentage,
            options.threshold_percent
        )));
    }

    Ok(result)
}

struct DecodedImage {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

fn decode_png(data: &[u8]) -> Result<DecodedImage, TestError> {
    let decoder = png::Decoder::new(std::io::Cursor::new(data));
    let mut reader = decoder
        .read_info()
        .map_err(|e| TestError::Other(format!("PNG decode error: {e}")))?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|e| TestError::Other(format!("PNG frame error: {e}")))?;

    let rgba = match info.color_type {
        png::ColorType::Rgba => buf[..info.buffer_size()].to_vec(),
        png::ColorType::Rgb => {
            let rgb = &buf[..info.buffer_size()];
            let mut rgba = Vec::with_capacity(rgb.len() / 3 * 4);
            for chunk in rgb.chunks_exact(3) {
                rgba.extend_from_slice(chunk);
                rgba.push(255);
            }
            rgba
        }
        png::ColorType::Grayscale => {
            let gray = &buf[..info.buffer_size()];
            let mut rgba = Vec::with_capacity(gray.len() * 4);
            for &g in gray {
                rgba.extend_from_slice(&[g, g, g, 255]);
            }
            rgba
        }
        other => {
            return Err(TestError::Other(format!(
                "unsupported PNG color type: {other:?}"
            )));
        }
    };

    Ok(DecodedImage {
        width: info.width,
        height: info.height,
        rgba,
    })
}

fn compute_diff(current: &DecodedImage, baseline: &DecodedImage, tolerance: u8) -> Vec<usize> {
    let mut diff_positions = Vec::new();
    let pixel_count = (current.width * current.height) as usize;

    for i in 0..pixel_count {
        let offset = i * 4;
        if offset + 3 >= current.rgba.len() || offset + 3 >= baseline.rgba.len() {
            break;
        }
        let dr = current.rgba[offset].abs_diff(baseline.rgba[offset]);
        let dg = current.rgba[offset + 1].abs_diff(baseline.rgba[offset + 1]);
        let db = current.rgba[offset + 2].abs_diff(baseline.rgba[offset + 2]);
        let da = current.rgba[offset + 3].abs_diff(baseline.rgba[offset + 3]);

        if dr > tolerance || dg > tolerance || db > tolerance || da > tolerance {
            diff_positions.push(i);
        }
    }

    diff_positions
}

fn write_diff_image(
    path: &Path,
    source: &DecodedImage,
    diff_positions: &[usize],
) -> Result<(), TestError> {
    let mut diff_rgba = source.rgba.clone();

    for &pos in diff_positions {
        let offset = pos * 4;
        if offset + 3 < diff_rgba.len() {
            diff_rgba[offset] = 255; // R
            diff_rgba[offset + 1] = 0; // G
            diff_rgba[offset + 2] = 0; // B
            diff_rgba[offset + 3] = 255; // A
        }
    }

    let file = std::fs::File::create(path)
        .map_err(|e| TestError::Other(format!("failed to create diff image: {e}")))?;
    let w = &mut std::io::BufWriter::new(file);
    let mut encoder = png::Encoder::new(w, source.width, source.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .map_err(|e| TestError::Other(format!("PNG encode error: {e}")))?;
    writer
        .write_image_data(&diff_rgba)
        .map_err(|e| TestError::Other(format!("PNG write error: {e}")))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_solid_png(width: u32, height: u32, r: u8, g: u8, b: u8) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut buf, width, height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            let mut data = Vec::with_capacity((width * height * 4) as usize);
            for _ in 0..(width * height) {
                data.extend_from_slice(&[r, g, b, 255]);
            }
            writer.write_image_data(&data).unwrap();
        }
        buf
    }

    fn to_base64(data: &[u8]) -> String {
        base64::engine::general_purpose::STANDARD.encode(data)
    }

    #[test]
    fn identical_images_match() {
        let dir = tempfile::tempdir().unwrap();
        let png = make_solid_png(10, 10, 128, 128, 128);
        let b64 = to_base64(&png);

        let opts = VisualOptions {
            snapshot_dir: dir.path().to_path_buf(),
            ..VisualOptions::default()
        };

        // First run saves baseline
        let result = compare_screenshot("test_identical", &b64, &opts).unwrap();
        assert_eq!(result.match_percentage, 100.0);

        // Second run compares — should match
        let result = compare_screenshot("test_identical", &b64, &opts).unwrap();
        assert_eq!(result.match_percentage, 100.0);
        assert_eq!(result.diff_pixel_count, 0);
    }

    #[test]
    fn different_images_detected() {
        let dir = tempfile::tempdir().unwrap();
        let baseline = make_solid_png(10, 10, 128, 128, 128);
        let changed = make_solid_png(10, 10, 255, 0, 0);

        let opts = VisualOptions {
            snapshot_dir: dir.path().to_path_buf(),
            generate_diff_image: true,
            threshold_percent: 0.1,
            ..VisualOptions::default()
        };

        // Save baseline
        compare_screenshot("test_diff", &to_base64(&baseline), &opts).unwrap();

        // Compare with different image — should fail
        let err = compare_screenshot("test_diff", &to_base64(&changed), &opts).unwrap_err();
        match err {
            TestError::VisualRegression(msg) => {
                assert!(msg.contains("visual regression"), "got: {msg}");
            }
            other => panic!("expected VisualRegression, got: {other:?}"),
        }

        // Diff image should exist
        assert!(dir.path().join("test_diff.diff.png").exists());
    }

    #[test]
    fn tolerance_allows_minor_diffs() {
        let dir = tempfile::tempdir().unwrap();
        let baseline = make_solid_png(10, 10, 128, 128, 128);
        let slightly_off = make_solid_png(10, 10, 129, 128, 128);

        let opts = VisualOptions {
            snapshot_dir: dir.path().to_path_buf(),
            channel_tolerance: 2,
            threshold_percent: 1.0,
            ..VisualOptions::default()
        };

        compare_screenshot("test_tol", &to_base64(&baseline), &opts).unwrap();
        let result = compare_screenshot("test_tol", &to_base64(&slightly_off), &opts).unwrap();
        assert_eq!(result.match_percentage, 100.0);
    }

    #[test]
    fn update_baselines_overwrites() {
        let dir = tempfile::tempdir().unwrap();
        let first = make_solid_png(5, 5, 100, 100, 100);
        let second = make_solid_png(5, 5, 200, 200, 200);

        let mut opts = VisualOptions {
            snapshot_dir: dir.path().to_path_buf(),
            ..VisualOptions::default()
        };

        compare_screenshot("test_update", &to_base64(&first), &opts).unwrap();

        opts.update_baselines = true;
        let result = compare_screenshot("test_update", &to_base64(&second), &opts).unwrap();
        assert_eq!(result.match_percentage, 100.0);

        // Now compare without update — should match the new baseline
        opts.update_baselines = false;
        let result = compare_screenshot("test_update", &to_base64(&second), &opts).unwrap();
        assert_eq!(result.match_percentage, 100.0);
    }

    #[test]
    fn size_mismatch_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let small = make_solid_png(5, 5, 128, 128, 128);
        let big = make_solid_png(10, 10, 128, 128, 128);

        let opts = VisualOptions {
            snapshot_dir: dir.path().to_path_buf(),
            ..VisualOptions::default()
        };

        compare_screenshot("test_size", &to_base64(&small), &opts).unwrap();
        let err = compare_screenshot("test_size", &to_base64(&big), &opts).unwrap_err();
        match err {
            TestError::Other(msg) => assert!(msg.contains("size"), "got: {msg}"),
            other => panic!("expected Other, got: {other:?}"),
        }
    }

    #[test]
    fn first_run_creates_baseline() {
        let dir = tempfile::tempdir().unwrap();
        let png = make_solid_png(3, 3, 64, 64, 64);

        let opts = VisualOptions {
            snapshot_dir: dir.path().to_path_buf(),
            ..VisualOptions::default()
        };

        assert!(!dir.path().join("new_test.png").exists());
        compare_screenshot("new_test", &to_base64(&png), &opts).unwrap();
        assert!(dir.path().join("new_test.png").exists());
    }
}
