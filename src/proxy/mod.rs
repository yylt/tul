

pub mod tj;
pub mod websocket;
pub mod api;
pub mod dns;

use std::collections::HashMap;
use std::net::{Ipv4Addr, Ipv6Addr};
use worker::*;
use sha2::{Sha224, Digest};
use tokio::{sync::OnceCell};
use std::io::{
    Error,
    ErrorKind,
};

static EXPECTED_HASH: OnceCell<Vec<u8>> = OnceCell::const_new();
static BUFSIZE: OnceCell<usize> = OnceCell::const_new();
static PREFIXTJ: OnceCell<String> = OnceCell::const_new();
static DOH_HOST: OnceCell<String> = OnceCell::const_new();
static COOKIE_HOST_KEY: &str = "tul_host";

#[derive(Debug, Clone)]
pub enum Address {
    Ipv4(Ipv4Addr),
    Ipv6(Ipv6Addr),
    Domain(String),
}

impl Into<String> for Address {
    fn into(self) -> String {
        match self {
            Address::Ipv4(ip) => ip.to_string(),
            Address::Ipv6(ip) => ip.to_string(),
            Address::Domain(domain) => domain,
        }
    }
}

async fn get_prefix_trojan(cx: &RouteContext<()>) -> String {
    let pre = cx.env
        .secret("PREFIX")
        .map_or("/tj".to_string(), |x| x.to_string());
    if ! pre.starts_with("/") {
        return format!("/{}", pre);
    }
    pre
}

async fn get_expected_hash(cx: &RouteContext<()>) -> Vec<u8> {
    let pw = cx.env
        .secret("PASSWORD")
        .map_or("password".to_string(), |x| x.to_string());
    Sha224::digest(pw.as_bytes())
        .iter()
        .map(|x| format!("{:02x}", x))
        .collect::<String>()
        .as_bytes()
        .to_vec()
}

async fn get_bufsize(cx: &RouteContext<()>) -> usize {
    cx.env.var("BUFSIZE")
    .map_or(2048, |x| x.to_string().parse::<usize>().unwrap_or(2048))
}

pub async fn get_doh_host(cx: &RouteContext<()>) -> String {
    cx.env
        .var("DOH_HOST")
        .map_or("1.1.1.1".to_string(), |x| x.to_string())
}

fn parse_path(url: &str) -> (Option<&str>, Option<&str>, Option<&str>) {
    if !url.starts_with('/') || url.len() == 1 {
        return (None, None, None);
    }
    
    let rest = &url[1..];
    
    let domain_end = rest.find(|c| c == ':' || c == '/').unwrap_or(rest.len());
    let domain = &rest[..domain_end];
    
    if domain.is_empty() {
        return (None, None, None);
    }
    
    let remaining = &rest[domain_end..];
    
    if remaining.is_empty() {
        return (Some(domain), None, None);
    }
    
    if remaining.starts_with(':') {
        if let Some(path_start) = remaining[1..].find('/') {
            let port_end = 1 + path_start;  // 明确类型
            let port = &remaining[1..port_end];
            let path = &remaining[port_end..];
            (Some(domain), Some(port), Some(path))
        } else {
            (Some(domain), Some(&remaining[1..]), None)
        }
    } else {
        (Some(domain), None, Some(remaining))
    }
}

fn parse_cookies(cookie_str: &str) -> HashMap<String, String> {
    cookie_str
        .split(';')
        .filter_map(|cookie| {
            let parts: Vec<&str> = cookie.trim().splitn(2, '=').collect();
            if parts.len() == 2 {
                Some((parts[0].to_string(), parts[1].to_string()))
            } else {
                None
            }
        })
        .collect()
}

pub async fn handler(req: Request, cx: RouteContext<()>) -> Result<Response> {
    let tj_prefix = PREFIXTJ.get_or_init(|| async {
        get_prefix_trojan(&cx).await
    }).await;
    let dns_host = DOH_HOST.get_or_init(|| async {
        get_doh_host(&cx).await
    }).await;
    let query = req
        .query()
        .map_or(None, |q: HashMap<String, String>| Some(q));
    let origin_path = req.path();
    match origin_path.as_str() {
        "/dns-query" => api::resolve_handler(req, dns_host, query).await,
        path if path.starts_with(tj_prefix.as_str()) => tj(req, cx).await,
        path if path.starts_with("/v2") => api::image_handler(req, query).await,
        _ => {
            let cookie_host = req.headers().get("cookie")?
                .map_or(None,|cookie| {
                match parse_cookies(&cookie).get(COOKIE_HOST_KEY) {
                    Some(host) => {
                        Some(host.to_string())
                    }
                    _ => None, 
                }}
            );

            let (mut domain, port, mut path) = parse_path(&origin_path);
            // not resolve will use cookie to replace domain.
            let mut notresolve= true;
            // only domain will set cookie, which is mirror.
            let mut onlydomain = false;

            match domain {
                Some(d) if d.contains('.') => {
                    match dns::is_cf_address(&Address::Domain(d.to_string()), dns_host).await {
                        Ok(_) => {
                            notresolve = false;
                            if path.is_none() || path.as_ref().unwrap().len()<2 {
                                onlydomain = true;
                            }
                        },
                        _ => {},
                    }
                },
                _ => {},
            }

            match (notresolve, &cookie_host) {
                (true, Some(host)) => {
                    domain = Some(host.as_ref());  
                    path = Some(origin_path.as_str());
                }
                (true, None) => {
                    return Response::error("Not Found", 404);
                }
                (false, _) => {},
            }

            let domain = domain.unwrap();
            console_debug!("finally domain: {:?}, port: {:?}, path: {:?}, query: {:?}", domain, port, path, query);
            let mut url = match (port, path) {
                (Some(p), Some(path)) => format!("https://{}:{}{}", domain, p, path),
                (Some(p), None) => format!("https://{}:{}", domain, p),
                (None, Some(path)) => format!("https://{}{}", domain, path),
                (None, None) => format!("https://{}", domain),
            };
            if let Some(v) = query {
                url.push('?');
                url.push_str(v.iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect::<Vec<_>>()
                    .join("&")
                    .as_str());
            }
            let mut resp = api::handler(req,  Url::parse(&url)?).await?;
            match resp.headers().get("content-type")? {
                Some(s) if s.contains("text/html") => {
                    if onlydomain {
                        console_debug!("set cookie domain: {:?}", domain);
                        let _ = resp.headers_mut().set("set-cookie", format!("{}={}; Path=/; Max-Age=3600", COOKIE_HOST_KEY, domain).as_str());
                    }
                }
                _ => {}
            }
            Ok(resp)    
        }
    }   
}

pub async fn tj(_req: Request, cx: RouteContext<()>) -> Result<Response> {
    
    let expected_hash = EXPECTED_HASH.get_or_init(|| async {
        get_expected_hash(&cx).await
    }).await;
    let buf_size = *BUFSIZE.get_or_init(|| async {
        get_bufsize(&cx).await
    }).await;
    let dns_host = DOH_HOST.get_or_init(|| async {
        get_doh_host(&cx).await
    }).await;
    
    let WebSocketPair { server, client } = WebSocketPair::new()?;
    let response = Response::from_websocket(client)?;
    // cloudflare not support early data!
    server.accept()?;
    wasm_bindgen_futures::spawn_local(async move {
        let events = server.events().expect("Failed to get event stream");
        let mut wsstream = websocket::WsStream::new(
            &server,
            events,
            buf_size,
            None,
            );

        let result = match tj::parse(expected_hash,&mut wsstream).await {
            Ok((hostname, port)) => {
                let addr = match dns::is_cf_address(&hostname, dns_host).await {
                    Ok((true,_)) => {
                        console_debug!("DNS query success, behind cloudflare for {:?}", &hostname);
                        //server.close(Some(1000u16), Some("use DoH then connect directly")).ok();
                        None
                    }
                    Ok((false, ip)) => Some(ip),
                    Err(e) => {
                        console_error!("DNS query failed for {:?}: {}", &hostname, e);
                        None
                    }
                };
                let hostname = match addr {
                    Some(ip) => ip.to_string(),
                    None => {
                        let _ = server.close(Some(1000u16), Some("Normal closure"));
                        return;
                    },
                };
                match Socket::builder().connect(hostname, port) {
                    Ok(mut upstream) => {
                        match tokio::io::copy_bidirectional(wsstream.as_mut(),&mut upstream).await {
                            Ok(_) => Ok(()),
                            Err(e) => {
                                console_error!("forward failed: {}", e);
                                Err(e)
                            }
                        }
                    }
                    Err(e) => {
                        console_error!("connect failed: {}", e);
                        Err(Error::new(ErrorKind::Other, e))
                    }
                }                       
            },
            Err(e) => {
                console_error!("parse request failed: {}", e);
                Err(e)
            }
        };
        if let Err(_e) = result {
             server.close(Some(1011u16), Some("Internal error or connection failure")).ok();
        } else {
             server.close(Some(1000u16), Some("Normal closure")).ok();
        }
    });
    Ok(response)
}


#[test]
fn test_parse_path() {
    let test_cases = [
        "/a:100/b/c",
        "/example.com",
        "/example.com:8080", 
        "/example.com/path",
        "/example.com:8080/path/to/resource",
        "/a/b/c",
        "/",
        "invalid",
        "/github.githubassets.com/assets/wp-runt",  // 边界情况
    ];
    
    eprintln!("Testing fixed version:");
    for case in test_cases {
        let (domain, port, path) = parse_path(case);
        eprintln!("{:20} -> domain: {:10?} port: {:6?} path: {:?}", 
                 case, domain, port, path);
    }
}