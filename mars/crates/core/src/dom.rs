/** Inert bounded document projections and text proposals share one identity across engines. */
use crate::{
    constants::{DOCUMENT_DEPTH, ID_BYTES, NODES, SCRIPT_BYTES, TEXT_BYTES},
    protocol::{EngineError, EngineResult},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Script output is data; it grants no agent or networking capabilities.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TextChange {
    /// Source HTML identifier, resolved against this candidate only.
    pub html_id: String,
    /// New text, validated before any change is applied.
    pub text: String,
}

/// Script-visible state carries no native handles, credentials or protected field values.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScriptDocument {
    /// Coordinator-selected page identity prevents references surviving navigation or tab changes.
    pub identity: String,
    /// Preorder nodes retain their original identities after descendant replacement.
    pub nodes: Vec<ScriptNode>,
}

/// Supported element capabilities are computed from the real owned document.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScriptNode {
    /// Stable local identity is inert data and grants no native control permission.
    pub local_id: usize,
    /// Actual author identifier; missing identifiers remain empty rather than synthetic selectors.
    pub html_id: String,
    /// Parent precedes its children and remains stable through text replacement.
    pub parent: Option<usize>,
    /// Only nonsecret input and textarea values may enter the bounded untainted interpreter.
    pub value: Option<String>,
    /// Mirrors the document mutation validator rather than inventing another target policy.
    pub text_writable: bool,
    /// Only enabled visible explicit ordinary buttons accept programmatic click listeners.
    pub click_listener: bool,
    /// Existing supported inline button source participates in the same click transaction.
    pub inline_handler: Option<String>,
}

impl ScriptDocument {
    /// Private wire snapshots are bounded trees, including fields not currently referenced by JS.
    pub fn validate(&self) -> EngineResult<()> {
        if self.identity.is_empty() || self.identity.len() > ID_BYTES {
            return Err(invalid_projection());
        }
        if self.nodes.len() > NODES {
            return Err(EngineError::limit("document nodes", NODES));
        }
        let mut seen = BTreeSet::new();
        for node in &self.nodes {
            self.validate_node(node, &seen)?;
            seen.insert(node.local_id);
        }
        Ok(())
    }

    /** Parent ordering removes cycles before any interpreter traversal can occur. */
    fn validate_node(&self, node: &ScriptNode, seen: &BTreeSet<usize>) -> EngineResult<()> {
        if node.local_id >= NODES
            || seen.contains(&node.local_id)
            || node.html_id.len() > ID_BYTES
            || node.parent.is_some_and(|parent| !seen.contains(&parent))
            || node
                .inline_handler
                .as_ref()
                .is_some_and(|source| source.len() > SCRIPT_BYTES)
        {
            return Err(invalid_projection());
        }
        if let Some(value) = &node.value {
            validate_text(value)?;
        }
        self.validate_depth(node)
    }

    /** The ordinary document depth limit also bounds retained projected ancestry. */
    fn validate_depth(&self, node: &ScriptNode) -> EngineResult<()> {
        let mut parent = node.parent;
        let mut depth = 0;
        while let Some(id) = parent {
            depth += 1;
            if depth >= DOCUMENT_DEPTH {
                return Err(EngineError::limit("document depth", DOCUMENT_DEPTH));
            }
            parent = self
                .nodes
                .iter()
                .find(|node| node.local_id == id)
                .and_then(|node| node.parent);
        }
        Ok(())
    }

    /// Empty identifiers match nothing and duplicates resolve in current document order.
    pub fn get_by_id(&self, html_id: &str) -> Option<usize> {
        (!html_id.is_empty())
            .then(|| self.nodes.iter().find(|node| node.html_id == html_id))
            .flatten()
            .map(|node| node.local_id)
    }

    /// Replaces descendants immediately so later reads observe the same candidate tree.
    pub fn set_text(&mut self, local_id: usize, text: impl AsRef<str>) -> EngineResult<TextChange> {
        let text = text.as_ref();
        validate_text(text)?;
        let node = self
            .nodes
            .iter()
            .find(|node| node.local_id == local_id)
            .ok_or_else(invalid_projection)?;
        if !node.text_writable || self.get_by_id(&node.html_id) != Some(local_id) {
            return Err(EngineError::unsupported(
                "textContent requires a supported element with its first matching nonempty ID.",
            ));
        }
        let change = TextChange {
            html_id: node.html_id.clone(),
            text: text.into(),
        };
        let removed = descendant_ids(
            self.nodes.iter().map(|node| (node.local_id, node.parent)),
            local_id,
        );
        self.nodes.retain(|node| !removed.contains(&node.local_id));
        Ok(change)
    }
}

/** Hostile private state is rejected without exposing values or native identities. */
fn invalid_projection() -> EngineError {
    EngineError::invalid("Invalid script document projection.")
}

/** Both representations remove the same preorder descendant identities synchronously. */
pub fn descendant_ids(
    nodes: impl Iterator<Item = (usize, Option<usize>)>,
    parent: usize,
) -> Vec<usize> {
    let mut removed = vec![parent];
    for (id, parent) in nodes {
        if parent.is_some_and(|parent| removed.contains(&parent)) {
            removed.push(id);
        }
    }
    removed.remove(0);
    removed
}

/// Bounds decoded text, not just the source representation.
pub fn validate_text(text: &str) -> EngineResult<()> {
    if text.len() > TEXT_BYTES {
        return Err(EngineError::limit("node text bytes", TEXT_BYTES));
    }
    Ok(())
}
