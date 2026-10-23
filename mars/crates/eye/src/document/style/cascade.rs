/** Ordered stylesheet and inline declarations select one bounded computed candidate. */
use super::{
    ComputedStyle,
    apply::apply,
    selectors::{matches, matches_wrapper},
    syntax::{Rule, declarations, parse_sheet},
    unsupported,
};
use crate::{
    constants::{STYLE_BYTES, STYLE_SHEETS},
    document::{Document, Node, RenderContext},
    protocol::{EngineError, EngineResult},
};
use std::collections::BTreeMap;
/** Cascade priority sorts importance, inline origin, specificity and source order. */
type Priority = (bool, bool, usize, usize);
/// Required sheets must all parse before a candidate's actions become visible.
pub fn compute_styles(document: &Document, sheets: &[String]) -> EngineResult<Document> {
    compute_styles_with_context(document, sheets, RenderContext::default())
}
/** Viewport and appearance-dependent values are recomputed from retained source declarations. */
pub fn compute_styles_with_context(
    document: &Document,
    sheets: &[String],
    context: RenderContext,
) -> EngineResult<Document> {
    context.validate()?;
    crate::document::validate_styles(document)?;
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
    wrappers(document, &mut candidate, &rules, context)?;
    for index in 0..candidate.nodes.len() {
        let node = &document.nodes[index];
        let parent = node
            .parent
            .and_then(|id| candidate.nodes.iter().find(|node| node.local_id == id))
            .map(|node| &node.style)
            .unwrap_or(&candidate.formatting.body.style);
        let mut style = super::resolve::inherited(parent);
        defaults(node, &mut style);
        let chosen = winning(&rules, &node.attributes, |selector| {
            matches(document, node, selector)
        })?;
        let default_link_color = node.href.is_some() && !chosen.contains_key("color");
        if chosen.contains_key("font-size") {
            style.font_size = parent.font_size;
        }
        resolve(&mut style, chosen, context, &mut candidate.diagnostics)?;
        if default_link_color {
            style.color = super::resolve::link_foreground(&style);
        }
        if style.margin_auto.iter().any(|auto| *auto)
            && (parent.display_flex
                || [
                    "table", "caption", "thead", "tbody", "tfoot", "tr", "th", "td",
                ]
                .contains(&node.tag.as_str()))
        {
            return Err(unsupported());
        }
        style.hidden |= node.attributes.contains_key("hidden")
            || node.tag == "option"
            || (node.tag == "input" && node.input_type() == "hidden");
        style.validate()?;
        candidate.nodes[index].style = style;
    }
    Ok(candidate)
}
/** Root and body are formatting subjects, never surrogate declarations on actual children. */
fn wrappers(
    document: &Document,
    candidate: &mut Document,
    rules: &[Rule],
    context: RenderContext,
) -> EngineResult<()> {
    let html = winning(rules, &document.formatting.html.attributes, |selector| {
        Ok(matches_wrapper(document, "html", selector))
    })?;
    if html.keys().any(|key| body_box_key(key)) {
        return Err(unsupported());
    }
    let mut style = ComputedStyle::default();
    resolve(&mut style, html, context, &mut candidate.diagnostics)?;
    style.hidden |= document.formatting.html.attributes.contains_key("hidden");
    if style.inline || style.display_flex {
        return Err(unsupported());
    }
    candidate.formatting.html.style = style.clone();
    let body = winning(rules, &document.formatting.body.attributes, |selector| {
        Ok(matches_wrapper(document, "body", selector))
    })?;
    candidate.formatting.body_box_authored = body.keys().any(|key| body_box_key(key));
    let mut style = super::resolve::inherited(&style);
    resolve(&mut style, body, context, &mut candidate.diagnostics)?;
    style.hidden |= document.formatting.body.attributes.contains_key("hidden");
    if style.inline || style.display_flex {
        return Err(unsupported());
    }
    candidate.formatting.body.style = style;
    Ok(())
}
/** Only winning body box declarations opt into authored geometry. */
fn body_box_key(key: &str) -> bool {
    key.starts_with("margin-")
        || key.starts_with("padding-")
        || ["border", "border-width", "border-color", "border-style"].contains(&key)
        || ["width", "min-width", "max-width", "box-sizing"].contains(&key)
}
/** Existing native defaults remain separate from inherited author properties. */
fn defaults(node: &Node, style: &mut ComputedStyle) {
    style.inline = [
        "span", "a", "strong", "em", "b", "i", "small", "code", "#text", "img",
    ]
    .contains(&node.tag.as_str());
    style.bold |= ["strong", "b", "h1", "h2", "h3", "th"].contains(&node.tag.as_str());
    if let Some((_, size)) = [("h1", 28.0), ("h2", 22.0), ("h3", 18.0)]
        .iter()
        .find(|(tag, _)| *tag == node.tag)
    {
        style.font_size = *size;
    }
}
/** Each longhand has one cascade winner before dependent values are resolved. */
fn winning(
    rules: &[Rule],
    attributes: &BTreeMap<String, String>,
    matches: impl Fn(&str) -> EngineResult<bool>,
) -> EngineResult<BTreeMap<String, (Priority, String)>> {
    let mut chosen = BTreeMap::new();
    for rule in rules {
        if matches(&rule.selector)? {
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
    if let Some(inline) = attributes.get("style") {
        for (key, value, important) in declarations(inline)? {
            choose(&mut chosen, &key, &value, (important, true, 0, rules.len()));
        }
    }
    Ok(chosen)
}
/** Font and scheme dependencies precede geometry, line height and light-dark color conversion. */
fn resolve(
    style: &mut ComputedStyle,
    mut chosen: BTreeMap<String, (Priority, String)>,
    context: RenderContext,
    diagnostics: &mut Vec<String>,
) -> EngineResult<()> {
    for key in ["font-size", "color-scheme"] {
        if let Some((_, value)) = chosen.remove(key) {
            apply(style, key, &value, context, diagnostics)?;
        }
    }
    style.used_scheme = super::resolve::used_scheme(style.color_scheme, context.appearance);
    for (key, (_, value)) in chosen {
        apply(style, &key, &value, context, diagnostics)?;
    }
    if style.color.is_none()
        && style
            .background
            .as_deref()
            .is_some_and(|color| color != "transparent")
    {
        style.color = Some(
            match style.used_scheme {
                crate::document::Appearance::Light => "#000000",
                crate::document::Appearance::Dark => "#ffffff",
            }
            .into(),
        );
    }
    style.validate()
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
