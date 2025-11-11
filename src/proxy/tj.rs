

use tokio::io::{AsyncRead, AsyncReadExt};
use std::net::{Ipv4Addr, Ipv6Addr};
use std::vec::Vec;

pub async fn parse<R: AsyncRead + Unpin>(pw_hash: &Vec<u8>, stream: &mut R) -> std::io::Result<(super::Address, u16)> {

    let mut password_hash = [0u8; 56];
    stream.read_exact(&mut password_hash).await?;
    if &password_hash != pw_hash.as_slice() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("Invalid hash, expected: {:?}, got: {:?}", 
                String::from_utf8_lossy(pw_hash.as_slice()), 
                String::from_utf8_lossy(&password_hash)),
        ));
    }
    
    // Read CLRF
    stream.read_u16().await?;

    // Extract command
    match stream.read_u8().await? {
        1|3 => {},
        cmd => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Unsupport command: {}", cmd),
            ));
        }
    };

    // Read address type
    let atype = stream.read_u8().await?;

    // Get address size and address object
    let host = match atype {
        1 => super::Address::Ipv4(Ipv4Addr::from(stream.read_u32().await?)),
        4 => super::Address::Ipv6(Ipv6Addr::from(stream.read_u128().await?)),
        3 => {
            // Read domain name size
            let size = stream.read_u8().await? as usize;

            // Read domain name context
            let mut domain_buf = vec![0u8; size];
            stream.read_exact(&mut domain_buf).await?;
            super::Address::Domain(String::from_utf8(domain_buf)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?)
        }
        _ => return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Unknown address type: {atype}"),
            )),
    };

    // Read port number
    let port = stream.read_u16().await?;

    // Read CLRF
    stream.read_u16().await?;

    Ok((host,  port))
}