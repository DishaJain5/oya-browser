/** Owned block and inline formatting lays out native controls and literal shaped text. */
pub(super) use super::{
    BLOCK_GAP, CONTENT_WIDTH, CONTROL_HEIGHT, DisplayItem, MARGIN, PaintLine, TABLE_MIN_ROW_HEIGHT,
    TABLE_PADDING,
};
pub(super) use crate::{
    document::{Document, Node},
    protocol::{EngineError, EngineResult},
};

/** Each containing block owns one inline cursor and measured line height. */
pub(super) struct Flow {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub cursor: f64,
    pub line: f64,
    pub bottom: f64,
}
impl Flow {
    /** A block starts at its content origin with no invented glyph width. */
    pub fn new(x: f64, y: f64, width: f64) -> Self {
        Self {
            x,
            y,
            width,
            cursor: 0.0,
            line: 0.0,
            bottom: y,
        }
    }
    /** A completed line advances only by the largest participating font. */
    pub fn newline(&mut self) {
        if self.cursor > 0.0 || self.line > 0.0 {
            self.y += self.line;
            self.bottom = self.bottom.max(self.y);
        }
        self.cursor = 0.0;
        self.line = 0.0;
    }
}
/** Top-level children share a virtual body formatting context and one request budget. */
pub(super) fn layout(
    document: &Document,
    presentation: crate::document::RenderContext,
) -> EngineResult<(Vec<DisplayItem>, f64, Option<super::DisplayDecoration>)> {
    let mut context = LayoutContext::new(document)?;
    let mut items = Vec::new();
    let body = &document.formatting.body.style;
    let authored = document.formatting.body_box_authored;
    let (x, y, width) = if authored {
        super::body::content(body, presentation.width)
    } else {
        (
            MARGIN,
            MARGIN,
            (presentation.width - MARGIN * 2.0).min(CONTENT_WIDTH),
        )
    };
    let mut flow = Flow::new(x, y, width);
    if !body.hidden {
        for node in document.nodes.iter().filter(|node| node.parent.is_none()) {
            node_layout(&mut context, node, &mut flow, &mut items, None)?;
        }
    }
    flow.newline();
    let bottom = if authored {
        body.padding[2] + body.border + body.margin[2]
    } else {
        MARGIN
    };
    let height = flow.bottom.max(flow.y) + bottom;
    let decoration = super::body::decoration(document, x, y, width, height - body.margin[2]);
    Ok((items, height, decoration))
}
/** Forced flex border boxes bypass authored sizing, while ordinary flow preserves its cursor. */
pub(super) fn node_layout(
    context: &mut LayoutContext<'_>,
    node: &Node,
    flow: &mut Flow,
    items: &mut Vec<DisplayItem>,
    forced: Option<BorderBox>,
) -> EngineResult<()> {
    context.charge(1)?;
    if node.hidden() {
        return Ok(());
    }
    if node.tag == "br" {
        flow.line = flow
            .line
            .max(node.style.line_height.used(node.style.font_size));
        flow.newline();
        return Ok(());
    }
    if node.tag == "img" {
        return super::image::image(context, node, flow, items, forced);
    }
    if node.tag == "table" {
        return super::table::table(context, node, flow, items);
    }
    if ["input", "button", "select", "textarea"].contains(&node.tag.as_str()) {
        return super::control::control(context, node, flow, items, forced);
    }
    if node.style.inline && !node.style.display_flex && forced.is_none() {
        return super::text::inline(context, node, flow, items);
    }
    block(context, node, flow, items, forced)
}
/** Blocks provide the same content origin to normal and flexible child formatting. */
fn block(
    context: &mut LayoutContext<'_>,
    node: &Node,
    flow: &mut Flow,
    items: &mut Vec<DisplayItem>,
    forced: Option<BorderBox>,
) -> EngineResult<()> {
    flow.newline();
    let style = &node.style;
    let y = forced
        .map(|bounds| bounds.y)
        .unwrap_or(flow.y + style.margin[0]);
    let width = forced.map(|bounds| bounds.width).unwrap_or_else(|| {
        border_width(
            node,
            flow.width,
            (flow.width - style.margin[1] - style.margin[3]).max(0.0),
            None,
        )
    });
    let x = forced
        .map(|bounds| bounds.x)
        .unwrap_or(flow.x + super::body::margin_start(style, flow.width, width));
    let mut inner = Flow::new(
        x + style.padding[3] + style.border,
        y + style.padding[0] + style.border,
        (width - horizontal_edges(node)).max(0.0),
    );
    let index = items.len();
    items.push(box_item(node, x, y, width, 0.0, Vec::new()));
    if node.style.display_flex {
        super::flex_layout::children(
            context,
            node,
            &mut inner,
            items,
            forced
                .and_then(|bounds| bounds.height)
                .map(|height| (height - vertical_edges(node)).max(0.0)),
        )?;
    } else {
        items[index].lines = super::text::text(context, &node.text, node, &mut inner)?;
        for child in context.children(node.local_id) {
            node_layout(context, child, &mut inner, items, None)?;
        }
    }
    inner.newline();
    super::text::align(context, node, &mut items[index..], inner.x, inner.width)?;
    let natural = (inner.bottom - y + style.padding[2] + style.border).max(vertical_edges(node));
    let height = forced.and_then(|bounds| bounds.height).unwrap_or(natural);
    items[index].height = height;
    flow.y = y
        + height
        + if forced.is_some() {
            0.0
        } else {
            style.margin[2] + BLOCK_GAP
        };
    flow.bottom = flow.bottom.max(flow.y);
    Ok(())
}
/** Every rendering collaborator uses the same forced-box and intrinsic sizing contract. */
pub(super) use super::sizing::{
    BorderBox, LayoutContext, border_width, horizontal_edges, vertical_edges,
};
/** One representation supplies both agent bounds and the native paint path. */
pub(super) fn box_item(
    node: &Node,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    lines: Vec<PaintLine>,
) -> DisplayItem {
    DisplayItem {
        local_id: node.local_id,
        tag: node.tag.clone(),
        role: super::role(node).into(),
        x,
        y,
        width,
        height,
        font_size: node.style.font_size,
        bold: node.style.bold,
        line_height: Some(node.style.line_height.used(node.style.font_size)),
        lines,
        color: node.style.color.clone().or_else(|| {
            if node.style.color_scheme == crate::document::ColorScheme::Normal {
                return None;
            }
            if node.href.is_some() {
                Some(
                    match node.style.used_scheme {
                        crate::document::Appearance::Light => "#0000ee",
                        crate::document::Appearance::Dark => "#9e9eff",
                    }
                    .into(),
                )
            } else {
                super::default_foreground(&node.style)
            }
        }),
        background: node.style.background.clone(),
        border: node.style.border,
        border_color: node.style.border_color.clone(),
        validation: Vec::new(),
        control_height: None,
    }
}
