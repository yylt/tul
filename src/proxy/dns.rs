use super::*;
use js_sys::Uint8Array;
use std::{collections::HashMap, net::Ipv4Addr};
use tokio::sync::OnceCell;

use ipnet::Ipv4Net;
use prefix_trie::set::PrefixSet;

static CF_TRIE: OnceCell<PrefixSet<Ipv4Net>> = OnceCell::const_new();
// ref: https://www.cloudflare.com/ips
async fn get_cf_trie() -> PrefixSet<Ipv4Net> {
    // TODO fetch from cloudflare
    let ipv4s = vec![
        "103.22.200.0/22",
        "103.31.4.0/22",
        "104.16.0.0/13",
        "104.24.0.0/14",
        "108.162.192.0/18",
        "131.0.72.0/22",
        "141.101.64.0/18",
        "162.158.0.0/15",
        "172.64.0.0/13",
        "173.245.48.0/20",
        "188.114.96.0/20",
        "190.93.240.0/20",
        "197.234.240.0/22",
        "198.41.128.0/17",
    ];

    let mut pm: PrefixSet<Ipv4Net> = PrefixSet::new();
    for ip in ipv4s {
        pm.insert(ip.parse().unwrap());
    }
    pm
}

pub async fn is_cf_address<T: AsRef<str>, K: AsRef<str>>(
    resolve: K,
    addr: &super::Address<T>,
) -> Result<(bool, Ipv4Addr)> {
    let trie = CF_TRIE.get_or_init(|| async { get_cf_trie().await }).await;
    let v4fn = |ip: &Ipv4Addr| -> Result<(bool, Ipv4Addr)> {
        let ipnet = Ipv4Net::new(*ip, 32).map_err(|e| {
            console_error!("parse ipv4 failed: {}", e);
            worker::Error::RustError(e.to_string())
        })?;
        Ok((trie.get_lpm(&ipnet).is_some(), *ip))
    };
    // TODO: only 1.1.1.1 support RFC 8484 and JSON API
    //let resolve = "1.1.1.1";
    match addr {
        super::Address::Ipv4(ipv4) => v4fn(ipv4),
        super::Address::Domain(domain) => {
            let header = Headers::new();
            header.set("accept", "application/dns-message")?;
            header.set("content-type", "application/dns-message")?;
            header.set("user-agent", "tul/0.1")?;

            let body = build_dns_query(domain.as_ref())?;
            let req_init = RequestInit {
                method: Method::Post,
                headers: header,
                body: Some(Uint8Array::from(body.as_slice()).into()),
                cf: CfProperties::default(),
                redirect: RequestRedirect::Follow,
                cache: None, // CacheMode::Default,
            };
            let req = Request::new_with_init("https://lo/dns-query", &req_init)?;

            let mut resp = resolve_handler(req, resolve, None).await?;
            let bytes = resp.bytes().await?;
            let ipv4 = extract_ipv4_from_dns_response(&bytes)?;
            v4fn(&ipv4)
        }
    }
}

fn build_dns_query(domain: &str) -> Result<Vec<u8>> {
    if domain.is_empty() {
        return Err(worker::Error::RustError(
            "dns query domain empty".to_string(),
        ));
    }
    let mut buffer = Vec::with_capacity(12 + domain.len() + 6);
    buffer.extend_from_slice(&0u16.to_be_bytes()); // ID
    buffer.extend_from_slice(&0x0100u16.to_be_bytes()); // RD flag
    buffer.extend_from_slice(&1u16.to_be_bytes()); // QDCOUNT
    buffer.extend_from_slice(&0u16.to_be_bytes()); // ANCOUNT
    buffer.extend_from_slice(&0u16.to_be_bytes()); // NSCOUNT
    buffer.extend_from_slice(&0u16.to_be_bytes()); // ARCOUNT

    for label in domain.split('.') {
        if label.is_empty() || label.len() > 63 {
            return Err(worker::Error::RustError("invalid dns label".to_string()));
        }
        buffer.push(label.len() as u8);
        buffer.extend_from_slice(label.as_bytes());
    }
    buffer.push(0); // terminator
    buffer.extend_from_slice(&1u16.to_be_bytes()); // QTYPE A
    buffer.extend_from_slice(&1u16.to_be_bytes()); // QCLASS IN

    Ok(buffer)
}

fn extract_ipv4_from_dns_response(bytes: &[u8]) -> Result<Ipv4Addr> {
    if bytes.len() < 12 {
        return Err(worker::Error::RustError(
            "dns response too short".to_string(),
        ));
    }
    let rcode = bytes[3] & 0x0f;
    if rcode != 0 {
        return Err(worker::Error::RustError(format!(
            "dns error rcode {}",
            rcode
        )));
    }
    let mut offset = 12;
    let qdcount = u16::from_be_bytes([bytes[4], bytes[5]]) as usize;
    let ancount = u16::from_be_bytes([bytes[6], bytes[7]]) as usize;

    for _ in 0..qdcount {
        offset = skip_name(bytes, offset)?;
        if offset + 4 > bytes.len() {
            return Err(worker::Error::RustError(
                "dns question truncated".to_string(),
            ));
        }
        offset += 4; // type + class
    }

    for _ in 0..ancount {
        offset = skip_name(bytes, offset)?;
        if offset + 10 > bytes.len() {
            return Err(worker::Error::RustError(
                "dns answer header truncated".to_string(),
            ));
        }
        let rtype = u16::from_be_bytes([bytes[offset], bytes[offset + 1]]);
        let rclass = u16::from_be_bytes([bytes[offset + 2], bytes[offset + 3]]);
        offset += 4; // type + class
        offset += 4; // ttl
        let rdlength = u16::from_be_bytes([bytes[offset], bytes[offset + 1]]) as usize;
        offset += 2;
        if offset + rdlength > bytes.len() {
            return Err(worker::Error::RustError("dns answer truncated".to_string()));
        }
        if rtype == 1 && rclass == 1 && rdlength == 4 {
            return Ok(Ipv4Addr::new(
                bytes[offset],
                bytes[offset + 1],
                bytes[offset + 2],
                bytes[offset + 3],
            ));
        }
        offset += rdlength;
    }

    Err(worker::Error::RustError(
        "dns a record not found".to_string(),
    ))
}

fn skip_name(bytes: &[u8], mut offset: usize) -> Result<usize> {
    loop {
        if offset >= bytes.len() {
            return Err(worker::Error::RustError(
                "dns name out of range".to_string(),
            ));
        }
        let len = bytes[offset];
        if len & 0xc0 == 0xc0 {
            if offset + 1 >= bytes.len() {
                return Err(worker::Error::RustError(
                    "dns pointer truncated".to_string(),
                ));
            }
            offset += 2;
            break;
        } else if len == 0 {
            offset += 1;
            break;
        } else {
            let next = offset + 1 + len as usize;
            if next > bytes.len() {
                return Err(worker::Error::RustError("dns label truncated".to_string()));
            }
            offset = next;
        }
    }
    Ok(offset)
}

pub async fn resolve_handler<T: AsRef<str>>(
    mut req: Request,
    host: T,
    query: Option<HashMap<String, String>>,
) -> Result<Response> {
    let hops = HOP_HEADERS
        .get_or_init(|| async { get_hop_headers().await })
        .await;
    let req_headers = Headers::new();
    for (key, value) in req.headers().entries() {
        if hops.contains(&key) {
            continue;
        }
        req_headers.set(&key, &value)?;
    }
    req_headers.set("host", host.as_ref())?;

    let mut req_init = RequestInit {
        method: req.method(),
        headers: req_headers,
        body: None,
        cf: CfProperties::default(),
        redirect: RequestRedirect::Follow,
        cache: None, // CacheMode::Default,
    };
    // body if exist
    if let Ok(body) = req.bytes().await {
        if !body.is_empty() {
            req_init.body = Some(wasm_bindgen::JsValue::from(body));
        }
    }
    let mut uri = format!("https://{}{}", host.as_ref(), req.path());
    if let Some(v) = query {
        uri.push('?');
        uri.push_str(
            v.iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join("&")
                .as_str(),
        );
    }

    let new_req = Request::new_with_init(&uri, &req_init)?;
    console_debug!("DNS Request: {:?}", new_req);
    return Fetch::Request(new_req).send().await;
}
