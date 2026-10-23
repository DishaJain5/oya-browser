/** Bounded type/class/ID selectors match only the owned document tree. */
use super::unsupported;
use crate::{
    document::{Document, Node},
    protocol::{EngineError, EngineResult},
};
/** Type/class/ID compounds and descendants are the complete supported selector grammar. */
pub(super) fn validate_selector(selector: &str) -> EngineResult<()> {
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
/** Virtual wrappers participate in ancestry without acquiring native node identities. */
pub(super) fn matches(document: &Document, node: &Node, selector: &str) -> EngineResult<bool> {
    let mut ancestry = vec![(node.tag.as_str(), &node.attributes)];
    let mut parent = node.parent;
    while let Some(id) = parent {
        let ancestor = document
            .nodes
            .iter()
            .find(|node| node.local_id == id)
            .ok_or_else(|| EngineError::invalid("Invalid document parent identity."))?;
        ancestry.push((ancestor.tag.as_str(), &ancestor.attributes));
        parent = ancestor.parent;
    }
    ancestry.push(("body", &document.formatting.body.attributes));
    ancestry.push(("html", &document.formatting.html.attributes));
    Ok(chain_matches(&ancestry, selector))
}
/** Wrapper selectors are resolved on their own element rather than on real root children. */
pub(super) fn matches_wrapper(document: &Document, tag: &str, selector: &str) -> bool {
    let mut ancestry = Vec::new();
    if tag == "body" {
        ancestry.push(("body", &document.formatting.body.attributes));
    }
    ancestry.push(("html", &document.formatting.html.attributes));
    chain_matches(&ancestry, selector)
}
/** Descendant combinators consume ancestors in order and never match the subject's parent instead. */
fn chain_matches(
    ancestry: &[(&str, &std::collections::BTreeMap<String, String>)],
    selector: &str,
) -> bool {
    let parts = selector.split_whitespace().collect::<Vec<_>>();
    let Some(subject) = parts.last() else {
        return false;
    };
    if !compound(ancestry[0].0, ancestry[0].1, subject) {
        return false;
    }
    let mut remaining = parts.len() - 1;
    for (tag, attributes) in &ancestry[1..] {
        if remaining > 0 && compound(tag, attributes, parts[remaining - 1]) {
            remaining -= 1;
        }
    }
    remaining == 0
}
/** One compound cannot escape into native selector dispatch. */
fn compound(
    tag: &str,
    attributes: &std::collections::BTreeMap<String, String>,
    source: &str,
) -> bool {
    let mut rest = source;
    let end = rest.find(['.', '#']).unwrap_or(rest.len());
    if !rest[..end].is_empty() && &rest[..end] != "*" && &rest[..end] != tag {
        return false;
    }
    rest = &rest[end..];
    while !rest.is_empty() {
        let prefix = rest.as_bytes()[0];
        rest = &rest[1..];
        let end = rest.find(['.', '#']).unwrap_or(rest.len());
        let name = &rest[..end];
        let matched = if prefix == b'#' {
            attributes.get("id").is_some_and(|id| id == name)
        } else {
            attributes
                .get("class")
                .is_some_and(|classes| classes.split_whitespace().any(|class| class == name))
        };
        if name.is_empty() || !matched {
            return false;
        }
        rest = &rest[end..];
    }
    true
}
/** This tuple-equivalent scalar leaves room for every bounded selector component. */
pub(super) fn specificity(selector: &str) -> usize {
    selector.matches('#').count() * 1_000_000
        + selector.matches('.').count() * 1000
        + selector
            .split_whitespace()
            .filter(|part| part.starts_with(char::is_alphabetic))
            .count()
}
