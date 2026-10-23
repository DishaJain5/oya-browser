mod flow;
/** Oya owns point-based block flow, wrapping and control placement. */
mod wrap;
use super::{Document, Node};
use crate::protocol::{EngineError, EngineResult};
use serde::{Deserialize, Serialize};

/// Default document margin in logical points, shared by paint and inspection.
pub const MARGIN: f64 = 32.0;
/// Readable text does not stretch across arbitrarily wide windows.
pub const CONTENT_WIDTH: f64 = 960.0;
/// Paragraph and control separation is independent of glyph metrics.
pub const BLOCK_GAP: f64 = 12.0;
/// The default body font is a native system font.
/// Native inputs/buttons retain comfortable desktop targets.
pub const CONTROL_HEIGHT: f64 = 32.0;
/// Table cells preserve readable separation without platform-specific widgets.
pub const TABLE_PADDING: f64 = 8.0;
/// Empty cells still receive a usable pointer and reading target.
pub const TABLE_MIN_ROW_HEIGHT: f64 = 32.0;
/// Supported viewport bounds constrain layout work and geometry.
pub const MIN_VIEWPORT: f64 = 280.0;
/// Bounds snapshots even for programmatic window requests.
pub const MAX_VIEWPORT: f64 = 4096.0;
/// Point geometry is explicitly different from the legacy abstract text layout.
pub const UNIT: &str = "logical_points";
/** A bounded display prevents tiny glyph runs from creating unbounded native paint work. */
const PAINT_LINES: usize = 2048;

/// One immutable display list is consumed by native painting and agent inspection.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DisplayList {
    /// Top-left-origin logical-point geometry.
    pub unit: String,
    /// Viewport width used for this layout generation.
    pub width: f64,
    /// Full document height, before viewport scrolling.
    pub height: f64,
    /// Element boxes retain current document-local identities.
    pub items: Vec<DisplayItem>,
}

/// Native controls and text drawing use exactly these boxes.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DisplayItem {
    /// Stable native identity within one page generation.
    pub local_id: usize,
    /// Supported element tag, never a native class name supplied by the page.
    pub tag: String,
    /// Semantic accessible role.
    pub role: String,
    /// Left edge in document coordinates.
    pub x: f64,
    /// Top edge in document coordinates.
    pub y: f64,
    /// Box width in logical points.
    pub width: f64,
    /// Box height in logical points.
    pub height: f64,
    /// Font selection chosen by owned element defaults.
    pub font_size: f64,
    /// Heading weight used for both measuring and drawing.
    pub bold: bool,
    /// Individually measured strings prevent a platform widget from choosing different wrapping.
    pub lines: Vec<PaintLine>,
    /// Literal colors come only from the owned CSS validator.
    #[serde(default)]
    pub color: Option<String>,
    /// Background and border use the exact same engine box as targeting.
    #[serde(default)]
    pub background: Option<String>,
    #[serde(default)]
    pub border: f64,
    #[serde(default)]
    pub border_color: Option<String>,
    /// Error text belongs to this exact control box and never contains its value.
    #[serde(default)]
    pub validation: Vec<PaintLine>,
    /// Error rows extend the engine box without stretching the native control itself.
    #[serde(default)]
    pub control_height: Option<f64>,
}

/// Each line preserves Unicode text and its exact measured native position.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PaintLine {
    /// Literal rendered text, never executable source.
    pub text: String,
    /// Left edge in document coordinates.
    pub x: f64,
    /// Top edge in document coordinates.
    pub y: f64,
    /// Shaped width of this exact string/font combination.
    pub width: f64,
}

/// Produces bounded native geometry without delegating HTML or flow to AppKit.
pub fn render(document: &Document, viewport: f64) -> EngineResult<DisplayList> {
    if !viewport.is_finite() || !(MIN_VIEWPORT..=MAX_VIEWPORT).contains(&viewport) {
        return Err(EngineError::invalid(
            "Viewport width is outside supported logical-point bounds.",
        ));
    }
    let (items, y) = flow::layout(document, viewport);
    if items.iter().map(|item| item.lines.len()).sum::<usize>() > PAINT_LINES {
        return Err(EngineError::limit("paint lines", PAINT_LINES));
    }
    Ok(DisplayList {
        unit: UNIT.into(),
        width: viewport,
        height: y + MARGIN,
        items,
    })
}

/// Semantic roles expose the same interactions as the visible native controls.
pub fn role(node: &Node) -> &'static str {
    match node.tag.as_str() {
        "h1" | "h2" | "h3" => "heading",
        "table" => "table",
        "tr" => "row",
        "td" => "cell",
        "th" if node
            .attributes
            .get("scope")
            .is_some_and(|scope| scope == "row") =>
        {
            "rowheader"
        }
        "th" => "columnheader",
        "caption" => "caption",
        "thead" | "tbody" | "tfoot" => "rowgroup",
        "div" | "form" | "section" | "main" | "fieldset" => "group",
        "a" if node.href.is_some() => "link",
        "img" => "image",
        "input" if node.input_type() == "password" => "password",
        "input" if node.input_type() == "checkbox" => "checkbox",
        "input" if node.input_type() == "radio" => "radio",
        "input" if node.input_type() == "submit" => "button",
        "input" | "textarea" => "textbox",
        "select" => "combobox",
        "option" => "option",
        "button" => "button",
        _ => "text",
    }
}
