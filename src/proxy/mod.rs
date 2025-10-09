

pub mod tj;
pub mod websocket;
pub mod api;

use regex::Regex;
use http::Uri;
use base64::{engine::general_purpose, Engine as _};
use worker::*;
use sha2::{Sha224, Digest};
use tokio::{sync::OnceCell};

static EXPECTED_HASH: OnceCell<Vec<u8>> = OnceCell::const_new();
static BUFSIZE: OnceCell<usize> = OnceCell::const_new();
static APIREGEX: OnceCell<Regex> = OnceCell::const_new();
static PREFIXTJ: OnceCell<String> = OnceCell::const_new();

async fn get_prefix_trojan(cx: &RouteContext<()>) -> String {
    let pre = cx.env
        .var("PREFIX")
        .map_or("/tj".to_string(), |x| x.to_string());
    if ! pre.starts_with("/") {
        return format!("/{}", pre);
    }
    pre
}

async fn get_regex() -> Regex {
    regex::Regex::new(r"^/(?P<domain>[^/]+)(?P<path>/[^?]*)?(?P<query>\?.*)?$").unwrap()
}

async fn get_expected_hash(cx: &RouteContext<()>) -> Vec<u8> {
    let pw = cx.env
        .var("PASSWORD")
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

pub async fn handler(req: Request, cx: RouteContext<()>) -> Result<Response> {
    console_debug!("Request url: {:?}", req.url().unwrap());

    let pre = PREFIXTJ.get_or_init(|| async {
        get_prefix_trojan(&cx).await
    }).await;
    match req.path().as_str() {
        path if path.starts_with(pre.as_str()) => tj(req, cx).await,
        path if path.starts_with("/v2") => api::image_handler(req).await,
        _ => {
            let reg = APIREGEX.get_or_init(|| async {
                get_regex().await
            }).await;

            if let Some(captures) = reg.captures(req.path().as_str()) {
                let domain = captures.name("domain").map_or("", |x| x.as_str());
                let path = captures.name("path").map_or("", |x| x.as_str());
                let query = captures.name("query").map_or("", |x| x.as_str());

                if !domain.contains('.') {
                    return Response::error("Not Found", 404);
                }
                if let Ok(url) = format!("https://{}{}{}", domain, path, query).parse::<Uri>() {                   
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

    let mut response = Response::from_websocket(client)?;
    let mut early_data = None;

    let ws_proto = req.headers().get("sec-websocket-protocol").unwrap_or(None);
    if let Some(proto) = ws_proto {
        response.headers_mut().set("sec-websocket-protocol", proto.as_str())?;
        
        let decoded_bytes = general_purpose::STANDARD_NO_PAD.decode(proto.as_bytes())
            .map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, format!("Invalid base64 encoding: {e}"))
            })?;
        
        early_data = Some(decoded_bytes);
    }

    server.accept()?;

    let early_data_for_async = early_data;
    
    wasm_bindgen_futures::spawn_local(async move {
        let events = server.events().expect("Failed to get server events");
        let mut wsstream = websocket::WsStream::new(
            &server,
            events,
            buf_size,
            early_data_for_async,
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
                        Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "connect failed:"),
                        )
                    }
                }                       
            },
            Err(e) => {
                console_error!("parse request failed: {}", e);
                Err(e)
            }
        };
        if let Err(_e) = result{
             server.close(Some(1000u16), Some("Normal closure")).ok();
        }
       
    });
    Ok(response)
}
