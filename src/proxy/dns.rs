use super::*;
use js_sys::Uint8Array;
use std::net::Ipv4Addr;
use worker::wasm_bindgen::JsValue;

const DNS_HEADER_SIZE: usize = 12;
const QTYPE_A: u16 = 1;
const QCLASS_IN: u16 = 1;

// DNS 记录类型常量
const TYPE_HTTPS: u16 = 65;
const CLASS_IN: u16 = 1;

// SVCB 参数键
const SVCB_KEY_ECH: u16 = 5;
const SVCB_KEY_IPV4HINT: u16 = 4;

const CF_NETWORKS: [(u32, u32); 14] = [
    (ip_to_u32(103, 22, 200, 0), 22),
    (ip_to_u32(103, 31, 4, 0), 22),
    (ip_to_u32(104, 16, 0, 0), 13),
    (ip_to_u32(104, 24, 0, 0), 14),
    (ip_to_u32(108, 162, 192, 0), 18),
    (ip_to_u32(131, 0, 72, 0), 22),
    (ip_to_u32(141, 101, 64, 0), 18),
    (ip_to_u32(162, 158, 0, 0), 15),
    (ip_to_u32(172, 64, 0, 0), 13),
    (ip_to_u32(173, 245, 48, 0), 20),
    (ip_to_u32(188, 114, 96, 0), 20),
    (ip_to_u32(190, 93, 240, 0), 20),
    (ip_to_u32(197, 234, 240, 0), 22),
    (ip_to_u32(198, 41, 128, 0), 17),
];

const fn ip_to_u32(a: u8, b: u8, c: u8, d: u8) -> u32 {
    (a as u32) << 24 | (b as u32) << 16 | (c as u32) << 8 | d as u32
}

fn is_cloudflare_ip(ip: Ipv4Addr) -> bool {
    let ip_int = u32::from(ip);

    CF_NETWORKS.iter().any(|&(net_start, mask_bits)| {
        let mask = if mask_bits == 0 {
            0
        } else {
            (!0u32) << (32 - mask_bits)
        };
        (ip_int & mask) == net_start
    })
}

// DNS 报文构建
fn build_dns_query(domain: &str, qtype: u16) -> Result<Vec<u8>> {
    if domain.is_empty() {
        return Err(Error::RustError("Empty domain name".into()));
    }
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
    buf.push(0);

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

    for _ in 0..qdcount {
        pos = skip_name(bytes, pos)?;
        if pos + 4 > bytes.len() {
            return Err(Error::RustError("Question truncated".into()));
        }
        pos += 4;
    }

    for _ in 0..ancount {
        pos = skip_name(bytes, pos)?;
        if pos + 10 > bytes.len() {
            return Err(Error::RustError("Answer header truncated".into()));
        }
        let rtype = u16::from_be_bytes([bytes[pos], bytes[pos + 1]]);
        let rclass = u16::from_be_bytes([bytes[pos + 2], bytes[pos + 3]]);
        pos += 4;
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

// HTTPS 记录结构
struct HttpsRecord {
    priority: u16,
    target: String,
    params: Vec<(u16, Vec<u8>)>,
}

// 解析 HTTPS 记录数据
fn parse_https_record(data: &[u8]) -> Result<HttpsRecord> {
    if data.len() < 4 {
        return Err(Error::RustError("HTTPS record too short".into()));
    }

    let priority = u16::from_be_bytes([data[0], data[1]]);
    let mut pos = 2;

    // 解析目标域名
    let (target, new_pos) = parse_dns_name(data, pos)?;
    pos = new_pos;

    let mut params = Vec::new();

    // 解析 SVCB 参数
    while pos < data.len() {
        if pos + 4 > data.len() {
            break;
        }
        let key = u16::from_be_bytes([data[pos], data[pos + 1]]);
        let len = u16::from_be_bytes([data[pos + 2], data[pos + 3]]) as usize;
        pos += 4;

        if pos + len > data.len() {
            break;
        }

        let value = data[pos..pos + len].to_vec();
        params.push((key, value));
        pos += len;
    }

    Ok(HttpsRecord {
        priority,
        target,
        params,
    })
}

// 解析 DNS 名称（支持压缩指针）
fn parse_dns_name(bytes: &[u8], mut pos: usize) -> Result<(String, usize)> {
    let mut labels = Vec::new();
    let mut jumped = false;
    let mut original_pos = pos;

    while pos < bytes.len() {
        let label_len = bytes[pos];

        if label_len & 0xC0 == 0xC0 {
            if pos + 1 >= bytes.len() {
                return Err(Error::RustError("Truncated pointer".into()));
            }
            let pointer = ((label_len & 0x3F) as usize) << 8 | bytes[pos + 1] as usize;
            if !jumped {
                original_pos = pos + 2;
            }
            pos = pointer;
            jumped = true;
            continue;
        } else if label_len == 0 {
            pos += 1;
            break;
        } else {
            pos += 1;
            if pos + label_len as usize > bytes.len() {
                return Err(Error::RustError("Label truncated".into()));
            }
            let label = std::str::from_utf8(&bytes[pos..pos + label_len as usize])
                .map_err(|_| Error::RustError("Invalid UTF-8 in label".into()))?;
            labels.push(label);
            pos += label_len as usize;
        }
    }

    let name = labels.join(".");
    Ok((name, if jumped { original_pos } else { pos }))
}

// 序列化 DNS 名称
fn serialize_dns_name(name: &str) -> Vec<u8> {
    let mut result = Vec::new();
    for label in name.split('.') {
        if !label.is_empty() {
            result.push(label.len() as u8);
            result.extend_from_slice(label.as_bytes());
        }
    }
    result.push(0);
    result
}

// 构建 HTTPS 记录数据
fn build_https_record_data(record: &HttpsRecord) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&record.priority.to_be_bytes());
    data.extend_from_slice(&serialize_dns_name(&record.target));

    for (key, value) in &record.params {
        data.extend_from_slice(&key.to_be_bytes());
        data.extend_from_slice(&(value.len() as u16).to_be_bytes());
        data.extend_from_slice(value);
    }

    data
}

// 解析 DNS 消息并提取 HTTPS 记录
fn extract_https_record(bytes: &[u8]) -> Result<Option<(usize, HttpsRecord)>> {
    if bytes.len() < DNS_HEADER_SIZE {
        return Ok(None);
    }

    let qdcount = u16::from_be_bytes([bytes[4], bytes[5]]) as usize;
    let ancount = u16::from_be_bytes([bytes[6], bytes[7]]) as usize;
    let mut pos = DNS_HEADER_SIZE;

    // 跳过 question section
    for _ in 0..qdcount {
        pos = skip_name(bytes, pos)?;
        pos += 4;
    }

    // 查找 HTTPS 记录
    for _i in 0..ancount {
        let record_start = pos;
        pos = skip_name(bytes, pos)?;
        if pos + 10 > bytes.len() {
            return Ok(None);
        }

        let rtype = u16::from_be_bytes([bytes[pos], bytes[pos + 1]]);
        let rclass = u16::from_be_bytes([bytes[pos + 2], bytes[pos + 3]]);
        pos += 4;
        pos += 4; // TTL
        let rdlength = u16::from_be_bytes([bytes[pos], bytes[pos + 1]]) as usize;
        pos += 2;

        if pos + rdlength > bytes.len() {
            return Ok(None);
        }

        if rtype == TYPE_HTTPS && rclass == CLASS_IN {
            let record_data = &bytes[pos..pos + rdlength];
            if let Ok(record) = parse_https_record(record_data) {
                return Ok(Some((record_start, record)));
            }
        }

        pos += rdlength;
    }

    Ok(None)
}

// 替换 DNS 响应中的 HTTPS 记录
fn replace_https_record(
    original: &[u8],
    new_record_data: &[u8],
    record_offset: usize,
) -> Result<Vec<u8>> {
    if record_offset + 10 > original.len() {
        return Err(Error::RustError("Invalid record offset".into()));
    }

    // 跳过 name 和 type/class/TTL，找到 rdlength 位置
    let mut pos = record_offset;
    pos = skip_name(original, pos)?;

    if pos + 10 > original.len() {
        return Err(Error::RustError("Answer header truncated".into()));
    }

    let rdlength_pos = pos + 8; // type(2) + class(2) + TTL(4)
    let old_rdlength =
        u16::from_be_bytes([original[rdlength_pos], original[rdlength_pos + 1]]) as usize;

    let mut result = original[..rdlength_pos].to_vec();
    result.extend_from_slice(&(new_record_data.len() as u16).to_be_bytes());
    result.extend_from_slice(new_record_data);

    let data_end = rdlength_pos + 2 + old_rdlength;
    if data_end <= original.len() {
        result.extend_from_slice(&original[data_end..]);
    }

    // 更新 ANCOUNT
    if result.len() >= 8 {
        let ancount = u16::from_be_bytes([original[6], original[7]]);
        result[6..8].copy_from_slice(&ancount.to_be_bytes());
    }

    Ok(result)
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

pub async fn is_cf_address(
    resolve: impl AsRef<str>,
    addr: &Address<impl AsRef<str>>,
) -> Result<(bool, Ipv4Addr)> {
    let ip = match addr {
        Address::Ipv4(ip) => *ip,
        Address::Domain(domain) => resolve_a(domain.as_ref(), resolve.as_ref()).await?,
    };

    Ok((is_cloudflare_ip(ip), ip))
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

// 检查 HTTPS 记录中是否包含 ECH 或非 CF IP
fn has_ech_or_non_cf_ip(record: &HttpsRecord) -> bool {
    for (key, value) in &record.params {
        match *key {
            SVCB_KEY_ECH => {
                return true;
            }
            SVCB_KEY_IPV4HINT if value.len() >= 4 => {
                let ip = Ipv4Addr::new(value[0], value[1], value[2], value[3]);
                if !is_cloudflare_ip(ip) {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

// 处理 DNS 响应，根据 HTTPS 记录内容决定是否替换响应
pub async fn process_response(
    response_bytes: &[u8],
    resolver: &str,
    ech_domain: &str,
) -> Result<Vec<u8>> {
    // 1. 提取 HTTPS 记录
    let https_record_info = match extract_https_record(response_bytes) {
        Ok(Some(info)) => info,
        Ok(None) => {
            console_debug!("[process_response] Return original: No HTTPS record found");
            return Ok(response_bytes.to_vec());
        }
        Err(e) => {
            console_debug!(
                "[process_response] Return original: Failed to parse DNS message: {}",
                e
            );
            return Ok(response_bytes.to_vec());
        }
    };

    let (record_offset, https_record) = https_record_info;

    // 2. 检查是否有 ECH 或非 CF IP
    if has_ech_or_non_cf_ip(&https_record) {
        console_debug!("[process_response] Return original: ECH or non-CF IP found");
        return Ok(response_bytes.to_vec());
    }

    // 3. 查询 ech_domain 的 HTTPS 记录
    let ech_response = match doh_query(ech_domain, TYPE_HTTPS, resolver).await {
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

    // 4. 从 ech_domain 响应中提取 HTTPS 记录
    let (_, ech_record) = match extract_https_record(&ech_response) {
        Ok(Some(info)) => info,
        Ok(None) => {
            console_debug!(
                "[process_response] Return original: No HTTPS record found for ech_domain"
            );
            return Ok(response_bytes.to_vec());
        }
        Err(e) => {
            console_debug!(
                "[process_response] Return original: Failed to parse ech_domain response: {}",
                e
            );
            return Ok(response_bytes.to_vec());
        }
    };

    // 5. 构建新的 HTTPS 记录数据并替换
    let new_record_data = build_https_record_data(&ech_record);

    match replace_https_record(response_bytes, &new_record_data, record_offset) {
        Ok(modified_response) => {
            console_debug!(
                "[process_response] Return modified response (original size: {}, modified size: {})",
                response_bytes.len(),
                modified_response.len()
            );
            Ok(modified_response)
        }
        Err(e) => {
            console_debug!(
                "[process_response] Return original: Failed to replace record: {}",
                e
            );
            Ok(response_bytes.to_vec())
        }
    }
}

#[test]
fn test_boundary_ips() {
    let test_cases = vec![
        ("104.16.0.0", true),
        ("104.23.255.255", true),
        ("104.15.255.255", false),
        ("104.24.0.0", true),
        ("104.27.255.255", true),
        ("104.28.0.0", false),
    ];

    for (ip_str, expected) in test_cases {
        let ip = ip_str.parse::<Ipv4Addr>().unwrap();
        assert_eq!(
            is_cloudflare_ip(ip),
            expected,
            "Boundary test failed for {}",
            ip_str
        );
    }
}
