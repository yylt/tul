pub mod api;
pub mod dns;
pub mod ip;
pub mod mcp;
pub mod tj;
pub mod websocket;

use sha2::{Digest, Sha224};
use std::collections::HashMap;
use std::net::Ipv4Addr;
use tokio::sync::OnceCell;
use worker::*;

// Cloudfalre ECH domain
static ECH_DOMAIN: OnceCell<String> = OnceCell::const_new();

// suport DoH domain, like 1.1.1.1, doh.pub, dns.google
static DOH_HOST: OnceCell<String> = OnceCell::const_new();

// cookie destination address key.
static COOKIE_HOST_KEY: &str = "tul_host";

// trojan password hash
static TJ_PASSWORD: OnceCell<Vec<u8>> = OnceCell::const_new();

// trojan request path
static TJ_PATH: OnceCell<String> = OnceCell::const_new();

#[derive(Debug, Clone)]
pub enum Address<T: AsRef<str>> {
    Ipv4(Ipv4Addr),
    Domain(T),
}

async fn get_or_init_env<'a>(
    cell: &'a OnceCell<String>,
    cx: &RouteContext<()>,
    key: &str,
    default: &str,
) -> &'a String {
    cell.get_or_init(|| async {
        cx.env
            .var(key)
            .map(|secret| secret.to_string()) // Secret → String
            .unwrap_or_else(|_| default.to_string())
    })
    .await
}

async fn get_trojan_path(cx: &RouteContext<()>) -> &'static String {
    TJ_PATH
        .get_or_init(|| async {
            let pre = cx
                .env
                .secret("PREFIX")
                .map_or("/tj".to_string(), |x| x.to_string());
            if !pre.starts_with("/") {
                return format!("/{}", pre);
            }
            pre
        })
        .await
}

async fn get_trojan_password(cx: &RouteContext<()>) -> &'static Vec<u8> {
    TJ_PASSWORD
        .get_or_init(|| async {
            let pw = cx
                .env
                .secret("PASSWORD")
                .map_or("password".to_string(), |x| x.to_string());
            Sha224::digest(pw.as_bytes())
                .iter()
                .map(|x| format!("{:02x}", x))
                .collect::<String>()
                .as_bytes()
                .to_vec()
        })
        .await
}

// parse path：[{scheme}://]{domain}:{port}{path}
fn parse_path(url: &str) -> (&str, Option<&str>, Option<&str>, Option<&str>) {
    if !url.starts_with('/') || url.len() == 1 {
        return ("https", None, None, None);
    }

    let mut scheme = "https";
    let mut rest = &url[1..];
    if let Some(idx) = rest.find("://") {
        // ensure scheme part is non-empty and alphabetic to avoid false positives
        let scheme_candidate = &rest[..idx];
        if !scheme_candidate.is_empty() && scheme_candidate.chars().all(|c| c.is_ascii_alphabetic())
        {
            let trimmed = &rest[idx + 3..];
            if trimmed.is_empty() {
                return (scheme_candidate, None, None, None);
            }
            rest = trimmed;
            scheme = scheme_candidate;
        }
    }

    let domain_end = rest.find([':', '/']).unwrap_or(rest.len());
    let domain = &rest[..domain_end];

    if domain.is_empty() {
        return (scheme, None, None, None);
    }

    let remaining = &rest[domain_end..];

    if remaining.is_empty() {
        return (scheme, Some(domain), None, None);
    }

    if let Some(stripped) = remaining.strip_prefix(':') {
        if let Some(path_start) = stripped.find('/') {
            let port = &stripped[..path_start];
            let path = &stripped[path_start..];
            (scheme, Some(domain), Some(port), Some(path))
        } else {
            (scheme, Some(domain), Some(stripped), None)
        }
    } else {
        (scheme, Some(domain), None, Some(remaining))
    }
}

fn get_cookie_by_name(cookie_str: &str, key: &str) -> Option<String> {
    cookie_str
        .split(';')
        .filter_map(|cookie| {
            let (cookie_key, cookie_value) = cookie.trim().split_once('=')?;
            Some((cookie_key, cookie_value))
        })
        .find(|(k, _)| *k == key)
        .map(|(_, v)| v.to_string())
}

fn build_search_url(query: &Option<HashMap<String, String>>) -> Result<(Url, &'static str)> {
    let query = query
        .as_ref()
        .ok_or(Error::RustError("missing query parameter: q".into()))?;
    let q = query.get("q").map(|s| s.trim());
    let backend = query.get("s").map(|s| s.as_str()).unwrap_or("ddg");
    let (host, mut url) = match backend {
        "sp" => (
            "www.startpage.com",
            Url::parse("https://www.startpage.com/sp/search")?,
        ),
        _ => ("duckduckgo.com", Url::parse("https://duckduckgo.com/")?),
    };

    url.query_pairs_mut().append_pair("q", q.unwrap_or(""));
    Ok((url, host))
}

pub async fn handler(req: Request, cx: RouteContext<()>) -> Result<Response> {
    let dns_host = get_or_init_env(&DOH_HOST, &cx, "DOH_HOST", "dns.google").await;
    let ech_domain = get_or_init_env(&ECH_DOMAIN, &cx, "ECH_DOMAIN", "linux.do").await;

    let query = req
        .query()
        .map_or(None, |q: HashMap<String, String>| Some(q));
    let origin_path = req.path();

    match origin_path.as_str() {
        "/dns-query" => {
            let mut resp = dns::resolve_handler(req, dns_host, query).await?;
            let bytes = dns::process_response(&resp.bytes().await?, dns_host, ech_domain).await?;
            let new_resp = Response::builder()
                .with_headers(resp.headers().clone())
                .with_status(200)
                .body(ResponseBody::Body(bytes));
            Ok(new_resp)
        }
        "/tulmcp" => mcp::handler(req, cx).await,
        path if path.starts_with(get_trojan_path(&cx).await) => tj(req, cx).await,
        path if path.starts_with("/v2") => api::image_handler(req, query).await,
        "/tuls" => {
            let (url, host) = build_search_url(&query)?;
            api::handler(req, url, host).await
        }
        "/tul_ip" => ip::handler_text(&req).await,
        "/" => ip::handler_html(&req).await,
        _ => {
            let req_url = req.url()?;
            let cookie_host = req
                .headers()
                .get("cookie")?
                .and_then(|cookie| get_cookie_by_name(&cookie, COOKIE_HOST_KEY));

            let (scheme, mut domain, port, mut path) = parse_path(&origin_path);

            // when not resolve, will try find domain by cookie.
            let resolve = match domain {
                Some(d) => {
                    d.contains('.')
                        && dns::is_cf_address(dns_host, &Address::Domain(d))
                            .await
                            .is_ok()
                }
                _ => false,
            };

            match (resolve, &cookie_host) {
                (false, Some(host)) => {
                    domain = Some(host.as_ref());
                    path = Some(origin_path.as_str());
                }
                (false, None) => return Response::error("Not Found", 404),
                (true, _) => {}
            }

            let host = domain.unwrap();

            console_debug!(
                "finally scheme: {:?}, host: {:?}, port: {:?}, path: {:?}, query: {:?}",
                scheme,
                host,
                port,
                path,
                req_url.query(),
            );

            let mut url = match (port, path) {
                (Some(p), Some(path)) => format!("{}://{}:{}{}", scheme, host, p, path),
                (Some(p), None) => format!("{}://{}:{}", scheme, host, p),
                (None, Some(path)) => format!("{}://{}{}", scheme, host, path),
                (None, None) => format!("{}://{}", scheme, host),
            };
            if let Some(raw_query) = req_url.query() {
                url.push('?');
                url.push_str(raw_query);
            }
            api::handler(req, Url::parse(&url)?, host).await
        }
    }
}

pub async fn tj(_req: Request, cx: RouteContext<()>) -> Result<Response> {
    let dns_host = get_or_init_env(&DOH_HOST, &cx, "DOH_HOST", "dns.google").await;

    let WebSocketPair { server, client } = WebSocketPair::new()?;
    let response = Response::from_websocket(client)?;
    // cloudflare not support early data!
    server.accept()?;

    worker::wasm_bindgen_futures::spawn_local(async move {
        let events = server.events().expect("Failed to get event stream");
        let mut wsstream = websocket::WsStream::new(&server, events, None);

        let result = match tj::parse(get_trojan_password(&cx).await, &mut wsstream).await {
            Ok((hostname, port)) => {
                let addr = match dns::is_cf_address(dns_host, &hostname).await {
                    Ok((true, _)) => {
                        console_debug!("DNS query success, behind cloudflare for {:?}", &hostname);
                        None
                    }
                    Ok((false, ip)) => Some(ip),
                    Err(e) => {
                        console_error!("DNS query failed for {:?}: {}", &hostname, e);
                        None
                    }
                };
                let host = match addr {
                    Some(ip) => ip.to_string(),
                    None => {
                        let _ = server.close(Some(1000u16), Some("Normal closure"));
                        return;
                    }
                };
                match Socket::builder().connect(host, port) {
                    Ok(mut upstream) => {
                        match tokio::io::copy_bidirectional(wsstream.as_mut(), &mut upstream).await
                        {
                            Ok(_) => Ok(()),
                            Err(e) => {
                                console_error!("forward failed: {}", e);
                                Err(Error::Io(e))
                            }
                        }
                    }
                    Err(e) => {
                        console_error!("connect failed: {}", e);
                        Err(e)
                    }
                }
            }
            Err(e) => {
                console_error!("parse request failed: {}", e);
                Err(Error::Io(e))
            }
        };
        if let Err(_e) = result {
            server
                .close(Some(1011u16), Some("Internal error or connection failure"))
                .ok();
        } else {
            server.close(Some(1000u16), Some("Normal closure")).ok();
        }
    });
    Ok(response)
}

#[test]
fn test_parse_path() {
    let test_cases = [
        ("/a:100/b/c", "https", Some("a"), Some("100"), Some("/b/c")),
        ("/example.com", "https", Some("example.com"), None, None),
        (
            "/example.com:8080",
            "https",
            Some("example.com"),
            Some("8080"),
            None,
        ),
        (
            "/example.com/path",
            "https",
            Some("example.com"),
            None,
            Some("/path"),
        ),
        (
            "/example.com:8080/path/to/resource",
            "https",
            Some("example.com"),
            Some("8080"),
            Some("/path/to/resource"),
        ),
        (
            "/https://example.com:8443/api",
            "https",
            Some("example.com"),
            Some("8443"),
            Some("/api"),
        ),
        (
            "/http://example.com/resource",
            "http",
            Some("example.com"),
            None,
            Some("/resource"),
        ),
        (
            "/https://example.com",
            "https",
            Some("example.com"),
            None,
            None,
        ),
        ("/a/b/c", "https", Some("a"), None, Some("/b/c")),
        ("/", "https", None, None, None),
        ("invalid", "https", None, None, None),
        (
            "/github.githubassets.com/assets/wp-runt",
            "https",
            Some("github.githubassets.com"),
            None,
            Some("/assets/wp-runt"),
        ),
    ];

    for (input, expected_scheme, expected_domain, expected_port, expected_path) in test_cases {
        let (scheme, domain, port, path) = parse_path(input);
        assert_eq!(
            (scheme, domain, port, path),
            (
                expected_scheme,
                expected_domain,
                expected_port,
                expected_path
            ),
            "parse_path failed for input {}",
            input
        );
    }
}
