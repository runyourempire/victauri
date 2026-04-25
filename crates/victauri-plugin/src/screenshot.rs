// Platform-native screenshot capture
//
// Phase 1: Windows via PrintWindow Win32 API
// Phase 2: Linux via xcap, macOS via CGWindowListCreateImage

#[cfg(windows)]
pub async fn capture_window(_hwnd: isize) -> anyhow::Result<Vec<u8>> {
    // TODO: Implement Win32 PrintWindow → PNG bytes
    // Uses windows crate: Win32_Graphics_Gdi::PrintWindow + BitBlt
    anyhow::bail!("screenshot capture not yet implemented")
}

#[cfg(not(windows))]
pub async fn capture_window(_window_id: isize) -> anyhow::Result<Vec<u8>> {
    // TODO: xcap crate for Linux, CGWindowListCreateImage for macOS
    anyhow::bail!("screenshot capture not yet implemented for this platform")
}
