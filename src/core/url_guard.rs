use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs};

use super::error::{AppError, AppResult};
use super::input_validator::clean;

const BLOCKED_HOST_SUFFIXES: [&str; 6] = [
    ".local",
    ".internal",
    ".home",
    ".lan",
    "localhost",
    "metadata.google.internal",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicUrl {
    pub scheme: String,
    pub host: String,
    pub port: u16,
    pub path: String,
    pub origin: String,
    pub text: String,
}

pub fn validate_public_url(value: &str) -> AppResult<PublicUrl> {
    parse_public_url(&clean(value)).ok_or_else(|| AppError::validation("url.blocked"))
}

pub fn is_public_url(value: &str) -> bool {
    parse_public_url(value).is_some()
}

pub fn parse_public_url(value: &str) -> Option<PublicUrl> {
    let text = value.trim();
    if text.is_empty() || text.contains('@') {
        return None;
    }
    let (scheme, rest) = split_scheme(text)?;
    if scheme != "http" && scheme != "https" {
        return None;
    }
    let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..authority_end];
    if authority.is_empty() || authority.contains('@') {
        return None;
    }
    let (host, port) = split_host_port(authority, &scheme)?;
    let lowered = host.to_ascii_lowercase();
    if lowered.is_empty() {
        return None;
    }
    if BLOCKED_HOST_SUFFIXES
        .iter()
        .any(|suffix| lowered.ends_with(suffix))
    {
        return None;
    }
    if let Ok(address) = host.parse::<IpAddr>() {
        if !is_allowed_address(address) {
            return None;
        }
    }
    let path = if authority_end >= rest.len() {
        "/".to_string()
    } else {
        rest[authority_end..].to_string()
    };
    let origin = format!("{scheme}://{authority}");
    let text = format!("{scheme}://{authority}{path}");
    Some(PublicUrl {
        scheme,
        host,
        port,
        path,
        origin,
        text,
    })
}

fn split_scheme(text: &str) -> Option<(String, &str)> {
    let index = text.find("://")?;
    let scheme = text[..index].to_ascii_lowercase();
    if scheme.is_empty() {
        return None;
    }
    Some((scheme, &text[index + 3..]))
}

fn split_host_port(authority: &str, scheme: &str) -> Option<(String, u16)> {
    if let Some(rest) = authority.strip_prefix('[') {
        let end = rest.find(']')?;
        let host = rest[..end].to_string();
        let remainder = &rest[end + 1..];
        let port = if remainder.is_empty() {
            default_port(scheme)
        } else {
            remainder.strip_prefix(':')?.parse::<u16>().ok()?
        };
        return Some((host, port));
    }
    match authority.rsplit_once(':') {
        Some((host, port)) => Some((host.to_string(), port.parse::<u16>().ok()?)),
        None => Some((authority.to_string(), default_port(scheme))),
    }
}

fn default_port(scheme: &str) -> u16 {
    if scheme == "https" {
        443
    } else {
        80
    }
}

pub fn resolve_public_addresses(host: &str, port: u16) -> std::io::Result<Vec<SocketAddr>> {
    let target = format!("{host}:{port}");
    let addresses: Vec<SocketAddr> = target.to_socket_addrs()?.collect();
    let allowed: Vec<SocketAddr> = addresses
        .into_iter()
        .filter(|address| is_allowed_address(address.ip()))
        .collect();
    if allowed.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "url.blocked",
        ));
    }
    Ok(allowed)
}

pub fn is_allowed_address(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(value) => {
            if value.is_loopback()
                || value.is_unspecified()
                || value.is_broadcast()
                || value.is_multicast()
                || value.is_private()
            {
                return false;
            }
            is_public_ipv4(value)
        }
        IpAddr::V6(value) => {
            if let Some(mapped) = value.to_ipv4_mapped() {
                return is_allowed_address(IpAddr::V4(mapped));
            }
            if value.is_loopback()
                || value.is_unspecified()
                || value.is_multicast()
                || value.is_unicast_link_local()
                || is_site_local(value)
                || is_unique_local(value)
            {
                return false;
            }
            is_public_ipv6(value)
        }
    }
}

fn is_site_local(address: Ipv6Addr) -> bool {
    let bytes = address.octets();
    bytes[0] == 0xFE && (bytes[1] & 0xC0) == 0xC0
}

fn is_unique_local(address: Ipv6Addr) -> bool {
    (address.octets()[0] & 0xFE) == 0xFC
}

fn is_public_ipv4(address: Ipv4Addr) -> bool {
    let bytes = address.octets();
    match bytes[0] {
        0 => false,
        10 => false,
        100 => !(64..=127).contains(&bytes[1]),
        127 => false,
        169 => bytes[1] != 254,
        172 => !(16..=31).contains(&bytes[1]),
        192 => match bytes[1] {
            0 | 168 => false,
            18 => !(18..=19).contains(&bytes[2]),
            _ => true,
        },
        198 => !(18..=31).contains(&bytes[1]),
        224 => false,
        255 => false,
        _ => true,
    }
}

fn is_public_ipv6(address: Ipv6Addr) -> bool {
    let bytes = address.octets();
    if (bytes[0] & 0xFE) == 0xFC {
        return false;
    }
    if bytes[0] == 0x20 && bytes[1] == 0x01 && bytes[2] == 0x0D && bytes[3] == 0xB8 {
        return false;
    }
    if bytes[0] == 0x00 && bytes[1..].iter().all(|value| *value == 0) {
        return false;
    }
    if bytes[0] == 0x64 && bytes[1] == 0xFF && bytes[2] == 0x9B {
        return is_public_ipv4(Ipv4Addr::new(bytes[12], bytes[13], bytes[14], bytes[15]));
    }
    if bytes[0] == 0x20 && bytes[1] == 0x02 {
        return is_public_ipv4(Ipv4Addr::new(bytes[2], bytes[3], bytes[4], bytes[5]));
    }
    if bytes[0] == 0x20 && bytes[1] == 0x01 && bytes[2] == 0x00 && (bytes[3] & 0xC0) == 0 {
        return false;
    }
    true
}
