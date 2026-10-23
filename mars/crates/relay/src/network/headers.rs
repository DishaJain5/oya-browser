/** Minimal HTTP framing is strict to prevent accepting ambiguous responses. */
use crate::constants::{HTML_BYTES, HTTP_OK};
use crate::protocol::{EngineError, EngineResult};
use std::collections::BTreeMap;

/** Returns an optional exact body length after validating the response. */
pub(super) fn parse(bytes: &[u8]) -> EngineResult<Option<usize>> {
    let source = std::str::from_utf8(bytes).map_err(|_| unsupported())?;
    let mut lines = source.split("\r\n");
    status(lines.next().unwrap_or_default())?;
    let headers = fields(lines)?;
    encoding(&headers)?;
    content_length(&headers)
}

/** Redirects and error statuses are unsupported and never followed. */
fn status(line: &str) -> EngineResult<()> {
    let mut parts = line.splitn(3, ' ');
    if !["HTTP/1.1", "HTTP/1.0"].contains(&parts.next().unwrap_or_default()) {
        return Err(unsupported());
    }
    if parts.next().and_then(|status| status.parse::<u16>().ok()) != Some(HTTP_OK) {
        return Err(unsupported());
    }
    if parts.next().is_none() {
        return Err(unsupported());
    }
    Ok(())
}

/** Duplicate fields are rejected even if their spelling differs in case. */
fn fields<'a>(lines: impl Iterator<Item = &'a str>) -> EngineResult<BTreeMap<String, String>> {
    let mut headers = BTreeMap::new();
    for line in lines {
        let (name, value) = line.split_once(':').ok_or_else(unsupported)?;
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return Err(unsupported());
        }
        if value.chars().any(|c| c.is_control() && c != '\t') {
            return Err(unsupported());
        }
        if headers
            .insert(name.to_ascii_lowercase(), value.trim().into())
            .is_some()
        {
            return Err(unsupported());
        }
    }
    Ok(headers)
}

/** Transfer and compression codecs must be implemented before they can be advertised. */
fn encoding(headers: &BTreeMap<String, String>) -> EngineResult<()> {
    if headers.contains_key("transfer-encoding") {
        return Err(unsupported());
    }
    if headers
        .get("content-encoding")
        .is_some_and(|value| !value.eq_ignore_ascii_case("identity"))
    {
        return Err(unsupported());
    }
    let content_type = headers.get("content-type").ok_or_else(unsupported)?;
    if !["text/html", "text/html; charset=utf-8"]
        .contains(&content_type.to_ascii_lowercase().as_str())
    {
        return Err(unsupported());
    }
    Ok(())
}

/** Length is bounded before any matching body allocation is attempted. */
fn content_length(headers: &BTreeMap<String, String>) -> EngineResult<Option<usize>> {
    let Some(value) = headers.get("content-length") else {
        return Ok(None);
    };
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(unsupported());
    }
    let length = value.parse::<usize>().map_err(|_| unsupported())?;
    if length > HTML_BYTES {
        return Err(EngineError::limit("HTML bytes", HTML_BYTES));
    }
    Ok(Some(length))
}

/** Includes the supported framing in the recovery hint. */
fn unsupported() -> EngineError {
    EngineError::unsupported(
        "HTTP requires status 200, UTF-8 text/html, identity encoding, no duplicate headers or transfer encoding, and a valid optional Content-Length.",
    )
}
