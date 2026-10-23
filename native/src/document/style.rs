/** Owned bounded CSS cascade controls geometry and fails closed on unsupported hiding rules. */
use super::{Document, Node};
use crate::{
    constants::{DIAGNOSTICS, STYLE_BYTES, STYLE_SHEETS},
    protocol::{EngineError, EngineResult},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Required resources retain their document order before acquisition.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum StyleSource {
    Inline(String),
    Linked(String),
}
/// One computed style is shared by rendering, native widgets and action guards.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct ComputedStyle {
    pub hidden: bool,
    pub interactive: bool,
    pub inline: bool,
    pub font_size: f64,
    pub bold: bool,
    pub color: Option<String>,
    pub background: Option<String>,
    pub margin: [f64; 4],
    pub padding: [f64; 4],
    pub width: Option<f64>,
    pub percent_width: bool,
    pub max_width: Option<f64>,
    pub border: f64,
    pub border_color: Option<String>,
    pub align: String,
}
impl Default for ComputedStyle {
    fn default() -> Self {
        Self {
            hidden: false,
            interactive: true,
            inline: false,
            font_size: 16.0,
            bold: false,
            color: None,
            background: None,
            margin: [0.0; 4],
            padding: [0.0; 4],
            width: None,
            percent_width: false,
            max_width: None,
            border: 0.0,
            border_color: None,
            align: "left".into(),
        }
    }
}
/** Cascade priority sorts importance, inline origin, specificity and source order. */
type Priority = (bool, bool, usize, usize);
/** The parser never evaluates URLs, variables or host callbacks. */
struct Rule {
    selector: String,
    declarations: Vec<(String, String, bool)>,
    specificity: usize,
    order: usize,
}

/// Required sheets must all parse before a candidate's actions become visible.
pub fn compute_styles(document: &Document, sheets: &[String]) -> EngineResult<Document> {
    if sheets.len() > STYLE_SHEETS * 2
        || sheets.iter().map(String::len).sum::<usize>() > STYLE_BYTES * STYLE_SHEETS
    {
        return Err(EngineError::limit(
            "aggregate style bytes",
            STYLE_BYTES * STYLE_SHEETS,
        ));
    }
    let mut rules = Vec::new();
    for sheet in sheets {
        parse_sheet(sheet, &mut rules)?;
    }
    let mut candidate = document.clone();
    for index in 0..candidate.nodes.len() {
        let node = &document.nodes[index];
        let parent = node
            .parent
            .and_then(|id| candidate.nodes.iter().find(|n| n.local_id == id));
        let mut style = ComputedStyle::default();
        if let Some(parent) = parent {
            style.font_size = parent.style.font_size;
            style.bold = parent.style.bold;
            style.color = parent.style.color.clone();
            style.align = parent.style.align.clone();
            style.hidden = parent.hidden();
            style.interactive = parent.style.interactive;
        }
        style.inline = [
            "span", "a", "strong", "em", "b", "i", "small", "code", "#text", "img",
        ]
        .contains(&node.tag.as_str());
        if ["strong", "b", "h1", "h2", "h3", "th"].contains(&node.tag.as_str()) {
            style.bold = true;
        }
        if let Some(size) = [("h1", 28.0), ("h2", 22.0), ("h3", 18.0)]
            .iter()
            .find(|(tag, _)| *tag == node.tag)
            .map(|(_, size)| *size)
        {
            style.font_size = size;
        }
        let mut chosen: BTreeMap<String, (Priority, String)> = BTreeMap::new();
        for rule in &rules {
            if matches(document, node, &rule.selector)? {
                for (key, value, important) in &rule.declarations {
                    choose(
                        &mut chosen,
                        key,
                        value,
                        (*important, false, rule.specificity, rule.order),
                    );
                }
            }
        }
        if let Some(inline) = node.attributes.get("style") {
            for (key, value, important) in declarations(inline)? {
                choose(&mut chosen, &key, &value, (important, true, 0, rules.len()));
            }
        }
        for (key, (_, value)) in chosen {
            apply(&mut style, &key, &value, &mut candidate.diagnostics)?;
        }
        style.hidden |= node.attributes.contains_key("hidden") || parent.is_some_and(Node::hidden);
        if node.tag == "option" || (node.tag == "input" && node.input_type() == "hidden") {
            style.hidden = true;
        }
        candidate.nodes[index].style = style;
    }
    Ok(candidate)
}
/** A stronger declaration wins without depending on iteration of a hash table. */
fn choose(
    chosen: &mut BTreeMap<String, (Priority, String)>,
    key: &str,
    value: &str,
    priority: Priority,
) {
    if chosen
        .get(key)
        .is_none_or(|(previous, _)| priority >= *previous)
    {
        chosen.insert(key.into(), (priority, value.into()));
    }
}
/** At-rules and unresolved nested syntax cannot silently expose hidden controls. */
fn parse_sheet(sheet: &str, rules: &mut Vec<Rule>) -> EngineResult<()> {
    if sheet.len() > STYLE_BYTES {
        return Err(EngineError::limit("stylesheet bytes", STYLE_BYTES));
    }
    let mut source = String::new();
    let mut rest = sheet;
    while let Some(start) = rest.find("/*") {
        source.push_str(&rest[..start]);
        let end = rest[start + 2..].find("*/").ok_or_else(unsupported)? + start + 4;
        rest = &rest[end..];
    }
    source.push_str(rest);
    let mut rest = source.trim();
    while !rest.is_empty() {
        let (selectors, tail) = rest.split_once('{').ok_or_else(unsupported)?;
        let (body, tail) = tail.split_once('}').ok_or_else(unsupported)?;
        if selectors.contains('@') || body.contains(['{', '}']) {
            return Err(unsupported());
        }
        let declarations = declarations(body)?;
        for selector in selectors.split(',') {
            let selector = selector.trim();
            validate_selector(selector)?;
            rules.push(Rule {
                selector: selector.into(),
                declarations: declarations.clone(),
                specificity: specificity(selector),
                order: rules.len(),
            });
            if rules.len() > crate::constants::NODES {
                return Err(EngineError::limit("CSS rules", crate::constants::NODES));
            }
        }
        rest = tail.trim();
    }
    Ok(())
}
/** Simple declaration grammar refuses unresolved values instead of guessing layout. */
fn declarations(source: &str) -> EngineResult<Vec<(String, String, bool)>> {
    let mut declarations = Vec::new();
    for declaration in source.split(';').filter(|s| !s.trim().is_empty()) {
        let (key, value) = declaration.split_once(':').ok_or_else(unsupported)?;
        let key = key.trim().to_ascii_lowercase();
        let mut value = value.trim().to_ascii_lowercase();
        let important = value.ends_with("!important");
        if important {
            value.truncate(value.len() - "!important".len());
            value = value.trim().into();
        }
        if key.is_empty() || value.is_empty() || value.contains(['{', '}', '!']) {
            return Err(unsupported());
        }
        declarations.push((key, value, important));
    }
    Ok(declarations)
}
/** Type/class/ID compounds and descendants are the complete supported selector grammar. */
fn validate_selector(selector: &str) -> EngineResult<()> {
    if selector.is_empty()
        || !selector
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_-.*# \t\r\n".contains(c))
        || selector
            .split_whitespace()
            .any(|part| part.ends_with(['.', '#']))
    {
        return Err(unsupported());
    }
    Ok(())
}
/** Ordered matching includes a virtual body ancestor without exporting a duplicate DOM. */
fn matches(document: &Document, node: &Node, selector: &str) -> EngineResult<bool> {
    let parts = selector.split_whitespace().collect::<Vec<_>>();
    let mut remaining = parts.len();
    if parts == ["body"] || parts == ["html"] {
        return Ok(node.parent.is_none());
    }
    if !compound(node, parts[remaining - 1]) {
        return Ok(false);
    }
    remaining -= 1;
    let mut parent = node.parent;
    while remaining > 0 {
        if let Some(id) = parent {
            let ancestor = &document.nodes[id];
            if compound(ancestor, parts[remaining - 1]) {
                remaining -= 1;
            }
            parent = ancestor.parent;
        } else {
            if remaining == 1 && ["body", "html"].contains(&parts[0]) {
                remaining = 0;
            }
            break;
        }
    }
    Ok(remaining == 0)
}
/** One compound cannot escape into native selector dispatch. */
fn compound(node: &Node, source: &str) -> bool {
    let mut rest = source;
    let end = rest.find(['.', '#']).unwrap_or(rest.len());
    let tag = &rest[..end];
    if !tag.is_empty() && tag != "*" && tag != node.tag {
        return false;
    }
    rest = &rest[end..];
    while !rest.is_empty() {
        let prefix = rest.as_bytes()[0];
        rest = &rest[1..];
        let end = rest.find(['.', '#']).unwrap_or(rest.len());
        let name = &rest[..end];
        if name.is_empty()
            || (prefix == b'#' && node.html_id != name)
            || (prefix == b'.'
                && !node
                    .attributes
                    .get("class")
                    .is_some_and(|s| s.split_whitespace().any(|class| class == name)))
        {
            return false;
        }
        rest = &rest[end..];
    }
    true
}
/** This tuple-equivalent scalar leaves room for every bounded selector component. */
fn specificity(selector: &str) -> usize {
    selector.matches('#').count() * 1_000_000
        + selector.matches('.').count() * 1000
        + selector
            .split_whitespace()
            .filter(|part| part.starts_with(char::is_alphabetic))
            .count()
}
/** Unsafe unsupported presentation prevents candidate publication with value-free copy. */
fn apply(
    style: &mut ComputedStyle,
    key: &str,
    value: &str,
    diagnostics: &mut Vec<String>,
) -> EngineResult<()> {
    match key {
        "display" => match value {
            "none" => style.hidden = true,
            "block" => style.inline = false,
            "inline" => style.inline = true,
            _ => return Err(unsupported()),
        },
        "visibility" => match value {
            "hidden" | "collapse" => style.hidden = true,
            "visible" => {}
            _ => return Err(unsupported()),
        },
        "pointer-events" => match value {
            "none" => style.interactive = false,
            "auto" => {}
            _ => return Err(unsupported()),
        },
        "font-size" => style.font_size = length(value, style.font_size)?.clamp(8.0, 72.0),
        "font-weight" => {
            style.bold = match value {
                "bold" | "bolder" | "600" | "700" | "800" | "900" => true,
                "normal" | "400" | "500" => false,
                _ => return Err(unsupported()),
            }
        }
        "color" => style.color = Some(color(value)?),
        "background" | "background-color" => style.background = Some(color(value)?),
        "width" => {
            style.percent_width = value.ends_with('%');
            style.width = if value == "auto" {
                None
            } else {
                Some(if style.percent_width {
                    number(value.trim_end_matches('%'))?
                } else {
                    length(value, style.font_size)?
                })
            };
        }
        "max-width" => {
            style.max_width = if value == "none" {
                None
            } else {
                Some(length(value, style.font_size)?)
            }
        }
        "margin" => style.margin = box_values(value, style.font_size)?,
        "padding" => style.padding = box_values(value, style.font_size)?,
        "margin-top" => style.margin[0] = length(value, style.font_size)?,
        "margin-right" => style.margin[1] = length(value, style.font_size)?,
        "margin-bottom" => style.margin[2] = length(value, style.font_size)?,
        "margin-left" => style.margin[3] = length(value, style.font_size)?,
        "padding-top" => style.padding[0] = length(value, style.font_size)?,
        "padding-right" => style.padding[1] = length(value, style.font_size)?,
        "padding-bottom" => style.padding[2] = length(value, style.font_size)?,
        "padding-left" => style.padding[3] = length(value, style.font_size)?,
        "border" => {
            if value == "none" || value == "0" {
                style.border = 0.0;
            } else {
                let parts = value.split_whitespace().collect::<Vec<_>>();
                if parts.len() != 3 || parts[1] != "solid" {
                    return Err(unsupported());
                }
                style.border = length(parts[0], style.font_size)?.min(8.0);
                style.border_color = Some(color(parts[2])?);
            }
        }
        "border-width" => style.border = length(value, style.font_size)?.min(8.0),
        "border-color" => style.border_color = Some(color(value)?),
        "border-style" if ["solid", "none"].contains(&value) => {
            if value == "none" {
                style.border = 0.0;
            }
        }
        "text-align" if ["left", "center", "right", "start"].contains(&value) => {
            style.align = value.into()
        }
        "font-family" | "font-style" | "border-radius" | "box-shadow" | "text-decoration"
        | "cursor" | "outline" | "outline-color" | "outline-width" | "outline-style" => {
            let message = format!("Style decoration {key} is not supported.");
            if diagnostics.len() < DIAGNOSTICS && !diagnostics.contains(&message) {
                diagnostics.push(message);
            }
        }
        _ => return Err(unsupported()),
    }
    Ok(())
}
/** Supported lengths are finite positive CSS pixels or font-relative values. */
fn length(value: &str, font: f64) -> EngineResult<f64> {
    let value = value.trim();
    if value == "auto" {
        return Ok(0.0);
    }
    if let Some(value) = value.strip_suffix("rem") {
        return Ok(number(value)? * 16.0);
    }
    if let Some(value) = value.strip_suffix("em") {
        return Ok(number(value)? * font);
    }
    number(value.strip_suffix("px").unwrap_or(value))
}
/** Resource-safe numeric ranges exclude negative placement and extreme native geometry. */
fn number(value: &str) -> EngineResult<f64> {
    value
        .parse::<f64>()
        .ok()
        .filter(|n| n.is_finite() && (0.0..=4096.0).contains(n))
        .ok_or_else(unsupported)
}
/** CSS shorthand follows clockwise expansion. */
fn box_values(value: &str, font: f64) -> EngineResult<[f64; 4]> {
    let values = value
        .split_whitespace()
        .map(|s| length(s, font))
        .collect::<EngineResult<Vec<_>>>()?;
    match values.as_slice() {
        [a] => Ok([*a; 4]),
        [a, b] => Ok([*a, *b, *a, *b]),
        [a, b, c] => Ok([*a, *b, *c, *b]),
        [a, b, c, d] => Ok([*a, *b, *c, *d]),
        _ => Err(unsupported()),
    }
}
/** Native color parsing accepts only these bounded literal forms. */
fn color(value: &str) -> EngineResult<String> {
    if [
        "black",
        "white",
        "red",
        "green",
        "blue",
        "gray",
        "grey",
        "transparent",
        "navy",
        "teal",
    ]
    .contains(&value)
        || (value.starts_with('#')
            && [4, 7].contains(&value.len())
            && value[1..].bytes().all(|b| b.is_ascii_hexdigit()))
    {
        Ok(value.into())
    } else {
        Err(unsupported())
    }
}
/** Never include arbitrary declarations or selectors in an error that could contain secrets. */
fn unsupported() -> EngineError {
    EngineError::unsupported(
        "This page uses a CSS selector, condition, value or layout property Oya Native does not support yet.",
    )
}
