use worker::*;
use serde::{Deserialize, Serialize};
use tokio::{sync::OnceCell};
use std::net::{Ipv4Addr};
use prefix_trie::map::PrefixMap;
use ipnet::Ipv4Net;
use crate::proxy::api;

static CF_CIDR_PREFIX: OnceCell<PrefixMap<Ipv4Net, Option<u8>>> = OnceCell::const_new();

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


async fn get_cf_cidr_prefix() -> PrefixMap<Ipv4Net, Option<u8>> {
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

   let mut pm: PrefixMap<Ipv4Net, Option<u8>> = PrefixMap::new();
   for ip in ipv4s {
       pm.insert(ip.parse().unwrap(), Some(1));
   }
   pm
}


pub async fn is_cf_address(addr: &super::Address, dns_host: &String) -> Result<bool> {
    let trie = CF_CIDR_PREFIX.get_or_init(|| async {
        get_cf_cidr_prefix().await
    }).await;
    let v4fn = |ip: &Ipv4Addr| {
        let ip = Ipv4Net::new(ip.clone(), 32).or_else(|e|{
            console_error!("parse ipv4 failed: {}", e);
            Err(Error::RustError(e.to_string()))
        })?;
        return  Ok(trie.get_lpm(&ip).is_some());
    };

    match addr {
        super::Address::Ipv6(_) => Ok(false),
        super::Address::Ipv4(ipv4) => v4fn(ipv4),
        super::Address::Domain(domain) => {
            let header = Headers::new();
            header.set("accept", "application/dns-json")?;
            header.set("user-agent", "tul/0.1")?;

            let req_init = RequestInit {
                method: Method::Get,
                headers: header,
                body: None,
                cf: CfProperties::default(),
                redirect: RequestRedirect::Follow,
            };
            let req = Request::new_with_init("https://localhost/dns-query", &req_init)?;
            
            let mut resp = api::resolve_handler(req, dns_host, Some(format!("name={}&type=A", domain))).await?;
            let dns_record = resp.json::<DnsJsonResponse>().await?;
            console_debug!("DNS Record: {:?}", dns_record);
            if let Some(records) = dns_record.answer {
                if let Some(answer) = records.first() {
                    let ip = answer.data.parse::<Ipv4Addr>().or_else(|e|{
                        console_error!("parse ipv4 failed: {}", e);
                        Err(Error::RustError(e.to_string()))
                    })?;
                    return v4fn(&ip);
                }
            }
            Ok(false)
        }
    }
}
