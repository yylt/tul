

pub mod tj;
pub mod websocket;
pub mod api;

use regex::Regex;
use base64::{engine::general_purpose, Engine as _};
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
    regex::Regex::new(r"^/(?P<domain>[^/]+)(?P<path>/[^?]*)?$").unwrap()
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

async fn get_doh_host(cx: &RouteContext<()>) -> String {
    cx.env
        .var("DOH_HOST")
        .map_or("dns.google".to_string(), |x| x.to_string())
}

async fn get_bufsize(cx: &RouteContext<()>) -> usize {
    cx.env.var("BUFSIZE")
    .map_or(2048, |x| x.to_string().parse::<usize>().unwrap_or(2048))
}

pub async fn handler(req: Request, cx: RouteContext<()>) -> Result<Response> {
    let pre = PREFIXTJ.get_or_init(|| async {
        get_prefix_trojan(&cx).await
    }).await;
    match req.path().as_str() {
        "/dns-query" | "/resolve"=> {
            let host = DOH_HOST.get_or_init(|| async {
                get_doh_host(&cx).await
            }).await;
            
            api::dns_handler(req, host).await
        }
        path if path.starts_with(pre.as_str()) => tj(req, cx).await,
        path if path.starts_with("/v2") => api::image_handler(req).await,
        _ => {
            let reg = APIREGEX.get_or_init(|| async {
                get_regex().await
            }).await;
            
            let query = match req.url() {
                Ok(url) => url.query().unwrap_or("").to_string(),
                Err(_) => "".to_string(),
            };
            if let Some(captures) = reg.captures(req.path().as_str()) {
                let domain = captures.name("domain").map_or("", |x| x.as_str());
                let path = captures.name("path").map_or("", |x| x.as_str());

                if !domain.contains('.') {
                    return Response::error("Not Found", 404);
                }
                let mut full_url = format!("https://{}{}", domain, path);
                if !query.is_empty() {
                    full_url.push('?');
                    full_url.push_str(&query);
                }
                
                if let Ok(url) = Url::parse(&full_url) {                   
                    return api::handler(req,  url).await;
                }
            } 
            return Response::error( "Not Found",404);
        }
    }   
}

pub async fn tj(req: Request, cx: RouteContext<()>) -> Result<Response> {
    
    let expected_hash = EXPECTED_HASH.get_or_init(|| async {
        get_expected_hash(&cx).await
    }).await;
    let buf_size = *BUFSIZE.get_or_init(|| async {
        get_bufsize(&cx).await
    }).await;

    let WebSocketPair { server, client } = WebSocketPair::new()?;
    let response = Response::from_websocket(client)?;
    let early_data = req.headers()
        .get("sec-websocket-protocol")?
        .and_then(|value| {
            general_purpose::STANDARD_NO_PAD.decode(value.as_bytes())
                .ok() 
                .inspect(|decoded| {
                    Some(decoded);
                })
        });

    server.accept()?;
    wasm_bindgen_futures::spawn_local(async move {
        let events = server.events().expect("Failed to get event stream");
        let mut wsstream = websocket::WsStream::new(
            &server,
            events,
            buf_size,
            early_data,
            );

        let result = match tj::parse(expected_hash,&mut wsstream).await {
            Ok((hostname, port)) => {
                match Socket::builder().connect( hostname, port) {
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
