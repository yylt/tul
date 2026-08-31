//! Text tools: encoding conversion, JSON format, Markdown→HTML,
//! line dedupe/sort, CSV→JSON.
//!
//! Pure-Rust logic lives in `*_impl` functions so it is host-testable;
//! the `#[wasm_bindgen]` wrappers only convert errors to JS exceptions.

use std::collections::BTreeSet;

use base64::Engine;
use wasm_bindgen::prelude::*;

use crate::js_err;

// ---------------------------------------------------------------------------
// encoding conversion
// ---------------------------------------------------------------------------

/// Convert raw bytes (UTF-8 string) to Base64.
#[wasm_bindgen]
pub fn to_base64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// Decode a Base64 string to raw bytes.
#[wasm_bindgen]
pub fn from_base64(s: &str) -> Result<Vec<u8>, JsValue> {
    from_base64_impl(s).map_err(js_err)
}

pub(crate) fn from_base64_impl(s: &str) -> Result<Vec<u8>, String> {
    base64::engine::general_purpose::STANDARD
        .decode(s.trim())
        .map_err(|e| format!("invalid base64: {e}"))
}

/// Encode bytes as lowercase hex.
#[wasm_bindgen]
pub fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Decode a hex string to bytes (accepts 0x prefix, case-insensitive).
#[wasm_bindgen]
pub fn from_hex(s: &str) -> Result<Vec<u8>, JsValue> {
    from_hex_impl(s).map_err(js_err)
}

pub(crate) fn from_hex_impl(s: &str) -> Result<Vec<u8>, String> {
    let s = s.trim().trim_start_matches("0x").trim_start_matches("0X");
    if !s.len().is_multiple_of(2) {
        return Err("hex string must have even length".to_string());
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|e| format!("invalid hex: {e}")))
        .collect()
}

/// Encode UTF-8 text to GBK bytes. Non-GBK characters become '?'.
#[wasm_bindgen]
pub fn utf8_to_gbk(text: &str) -> Vec<u8> {
    let (cow, _, _) = encoding_rs::GBK.encode(text);
    cow.into_owned()
}

/// Decode GBK bytes to UTF-8 text. Invalid sequences become U+FFFD.
#[wasm_bindgen]
pub fn gbk_to_utf8(bytes: &[u8]) -> String {
    let (cow, _, _) = encoding_rs::GBK.decode(bytes);
    cow.into_owned()
}

// ---------------------------------------------------------------------------
// JSON
// ---------------------------------------------------------------------------

/// Format JSON with 2-space indent. Throws on invalid JSON.
#[wasm_bindgen]
pub fn json_format(text: &str) -> Result<String, JsValue> {
    json_format_impl(text).map_err(js_err)
}

pub(crate) fn json_format_impl(text: &str) -> Result<String, String> {
    let v: serde_json::Value =
        serde_json::from_str(text).map_err(|e| format!("invalid JSON: {e}"))?;
    serde_json::to_string_pretty(&v).map_err(|e| format!("serialize failed: {e}"))
}

/// Minify JSON (no whitespace). Throws on invalid JSON.
#[wasm_bindgen]
pub fn json_minify(text: &str) -> Result<String, JsValue> {
    json_minify_impl(text).map_err(js_err)
}

pub(crate) fn json_minify_impl(text: &str) -> Result<String, String> {
    let v: serde_json::Value =
        serde_json::from_str(text).map_err(|e| format!("invalid JSON: {e}"))?;
    serde_json::to_string(&v).map_err(|e| format!("serialize failed: {e}"))
}

// ---------------------------------------------------------------------------
// Markdown
// ---------------------------------------------------------------------------

/// Render CommonMark Markdown to an HTML fragment (no <html>/<body> wrapper).
#[wasm_bindgen]
pub fn markdown_to_html(text: &str) -> String {
    let parser = pulldown_cmark::Parser::new(text);
    let mut out = String::new();
    pulldown_cmark::html::push_html(&mut out, parser);
    out
}

// ---------------------------------------------------------------------------
// lines
// ---------------------------------------------------------------------------

/// Remove duplicate lines, preserving first-occurrence order.
#[wasm_bindgen]
pub fn dedupe_lines(text: &str) -> String {
    let mut seen = BTreeSet::new();
    text.lines()
        .filter(|l| seen.insert(l.to_string()))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Sort lines; `numeric` sorts by leading number when present, `desc` reverses.
#[wasm_bindgen]
pub fn sort_lines(text: &str, numeric: bool, desc: bool) -> String {
    let mut lines: Vec<&str> = text.lines().collect();
    if numeric {
        lines.sort_by(|a, b| {
            let na = a
                .split(|c: char| !c.is_ascii_digit())
                .next()
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(f64::INFINITY);
            let nb = b
                .split(|c: char| !c.is_ascii_digit())
                .next()
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(f64::INFINITY);
            na.partial_cmp(&nb).unwrap_or(std::cmp::Ordering::Equal)
        });
    } else {
        lines.sort();
    }
    if desc {
        lines.reverse();
    }
    lines.join("\n")
}

// ---------------------------------------------------------------------------
// CSV → JSON
// ---------------------------------------------------------------------------

/// Parse CSV text (header row required) into a JSON array of objects.
#[wasm_bindgen]
pub fn csv_to_json(text: &str, delimiter: char) -> Result<String, JsValue> {
    csv_to_json_impl(text, delimiter).map_err(js_err)
}

pub(crate) fn csv_to_json_impl(text: &str, delimiter: char) -> Result<String, String> {
    let delimiter = if delimiter == '\0' {
        b','
    } else {
        delimiter as u8
    };
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .from_reader(text.as_bytes());
    let headers = rdr
        .headers()
        .map_err(|e| format!("csv header: {e}"))?
        .clone();
    let mut out = Vec::new();
    for rec in rdr.records() {
        let rec = rec.map_err(|e| format!("csv row: {e}"))?;
        let mut obj = serde_json::Map::new();
        for (h, v) in headers.iter().zip(rec.iter()) {
            obj.insert(h.to_string(), serde_json::Value::String(v.to_string()));
        }
        out.push(serde_json::Value::Object(obj));
    }
    serde_json::to_string(&out).map_err(|e| format!("serialize failed: {e}"))
}
