/** Replaced image geometry uses owned intrinsic dimensions and the common inline cursor. */
use super::flow::*;
/** Allocated flex image boxes never resolve authored widths for a second time. */
pub(super) fn image(
    context: &mut LayoutContext<'_>,
    node: &Node,
    flow: &mut Flow,
    items: &mut Vec<DisplayItem>,
    forced: Option<BorderBox>,
) -> EngineResult<()> {
    if let Some(bounds) = forced {
        let mut inner = Flow::new(
            bounds.x + node.style.padding[3] + node.style.border,
            bounds.y + node.style.padding[0] + node.style.border,
            (bounds.width - horizontal_edges(node)).max(0.0),
        );
        let lines = if node.image_size.is_none() {
            super::text::text(context, &node.text, node, &mut inner)?
        } else {
            Vec::new()
        };
        let natural_height = if let Some((width, height)) = node.image_size {
            (bounds.width - horizontal_edges(node)).max(0.0) * f64::from(height)
                / f64::from(width.max(1))
                + vertical_edges(node)
        } else {
            inner.bottom - bounds.y + node.style.padding[2] + node.style.border
        };
        let height = bounds.height.unwrap_or(natural_height);
        items.push(box_item(
            node,
            bounds.x,
            bounds.y,
            bounds.width,
            height,
            lines,
        ));
        flow.y = bounds.y + height;
        flow.bottom = flow.bottom.max(flow.y);
        return Ok(());
    }
    if node.style.inline {
        inline_image(context, node, flow, items)
    } else {
        block_image(context, node, flow, items)
    }
}
/** Intrinsic image sizes preserve aspect ratio and clamp to the containing flow. */
fn block_image(
    context: &mut LayoutContext<'_>,
    node: &Node,
    flow: &mut Flow,
    items: &mut Vec<DisplayItem>,
) -> EngineResult<()> {
    flow.newline();
    if node.image_size.is_none() {
        let lines = super::text::text(context, &node.text, node, flow)?;
        let width = lines
            .iter()
            .map(|line| line.x + line.width)
            .fold(flow.x, f64::max)
            - flow.x;
        let height = flow
            .line
            .max(node.style.line_height.used(node.style.font_size));
        items.push(box_item(node, flow.x, flow.y, width, height, lines));
        flow.y += height + BLOCK_GAP;
        flow.newline();
        return Ok(());
    }
    let (natural_width, natural_height) = image_size(node, flow.width);
    let item = box_item(
        node,
        flow.x,
        flow.y,
        natural_width,
        natural_height,
        Vec::new(),
    );
    items.push(item);
    flow.y += natural_height + BLOCK_GAP;
    flow.bottom = flow.bottom.max(flow.y);
    Ok(())
}
/** Intrinsic sizing and HTML dimensions share clamping in either flow mode. */
fn image_size(node: &Node, available: f64) -> (f64, f64) {
    let (intrinsic_width, intrinsic_height) = node.image_size.unwrap_or((1, 1));
    let available = available.max(0.0);
    let natural_content = dimension(node, "width").unwrap_or(f64::from(intrinsic_width));
    let natural_width = border_width(node, available, available, Some(natural_content));
    let content_width = (natural_width - horizontal_edges(node)).max(0.0);
    let natural_height = dimension(node, "height").unwrap_or_else(|| {
        f64::from(intrinsic_height) * content_width / f64::from(intrinsic_width.max(1))
    }) + vertical_edges(node);
    (natural_width, natural_height)
}
/** Inline images occupy replaced-content geometry in the same line as adjacent text. */
fn inline_image(
    context: &mut LayoutContext<'_>,
    node: &Node,
    flow: &mut Flow,
    items: &mut Vec<DisplayItem>,
) -> EngineResult<()> {
    let (width, height) = image_size(node, flow.width);
    if node.image_size.is_none() {
        return inline_alt(context, node, flow, items);
    }
    if flow.cursor > 0.0 && flow.cursor + width > flow.width {
        flow.newline();
    }
    items.push(box_item(
        node,
        flow.x + flow.cursor,
        flow.y,
        width,
        height,
        Vec::new(),
    ));
    flow.cursor += width;
    flow.line = flow.line.max(height);
    flow.bottom = flow.bottom.max(flow.y + height);
    Ok(())
}
/** Failed inline images remain readable text without dropping adjacent content. */
fn inline_alt(
    context: &mut LayoutContext<'_>,
    node: &Node,
    flow: &mut Flow,
    items: &mut Vec<DisplayItem>,
) -> EngineResult<()> {
    let start = flow.cursor;
    let y = flow.y;
    let lines = super::text::text(context, &node.text, node, flow)?;
    let left = lines
        .iter()
        .map(|line| line.x)
        .fold(flow.x + start, f64::min);
    let right = lines
        .iter()
        .map(|line| line.x + line.width)
        .fold(left, f64::max);
    items.push(box_item(
        node,
        left,
        y,
        right - left,
        (flow.y + flow.line - y).max(node.style.line_height.used(node.style.font_size)),
        lines,
    ));
    Ok(())
}
/** HTML dimensions accept only positive integers; malformed values fall back to intrinsic size. */
fn dimension(node: &Node, name: &str) -> Option<f64> {
    node.attributes
        .get(name)?
        .parse::<u32>()
        .ok()
        .filter(|value| *value > 0)
        .map(f64::from)
}
