//! Client-side WASM tools for /tul_cv.
//!
//! All processing happens in the browser: image conversion, text tools,
//! unit conversion. Compiled with `wasm-pack build --target web`.

use wasm_bindgen::prelude::*;

pub mod image;
pub mod pdf;
pub mod text;
pub mod unit;

/// Convert an error into a JS exception message.
pub(crate) fn js_err(e: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&e.to_string())
}

#[cfg(test)]
mod tests {
    use crate::image;
    use crate::pdf;
    use crate::text;
    use crate::unit;

    fn tiny_png() -> Vec<u8> {
        // 2x2 red PNG
        let mut img = ::image::RgbaImage::new(2, 2);
        for p in img.pixels_mut() {
            *p = ::image::Rgba([255, 0, 0, 255]);
        }
        let mut out = std::io::Cursor::new(Vec::new());
        ::image::DynamicImage::ImageRgba8(img)
            .write_to(&mut out, ::image::ImageFormat::Png)
            .unwrap();
        out.into_inner()
    }

    #[test]
    fn png_to_jpeg_roundtrip() {
        let jpg = image::convert_to_jpeg_impl(&tiny_png(), 85).unwrap();
        assert!(jpg.len() > 20);
        assert_eq!(&jpg[..2], &[0xFF, 0xD8]);
    }

    #[test]
    fn png_to_webp_roundtrip() {
        let webp = image::convert_to_webp_impl(&tiny_png()).unwrap();
        assert_eq!(&webp[..4], b"RIFF");
    }

    #[test]
    fn jpeg_to_png_roundtrip() {
        let jpg = image::convert_to_jpeg_impl(&tiny_png(), 85).unwrap();
        let png = image::convert_to_png_impl(&jpg).unwrap();
        assert_eq!(&png[..4], &[0x89, b'P', b'N', b'G']);
    }

    #[test]
    fn resize_keeps_ratio() {
        let jpg = image::convert_to_jpeg_impl(&tiny_png(), 85).unwrap();
        let out = image::resize_image_impl(&jpg, 1024, true, "png").unwrap();
        let img = ::image::load_from_memory(&out).unwrap();
        assert_eq!(img.width(), 1024);
        assert_eq!(img.height(), 1024);
    }

    #[test]
    fn crop_image_basic() {
        let png = tiny_png();
        let out = image::crop_image_impl(&png, 0, 0, 1, 1, "png").unwrap();
        let img = ::image::load_from_memory(&out).unwrap();
        assert_eq!((img.width(), img.height()), (1, 1));
    }

    #[test]
    fn text_to_image_png() {
        let out = image::text_to_image_impl("AB", 2).unwrap();
        let img = ::image::load_from_memory(&out).unwrap();
        assert!(img.width() >= 16);
    }

    #[test]
    fn watermark_runs() {
        let out = image::add_text_watermark_impl(&tiny_png(), "TUL", 96, "png").unwrap();
        assert_eq!(&out[..4], &[0x89, b'P', b'N', b'G']);
    }

    #[test]
    fn base64_roundtrip() {
        let enc = text::to_base64(b"hello");
        assert_eq!(text::from_base64_impl(&enc).unwrap(), b"hello");
    }

    #[test]
    fn hex_roundtrip() {
        assert_eq!(text::to_hex(b"\x00\xff\x10"), "00ff10");
        assert_eq!(
            text::from_hex_impl("0x00FF10").unwrap(),
            vec![0x00, 0xFF, 0x10]
        );
    }

    #[test]
    fn gbk_roundtrip() {
        let gbk = text::utf8_to_gbk("中文");
        assert_eq!(text::gbk_to_utf8(&gbk), "中文");
    }

    #[test]
    fn json_format_minify() {
        let pretty = text::json_format_impl(r#"{"a":1}"#).unwrap();
        assert!(pretty.contains('\n'));
        assert_eq!(text::json_minify_impl(&pretty).unwrap(), r#"{"a":1}"#);
        assert!(text::json_format_impl("{bad").is_err());
    }

    #[test]
    fn markdown_to_html() {
        let html = text::markdown_to_html("# hi\n\n**bold**");
        assert!(html.contains("<h1>"));
        assert!(html.contains("<strong>"));
    }

    #[test]
    fn dedupe_sort_lines() {
        assert_eq!(text::dedupe_lines("b\na\nb"), "b\na");
        assert_eq!(text::sort_lines("10\n2\n1", true, false), "1\n2\n10");
        assert_eq!(text::sort_lines("b\na", false, true), "b\na");
    }

    #[test]
    fn csv_to_json_basic() {
        let json = text::csv_to_json_impl("name,age\nalice,30", ',').unwrap();
        assert!(json.contains("alice"));
        assert!(json.contains("30"));
    }

    #[test]
    fn unit_conversions() {
        assert!((unit::convert_unit_impl("length", 1.0, "km", "m").unwrap() - 1000.0).abs() < 1e-9);
        assert!(
            (unit::convert_unit_impl("weight", 1.0, "lb", "kg").unwrap() - 0.45359237).abs() < 1e-9
        );
        assert!(
            (unit::convert_unit_impl("temperature", 0.0, "C", "F").unwrap() - 32.0).abs() < 1e-9
        );
        assert!((unit::convert_unit_impl("data", 1.0, "GB", "MB").unwrap() - 1024.0).abs() < 1e-9);
        assert!((unit::convert_unit_impl("time", 1.0, "h", "min").unwrap() - 60.0).abs() < 1e-9);
        assert!(unit::convert_unit_impl("length", 1.0, "bogus", "m").is_err());
    }

    #[test]
    fn text_to_pdf_bytes() {
        let pdf_bytes = pdf::text_to_pdf_impl("hello\nworld", "Title").unwrap();
        assert!(pdf_bytes.starts_with(b"%PDF"));
        assert!(pdf_bytes.len() > 100);
    }
}
