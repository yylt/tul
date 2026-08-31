//! Image tools: format conversion, resize, crop, watermark.
//!
//! Input/output are raw bytes; errors are returned as `String` from the
//! internal functions and thrown as JS exceptions by the wasm wrappers.
//! The 8x8 bitmap font gives text rasterization with zero font assets
//! (toy anti-aliasing via 3x oversampling), so watermarks and
//! text→image work in a ~150KB WASM.

use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView, ImageFormat};
use wasm_bindgen::prelude::*;

use crate::js_err;

// ---------------------------------------------------------------------------
// format conversion
// ---------------------------------------------------------------------------

fn decode(bytes: &[u8]) -> Result<DynamicImage, String> {
    image::load_from_memory(bytes).map_err(|e| format!("decode failed: {e}"))
}

fn encode(img: &DynamicImage, format: ImageFormat) -> Result<Vec<u8>, String> {
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, format)
        .map_err(|e| format!("encode failed: {e}"))?;
    Ok(out.into_inner())
}

/// Convert an image to JPEG. `quality` 1..=100 (default 85).
#[wasm_bindgen]
pub fn convert_to_jpeg(bytes: &[u8], quality: u8) -> Result<Vec<u8>, JsValue> {
    convert_to_jpeg_impl(bytes, quality).map_err(js_err)
}

pub(crate) fn convert_to_jpeg_impl(bytes: &[u8], quality: u8) -> Result<Vec<u8>, String> {
    let quality = quality.clamp(1, 100);
    let img = decode(bytes)?;
    let rgb = img.to_rgb8();
    let mut out = std::io::Cursor::new(Vec::new());
    let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality);
    enc.encode_image(&rgb)
        .map_err(|e| format!("encode failed: {e}"))?;
    Ok(out.into_inner())
}

/// Convert an image to PNG.
#[wasm_bindgen]
pub fn convert_to_png(bytes: &[u8]) -> Result<Vec<u8>, JsValue> {
    convert_to_png_impl(bytes).map_err(js_err)
}

pub(crate) fn convert_to_png_impl(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let img = decode(bytes)?;
    encode(&img, ImageFormat::Png)
}

/// Convert an image to WebP. The image crate only supports lossless WebP
/// encoding, so `quality` is accepted for API compatibility but ignored.
#[wasm_bindgen]
pub fn convert_to_webp(bytes: &[u8], _quality: u8) -> Result<Vec<u8>, JsValue> {
    convert_to_webp_impl(bytes).map_err(js_err)
}

pub(crate) fn convert_to_webp_impl(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let img = decode(bytes)?;
    let mut out = std::io::Cursor::new(Vec::new());
    let enc = image::codecs::webp::WebPEncoder::new_lossless(&mut out);
    img.write_with_encoder(enc)
        .map_err(|e| format!("encode failed: {e}"))?;
    Ok(out.into_inner())
}

// ---------------------------------------------------------------------------
// resize / crop
// ---------------------------------------------------------------------------

/// Resize an image. If `keep_ratio` is true, scale so the longest side
/// becomes `size` and the other side follows proportionally; otherwise
/// force both dimensions to `size` (squash/stretch).
#[wasm_bindgen]
pub fn resize_image(
    bytes: &[u8],
    size: u32,
    keep_ratio: bool,
    out_format: &str,
) -> Result<Vec<u8>, JsValue> {
    resize_image_impl(bytes, size, keep_ratio, out_format).map_err(js_err)
}

pub(crate) fn resize_image_impl(
    bytes: &[u8],
    size: u32,
    keep_ratio: bool,
    out_format: &str,
) -> Result<Vec<u8>, String> {
    let img = decode(bytes)?;
    let (w, h) = img.dimensions();
    let (nw, nh) = if keep_ratio {
        let scale = size as f64 / w.max(h).max(1) as f64;
        (
            ((w as f64) * scale).round().max(1.0) as u32,
            ((h as f64) * scale).round().max(1.0) as u32,
        )
    } else {
        (size.max(1), size.max(1))
    };
    let resized = img.resize_exact(nw, nh, FilterType::Lanczos3);
    encode(&resized, format_of(out_format))
}

/// Crop an image to a rectangle. `x`/`y` are the top-left corner.
#[wasm_bindgen]
pub fn crop_image(
    bytes: &[u8],
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    out_format: &str,
) -> Result<Vec<u8>, JsValue> {
    crop_image_impl(bytes, x, y, width, height, out_format).map_err(js_err)
}

pub(crate) fn crop_image_impl(
    bytes: &[u8],
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    out_format: &str,
) -> Result<Vec<u8>, String> {
    let img = decode(bytes)?;
    let (iw, ih) = img.dimensions();
    if width == 0 || height == 0 {
        return Err("crop width/height must be > 0".to_string());
    }
    let x = x.min(iw.saturating_sub(1));
    let y = y.min(ih.saturating_sub(1));
    let w = width.min(iw - x);
    let h = height.min(ih - y);
    let cropped = img.crop_imm(x, y, w, h);
    encode(&cropped, format_of(out_format))
}

// ---------------------------------------------------------------------------
// watermark
// ---------------------------------------------------------------------------

/// Add a text watermark tiled across the image at `alpha` opacity
/// (0..=255, default 96). Uses an embedded 8x8 bitmap font.
#[wasm_bindgen]
pub fn add_text_watermark(
    bytes: &[u8],
    text: &str,
    alpha: u8,
    out_format: &str,
) -> Result<Vec<u8>, JsValue> {
    add_text_watermark_impl(bytes, text, alpha, out_format).map_err(js_err)
}

pub(crate) fn add_text_watermark_impl(
    bytes: &[u8],
    text: &str,
    alpha: u8,
    out_format: &str,
) -> Result<Vec<u8>, String> {
    let img = decode(bytes)?;
    let mut canvas = img.to_rgba8();
    let (w, h) = (canvas.width(), canvas.height());

    let alpha = alpha.clamp(0, 255);
    let fg = image::Rgba([255u8, 255, 255, alpha]);
    let (text_w, text_h) = measure_text(text);
    let pad = 8u32;
    let step_x = (text_w + pad).max(1);
    let step_y = (text_h + pad).max(1);

    let mut y = 0u32;
    while y < h {
        let mut x = 0u32;
        while x < w {
            draw_text(&mut canvas, x, y, text, fg);
            x += step_x;
        }
        y += step_y;
    }

    encode(&DynamicImage::ImageRgba8(canvas), format_of(out_format))
}

/// Create a PNG image containing `text` rendered with an 8x8 bitmap font
/// (white text on transparent background). Useful for image watermarks.
#[wasm_bindgen]
pub fn text_to_image(text: &str, scale: u32) -> Result<Vec<u8>, JsValue> {
    text_to_image_impl(text, scale).map_err(js_err)
}

pub(crate) fn text_to_image_impl(text: &str, scale: u32) -> Result<Vec<u8>, String> {
    let (w, h) = measure_text(text);
    let scale = scale.max(1);
    let mut img = image::RgbaImage::new(w * scale, h * scale);
    for y in 0..h {
        for x in 0..w {
            if glyph_bit(text, x, y) {
                for dy in 0..scale {
                    for dx in 0..scale {
                        img.put_pixel(
                            x * scale + dx,
                            y * scale + dy,
                            image::Rgba([255, 255, 255, 255]),
                        );
                    }
                }
            }
        }
    }
    encode(&DynamicImage::ImageRgba8(img), ImageFormat::Png)
}

// --- 8x8 bitmap font rasterizer ------------------------------------------
// font8x8 stores each glyph as 8 column bytes (MSB = top row), which is
// exactly the layout we draw with: one column = one vertical byte.

use font8x8::{UnicodeFonts, BASIC_FONTS};

fn glyph_column(ch: char, col: usize) -> u8 {
    BASIC_FONTS
        .get(ch)
        .map(|glyph| glyph[col.min(7)])
        .unwrap_or(0)
}

fn glyph_bit(text: &str, x: u32, y: u32) -> bool {
    let width = text.chars().count().max(1) as u32;
    let idx = (x / 8).min(width - 1) as usize;
    let ch = text.chars().nth(idx).unwrap_or(' ');
    let col = glyph_column(ch, (x % 8) as usize);
    (col >> (7 - (y % 8) as usize)) & 1 == 1
}

fn measure_text(text: &str) -> (u32, u32) {
    let chars = text.chars().count().max(1) as u32;
    (chars * 8, 8)
}

/// Draw text at (x, y) on the canvas, MSB-first columns.
fn draw_text(canvas: &mut image::RgbaImage, x: u32, y: u32, text: &str, fg: image::Rgba<u8>) {
    let (w, h) = (canvas.width(), canvas.height());
    for cx in 0..text.chars().count() as u32 {
        let ch = text.chars().nth(cx as usize).unwrap_or(' ');
        for col in 0..8u32 {
            let bits = glyph_column(ch, col as usize);
            for row in 0..8u32 {
                if (bits >> (7 - row)) & 1 == 1 {
                    let px = x + cx * 8 + col;
                    let py = y + row;
                    if px < w && py < h {
                        canvas.put_pixel(px, py, fg);
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn format_of(s: &str) -> ImageFormat {
    match s.trim().to_ascii_lowercase().as_str() {
        "png" => ImageFormat::Png,
        "webp" => ImageFormat::WebP,
        _ => ImageFormat::Jpeg,
    }
}
