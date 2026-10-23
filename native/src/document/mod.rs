mod forms;
/** Oya's own bounded document model and character-cell layout, not Blink. */
mod markup;
mod parser;
mod render;
mod style;
mod table;
mod tokenizer;
use crate::constants::{LAYOUT_UNIT, TEXT_BYTES};
use crate::protocol::{EngineError, EngineResult};
pub use forms::{Submission, form_owner, label, select, set_checked, submission};
pub use parser::parse;
pub use render::{DisplayItem, DisplayList, PaintLine, render, role};
use serde::{Deserialize, Serialize};
pub use style::{ComputedStyle, StyleSource, compute_styles};

/// A document candidate is published only after all script changes validate.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Document {
    /// Source order determines deterministic layout order.
    pub nodes: Vec<Node>,
    /// Ordered classic programs are interpreted by V9, never by the host runtime.
    #[serde(default)]
    pub scripts: Vec<ScriptResource>,
    /// Metadata belongs to the document, not an extra page view.
    #[serde(default)]
    pub title: String,
    /// Required CSS sources retain source order across inline and linked sheets.
    #[serde(default)]
    pub styles: Vec<StyleSource>,
    /// Value-free compatibility diagnostics remain bounded.
    #[serde(default)]
    pub diagnostics: Vec<String>,
}

/// Page scripts retain order while the coordinator resolves external source bytes.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScriptResource {
    /// Inline code or the resolved body of an accepted external classic script.
    pub source: String,
    /// External location remains private coordinator data until fetched.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub src: Option<String>,
}

/// Native node identity is separate from its source HTML identifier.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Node {
    /// Document-local identity; the coordinator adds the page generation.
    pub local_id: usize,
    /// Used only by the supported script DOM operation.
    pub html_id: String,
    /// One of the supported text element tags.
    pub tag: String,
    /// Decoded UTF-8 text, with a bound shared by every mutation path.
    pub text: String,
    /// Preorder tree relationships retain native identities during text replacement.
    #[serde(default)]
    pub parent: Option<usize>,
    /// Input values remain literal data, independent of textContent.
    #[serde(default)]
    pub value: String,
    /// Relative addresses are resolved by the runtime against its committed URL.
    #[serde(default)]
    pub href: Option<String>,
    /// Supported V9 event source has no access to native agent authority.
    #[serde(default)]
    pub handler: Option<String>,
    /// Both native fields and agent inspection use this placeholder.
    #[serde(default)]
    pub placeholder: String,
    /// Disabled controls reject every input path.
    #[serde(default)]
    pub disabled: bool,
    /// Inert author attributes never select native classes, callbacks or authority.
    #[serde(default)]
    pub attributes: std::collections::BTreeMap<String, String>,
    /// Current control state is independent of the initial source attributes.
    #[serde(default)]
    pub checked: bool,
    /// Options retain current selection independently of source markup.
    #[serde(default)]
    pub selected: bool,
    /// Owned CSS supplies both paint and action visibility.
    #[serde(default, skip_serializing_if = "default_style")]
    pub style: ComputedStyle,
    /// Only implemented constraints produce a value-free associated error.
    #[serde(default)]
    pub validation: Option<String>,
    /// Image address remains private document data until governed navigation fetches it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_src: Option<String>,
    /// Alternative text is visible and inspectable when image bytes are unavailable.
    #[serde(default)]
    pub image_alt: String,
    /// Intrinsic dimensions are set by the coordinator after bounded decode.
    #[serde(default)]
    pub image_size: Option<(u32, u32)>,
    /// Parsed cell relationships let inspection use the same stable native identities.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub table_cell: Option<TableCell>,
}

/// Table relationships are derived from bounded source order during parsing.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TableCell {
    /// Stable page-local table owner identity.
    pub table: usize,
    /// Stable page-local row owner identity.
    pub row: usize,
    /// One-based row position within the table.
    pub row_index: usize,
    /// One-based column position within the row.
    pub column_index: usize,
    /// Header node identities associated with this cell.
    pub headers: Vec<usize>,
}
/** Default style metadata need not consume private wire budget for every unstyled node. */
fn default_style(style: &ComputedStyle) -> bool {
    *style == ComputedStyle::default()
}
impl Node {
    /// Control semantics are explicit rather than inferred by AppKit.
    pub fn input_type(&self) -> &str {
        self.attributes
            .get("type")
            .map(String::as_str)
            .unwrap_or(if self.tag == "button" {
                "submit"
            } else {
                "text"
            })
    }
    /// Protected and hidden values stay inside the private document boundary.
    pub fn secret(&self) -> bool {
        self.tag == "input" && ["password", "hidden"].contains(&self.input_type())
    }
    /// Inherited visibility is resolved before native targeting.
    pub fn hidden(&self) -> bool {
        self.style.hidden
            || self.attributes.contains_key("hidden")
            || (self.tag == "input" && self.input_type() == "hidden")
    }
    /// Read-only fields submit but refuse editing.
    pub fn readonly(&self) -> bool {
        self.attributes.contains_key("readonly")
    }
}

/// Script output is data; it grants no agent or networking capabilities.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TextChange {
    /// Source HTML identifier, resolved against this candidate only.
    pub html_id: String,
    /// New text, validated before any change is applied.
    pub text: String,
}

/// Native layout uses named units so callers cannot mistake it for pixels.
#[derive(Debug, Serialize)]
pub struct Layout {
    /// Unicode scalar count, without font metrics or terminal width inference.
    pub unit: &'static str,
    /// One row per source element.
    pub rows: Vec<Row>,
}

/// One bounded text element's geometry.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Row {
    /// Retains identity for agent targeting.
    pub local_id: usize,
    /// All rows start in the first column in this subset.
    pub x: usize,
    /// Source-order row number.
    pub y: usize,
    /// Number of Unicode scalar values in the text.
    pub width: usize,
    /// Every text element occupies one abstract row.
    pub height: usize,
}

/// An invalid change leaves the entire input document unchanged.
pub fn apply(document: &Document, changes: &[TextChange]) -> EngineResult<Document> {
    for change in changes {
        validate_change(document, change)?;
    }
    let mut candidate = document.clone();
    for change in changes {
        let local = candidate
            .nodes
            .iter()
            .find(|node| node.html_id == change.html_id)
            .map(|node| node.local_id)
            .ok_or_else(invalid_target)?;
        let removed = descendants(&candidate, local);
        candidate
            .nodes
            .retain(|node| !removed.contains(&node.local_id));
        candidate
            .nodes
            .iter_mut()
            .find(|node| node.local_id == local)
            .ok_or_else(invalid_target)?
            .text
            .clone_from(&change.text);
    }
    Ok(candidate)
}

/** Every mutation shares the parser's text and exact-document target limits. */
fn validate_change(document: &Document, change: &TextChange) -> EngineResult<()> {
    validate_text(&change.text)?;
    let node = document
        .nodes
        .iter()
        .find(|node| node.html_id == change.html_id)
        .ok_or_else(invalid_target)?;
    if ["input", "br"].contains(&node.tag.as_str()) {
        return Err(EngineError::unsupported(
            "textContent cannot replace this control.",
        ));
    }
    Ok(())
}

/// Value editing never changes native identity or executes input text.
pub fn set_value(document: &Document, local: usize, value: &str) -> EngineResult<Document> {
    validate_text(value)?;
    let mut candidate = document.clone();
    let node = candidate
        .nodes
        .iter_mut()
        .find(|node| node.local_id == local)
        .ok_or_else(invalid_target)?;
    if !["input", "textarea"].contains(&node.tag.as_str())
        || node.disabled
        || node.hidden()
        || node.readonly()
        || (node.tag == "input"
            && !["text", "password", "email", "search", "url", "tel"].contains(&node.input_type()))
    {
        return Err(EngineError::new(
            "invalid_target",
            "The target is not an enabled input.",
            "Inspect and choose an editable input.",
        ));
    }
    node.value = value.into();
    node.validation = None;
    Ok(candidate)
}

/** Descendant replacement is bounded and never reuses deleted local identities. */
fn descendants(document: &Document, parent: usize) -> Vec<usize> {
    let mut removed = vec![parent];
    for node in &document.nodes {
        if node.parent.is_some_and(|parent| removed.contains(&parent)) {
            removed.push(node.local_id);
        }
    }
    removed.remove(0);
    removed
}

/** All mutation paths resolve exactly one current-document target. */
fn invalid_target() -> EngineError {
    EngineError::new(
        "invalid_target",
        "The node is not part of the current document.",
        "Inspect the current page and use a returned node ID.",
    )
}

/// Bounds decoded text, not just the source representation.
pub fn validate_text(text: &str) -> EngineResult<()> {
    if text.len() > TEXT_BYTES {
        return Err(EngineError::limit("node text bytes", TEXT_BYTES));
    }
    Ok(())
}

/// Produces repeatable geometry without pretending to implement CSS.
pub fn layout(document: &Document) -> Layout {
    let rows = document
        .nodes
        .iter()
        .enumerate()
        .map(|(y, node)| Row {
            local_id: node.local_id,
            x: 0,
            y,
            width: node.text.chars().count(),
            height: 1,
        })
        .collect();
    Layout {
        unit: LAYOUT_UNIT,
        rows,
    }
}
