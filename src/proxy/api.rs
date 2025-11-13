
use worker::*;
use std::collections::HashMap;

static REGISTRY: &str = "registry-1.docker.io";

pub async fn image_handler(req: Request) -> Result<Response> {
    let req_url = req.url()?;
    let domain   = req.query().map_or(REGISTRY, |query: HashMap<String, String>| {
        match query.get("ns").map(|s| s.as_str()) {
            Some("gcr.io") => "gcr.io",
            Some("quay.io") => "quay.io",
            Some("ghcr.io") => "ghcr.io",
            Some("registry.k8s.io") => "registry.k8s.io",
            _ => REGISTRY,
        }
    });
    let full_url = format!("https://{}{}", domain, req_url.path());
    if let Ok(url) = Url::parse(&full_url) {                   
        return handler(req,  url).await;
    }
    return Response::error( "Not Found",404);
}

pub async fn handler(mut req: Request, uri: Url) -> Result<Response> {
    let hops = super::HOP_HEADERS.get_or_init(|| async {
        super::get_hop_headers().await
    }).await;
    let my_host = req.headers()
        .get("host")?
        .ok_or("Host header not found")?;
    let dst_host = uri.host_str().ok_or("Host not found")?;
    // build request
    let req_headers = Headers::new();
    for (key, value) in req.headers().entries() {
        if hops.contains(&key) {
            continue;
        }
        req_headers.set(&key, &value)?;
    }
    req_headers.set("host", dst_host)?;

    let mut req_init = RequestInit {
        method: req.method(),
        headers: req_headers,
        body: None,
        cf: CfProperties::default(),
        redirect: RequestRedirect::Manual,
    };
    // body if exist
    if let Ok(body) = req.bytes().await {
        if !body.is_empty() {
            req_init.body = Some(wasm_bindgen::JsValue::from(body));
        }
    }
    let new_req = Request::new_with_init(&uri.to_string(), &req_init)?;

    // send request
    let mut response = Fetch::Request(new_req).send().await?;

    // update response
    let resp_header = Headers::new();
    let status = response.status_code();

    for (key, value) in response.headers().entries() {
        if hops.contains(&key) {
            continue;
        }
        let new_value = match (status, key.as_str()){
            (301..= 308, "location") => {
                if value.starts_with('/') {
                    format!("/{}{}", uri.host().unwrap(), value)
                } else if value.starts_with("https://") {
                    if let Ok(url) = Url::parse(&value) {
                        if url.host_str().map_or(false, |host| host.contains("cloudflarestorage")) {
                            value
                        } else {
                            value.replace("https://", &format!("http://{}/", my_host))
                        }
                    } else {
                        value.replace("https://", &format!("https://{}/", my_host))
                    }
                } else {
                    value
                }         
            }
            (401, "www-authenticate") => {
                value.replace("https://", &format!("https://{}/", my_host))
            }
            _ => value,
        };
        resp_header.set(&key, &new_value)?;
    }

    let body = response.bytes().await.map_err(|e| {
        Error::RustError(format!("Failed to read response body: {:?}", e))
    })?;

    return Ok(Response::builder()
        .with_status(status)
        .with_headers(resp_header)
        .body(ResponseBody::Body(body.to_vec())));    
} 

