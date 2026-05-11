use super::*;
use js_sys::Uint8Array;
use std::net::Ipv4Addr;
use tokio::sync::OnceCell;
use wasm_bindgen::JsValue;

use hickory_proto::op::Message;
use hickory_proto::rr::rdata::svcb::SvcParamValue;
use hickory_proto::rr::{RData, RecordType};

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
    Fetch::Request(req).send().await?.bytes().await
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

pub async fn resolve_handler<T: AsRef<str>>(
    mut req: Request,
    host: T,
    query: Option<HashMap<String, String>>,
) -> Result<Response> {
    let body = if req.method() == Method::Post {
        Some(req.bytes().await?.into())
    } else {
        None
    };

    let req_init = RequestInit {
        method: req.method().clone(),
        headers: req.headers().clone(),
        body,
        cf: CfProperties::default(),
        redirect: RequestRedirect::Follow,
        cache: None,
    };

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
    console_debug!("Forwarding DNS request to {:?}", &uri);
    let new_req = Request::new_with_init(&uri, &req_init)?;
    Fetch::Request(new_req).send().await
}

// 处理 DNS 响应，根据 HTTPS 记录内容决定是否替换响应
pub async fn process_response(
    response_bytes: &[u8],
    resolver: &str,
    ech_domain: &str,
) -> Result<Vec<u8>> {
    // 1. 解析 DNS 消息
    let mut message = match Message::from_vec(response_bytes) {
        Ok(msg) => msg,
        Err(e) => {
            console_debug!(
                "[process_response] Return original: Failed to parse DNS message: {}",
                e
            );
            return Ok(response_bytes.to_vec());
        }
    };

    // 2. 查找 HTTPS 类型的记录
    let record = message
        .answers
        .iter_mut()
        .find(|r| r.record_type() == RecordType::HTTPS);

    let mut ipv4_hint = None;

    // 3. 如果没有 HTTPS 记录，直接返回原始响应
    let record = match record {
        None => {
            console_debug!("[process_response] Return original: No HTTPS record found in answers");
            return Ok(response_bytes.to_vec());
        }
        Some(rc) => rc,
    };

    // 4. 从 HTTPS 记录中提取 IPv4 提示（Ipv4Hint）和 ECH 配置
    if let RData::HTTPS(ref hs) = record.data {
        for (_key, value) in hs.0.svc_params.iter() {
            match value {
                SvcParamValue::EchConfigList(_) => {
                    console_debug!("[process_response] Return original: ECH already configured");
                    return Ok(response_bytes.to_vec());
                }
                SvcParamValue::Ipv4Hint(v4hint) => ipv4_hint = v4hint.0.first().copied(),
                _ => {}
            }
        }
    } else {
        console_debug!("[process_response] Return original: HTTPS record data is not SVCB type");
        return Ok(response_bytes.to_vec());
    }

    // 5. 检查 IP 地址是否属于 Cloudflare
    let addr = match ipv4_hint {
        None => {
            console_debug!("[process_response] Return original: No Ipv4Hint found in HTTPS record");
            return Ok(response_bytes.to_vec());
        }
        Some(addr) => addr,
    };

    let trie = CF_TRIE.get_or_init(|| async { get_cf_trie().await }).await;
    let ipnet = match Ipv4Net::new(addr.0, 32) {
        Ok(net) => net,
        Err(e) => {
            console_debug!(
                "[process_response] Return original: Invalid IPv4 {}: {}",
                addr,
                e
            );
            return Ok(response_bytes.to_vec());
        }
    };

    if trie.get_lpm(&ipnet).is_none() {
        console_debug!(
            "[process_response] Return original: IP {} is not in Cloudflare range",
            addr
        );
        return Ok(response_bytes.to_vec());
    }

    // 6. 查询 ech_domain 的 HTTPS 记录
    let ech_response = match doh_query(ech_domain, RecordType::HTTPS.into(), resolver).await {
        Ok(resp) => resp,
        Err(e) => {
            console_debug!(
                "[process_response] Return original: Failed to query ech_domain {}: {}",
                ech_domain,
                e
            );
            return Ok(response_bytes.to_vec());
        }
    };

    let mut ech_message = match Message::from_vec(&ech_response) {
        Ok(msg) => msg,
        Err(e) => {
            console_debug!(
                "[process_response] Return original: Failed to parse ech_domain DNS message: {}",
                e
            );
            return Ok(response_bytes.to_vec());
        }
    };

    // 7. 替换原响应中的 HTTPS 记录数据
    if let Some(https_record) = ech_message
        .answers
        .iter_mut()
        .find(|r| r.record_type() == RecordType::HTTPS)
    {
        record.data = https_record.data.clone();

        match message.to_vec() {
            Ok(modified_response) => {
                console_debug!("[process_response] Return modified response (original size: {}, modified size: {})",
                    response_bytes.len(), modified_response.len());
                Ok(modified_response)
            }
            Err(e) => {
                console_debug!(
                    "[process_response] Return original: Failed to serialize modified message: {}",
                    e
                );
                Ok(response_bytes.to_vec())
            }
        }
    } else {
        console_debug!("[process_response] Return original: No HTTPS record found in ech_domain response for {}", ech_domain);
        Ok(response_bytes.to_vec())
    }
}
