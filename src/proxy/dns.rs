use super::*;
use js_sys::Uint8Array;
use std::net::Ipv4Addr;
use tokio::sync::OnceCell;
use wasm_bindgen::JsValue;

use ipnet::Ipv4Net;
use prefix_trie::set::PrefixSet;

const DNS_HEADER_SIZE: usize = 12;
const QTYPE_A: u16 = 1;
const QCLASS_IN: u16 = 1;

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

// DNS 报文构建
fn build_dns_query(domain: &str, qtype: u16) -> Result<Vec<u8>> {
    if domain.is_empty() {
        return Err(Error::RustError("Empty domain name".into()));
    }
    // 预估容量: header(12) + domain labels + terminator(1) + qtype(2) + qclass(2)
    let mut buf = Vec::with_capacity(12 + domain.len() + 6);
    buf.extend_from_slice(&0u16.to_be_bytes()); // ID
    buf.extend_from_slice(&0x0100u16.to_be_bytes()); // RD flag
    buf.extend_from_slice(&1u16.to_be_bytes()); // QDCOUNT
    buf.extend_from_slice(&0u16.to_be_bytes()); // ANCOUNT
    buf.extend_from_slice(&0u16.to_be_bytes()); // NSCOUNT
    buf.extend_from_slice(&0u16.to_be_bytes()); // ARCOUNT

    for label in domain.split('.') {
        if label.is_empty() || label.len() > 63 {
            return Err(Error::RustError("Invalid DNS label".into()));
        }
        buf.push(label.len() as u8);
        buf.extend_from_slice(label.as_bytes());
    }
    buf.push(0); // name terminator

    buf.extend_from_slice(&qtype.to_be_bytes());
    buf.extend_from_slice(&QCLASS_IN.to_be_bytes());
    Ok(buf)
}

fn skip_name(bytes: &[u8], mut pos: usize) -> Result<usize> {
    while pos < bytes.len() {
        let label_len = bytes[pos];
        if label_len & 0xC0 == 0xC0 {
            if pos + 1 >= bytes.len() {
                return Err(Error::RustError("Truncated pointer".into()));
            }
            pos += 2;
            break;
        } else if label_len == 0 {
            pos += 1;
            break;
        } else {
            pos += 1 + label_len as usize;
        }
    }
    Ok(pos)
}

fn extract_ipv4_from_response(bytes: &[u8]) -> Result<Ipv4Addr> {
    if bytes.len() < DNS_HEADER_SIZE {
        return Err(Error::RustError("Response too short".into()));
    }
    let rcode = bytes[3] & 0x0F;
    if rcode != 0 {
        return Err(Error::RustError(format!("DNS error rcode={}", rcode)));
    }
    let qdcount = u16::from_be_bytes([bytes[4], bytes[5]]) as usize;
    let ancount = u16::from_be_bytes([bytes[6], bytes[7]]) as usize;
    let mut pos = DNS_HEADER_SIZE;

    // 跳过 question section
    for _ in 0..qdcount {
        pos = skip_name(bytes, pos)?;
        if pos + 4 > bytes.len() {
            return Err(Error::RustError("Question truncated".into()));
        }
        pos += 4; // QTYPE + QCLASS
    }

    // 查找 A 记录
    for _ in 0..ancount {
        pos = skip_name(bytes, pos)?;
        if pos + 10 > bytes.len() {
            return Err(Error::RustError("Answer header truncated".into()));
        }
        let rtype = u16::from_be_bytes([bytes[pos], bytes[pos + 1]]);
        let rclass = u16::from_be_bytes([bytes[pos + 2], bytes[pos + 3]]);
        pos += 4; // type + class
        pos += 4; // TTL
        let rdlength = u16::from_be_bytes([bytes[pos], bytes[pos + 1]]) as usize;
        pos += 2;
        if pos + rdlength > bytes.len() {
            return Err(Error::RustError("Answer data truncated".into()));
        }
        if rtype == QTYPE_A && rclass == QCLASS_IN && rdlength == 4 {
            return Ok(Ipv4Addr::new(
                bytes[pos],
                bytes[pos + 1],
                bytes[pos + 2],
                bytes[pos + 3],
            ));
        }
        pos += rdlength;
    }
    Err(Error::RustError("A record not found".into()))
}

pub async fn doh_query(domain: &str, qtype: u16, resolver: &str) -> Result<Vec<u8>> {
    let query = build_dns_query(domain, qtype)?;
    let url = format!("https://{}/dns-query", resolver);
    let headers = Headers::new();
    headers.set("accept", "application/dns-message")?;
    headers.set("content-type", "application/dns-message")?;
    let req_init = RequestInit {
        method: Method::Post,
        headers,
        body: Some(JsValue::from(Uint8Array::from(query.as_slice()))),
        cf: CfProperties::default(),
        redirect: RequestRedirect::Follow,
        cache: None,
    };
    let req = Request::new_with_init(&url, &req_init)?;
    Ok(Fetch::Request(req).send().await?.bytes().await?)
}

pub async fn resolve_a(domain: &str, resolver: &str) -> Result<Ipv4Addr> {
    let resp_bytes = doh_query(domain, QTYPE_A, resolver).await?;
    extract_ipv4_from_response(&resp_bytes)
}

pub async fn is_cf_address<T: AsRef<str>, K: AsRef<str>>(
    resolve: K,
    addr: &Address<T>,
) -> Result<(bool, Ipv4Addr)> {
    let trie = CF_TRIE.get_or_init(|| async { get_cf_trie().await }).await;
    let v4fn = |ip: Ipv4Addr| -> Result<(bool, Ipv4Addr)> {
        let ipnet =
            Ipv4Net::new(ip, 32).map_err(|e| Error::RustError(format!("Invalid IPv4: {}", e)))?;
        Ok((trie.get_lpm(&ipnet).is_some(), ip))
    };

    match addr {
        Address::Ipv4(ip) => v4fn(*ip),
        Address::Domain(domain) => {
            let ip = resolve_a(domain.as_ref(), resolve.as_ref()).await?;
            v4fn(ip)
        }
    }
}

pub async fn resolve_handler<T: AsRef<str>>(mut req: Request, host: T) -> Result<Response> {
    if req.method() != Method::Post {
        return Response::error("Method not allowed", 405);
    }

    let headers = Headers::new();
    headers.set("accept", "application/dns-message")?;
    headers.set("content-type", "application/dns-message")?;

    let body_bytes = req.bytes().await?.into();
    let req_init = RequestInit {
        method: req.method().clone(),
        headers,
        body: Some(body_bytes),
        cf: CfProperties::default(),
        redirect: RequestRedirect::Follow,
        cache: None,
    };

    let uri = format!("https://{}{}", host.as_ref(), req.path());
    let new_req = Request::new_with_init(&uri, &req_init)?;
    console_debug!("Forwarding request: {:?}", new_req);
    Fetch::Request(new_req).send().await
}
