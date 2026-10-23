/** Static source text has independent byte bounds while native controls and writes stay small. */
use super::{Document, Node, validate_text};
use crate::{
    constants::{DOCUMENT_DEPTH, ID_BYTES, NODES, STATIC_TEXT_NODE_BYTES, STATIC_TEXT_TOTAL_BYTES},
    protocol::{EngineError, EngineResult},
};
use std::collections::BTreeMap;

/** Validate the actual document before recursive labels, projection, serialization or measurement. */
pub fn validate_content(document: &Document) -> EngineResult<()> {
    validate_structure(document)?;
    validate_text(&document.title)?;
    for wrapper in [&document.formatting.html, &document.formatting.body] {
        for value in wrapper.attributes.values() {
            validate_text(value)?;
        }
    }
    let mut total = 0;
    for node in &document.nodes {
        validate_node(node)?;
        total = add_static_bytes(total, node.text.len())?;
    }
    validate_control_labels(document)
}

/** Parent-before-child ordering and bounded ancestry prevent forged cycles before label traversal. */
fn validate_structure(document: &Document) -> EngineResult<()> {
    if document.nodes.len() > NODES {
        return Err(EngineError::limit("document nodes", NODES));
    }
    let mut depths = BTreeMap::new();
    for node in &document.nodes {
        if node.local_id >= NODES || depths.contains_key(&node.local_id) {
            return Err(EngineError::invalid("Invalid document tree."));
        }
        let depth = match node.parent {
            Some(parent) => {
                depths
                    .get(&parent)
                    .copied()
                    .ok_or_else(|| EngineError::invalid("Invalid document tree."))?
                    + 1
            }
            None => 0,
        };
        if depth >= DOCUMENT_DEPTH {
            return Err(EngineError::limit("document depth", DOCUMENT_DEPTH));
        }
        depths.insert(node.local_id, depth);
    }
    Ok(())
}

/** Static text never relaxes initial field values, captions, metadata or decoded attributes. */
fn validate_node(node: &Node) -> EngineResult<()> {
    validate_node_text(node, node.text.len())?;
    if node.html_id.is_empty() || node.html_id.len() > ID_BYTES {
        return Err(EngineError::invalid("Invalid document identifier."));
    }
    for value in [&node.value, &node.placeholder, &node.image_alt] {
        validate_text(value)?;
    }
    for value in node.attributes.values() {
        validate_text(value)?;
    }
    for value in [&node.href, &node.handler, &node.validation, &node.image_src]
        .into_iter()
        .flatten()
    {
        validate_text(value)?;
    }
    Ok(())
}

/** The byte length is checked before parser appends allocate the combined source text. */
pub(super) fn validate_node_text(node: &Node, bytes: usize) -> EngineResult<()> {
    let limit = if ["input", "textarea", "select", "button", "option"].contains(&node.tag.as_str())
    {
        crate::constants::TEXT_BYTES
    } else {
        STATIC_TEXT_NODE_BYTES
    };
    if bytes > limit {
        return Err(EngineError::limit("node text bytes", limit));
    }
    Ok(())
}

/** Checked aggregate arithmetic counts stored text once, without duplicated derived labels. */
pub(super) fn add_static_bytes(total: usize, bytes: usize) -> EngineResult<usize> {
    total
        .checked_add(bytes)
        .filter(|total| *total <= STATIC_TEXT_TOTAL_BYTES)
        .ok_or_else(|| EngineError::limit("document text bytes", STATIC_TEXT_TOTAL_BYTES))
}

/** Descendant captions and associated labels are bounded before native control construction. */
fn validate_control_labels(document: &Document) -> EngineResult<()> {
    for node in &document.nodes {
        if ["input", "textarea", "select", "button", "option"].contains(&node.tag.as_str()) {
            validate_text(&super::label(document, node))?;
            if ["button", "option"].contains(&node.tag.as_str()) {
                validate_text(&caption(document, node))?;
            }
        }
    }
    Ok(())
}

/** Caption extraction follows only validated ancestry and omits unrelated editable field values. */
fn caption(document: &Document, node: &Node) -> String {
    let mut text = node.text.clone();
    for child in document
        .nodes
        .iter()
        .filter(|child| child.parent == Some(node.local_id))
    {
        if !["input", "textarea", "select", "button"].contains(&child.tag.as_str()) {
            text.push(' ');
            text.push_str(&caption(document, child));
        }
    }
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}
