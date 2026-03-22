use core_graphics::geometry::CGRect;
use core_graphics::window::{
    create_image, kCGWindowImageBestResolution, kCGWindowImageBoundsIgnoreFraming,
    kCGWindowListOptionIncludingWindow, kCGWindowListOptionOnScreenOnly, CGWindowID,
};
use image::codecs::png::PngEncoder;
use image::{ImageBuffer, ImageEncoder, Rgba};
use loki_core::{LokiError, LokiResult};
use tracing::debug;

/// Capture a single window by its CGWindowID, returning PNG bytes.
pub fn capture_window(window_id: u32) -> LokiResult<Vec<u8>> {
    debug!(window_id, "capturing window screenshot");

    // CGRectNull (all zeros) = capture full window bounds
    let rect = CGRect::new(
        &core_graphics::geometry::CGPoint::new(0.0, 0.0),
        &core_graphics::geometry::CGSize::new(0.0, 0.0),
    );

    let image_option = kCGWindowImageBoundsIgnoreFraming | kCGWindowImageBestResolution;

    let cg_image = create_image(
        rect,
        kCGWindowListOptionIncludingWindow,
        window_id as CGWindowID,
        image_option,
    )
    .ok_or_else(|| {
        LokiError::ScreenshotFailed(format!(
            "CGWindowListCreateImage returned null for window {window_id}"
        ))
    })?;

    cgimage_to_png(&cg_image)
}

/// Capture the full screen, returning PNG bytes.
pub fn capture_screen() -> LokiResult<Vec<u8>> {
    debug!("capturing full screen screenshot");

    // Use CGRectInfinite for full screen capture
    let rect = unsafe { CGRectInfinite };

    let cg_image = create_image(
        rect,
        kCGWindowListOptionOnScreenOnly,
        0, // kCGNullWindowID
        0, // kCGWindowImageDefault
    )
    .ok_or_else(|| {
        LokiError::ScreenshotFailed("CGWindowListCreateImage returned null for screen".into())
    })?;

    cgimage_to_png(&cg_image)
}

/// Convert a CGImage to PNG bytes.
fn cgimage_to_png(cg_image: &core_graphics::image::CGImage) -> LokiResult<Vec<u8>> {
    let width = cg_image.width();
    let height = cg_image.height();
    let bytes_per_row = cg_image.bytes_per_row();
    let bits_per_pixel = cg_image.bits_per_pixel();

    debug!(
        width,
        height, bytes_per_row, bits_per_pixel, "converting CGImage to PNG"
    );

    let raw_data = cg_image.data();
    let raw_bytes: &[u8] = &raw_data;

    // macOS screenshots are typically BGRA (32-bit, 8 bits per component)
    // with possible padding in bytes_per_row
    if bits_per_pixel != 32 {
        return Err(LokiError::ScreenshotFailed(format!(
            "unexpected bits_per_pixel: {bits_per_pixel} (expected 32)"
        )));
    }

    // Build RGBA image buffer, handling row stride
    let mut img_buf: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(width as u32, height as u32);

    for y in 0..height {
        let row_start = y * bytes_per_row;
        for x in 0..width {
            let px_offset = row_start + x * 4;
            if px_offset + 3 >= raw_bytes.len() {
                break;
            }
            // macOS CGImage with default color space: BGRA (or ARGB premultiplied)
            // Most common is kCGImageAlphaPremultipliedFirst = ARGB in memory,
            // but with 32Big byte order it's actually BGRA on little-endian.
            // We'll handle the common case: bytes are [B, G, R, A]
            let b = raw_bytes[px_offset];
            let g = raw_bytes[px_offset + 1];
            let r = raw_bytes[px_offset + 2];
            let a = raw_bytes[px_offset + 3];
            img_buf.put_pixel(x as u32, y as u32, Rgba([r, g, b, a]));
        }
    }

    // Encode as PNG
    let mut png_bytes: Vec<u8> = Vec::new();
    let encoder = PngEncoder::new(&mut png_bytes);
    encoder
        .write_image(
            img_buf.as_raw(),
            width as u32,
            height as u32,
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|e| LokiError::ScreenshotFailed(format!("PNG encoding failed: {e}")))?;

    debug!(png_size = png_bytes.len(), "screenshot encoded as PNG");
    Ok(png_bytes)
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    static CGRectInfinite: CGRect;
}
