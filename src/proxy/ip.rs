use worker::*;

pub async fn handler_text(req: &Request) -> Result<Response> {
    let ip = req
        .headers()
        .get("CF-Connecting-IP")?
        .unwrap_or_else(|| "unknown".to_string());

    Ok(Response::builder()
        .with_status(200)
        .body(ResponseBody::Body(ip.into_bytes())))
}

pub async fn handler_html(req: &Request) -> Result<Response> {
    let ip = req
        .headers()
        .get("CF-Connecting-IP")?
        .unwrap_or_else(|| "unknown".to_string());
    let ua = req
        .headers()
        .get("User-Agent")?
        .unwrap_or_else(|| "-".to_string());
    let lang = req
        .headers()
        .get("Accept-Language")?
        .unwrap_or_else(|| "-".to_string());
    let referer = req
        .headers()
        .get("Referer")?
        .unwrap_or_else(|| "-".to_string());
    let method = req.method().to_string();
    let host = req
        .headers()
        .get("Host")?
        .unwrap_or_else(|| "-".to_string());
    let encoding = req
        .headers()
        .get("Accept-Encoding")?
        .unwrap_or_else(|| "-".to_string());
    let mime = req
        .headers()
        .get("Accept")?
        .unwrap_or_else(|| "-".to_string());
    let xff = req
        .headers()
        .get("X-Forwarded-For")?
        .unwrap_or_else(|| "-".to_string());
    let cf_country = req
        .headers()
        .get("CF-IPCountry")?
        .unwrap_or_else(|| "-".to_string());

    let rows = [
        ("IP Address", &ip),
        ("Country", &cf_country),
        ("User Agent", &ua),
        ("Language", &lang),
        ("Referer", &referer),
        ("Host", &host),
        ("Method", &method),
        ("Encoding", &encoding),
        ("MIME Type", &mime),
        ("X-Forwarded-For", &xff),
    ];

    let mut html = String::from(include_str!("ip.html"));

    let mut table_rows = String::new();
    for (label, value) in &rows {
        table_rows.push_str(&format!(
            "<tr><td>{}</td><td>{}</td></tr>",
            escape_html(label),
            escape_html(value)
        ));
    }
    html = html.replace("<!-- ROWS -->", &table_rows);
    html = html.replace("{IP}", &escape_html(&ip));
    html = html.replace("{HOST}", &escape_html(&host));

    let headers = Headers::new();
    headers.set("Content-Type", "text/html; charset=utf-8")?;
    headers.set("Cache-Control", "no-store")?;

    Ok(Response::builder()
        .with_headers(headers)
        .with_status(200)
        .body(ResponseBody::Body(html.into_bytes())))
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
