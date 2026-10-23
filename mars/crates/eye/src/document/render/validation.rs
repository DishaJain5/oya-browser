/** Tree and geometry bounds are checked before recursion and before native publication. */
use crate::{
    constants::{DOCUMENT_DEPTH, LAYOUT_EXTENT, NODES},
    document::Document,
    protocol::{EngineError, EngineResult},
};
use std::collections::BTreeSet;
/** Parent-before-child ordering and a depth cap reject cycles before recursion. */
pub(super) fn tree(document: &Document) -> EngineResult<()> {
    crate::document::validate_content(document)?;
    if document.nodes.len() > NODES {
        return Err(EngineError::limit("layout nodes", NODES));
    }
    let mut seen = BTreeSet::new();
    for node in &document.nodes {
        validate_auto_margins(document, node)?;
        if node.style.display_flex
            && [
                "table", "caption", "thead", "tbody", "tfoot", "tr", "th", "td", "input", "button",
                "select", "textarea",
            ]
            .contains(&node.tag.as_str())
        {
            return Err(EngineError::unsupported(
                "Flex layout on table internals or native controls is not supported.",
            ));
        }
        if node.local_id >= NODES
            || !seen.insert(node.local_id)
            || node
                .parent
                .is_some_and(|id| id == node.local_id || !seen.contains(&id))
        {
            return Err(EngineError::invalid("Invalid layout tree."));
        }
        let mut depth = 0;
        let mut parent = node.parent;
        while let Some(id) = parent {
            depth += 1;
            if depth >= DOCUMENT_DEPTH {
                return Err(EngineError::limit("layout depth", DOCUMENT_DEPTH));
            }
            let ancestor = document.nodes.iter().find(|node| node.local_id == id);
            if node.style.display_flex && ancestor.is_some_and(|node| node.tag == "table") {
                return Err(EngineError::unsupported(
                    "Flex layout inside tables is not supported.",
                ));
            }
            parent = ancestor.and_then(|node| node.parent);
        }
    }
    Ok(())
}
/** Finite coordinate bounds are checked before a candidate reaches native painting. */
pub(super) fn bounded(value: f64) -> EngineResult<()> {
    if !(0.0..=LAYOUT_EXTENT).contains(&value) {
        return Err(EngineError::limit("layout extent", LAYOUT_EXTENT as usize));
    }
    Ok(())
}

/** Unsupported auto-margin formatting cannot silently expose incorrectly placed native targets. */
fn validate_auto_margins(document: &Document, node: &crate::document::Node) -> EngineResult<()> {
    if !node.style.margin_auto.into_iter().any(|auto| auto) {
        return Ok(());
    }
    let table = [
        "table", "caption", "thead", "tbody", "tfoot", "tr", "th", "td",
    ]
    .contains(&node.tag.as_str());
    let flex_child = node
        .parent
        .and_then(|id| document.nodes.iter().find(|parent| parent.local_id == id))
        .is_some_and(|parent| parent.style.display_flex);
    if table || flex_child {
        return Err(EngineError::unsupported(
            "Automatic margins on flex items or table structures are not supported.",
        ));
    }
    Ok(())
}
