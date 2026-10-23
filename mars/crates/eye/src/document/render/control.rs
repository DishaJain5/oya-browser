/** Native controls receive the exact border boxes returned by owned layout. */
use super::flow::*;
/** Native controls occupy engine boxes; AppKit never lays out their surrounding HTML. */
pub(super) fn control(
    context: &mut LayoutContext<'_>,
    node: &Node,
    flow: &mut Flow,
    items: &mut Vec<DisplayItem>,
    forced: Option<BorderBox>,
) -> EngineResult<()> {
    let document = context.document;
    flow.newline();
    let style = &node.style;
    let y = forced
        .map(|bounds| bounds.y)
        .unwrap_or(flow.y + style.margin[0]);
    let available = (flow.width - style.margin[1] - style.margin[3]).max(1.0);
    let button = node.tag == "button"
        || (node.tag == "input" && ["submit", "button"].contains(&node.input_type()));
    let title = if node.tag == "input" && button {
        if node.value.is_empty() {
            "Submit".into()
        } else {
            node.value.clone()
        }
    } else if node.tag == "select" {
        document
            .nodes
            .iter()
            .find(|n| n.parent == Some(node.local_id) && n.selected)
            .map(|n| n.text.clone())
            .unwrap_or_default()
    } else {
        crate::document::label(document, node)
    };
    let natural = if button {
        (context.measure(&title, style.font_size, style.bold)? + 32.0).max(120.0)
    } else {
        480.0
    };
    let width = forced
        .map(|bounds| bounds.width)
        .unwrap_or_else(|| border_width(node, flow.width, available, Some(natural)));
    let x = forced
        .map(|bounds| bounds.x)
        .unwrap_or(flow.x + super::body::margin_start(style, flow.width, width));
    let mut local = Flow::new(
        x + style.padding[3] + style.border + 8.0,
        y + style.padding[0] + style.border + 4.0,
        (width - horizontal_edges(node) - 16.0).max(1.0),
    );
    let lines = if button {
        super::text::text(context, &title, node, &mut local)?
    } else {
        Vec::new()
    };
    let height = if node.tag == "textarea" {
        (style.line_height.used(style.font_size) * 3.0).max(CONTROL_HEIGHT)
    } else {
        CONTROL_HEIGHT.max(local.bottom - y - style.padding[0] - style.border + 4.0)
    };
    let natural_height = height + vertical_edges(node);
    let mut validation = Vec::new();
    let mut validation_extent = 0.0;
    if let Some(error) = &node.validation {
        let mut error_flow = Flow::new(x, y + natural_height + 2.0, width);
        validation = super::text::text(context, error, node, &mut error_flow)?;
        validation_extent = error_flow.bottom - (y + natural_height);
    }
    let total = forced
        .and_then(|bounds| bounds.height)
        .unwrap_or(natural_height + validation_extent);
    let control_height = (total - validation_extent).max(0.0);
    for line in &mut validation {
        line.y += control_height - natural_height;
    }
    let mut item = box_item(node, x, y, width, total, lines);
    item.control_height = Some(control_height);
    item.validation = validation;
    let total_height = item.height;
    items.push(item);
    flow.y = y
        + total_height
        + if forced.is_some() {
            0.0
        } else {
            style.margin[2] + BLOCK_GAP
        };
    flow.bottom = flow.bottom.max(flow.y);
    Ok(())
}
