/** Bounded HTTP status, headers and chunk framing are implemented in Oya. */
use super::transport::{Connection, READ_CHUNK};
use crate::constants::{HEADER_BYTES, HTML_BYTES, IMAGE_BYTES};
use crate::protocol::{EngineError, EngineResult};
use std::collections::BTreeMap;
use std::time::Instant;

/** A complete decoded response, not a prematurely published page. */
pub(super) struct Response {
    /** Valid three-digit HTTP status. */
    pub status: u16,
    /** Canonical header names after strict duplicate checks. */
    pub headers: BTreeMap<String, String>,
    /** Cookies preserve individual fields without relaxing singleton framing. */
    pub set_cookies: Vec<String>,
    /** Transfer-decoded bounded bytes. */
    pub body: Vec<u8>,
}

/** Header bytes and decoded body have independent budgets. */
pub(super) fn response(
    connection: &mut Connection,
    deadline: Instant,
    image: bool,
) -> EngineResult<Response> {
    let mut input = Input {
        connection,
        deadline,
        bytes: Vec::new(),
        position: 0,
    };
    let status_line = input.line(HEADER_BYTES)?;
    let status = status(&status_line)?;
    let mut used = status_line.len() + 2;
    let (headers, set_cookies) = headers(&mut input, &mut used)?;
    if headers
        .get("content-encoding")
        .is_some_and(|value| !value.eq_ignore_ascii_case("identity"))
    {
        return Err(unsupported());
    }
    let body = body(
        &mut input,
        &headers,
        if image { IMAGE_BYTES } else { HTML_BYTES },
    )?;
    Ok(Response {
        status,
        headers,
        set_cookies,
        body,
    })
}

/** Exactly one header spelling/value is accepted for every canonical name. */
fn headers(
    input: &mut Input<'_>,
    used: &mut usize,
) -> EngineResult<(BTreeMap<String, String>, Vec<String>)> {
    let mut headers = BTreeMap::new();
    let mut cookies = Vec::new();
    loop {
        let line = input.line(HEADER_BYTES.saturating_sub(*used))?;
        *used += line.len() + 2;
        if *used > HEADER_BYTES {
            return Err(EngineError::limit("HTTP header bytes", HEADER_BYTES));
        }
        if line.is_empty() {
            return Ok((headers, cookies));
        }
        let (name, value) = line.split_once(':').ok_or_else(unsupported)?;
        if name.is_empty()
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            || value.chars().any(|c| c.is_control() && c != '\t')
        {
            return Err(unsupported());
        }
        if name.eq_ignore_ascii_case("set-cookie") {
            cookies.push(value.trim().to_owned());
            continue;
        }
        if headers
            .insert(name.to_ascii_lowercase(), value.trim().to_owned())
            .is_some()
        {
            return Err(unsupported());
        }
    }
}

/** Content-Length and chunked framing cannot be mixed. */
fn body(
    input: &mut Input<'_>,
    headers: &BTreeMap<String, String>,
    limit: usize,
) -> EngineResult<Vec<u8>> {
    let transfer = headers.get("transfer-encoding");
    let length = headers.get("content-length");
    if transfer.is_some() && length.is_some() {
        return Err(unsupported());
    }
    if let Some(transfer) = transfer {
        if !transfer.eq_ignore_ascii_case("chunked") {
            return Err(unsupported());
        }
        return chunked(input, limit);
    }
    if let Some(length) = length {
        if length.is_empty() || !length.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(unsupported());
        }
        let length = length.parse::<usize>().map_err(|_| unsupported())?;
        if length > limit {
            return Err(EngineError::limit("HTTP body bytes", limit));
        }
        return input.exact(length);
    }
    let mut body = Vec::new();
    while let Some(byte) = input.byte()? {
        if body.len() == limit {
            return Err(EngineError::limit("HTTP body bytes", limit));
        }
        body.push(byte);
    }
    Ok(body)
}

/** Chunk sizes, terminators and trailers have explicit total bounds. */
fn chunked(input: &mut Input<'_>, limit: usize) -> EngineResult<Vec<u8>> {
    let mut body = Vec::new();
    let mut framing = 0;
    loop {
        let size = input.line(HEADER_BYTES.saturating_sub(framing))?;
        framing += size.len() + 2;
        if size.is_empty() || !size.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(unsupported());
        }
        let size = usize::from_str_radix(&size, 16).map_err(|_| unsupported())?;
        if size == 0 {
            let _ = headers(input, &mut framing)?;
            return Ok(body);
        }
        if size > limit.saturating_sub(body.len()) {
            return Err(EngineError::limit("HTTP body bytes", limit));
        }
        body.extend(input.exact(size)?);
        if input.exact(2)? != b"\r\n" {
            return Err(unsupported());
        }
        framing += 2;
        if framing > HEADER_BYTES {
            return Err(EngineError::limit("HTTP chunk framing bytes", HEADER_BYTES));
        }
    }
}

/** A small buffered cursor avoids allocating for the entire encoded response. */
struct Input<'a> {
    /** Exact-address transport with an absolute deadline. */
    connection: &'a mut Connection,
    /** Shared across every read and redirect. */
    deadline: Instant,
    /** At most one socket read chunk is retained. */
    bytes: Vec<u8>,
    /** Next unread byte within that chunk. */
    position: usize,
}

impl Input<'_> {
    /** Stream consumption never retries after EOF. */
    fn byte(&mut self) -> EngineResult<Option<u8>> {
        if self.position == self.bytes.len() {
            self.bytes.resize(READ_CHUNK, 0);
            let length = self.connection.read(&mut self.bytes, self.deadline)?;
            self.bytes.truncate(length);
            self.position = 0;
            if length == 0 {
                return Ok(None);
            }
        }
        let byte = self.bytes[self.position];
        self.position += 1;
        Ok(Some(byte))
    }
    /** A promised body or CRLF must arrive completely. */
    fn exact(&mut self, length: usize) -> EngineResult<Vec<u8>> {
        let mut output = Vec::with_capacity(length);
        for _ in 0..length {
            output.push(self.byte()?.ok_or_else(unsupported)?);
        }
        Ok(output)
    }
    /** Header and chunk lines stop at their budget rather than growing until newline. */
    fn line(&mut self, budget: usize) -> EngineResult<String> {
        let mut output = Vec::new();
        loop {
            let byte = self.byte()?.ok_or_else(unsupported)?;
            if output.len() >= budget {
                return Err(EngineError::limit("HTTP framing bytes", HEADER_BYTES));
            }
            output.push(byte);
            if output.ends_with(b"\r\n") {
                output.truncate(output.len() - 2);
                return String::from_utf8(output).map_err(|_| unsupported());
            }
            if byte == b'\n' {
                return Err(unsupported());
            }
        }
    }
}

/** Malformed statuses are not normalized into a successful response. */
fn status(line: &str) -> EngineResult<u16> {
    let mut fields = line.splitn(3, ' ');
    if !["HTTP/1.1", "HTTP/1.0"].contains(&fields.next().unwrap_or_default()) {
        return Err(unsupported());
    }
    let value = fields.next().ok_or_else(unsupported)?;
    if value.len() != 3
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || fields.next().is_none()
    {
        return Err(unsupported());
    }
    value.parse().map_err(|_| unsupported())
}

/** An unsupported response remains an error instead of being partially interpreted. */
fn unsupported() -> EngineError {
    EngineError::unsupported(
        "Unsupported or incomplete HTTP status, headers, encoding or body framing.",
    )
}
