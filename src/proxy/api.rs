use super::*;
use regex::Regex;
use std::collections::{HashMap, HashSet};
use tokio::sync::OnceCell;

static REGISTRY: &str = "registry-1.docker.io";

static HOP_HEADERS: OnceCell<HashSet<&'static str>> = OnceCell::const_new();

async fn get_hop_headers() -> &'static HashSet<&'static str> {
    HOP_HEADERS
        .get_or_init(|| async {
            HashSet::from([
                // RFC 2616 hop-by-hop
                "authorization",
                "connection",
                "content-length",
                "host",
                "keep-alive",
                "proxy-authenticate",
                "proxy-authorization",
                "referer",
                "te",
                "trailer",
                "transfer-encoding",
                "upgrade",
                // content negotiation / integrity
                "accept-encoding",
                "content-md5",
                // Azure Storage signing headers
                "x-ms-date",
                "x-ms-version",
                "x-ms-blob-type",
                // Cloudflare metadata
                "cf-connecting-ip",
                "cf-ew-via",
                "cf-ipcountry",
                "cf-ray",
                "cf-request-id",
                "cf-visitor",
                "cf-worker",
                // loop prevention
                "cdn-loop",
                // proxy generated
                "via",
                "x-forwarded-for",
                "x-forwarded-host",
                "x-forwarded-port",
                "x-forwarded-proto",
                "x-forwarded-server",
                "x-real-ip",
            ])
        })
        .await
}

fn rewrite_location(value: &str, uri: &Url, my_host: &str) -> String {
    if value.starts_with('/') {
        return format!("/{}{}", uri.host().unwrap(), value);
    }

    if value.starts_with("https://") {
        if let Ok(url) = Url::parse(value) {
            if url
                .host_str()
                .is_some_and(|h| h.contains("cloudflarestorage"))
            {
                return value.to_string();
            }
        }
        return value.replace("https://", &format!("https://{}/", my_host));
    }

    value.to_string()
}

fn replace_host(content: &mut str, src: &str, dest: &str) -> Result<String> {
    let re = Regex::new(r#"(?P<attr>src|href)(?P<eq>=)(?P<quote>['"]?)(?P<url>(//|https://))"#)
        .map_err(|_e| worker::Error::BadEncoding)?;

    let result = re.replace_all(content, |caps: &regex::Captures| {
        let attr = &caps["attr"];
        let eq = &caps["eq"];
        let quote = &caps["quote"];
        let url = &caps["url"];

        if url.starts_with("https://") || url.starts_with("//") {
            format!("{}{}{}https://{}/", attr, eq, quote, dest)
        } else {
            caps[0].to_string()
        }
    });
    Ok(result
        .into_owned()
        .replace(&format!("//{}", src), &format!("//{}/{}", dest, src)))
}

pub async fn image_handler(
    req: Request,
    query: Option<HashMap<String, String>>,
) -> Result<Response> {
    let req_url = req.url()?;
    let domain = query.map_or(REGISTRY, |q| match q.get("ns").map(|s| s.as_str()) {
        Some("gcr.io") => "gcr.io",
        Some("quay.io") => "quay.io",
        Some("ghcr.io") => "ghcr.io",
        Some("registry.k8s.io") => "registry.k8s.io",
        _ => REGISTRY,
    });

    let full_url = format!("https://{}{}", domain, req_url.path());
    if let Ok(url) = Url::parse(&full_url) {
        handler(req, url, domain, None).await
    } else {
        Response::error("Not Found", 404)
    }
}

pub async fn handler(
    mut req: Request,
    uri: Url,
    dst_host: &str,
    query: Option<HashMap<String, String>>,
) -> Result<Response> {
    let my_host = req.headers().get("host")?.ok_or("Host header not found")?;
    let hops = get_hop_headers().await;
    // build request
    let req_headers = Headers::new();
    for (key, value) in req.headers().entries() {
        if hops.contains(key.as_str()) {
            continue;
        }
        req_headers.set(&key, &value)?;
    }
    req_headers.set("host", dst_host)?;
    req_headers.set("referer", "")?;

    let body = req.bytes().await?;
    let body = (!body.is_empty()).then(|| worker::wasm_bindgen::JsValue::from(body));

    let req_init = RequestInit {
        method: req.method(),
        headers: req_headers,
        body,
        cf: CfProperties::default(),
        redirect: RequestRedirect::Manual,
        cache: None, // CacheMode::Default,
    };
    // send request
    let new_req = Request::new_with_init(uri.as_ref(), &req_init)?;
    let mut response = Fetch::Request(new_req).send().await?;

    // update response
    let resp_header = Headers::new();
    let status = response.status_code();

    for (key, value) in response.headers().entries() {
        let new_value = match (status, key.as_str()) {
            (301..=308, "location") => rewrite_location(&value, &uri, &my_host),
            (401, "www-authenticate") => {
                value.replace("https://", &format!("https://{}/", my_host))
            }
            _ => value,
        };
        resp_header.set(&key, &new_value)?;
    }
    resp_header.delete("content-security-policy")?;
    resp_header.set("access-control-allow-origin", "*")?;

    if resp_header
        .get("content-type")?
        .is_some_and(|ct| ct.contains("text/html"))
    {
        resp_header.delete("content-encoding")?;
        resp_header.set(
            "set-cookie",
            format!("{}={}; Path=/; Max-Age=3600", COOKIE_HOST_KEY, dst_host).as_str(),
        )?;

        let mut body = response.text().await?;
        let should_replace = query.as_ref().and_then(|q| q.get("tul_rh")) != Some(&"n".to_string());

        if should_replace {
            body = replace_host(&mut body, dst_host, &my_host)?;
        }

        return Ok(Response::builder()
            .with_headers(resp_header)
            .with_status(status)
            .body(ResponseBody::Body(body.into_bytes())));
    }

    let resp = match response.stream() {
        Ok(stream) => Response::builder()
            .with_status(status)
            .with_headers(resp_header)
            .from_stream(stream)?,
        Err(_) => Response::builder()
            .with_status(status)
            .with_headers(resp_header)
            .empty(),
    };

    Ok(resp)
}
