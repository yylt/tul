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

pub async fn handler_colo(req: &Request) -> Result<Response> {
    let colo = req
        .cf()
        .map(|cf| cf.colo())
        .unwrap_or_else(|| "unknown".to_string());

    Ok(Response::builder()
        .with_status(200)
        .with_header("Content-Type", "text/plain; charset=utf-8")?
        .body(ResponseBody::Body(colo.into_bytes())))
}

fn lookup_ip(query_ip: Option<&str>, client_ip: &str) -> Option<String> {
    query_ip
        .unwrap_or(client_ip)
        .trim()
        .parse::<std::net::IpAddr>()
        .ok()
        .map(|ip| ip.to_string())
}

pub async fn handler_details(req: &Request, env: &Env) -> Result<Response> {
    let Ok(mapurl) = env.secret("MAPURL") else {
        return Response::error("IP details are not configured", 404);
    };
    let request_url = req.url()?;
    let query_ip = request_url
        .query_pairs()
        .find(|(key, _)| key == "ip")
        .map(|(_, value)| value.into_owned());
    let client_ip = req.headers().get("CF-Connecting-IP")?.unwrap_or_default();
    let Some(ip) = lookup_ip(query_ip.as_deref(), &client_ip) else {
        return Response::error("Invalid IP address", 400);
    };
    let Ok(mut url) = Url::parse(&mapurl.to_string()) else {
        return Response::error("IP lookup is unavailable", 502);
    };
    if !matches!(url.scheme(), "http" | "https") {
        return Response::error("IP lookup is unavailable", 502);
    }
    let pairs: Vec<_> = url
        .query_pairs()
        .filter(|(key, _)| key != "ip")
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect();
    url.query_pairs_mut()
        .clear()
        .extend_pairs(pairs)
        .append_pair("ip", &ip);
    let data = async {
        let mut response = Fetch::Url(url).send().await?;
        if response.status_code() != 200 {
            return Err(Error::RustError("IP lookup failed".into()));
        }
        let data = response.json::<serde_json::Value>().await?;
        if !data.is_array() {
            return Err(Error::RustError("Invalid IP lookup response".into()));
        }
        Ok(data)
    }
    .await;
    match data {
        Ok(data) => {
            let mut response = Response::from_json(&data)?;
            response.headers_mut().set("Cache-Control", "no-store")?;
            Ok(response)
        }
        Err(_) => Response::error("IP lookup is unavailable", 502),
    }
}

pub async fn handler_html(req: &Request, env: &Env) -> Result<Response> {
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
        ("IP Address", "row_ip", ip.clone()),
        ("Country", "row_country", cf_country.clone()),
        ("City", "row_city", cf_city.clone()),
        ("CF Edge Location", "row_colo", colo.clone()),
        ("X-Real-IP", "row_xreal", x_real_ip.clone()),
        ("X-Forwarded-For", "row_xff", xff.clone()),
        ("User Agent", "row_ua", ua.clone()),
        ("Language", "row_lang", lang.clone()),
        ("Referer", "row_referer", referer.clone()),
        ("Host", "row_host", host.clone()),
        ("Method", "row_method", method.clone()),
        ("Encoding", "row_encoding", encoding.clone()),
        ("MIME Type", "row_mime", mime.clone()),
    ];

    let normal_css = include_str!("../html/tul_normal.css");
    let mut html = String::from(include_str!("../html/index.html"));
    html = html.replace("<!-- NORMAL_CSS -->", normal_css);
    let details_script = if env.secret("MAPURL").is_ok() {
        include_str!("../html/ip_details.js")
    } else {
        ""
    };
    html = html.replace("/* IP_DETAILS */", details_script);

    // CV Tools entry is only shown when the tul_cv feature is compiled in.
    #[cfg(feature = "tul_cv")]
    let cv_tools = r#"<p><a href="/tul_cv" data-i18n="tool_cv_name">CV Tools</a> <span data-i18n="tool_cv_desc">&mdash; browser-side converters (image / text / unit)</span></p>"#;
    #[cfg(not(feature = "tul_cv"))]
    let cv_tools = "";
    html = html.replace("<!-- CV_TOOLS -->", cv_tools);

    let mut table_rows = String::new();
    for (label, i18n_key, value) in &rows {
        let value_html = if *i18n_key == "row_colo" {
            format!(
                r#"<a href="https://www.iata.org/en/publications/directories/code-search/?airport.search={}" title="IATA airport code">{}</a>"#,
                escape_html(value),
                escape_html(value)
            )
        } else {
            escape_html(value)
        };
        table_rows.push_str(&format!(
            "<tr><td data-i18n=\"{}\">{}</td><td>{}</td></tr>",
            i18n_key,
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

    let normal_css = include_str!("../html/tul_normal.css");
    let mut html = String::from(include_str!("../html/tul_dl.html"));
    html = html.replace("<!-- NORMAL_CSS -->", normal_css);
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
    let normal_css = include_str!("../html/tul_normal.css");
    let mut html = String::from(include_str!("../html/tul_s.html"));
    html = html.replace("<!-- NORMAL_CSS -->", normal_css);

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

#[cfg(test)]
mod tests {
    use super::lookup_ip;

    #[test]
    fn lookup_ip_validates_query_and_defaults_to_client() {
        assert_eq!(lookup_ip(None, "1.1.1.1").as_deref(), Some("1.1.1.1"));
        assert_eq!(
            lookup_ip(Some("8.8.8.8"), "1.1.1.1").as_deref(),
            Some("8.8.8.8")
        );
        assert_eq!(
            lookup_ip(Some("2001:db8::1"), "").as_deref(),
            Some("2001:db8::1")
        );
        for invalid in ["", "example.com", "127.0.0.1&ip=8.8.8.8", "<script>"] {
            assert_eq!(lookup_ip(Some(invalid), "1.1.1.1"), None);
        }
        assert_eq!(lookup_ip(None, "unknown"), None);
    }
}
