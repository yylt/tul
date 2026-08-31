//! Office WASM module for /tul_cv: xlsx → CSV/JSON, text → docx.
//!
//! Split out of the main tul-cv-wasm crate so its heavier dependencies
//! (calamine, docx-rs) are loaded lazily in the browser.

use std::io::Cursor;

use calamine::{open_workbook_from_rs, Data, Reader, Xlsx};
use wasm_bindgen::prelude::*;

fn js_err(e: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&e.to_string())
}

/// Read the first worksheet of an .xlsx file and return it as CSV text.
/// An empty `sheet` selects the first sheet.
#[wasm_bindgen]
pub fn xlsx_to_csv(bytes: &[u8], sheet: &str) -> Result<String, JsValue> {
    let mut workbook: Xlsx<Cursor<Vec<u8>>> = open_workbook_from_rs(Cursor::new(bytes.to_vec()))
        .map_err(|e| js_err(format!("cannot open xlsx: {e:?}")))?;

    let sheet = if sheet.is_empty() {
        workbook
            .sheet_names()
            .first()
            .cloned()
            .ok_or_else(|| js_err("workbook has no sheets"))?
    } else {
        sheet.to_string()
    };

    let range = workbook
        .worksheet_range(&sheet)
        .map_err(|e| js_err(format!("sheet '{sheet}' error: {e:?}")))?;

    let mut wtr = csv::WriterBuilder::new()
        .has_headers(false)
        .from_writer(Vec::new());
    for row in range.rows() {
        wtr.write_record(row.iter().map(cell_to_string))
            .map_err(|e| js_err(format!("csv write: {e}")))?;
    }
    wtr.flush().map_err(|e| js_err(format!("csv flush: {e}")))?;
    String::from_utf8(wtr.into_inner().map_err(|e| js_err(format!("csv: {e}")))?)
        .map_err(|e| js_err(format!("csv is not utf-8: {e}")))
}

/// Read the first worksheet of an .xlsx file and return it as a JSON array
/// of objects, using the first row as headers.
#[wasm_bindgen]
pub fn xlsx_to_json(bytes: &[u8], sheet: &str) -> Result<String, JsValue> {
    let mut workbook: Xlsx<Cursor<Vec<u8>>> = open_workbook_from_rs(Cursor::new(bytes.to_vec()))
        .map_err(|e| js_err(format!("cannot open xlsx: {e:?}")))?;

    let sheet = if sheet.is_empty() {
        workbook
            .sheet_names()
            .first()
            .cloned()
            .ok_or_else(|| js_err("workbook has no sheets"))?
    } else {
        sheet.to_string()
    };

    let range = workbook
        .worksheet_range(&sheet)
        .map_err(|e| js_err(format!("sheet '{sheet}' error: {e:?}")))?;

    let mut rows = range.rows();
    let headers: Vec<String> = rows
        .next()
        .map(|r| r.iter().map(cell_to_string).collect())
        .unwrap_or_default();

    let mut out = Vec::new();
    for row in rows {
        let mut obj = serde_json::Map::new();
        for (i, cell) in row.iter().enumerate() {
            let key = headers
                .get(i)
                .cloned()
                .unwrap_or_else(|| format!("col{}", i + 1));
            if key.is_empty() {
                continue;
            }
            obj.insert(key, serde_json::Value::String(cell_to_string(cell)));
        }
        out.push(serde_json::Value::Object(obj));
    }
    serde_json::to_string(&out).map_err(|e| js_err(format!("serialize failed: {e}")))
}

fn cell_to_string(cell: &Data) -> String {
    match cell {
        Data::Float(f) if f.fract() == 0.0 => format!("{:.0}", f),
        other => other.to_string(),
    }
}

/// Generate a .docx from plain text (one paragraph per line).
/// Returns the docx bytes.
#[wasm_bindgen]
pub fn text_to_docx(text: &str) -> Result<Vec<u8>, JsValue> {
    use docx_rs::{Docx, Paragraph, Run};

    let mut doc = Docx::new();
    for line in text.lines() {
        doc = doc.add_paragraph(Paragraph::new().add_run(Run::new().add_text(line)));
    }

    let mut buf = Cursor::new(Vec::new());
    doc.build()
        .pack(&mut buf)
        .map_err(|e| js_err(format!("docx pack failed: {e}")))?;
    Ok(buf.into_inner())
}
