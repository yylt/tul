

pub mod tj;
pub mod websocket;
pub mod api;
pub mod dns;

use regex::Regex;
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
static APIREGEX: OnceCell<Regex> = OnceCell::const_new();
static PREFIXTJ: OnceCell<String> = OnceCell::const_new();
static DOH_HOST: OnceCell<String> = OnceCell::const_new();

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

async fn get_regex() -> Regex {
    regex::Regex::new(r"^/(?P<domain>[^:/]+)(?::(?P<port>\d+))?(?P<path>/[^?]*)?$").unwrap()
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

    match req.path().as_str() {
        "/dns-query" => api::resolve_handler(req, dns_host, query).await,
        path if path.starts_with(tj_prefix.as_str()) => tj(req, cx).await,
        path if path.starts_with("/v2") => api::image_handler(req, query).await,
        _ => {
            let reg = APIREGEX.get_or_init(|| async {
                get_regex().await
            }).await;
            
            if let Some(captures) = reg.captures(req.path().as_str()) {
                let domain = captures.name("domain").map_or("", |x| x.as_str());
                let port = captures.name("port").map_or("", |x| x.as_str());
                let path = captures.name("path").map_or("", |x| x.as_str());

                if !domain.contains('.') {
                    return Response::error("Not Found", 404);
                }
                console_debug!("domain: {}, path: {}", domain, path);
                // TODO ip?
                match dns::is_cf_address(&Address::Domain(domain.to_string()), dns_host).await {
                    Ok((_,_)) => (),
                    Err(_e) => return Response::error( "Not Found",404),
                };
                let mut fulldomain = domain.to_string();
                if port != "" {
                    fulldomain = format!("{}:{}", domain, port);
                }
                let mut url = format!("https://{}{}", fulldomain, path);
                if let Some(v) = query {
                    url.push('?');
                    url.push_str(v.iter()
                        .map(|(k, v)| format!("{}={}", k, v))
                        .collect::<Vec<_>>()
                        .join("&")
                        .as_str());
                }
                
                if let Ok(url) = Url::parse(&url) {                   
                    return api::handler(req,  url).await;
                }
            }
            return Response::error( "Not Found",404);
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
