/** Owned desktop HTTP framing over validated OS sockets and verified Rustls TLS. */
mod body;
mod lease;
mod transport;
use super::policy::{fixture_granted, web_url};
use super::{WebRequest, WebResponse};
use crate::constants::{REDIRECTS, WEB_TIMEOUT};
use crate::protocol::{EngineError, EngineResult};
use ::url::Url;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::time::Instant;

/// Fetch results retain the final validated URL after a bounded redirect chain.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WebPage {
    /// Fully received UTF-8 supported-markup candidate.
    pub source: String,
    /// Final destination for relative resolution and committed address state.
    pub url: String,
}

/// A one-use exact connection expires with the navigation's remaining deadline.
pub struct Prepared {
    connection: transport::Connection,
    request: WebRequest,
    url: Url,
    deadline: Instant,
    _lease: lease::Lease,
}
impl Prepared {
    /// Validate, resolve, connect and negotiate TLS without HTTP application bytes.
    pub fn connect(
        request: WebRequest,
        grants: &[String],
        remaining_ms: u64,
    ) -> EngineResult<Self> {
        request.validate()?;
        let url = web_url(&request.url)?;
        let deadline =
            Instant::now() + std::time::Duration::from_millis(remaining_ms).min(WEB_TIMEOUT);
        let connection = transport::connect(&url, fixture_granted(&url, grants), deadline)?;
        let lease = connection.lease(deadline)?;
        Ok(Self {
            connection,
            request,
            url,
            deadline,
            _lease: lease,
        })
    }
    /// Consuming the token prevents reuse, replay or hidden automatic redirects.
    pub fn transmit(mut self) -> EngineResult<WebResponse> {
        self.connection
            .write_web_request(&self.url, &self.request, self.deadline)?;
        let response = body::response(&mut self.connection, self.deadline, self.request.image)?;
        let mime = response
            .headers
            .get("content-type")
            .map(|s| {
                s.split(';')
                    .next()
                    .unwrap_or_default()
                    .trim()
                    .to_ascii_lowercase()
            })
            .unwrap_or_default();
        if response.headers.get("content-type").is_some_and(|s| {
            s.split(';')
                .skip(1)
                .any(|p| !p.trim().eq_ignore_ascii_case("charset=utf-8"))
        }) {
            return Err(EngineError::unsupported("Unsupported resource encoding."));
        }
        let image_data = self.request.image;
        if image_data && !["image/png", "image/jpeg"].contains(&mime.as_str()) {
            return Err(EngineError::unsupported(
                "Only PNG and JPEG images are supported.",
            ));
        }
        Ok(WebResponse {
            url: self.url.into(),
            status: response.status,
            source: if image_data {
                base64(&response.body)
            } else {
                String::from_utf8(response.body)
                    .map_err(|_| EngineError::unsupported("Only UTF-8 resources are supported."))?
            },
            image_data,
            mime,
            location: response.headers.get("location").cloned(),
            set_cookies: response.set_cookies,
        })
    }
}

/// Every redirect and DNS answer passes policy before a request is sent.
pub fn fetch(value: &str, fixture_origins: &[String]) -> EngineResult<WebPage> {
    let mut url = web_url(value)?;
    let deadline = Instant::now() + WEB_TIMEOUT;
    for redirects in 0..=REDIRECTS {
        let response = exchange(&url, fixture_origins, deadline)?;
        if response.status == crate::constants::HTTP_OK {
            return complete(response, url);
        }
        if redirects == REDIRECTS || ![301, 302, 303, 307, 308].contains(&response.status) {
            return Err(EngineError::unsupported(
                "Unsupported HTTP status or redirect limit exceeded.",
            ));
        }
        url = redirect(&url, &response.headers, fixture_origins)?;
    }
    Err(EngineError::unsupported("Redirect limit exceeded."))
}

/** Header parsing and body decoding stay owned by Oya rather than an HTTP client crate. */
fn exchange(url: &Url, grants: &[String], deadline: Instant) -> EngineResult<body::Response> {
    let mut connection = transport::connect(url, fixture_granted(url, grants), deadline)?;
    connection.write_request(url, deadline)?;
    body::response(&mut connection, deadline, false)
}

/** Bounded binary bodies cross the private JSON pipe without UTF-8 loss. */
fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let value = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        output.push(TABLE[((value >> 18) & 63) as usize] as char);
        output.push(TABLE[((value >> 12) & 63) as usize] as char);
        output.push(if chunk.len() > 1 {
            TABLE[((value >> 6) & 63) as usize] as char
        } else {
            '='
        });
        output.push(if chunk.len() > 2 {
            TABLE[(value & 63) as usize] as char
        } else {
            '='
        });
    }
    output
}

/** Final bodies must have the documented media type and encoding. */
fn complete(response: body::Response, url: Url) -> EngineResult<WebPage> {
    let mime = response
        .headers
        .get("content-type")
        .ok_or_else(|| EngineError::unsupported("The response needs Content-Type: text/html."))?;
    if !["text/html", "text/html; charset=utf-8"].contains(&mime.to_ascii_lowercase().as_str()) {
        return Err(EngineError::unsupported(
            "Only UTF-8 text/html is supported.",
        ));
    }
    let source = String::from_utf8(response.body)
        .map_err(|_| EngineError::unsupported("Only UTF-8 documents are supported."))?;
    Ok(WebPage {
        source,
        url: url.to_string(),
    })
}

/** Redirects cannot downgrade HTTPS or gain access to another private fixture origin. */
fn redirect(url: &Url, headers: &BTreeMap<String, String>, grants: &[String]) -> EngineResult<Url> {
    let location = headers
        .get("location")
        .ok_or_else(|| EngineError::unsupported("Redirect has no Location."))?;
    redirect_url(url, location, grants)
}
/// Every coordinator-managed redirect repeats the same downgrade and fixture-origin policy.
pub fn redirect_url(url: &Url, location: &str, grants: &[String]) -> EngineResult<Url> {
    if location.chars().any(char::is_control) || location.contains('\\') {
        return Err(super::policy::denied());
    }
    let next = web_url(
        url.join(location)
            .map_err(|_| super::policy::denied())?
            .as_str(),
    )?;
    if url.scheme() == "https" && next.scheme() != "https" {
        return Err(super::policy::denied());
    }
    if fixture_granted(url, grants)
        && next.origin() != url.origin()
        && next.host_str().is_some_and(|host| {
            host.trim_matches(['[', ']'])
                .parse::<std::net::IpAddr>()
                .is_ok_and(|ip| !super::policy::public_address(ip))
        })
    {
        return Err(super::policy::denied());
    }
    Ok(next)
}
