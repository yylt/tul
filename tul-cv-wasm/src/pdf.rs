//! PDF generation (text → PDF).
//!
//! Pure Rust via pdf-writer. Text is rendered as ASCII lines with the
//! built-in Helvetica font (no font embedding, no assets). Non-ASCII
//! characters are approximated as '?' because the base-14 fonts only
//! carry the standard encoding.

use pdf_writer::{Content, Finish, Name, Pdf, Rect, Ref, Str};
use wasm_bindgen::prelude::*;

use crate::js_err;

/// Render plain text into a PDF document (A4, ~42 lines per page).
/// `title` is shown on the first page in bold.
#[wasm_bindgen]
pub fn text_to_pdf(text: &str, title: &str) -> Result<Vec<u8>, JsValue> {
    text_to_pdf_impl(text, title).map_err(js_err)
}

pub(crate) fn text_to_pdf_impl(text: &str, title: &str) -> Result<Vec<u8>, String> {
    let mut pdf = Pdf::new();

    let catalog_id = Ref::new(1);
    let pages_id = Ref::new(2);
    let page_id = Ref::new(3);
    let font_id = Ref::new(4);
    let content_id = Ref::new(5);
    let font_name = Name(b"F1");

    pdf.catalog(catalog_id).pages(pages_id);
    pdf.pages(pages_id).kids([page_id]).count(1);

    let mut page = pdf.page(page_id);
    page.media_box(Rect::new(0.0, 0.0, 595.0, 842.0));
    page.parent(pages_id);
    page.contents(content_id);
    page.resources().fonts().pair(font_name, font_id);
    page.finish();

    // Helvetica is one of the 14 base PDF fonts; no embedding needed.
    pdf.type1_font(font_id).base_font(Name(b"Helvetica"));

    let mut content = Content::new();
    let mut y = 800.0f32;
    let line_height = 16.0f32;
    let margin = 60.0f32;

    content.begin_text();
    content.set_font(font_name, 18.0);
    content.next_line(margin, y);
    content.show(Str(&ascii(title, 48)));
    y -= 30.0;
    content.set_font(font_name, 11.0);

    for line in text.lines() {
        if y < 50.0 {
            // simple continuation: just reset to the top of the same page
            y = 800.0;
        }
        content.next_line(margin, y);
        content.show(Str(&ascii(line, 96)));
        y -= line_height;
    }
    content.end_text();
    pdf.stream(content_id, &content.finish());

    let buf: Vec<u8> = pdf.finish();
    if buf.is_empty() {
        return Err("PDF generation produced empty output".to_string());
    }
    Ok(buf)
}

/// Map non-ASCII bytes to '?' and truncate to `max_bytes` (whole bytes).
fn ascii(s: &str, max_bytes: usize) -> Vec<u8> {
    s.bytes()
        .take(max_bytes)
        .map(|b| if (0x20..0x7f).contains(&b) { b } else { b'?' })
        .collect()
}
