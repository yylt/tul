
use super::*;
use serde::{Deserialize, Serialize};
use tokio::{sync::OnceCell};
use std::{collections::HashMap, net::Ipv4Addr};

use prefix_trie::set::PrefixSet;
use ipnet::Ipv4Net;


static CF_TRIE: OnceCell<PrefixSet<Ipv4Net>> = OnceCell::const_new();

// DNS JSON API response format
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct DnsJsonResponse {
    #[serde(rename = "Status")]
    pub status: u16,
    #[serde(rename = "TC")]
    pub tc: bool,
    #[serde(rename = "RD")]
    pub rd: bool,
    #[serde(rename = "RA")]
    pub ra: bool,
    #[serde(rename = "AD")]
    pub ad: bool,
    #[serde(rename = "CD")]
    pub cd: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub question: Option<Vec<DnsQuestion>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub answer: Option<Vec<DnsAnswer>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authority: Option<Vec<DnsAnswer>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional: Option<Vec<DnsAnswer>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DnsQuestion {
    pub name: String,
    #[serde(rename = "type")]
    pub qtype: u16,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DnsAnswer {
    pub name: String,
    #[serde(rename = "type")]
    pub rtype: u16,
    #[serde(rename = "TTL")]
    pub ttl: u32,
    pub data: String,
}

// ref: https://www.cloudflare.com/ips
async fn get_cf_trie() -> PrefixSet<Ipv4Net> {
    // TODO fetch from cloudflare
    let ipv4s = vec![
        "103.22.200.0/22"
        ,"103.31.4.0/22"
        ,"104.16.0.0/13"
        ,"104.24.0.0/14"
        ,"108.162.192.0/18"
        ,"131.0.72.0/22"
        ,"141.101.64.0/18"
        ,"162.158.0.0/15"
        ,"172.64.0.0/13"
        ,"173.245.48.0/20"
        ,"188.114.96.0/20"
        ,"190.93.240.0/20"
        ,"197.234.240.0/22"
        ,"198.41.128.0/17"
   ];

   let mut pm: PrefixSet<Ipv4Net> = PrefixSet::new();
   for ip in ipv4s {
       pm.insert(ip.parse().unwrap());
   }
   pm
}


pub async fn is_cf_address<T: AsRef<str>>(addr: &super::Address<T>) -> Result<(bool, Ipv4Addr)> {
    let trie = CF_TRIE.get_or_init(|| async {
        get_cf_trie().await
    }).await;
    let v4fn = |ip: &Ipv4Addr| -> Result<(bool, Ipv4Addr)> {
        let ipnet = Ipv4Net::new(*ip, 32).map_err(|e|{
            console_error!("parse ipv4 failed: {}", e);
            worker::Error::RustError(e.to_string())
        })?;
        Ok((trie.get_lpm(&ipnet).is_some(), *ip))
    };
    // TODO: only 1.1.1.1 support RFC 8484 and JSON API
    let resolve = "1.1.1.1";
    match addr {
        super::Address::Ipv4(ipv4) => v4fn(ipv4),
        super::Address::Domain(domain) => {
            let header = Headers::new();
            header.set("accept", "application/dns-json")?;
            header.set("user-agent", "tul/0.1")?;
            console_debug!("DNS query: {:?}", domain.as_ref());

            let req_init = RequestInit {
                method: Method::Get,
                headers: header,
                body: None,
                cf: CfProperties::default(),
                redirect: RequestRedirect::Follow,
                cache: None, // CacheMode::Default,
            };
            let req = Request::new_with_init("https://lo/dns-query", &req_init)?;
            let mut map = HashMap::new();
            map.insert("name".to_string(), domain.as_ref().to_string());
            map.insert("type".to_string(), "A".to_string());

            let mut resp = resolve_handler(req, resolve, Some(map)).await?;
            let dns_record = resp.json::<DnsJsonResponse>().await?;
            console_debug!("DNS Record: {:?}", dns_record);
            if let Some(records) = dns_record.answer {
                for answer in records {
                    if answer.rtype == 1 {  
                        let ip = answer.data.parse::<Ipv4Addr>().map_err(|e| {
                            console_error!("parse ipv4 failed: {}", e);
                            worker::Error::RustError(e.to_string())
                        })?;
                        return v4fn(&ip);
                    }
                }
            }
            Err(worker::Error::Infallible)
        }
    }
}


pub async fn resolve_handler<T: AsRef<str>>(mut req: Request, host: T, query: Option<HashMap<String, String>>) -> Result<Response> {
    let hops = HOP_HEADERS.get_or_init(|| async {
        get_hop_headers().await
    }).await;
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
        uri.push_str(v.iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("&")
            .as_str());
    }

    let new_req = Request::new_with_init(&uri, &req_init)?;
    //console_debug!("DNS Request: {:?}", new_req);
    return Fetch::Request(new_req).send().await;
}
