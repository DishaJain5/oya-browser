/** Oya owns point-based block flow, wrapping and control placement. */
mod body;
mod control;
mod flex;
mod flex_column;
mod flex_layout;
mod flow;
mod image;
mod sizing;
mod table;
mod text;
mod validation;
mod wrap;
use super::{Appearance, ColorScheme, Document, Node, RenderContext};
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
    /// Root or propagated body color covers the full document canvas, including empty viewport area.
    #[serde(default)]
    pub canvas_background: Option<String>,
    /// Scheme-sensitive text fallback remains consistent with the canvas color.
    #[serde(default)]
    pub canvas_foreground: Option<String>,
    /// Non-target body decoration remains separate when the root already supplies the canvas background.
    #[serde(default)]
    pub body_decoration: Option<DisplayDecoration>,
}

/// A formatting box can paint author decoration without inventing a DOM or agent identity.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DisplayDecoration {
    /// Logical coordinates share the same document origin as ordinary display items.
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// None permits a border without double-painting a propagated body background.
    pub color: Option<String>,
    pub border: f64,
    pub border_color: Option<String>,
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
    /// New displays carry the engine's used line advance; older snapshots retain the native default.
    #[serde(default)]
    pub line_height: Option<f64>,
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
    render_with_context(
        document,
        RenderContext {
            width: viewport,
            ..RenderContext::default()
        },
    )
}
/// Validated presentation facts drive layout and whole-canvas painting in one transaction.
pub fn render_with_context(
    document: &Document,
    context: RenderContext,
) -> EngineResult<DisplayList> {
    context.validate()?;
    super::validate_styles(document)?;
    let (items, height, body_decoration) = flow::layout(document, context)?;
    validate_decoration(body_decoration.as_ref())?;
    validate_geometry(&items, height)?;
    if items
        .iter()
        .map(|item| item.lines.len() + item.validation.len())
        .sum::<usize>()
        > PAINT_LINES
    {
        return Err(EngineError::limit("paint lines", PAINT_LINES));
    }
    let root = &document.formatting.html.style;
    let body = &document.formatting.body.style;
    let canvas_background = opaque(&root.background)
        .or_else(|| opaque(&body.background))
        .cloned()
        .or_else(|| {
            (root.color_scheme != ColorScheme::Normal).then(|| match root.used_scheme {
                Appearance::Light => "#ffffff".into(),
                Appearance::Dark => "#000000".into(),
            })
        });
    let scheme = if root.color_scheme == ColorScheme::Normal {
        body
    } else {
        root
    };
    let canvas_foreground = scheme.color.clone().or_else(|| default_foreground(scheme));
    Ok(DisplayList {
        unit: UNIT.into(),
        width: context.width,
        height,
        items,
        canvas_background,
        canvas_foreground,
        body_decoration,
    })
}
/** Decoration shares the finite geometry and validated literal colors of actionable display items. */
fn validate_decoration(decoration: Option<&DisplayDecoration>) -> EngineResult<()> {
    if let Some(decoration) = decoration {
        for value in [
            decoration.x,
            decoration.y,
            decoration.width,
            decoration.height,
            decoration.border,
        ] {
            validation::bounded(value)?;
        }
    }
    Ok(())
}
/** Transparent root paint delegates to the HTML body's canvas propagation rule. */
fn opaque(color: &Option<String>) -> Option<&String> {
    color
        .as_ref()
        .filter(|color| color.as_str() != "transparent")
}
/** Native semantic colors remain the fallback for an unstyled document. */
fn default_foreground(style: &super::ComputedStyle) -> Option<String> {
    (style.color_scheme != ColorScheme::Normal || opaque(&style.background).is_some()).then(|| {
        match style.used_scheme {
            Appearance::Light => "#000000".into(),
            Appearance::Dark => "#ffffff".into(),
        }
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

/** Private styles and arithmetic must never send NaN, infinity or excessive extents to AppKit. */
fn validate_geometry(items: &[DisplayItem], height: f64) -> EngineResult<()> {
    validation::bounded(height)?;
    for item in items {
        for value in [
            item.x,
            item.y,
            item.width,
            item.height,
            item.font_size,
            item.border,
        ] {
            validation::bounded(value)?;
        }
        if let Some(advance) = item.line_height {
            validation::bounded(advance)?;
            if advance <= 0.0 {
                return Err(EngineError::invalid("Invalid rendered line advance."));
            }
        }
        if let Some(height) = item.control_height {
            validation::bounded(height)?;
        }
        for line in item.lines.iter().chain(&item.validation) {
            for value in [line.x, line.y, line.width] {
                validation::bounded(value)?;
            }
        }
    }
    Ok(())
}
