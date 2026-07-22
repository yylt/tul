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
    let cf_city = req
        .headers()
        .get("CF-City")?
        .unwrap_or_else(|| "-".to_string());
    let colo = req
        .cf()
        .map(|cf| cf.colo())
        .unwrap_or_else(|| "-".to_string());
    let x_real_ip = req
        .headers()
        .get("X-Real-IP")?
        .unwrap_or_else(|| "-".to_string());

    let rows = [
        ("IP Address", ip.clone()),
        ("Country", cf_country.clone()),
        ("City", cf_city.clone()),
        ("Colo", colo.clone()),
        ("X-Real-IP", x_real_ip.clone()),
        ("X-Forwarded-For", xff.clone()),
        ("User Agent", ua.clone()),
        ("Language", lang.clone()),
        ("Referer", referer.clone()),
        ("Host", host.clone()),
        ("Method", method.clone()),
        ("Encoding", encoding.clone()),
        ("MIME Type", mime.clone()),
    ];

    let mut html = String::from(include_str!("../html/ip.html"));

    let mut table_rows = String::new();
    for (label, value) in &rows {
        let value_html = if *label == "Colo" {
            format!(
                r#"<a href="https://www.iata.org/en/publications/directories/code-search/?airport.search={}" title="IATA airport code">{}</a>"#,
                escape_html(value),
                escape_html(value)
            )
        } else {
            escape_html(value)
        };
        table_rows.push_str(&format!(
            "<tr><td>{}</td><td>{}</td></tr>",
            escape_html(label),
            value_html
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

pub async fn handler_dl(req: &Request) -> Result<Response> {
    let host = req
        .headers()
        .get("Host")?
        .unwrap_or_else(|| "-".to_string());

    let mut html = String::from(include_str!("../html/tul_dl.html"));
    html = html.replace("{HOST}", &escape_html(&host));

    let headers = Headers::new();
    headers.set("Content-Type", "text/html; charset=utf-8")?;
    headers.set("Cache-Control", "no-store")?;

    Ok(Response::builder()
        .with_headers(headers)
        .with_status(200)
        .body(ResponseBody::Body(html.into_bytes())))
}

pub async fn handler_s(_req: &Request) -> Result<Response> {
    let html = String::from(include_str!("../html/tul_s.html"));

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
