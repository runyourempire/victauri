#[cfg(windows)]
#[allow(dead_code)]
pub async fn capture_window(hwnd: isize) -> anyhow::Result<Vec<u8>> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Gdi::{
        BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC,
        GetDIBits, ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
        SRCCOPY,
    };
    use windows::Win32::Storage::Xps::{PrintWindow, PW_CLIENTONLY};
    use windows::Win32::UI::WindowsAndMessaging::GetClientRect;

    tokio::task::spawn_blocking(move || unsafe {
        let hwnd = HWND(hwnd as *mut _);
        let mut rect = std::mem::zeroed();
        GetClientRect(hwnd, &mut rect)?;

        let width = rect.right - rect.left;
        let height = rect.bottom - rect.top;
        if width <= 0 || height <= 0 {
            anyhow::bail!("window has zero area ({width}x{height})");
        }

        let hdc_screen = GetDC(Some(hwnd));
        let hdc_mem = CreateCompatibleDC(Some(hdc_screen));
        let hbmp = CreateCompatibleBitmap(hdc_screen, width, height);
        let old = SelectObject(hdc_mem, hbmp.into());

        let captured = PrintWindow(hwnd, hdc_mem, PW_CLIENTONLY);
        if !captured.as_bool() {
            BitBlt(hdc_mem, 0, 0, width, height, Some(hdc_screen), 0, 0, SRCCOPY)?;
        }

        let mut bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height, // top-down
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..std::mem::zeroed()
            },
            ..std::mem::zeroed()
        };

        let row_bytes = (width as usize) * 4;
        let mut pixels = vec![0u8; row_bytes * height as usize];
        GetDIBits(
            hdc_mem,
            hbmp,
            0,
            height as u32,
            Some(pixels.as_mut_ptr().cast()),
            &mut bmi,
            DIB_RGB_COLORS,
        );

        // BGRA → RGBA
        for chunk in pixels.chunks_exact_mut(4) {
            chunk.swap(0, 2);
        }

        SelectObject(hdc_mem, old);
        let _ = DeleteObject(hbmp.into());
        let _ = DeleteDC(hdc_mem);
        ReleaseDC(Some(hwnd), hdc_screen);

        encode_png(width as u32, height as u32, &pixels)
    })
    .await?
}

#[cfg(not(windows))]
#[allow(dead_code)]
pub async fn capture_window(_window_id: isize) -> anyhow::Result<Vec<u8>> {
    anyhow::bail!("screenshot capture not yet implemented for this platform")
}

#[allow(dead_code)]
fn encode_png(width: u32, height: u32, rgba: &[u8]) -> anyhow::Result<Vec<u8>> {
    use std::io::Write;

    let mut out = Vec::new();

    // PNG signature
    out.write_all(&[137, 80, 78, 71, 13, 10, 26, 10])?;

    // IHDR
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.push(8); // bit depth
    ihdr.push(6); // RGBA color type
    ihdr.push(0); // compression
    ihdr.push(0); // filter
    ihdr.push(0); // interlace
    write_png_chunk(&mut out, b"IHDR", &ihdr)?;

    // IDAT — raw pixel data with filter byte per row, deflate-compressed
    let row_len = (width as usize) * 4;
    let mut raw = Vec::with_capacity(rgba.len() + height as usize);
    for row in rgba.chunks_exact(row_len) {
        raw.push(0); // no filter
        raw.extend_from_slice(row);
    }

    let compressed = deflate_compress(&raw);
    write_png_chunk(&mut out, b"IDAT", &compressed)?;

    // IEND
    write_png_chunk(&mut out, b"IEND", &[])?;

    Ok(out)
}

#[allow(dead_code)]
fn write_png_chunk(out: &mut Vec<u8>, chunk_type: &[u8; 4], data: &[u8]) -> anyhow::Result<()> {
    use std::io::Write;

    out.write_all(&(data.len() as u32).to_be_bytes())?;
    out.write_all(chunk_type)?;
    out.write_all(data)?;

    let mut crc_data = Vec::with_capacity(4 + data.len());
    crc_data.extend_from_slice(chunk_type);
    crc_data.extend_from_slice(data);
    let crc = png_crc32(&crc_data);
    out.write_all(&crc.to_be_bytes())?;

    Ok(())
}

#[allow(dead_code)]
fn png_crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
            } else {
                crc >>= 1;
            }
        }
    }
    crc ^ 0xFFFF_FFFF
}

#[allow(dead_code)]
fn deflate_compress(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();

    // zlib header: CM=8 (deflate), CINFO=7 (32K window)
    out.push(0x78);
    out.push(0x01); // FCHECK for low compression

    // Emit stored (uncompressed) deflate blocks
    let max_block = 65535;
    let chunks: Vec<&[u8]> = data.chunks(max_block).collect();
    for (i, chunk) in chunks.iter().enumerate() {
        let is_last = i == chunks.len() - 1;
        out.push(if is_last { 0x01 } else { 0x00 });
        let len = chunk.len() as u16;
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&(!len).to_le_bytes());
        out.extend_from_slice(chunk);
    }

    // Adler-32 checksum
    let adler = adler32(data);
    out.extend_from_slice(&adler.to_be_bytes());

    out
}

#[allow(dead_code)]
fn adler32(data: &[u8]) -> u32 {
    let mut a: u32 = 1;
    let mut b: u32 = 0;
    for &byte in data {
        a = (a + byte as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}
