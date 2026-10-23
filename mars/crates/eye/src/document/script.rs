/** Eye derives inert script capabilities only from the actual validated document. */
use super::{Document, Node, ScriptDocument, ScriptNode, text_writable};
use crate::protocol::EngineResult;

/// Derives a fresh snapshot from actual nodes, never from script-provided capabilities.
pub fn script_document(document: &Document, identity: &str) -> EngineResult<ScriptDocument> {
    super::validate_content(document)?;
    let projection = ScriptDocument {
        identity: identity.into(),
        nodes: document
            .nodes
            .iter()
            .map(|node| project_node(document, node))
            .collect(),
    };
    projection.validate()?;
    Ok(projection)
}

/** Explicit author identifiers cannot expose the parser's synthetic private targeting IDs. */
fn project_node(document: &Document, node: &Node) -> ScriptNode {
    ScriptNode {
        local_id: node.local_id,
        html_id: node.attributes.get("id").cloned().unwrap_or_default(),
        parent: node.parent,
        value: (["input", "textarea"].contains(&node.tag.as_str()) && !node.secret())
            .then(|| node.value.clone()),
        text_writable: text_writable(node)
            && document
                .nodes
                .iter()
                .find(|candidate| candidate.html_id == node.html_id)
                .is_some_and(|candidate| candidate.local_id == node.local_id),
        click_listener: click_listener(node),
        inline_handler: node.handler.clone(),
    }
}

/** Native submission and command behavior remain outside the supported listener subset. */
fn click_listener(node: &Node) -> bool {
    node.tag == "button"
        && node
            .attributes
            .get("type")
            .is_some_and(|kind| kind.eq_ignore_ascii_case("button"))
        && !node.disabled
        && !node.hidden()
        && node.style.interactive
        && ![
            "command",
            "commandfor",
            "popovertarget",
            "popovertargetaction",
        ]
        .iter()
        .any(|name| node.attributes.contains_key(*name))
}
