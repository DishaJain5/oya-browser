/** Attribute semantics and character references stay owned and bounded. */
use super::{Node, validate_text};
use crate::constants::{ID_BYTES, SCRIPT_BYTES};
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
    let mut output = String::new();
    let mut rest = source;
    while let Some(start) = rest.find('&') {
        output.push_str(&rest[..start]);
        rest = &rest[start..];
        let Some(end) = rest.find(';').filter(|end| *end <= 32) else {
            output.push('&');
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
            output.push(value);
            rest = &rest[end + 1..];
        } else {
            output.push('&');
            rest = &rest[1..];
        }
    }
    output.push_str(rest);
    validate_text(&output)?;
    Ok(output)
}
/** Unsupported syntax cannot create partially trusted native targets. */
pub(super) fn syntax() -> EngineError {
    EngineError::unsupported("This page uses unsupported or malformed HTML markup.")
}
