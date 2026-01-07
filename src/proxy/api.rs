
use super::*;
use std::collections::HashMap;
use regex::Regex;


static REGISTRY: &str = "registry-1.docker.io";


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
    Ok(result.into_owned()
        .replace(&format!("//{}", src), &format!("//{}/{}", dest, src)))
}

pub async fn image_handler(req: Request, query: Option<HashMap<String, String>>) -> Result<Response> {
    let req_url = req.url()?;
    let domain = query.map_or(REGISTRY, |q|{
        match q.get("ns").map(|s| s.as_str()) {
            Some("gcr.io") => "gcr.io",
            Some("quay.io") => "quay.io",
            Some("ghcr.io") => "ghcr.io",
            Some("registry.k8s.io") => "registry.k8s.io",
            _ => REGISTRY,
        }
    });

    let full_url = format!("https://{}{}", domain, req_url.path());
    if let Ok(url) = Url::parse(&full_url) {
        handler(req,  url, domain).await
    } else {
        Response::error( "Not Found",404)
    }
}

pub async fn handler(mut req: Request, uri: Url, dst_host: &str) -> Result<Response> {
    let hops = HOP_HEADERS.get_or_init(|| async {
        get_hop_headers().await
    }).await;
    let my_host = req.headers()
        .get("host")?
        .ok_or("Host header not found")?;

    // build request
    let req_headers = Headers::new();
    for (key, value) in req.headers().entries() {
        if hops.contains(&key) {
            continue;
        }
        req_headers.set(&key, &value)?;
    }
    req_headers.set("host", dst_host)?;
    req_headers.set("referer", "")?;

    let mut req_init = RequestInit {
        method: req.method(),
        headers: req_headers,
        body: None,
        cf: CfProperties::default(),
        redirect: RequestRedirect::Manual,
    };
    // request body
    if let Ok(body) = req.bytes().await {
        if !body.is_empty() {
            req_init.body = Some(wasm_bindgen::JsValue::from(body));
        }
    }
    let new_req = Request::new_with_init(uri.as_ref(), &req_init)?;

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
                        if url.host_str().is_some_and(|host| host.contains("cloudflarestorage")) {
                            value
                        } else {
                            value.replace("https://", &format!("https://{}/", my_host))
                        }
                    } else {
                        value.replace("https://", &format!("https://{}/", my_host))
                    }
                } else {
                    value
                }         
            }
            (401, "www-authenticate") => value.replace("https://", &format!("https://{}/", my_host)),
            (_, "set-cookie") => value.replace(dst_host, &my_host),
            _ => value,
        };
        resp_header.set(&key, &new_value)?;
    }
    let _ = resp_header.delete("content-security-policy");
    let _ = resp_header.set("access-control-allow-origin", "*");
    if let Some(s) = resp_header.get("content-type")? {
        if s.contains("text/html")  {
            let mut body = response.text().await?;
            let newbody = replace_host(&mut body, dst_host, &my_host)?;
            let _ = resp_header.delete("content-encoding");
            let resp = Response::builder()
                .with_headers(resp_header)
                .with_status(status)
                .body(ResponseBody::Body(newbody.into_bytes()));
            return Ok(resp);
        }
    }
    
    let resp = match response.stream() {
        Err(_) => Response::builder()
            .with_status(status)
            .with_headers(resp_header)
            .empty(),
        Ok(stream) => Response::builder()
            .with_status(status)
            .with_headers(resp_header)
            .from_stream(stream)?,
    };

    Ok(resp)
} 

