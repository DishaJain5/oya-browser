/** Attribute semantics and character references stay owned and bounded. */
use super::Node;
use crate::constants::{ID_BYTES, SCRIPT_BYTES, STATIC_TEXT_NODE_BYTES, TEXT_BYTES};
use crate::protocol::{EngineError, EngineResult};
use std::collections::BTreeMap;
pub(super) type Attributes = BTreeMap<String, String>;

/** Inert attributes survive; unsupported executable attributes refuse loading. */
pub(super) fn node(
    tag: &str,
    local_id: usize,
    parent: Option<usize>,
    mut attributes: Attributes,
) -> EngineResult<Node> {
    if attributes
        .keys()
        .any(|key| key.starts_with("on") && !(tag == "button" && key == "onclick"))
    {
        return Err(EngineError::unsupported(
            "This page uses an unsupported event handler.",
        ));
    }
    if let Some(kind) = attributes.get_mut("type") {
        *kind = kind.to_ascii_lowercase();
    }
    if tag == "input"
        && attributes.get("type").is_some_and(|s| {
            ![
                "text", "password", "hidden", "checkbox", "radio", "submit", "button", "email",
                "search", "url", "tel",
            ]
            .contains(&s.as_str())
        })
    {
        return Err(EngineError::unsupported(
            "This input type is not supported.",
        ));
    }
    if tag == "select" && attributes.contains_key("multiple") {
        return Err(EngineError::unsupported(
            "Multiple selection is not supported yet.",
        ));
    }
    let id = attributes
        .get("id")
        .cloned()
        .unwrap_or_else(|| format!("@{local_id}"));
    if id.is_empty()
        || id.len() > ID_BYTES
        || attributes
            .get("onclick")
            .is_some_and(|s| s.len() > SCRIPT_BYTES)
    {
        return Err(syntax());
    }
    Ok(Node {
        local_id,
        html_id: id,
        tag: tag.into(),
        text: String::new(),
        parent,
        value: attributes.get("value").cloned().unwrap_or_default(),
        href: attributes.get("href").cloned(),
        handler: attributes.get("onclick").cloned(),
        placeholder: attributes.get("placeholder").cloned().unwrap_or_default(),
        disabled: attributes.contains_key("disabled"),
        checked: attributes.contains_key("checked"),
        selected: attributes.contains_key("selected"),
        attributes: attributes.clone(),
        style: Default::default(),
        validation: None,
        image_src: attributes.get("src").cloned().filter(|_| tag == "img"),
        image_alt: attributes.get("alt").cloned().unwrap_or_default(),
        image_size: None,
        table_cell: None,
    })
}
/** Numeric references and common entities decode without interpreting literal markup. */
pub(super) fn entities(source: &str) -> EngineResult<String> {
    decode_entities(source, TEXT_BYTES)
}

/** Only source body text receives the larger static allowance. */
pub(super) fn static_entities(source: &str) -> EngineResult<String> {
    decode_entities(source, STATIC_TEXT_NODE_BYTES)
}

/** Every decoded append checks bytes before allocation, including expanded numeric references. */
fn decode_entities(source: &str, limit: usize) -> EngineResult<String> {
    let mut output = String::new();
    let mut rest = source;
    while let Some(start) = rest.find('&') {
        append_decoded(&mut output, &rest[..start], limit)?;
        rest = &rest[start..];
        let Some(end) = rest.find(';').filter(|end| *end <= 32) else {
            append_decoded(&mut output, "&", limit)?;
            rest = &rest[1..];
            continue;
        };
        let name = &rest[1..end];
        let value = match name {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some('\u{a0}'),
            _ => name
                .strip_prefix("#x")
                .or_else(|| name.strip_prefix("#X"))
                .and_then(|s| u32::from_str_radix(s, 16).ok())
                .or_else(|| name.strip_prefix('#').and_then(|s| s.parse().ok()))
                .and_then(char::from_u32),
        };
        if let Some(value) = value {
            append_decoded(&mut output, value.encode_utf8(&mut [0; 4]), limit)?;
            rest = &rest[end + 1..];
        } else {
            append_decoded(&mut output, "&", limit)?;
            rest = &rest[1..];
        }
    }
    append_decoded(&mut output, rest, limit)?;
    Ok(output)
}
/** Unsupported syntax cannot create partially trusted native targets. */
pub(super) fn syntax() -> EngineError {
    EngineError::unsupported("This page uses unsupported or malformed HTML markup.")
}

/** Source slices and decoded scalars share the same pre-allocation byte check. */
fn append_decoded(output: &mut String, text: &str, limit: usize) -> EngineResult<()> {
    if text.len() > limit.saturating_sub(output.len()) {
        return Err(crate::protocol::EngineError::limit(
            "node text bytes",
            limit,
        ));
    }
    output.push_str(text);
    Ok(())
}
