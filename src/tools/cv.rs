use worker::*;

/// `/tul_cv` — browser-side converter tools page.
///
/// The page is fully client-side: the WASM modules (tul_cv_wasm.js +
/// tul_cv_wasm_bg.wasm for the core, tul_cv_office_wasm.js + ..._bg.wasm
/// for the lazily-loaded office tools) are served as static assets next
/// to the HTML.
pub async fn handler(_req: &Request) -> Result<Response> {
    let normal_css = include_str!("../html/tul_normal.css");
    let mut html = String::from(include_str!("../html/cv.html"));
    html = html.replace("<!-- NORMAL_CSS -->", normal_css);

    let headers = Headers::new();
    headers.set("Content-Type", "text/html; charset=utf-8")?;
    headers.set("Cache-Control", "no-store")?;

    Ok(Response::builder()
        .with_headers(headers)
        .with_status(200)
        .body(ResponseBody::Body(html.into_bytes())))
}

/// Serve the WASM JS glue and binaries for `/tul_cv/*`.
pub async fn asset_handler(req: &Request) -> Result<Response> {
    let path = req.path();
    let (bytes, content_type, cache) = match path.strip_prefix("/tul_cv/") {
        Some("tul_cv_wasm.js") => (
            include_bytes!("../html/tul_cv_wasm.js").as_slice(),
            "application/javascript",
            "public, max-age=3600",
        ),
        Some("tul_cv_wasm_bg.wasm") => (
            include_bytes!("../html/tul_cv_wasm_bg.wasm").as_slice(),
            "application/wasm",
            "public, max-age=3600",
        ),
        Some("tul_cv_office_wasm.js") => (
            include_bytes!("../html/tul_cv_office_wasm.js").as_slice(),
            "application/javascript",
            "public, max-age=3600",
        ),
        Some("tul_cv_office_wasm_bg.wasm") => (
            include_bytes!("../html/tul_cv_office_wasm_bg.wasm").as_slice(),
            "application/wasm",
            "public, max-age=3600",
        ),
        _ => return Response::error("Not Found", 404),
    };

    let headers = Headers::new();
    headers.set("Content-Type", content_type)?;
    headers.set("Cache-Control", cache)?;

    Ok(Response::builder()
        .with_headers(headers)
        .with_status(200)
        .body(ResponseBody::Body(bytes.to_vec())))
}
