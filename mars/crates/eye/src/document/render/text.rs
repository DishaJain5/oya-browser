/** Inline text and native shaping share the same containing content box. */
use super::flow::*;
/** Inline descendants preserve their own paint and exact hit geometry. */
pub(super) fn inline(
    context: &mut LayoutContext<'_>,
    node: &Node,
    flow: &mut Flow,
    items: &mut Vec<DisplayItem>,
) -> EngineResult<()> {
    let document = context.document;
    let index = items.len();
    let y = flow.y;
    let x = flow.x + flow.cursor;
    let lines = text(context, &node.text, node, flow)?;
    items.push(box_item(node, x, y, 0.0, 0.0, lines));
    for child in document
        .nodes
        .iter()
        .filter(|n| n.parent == Some(node.local_id))
    {
        node_layout(context, child, flow, items, None)?;
    }
    let descendants = &items[index..];
    let left = descendants
        .iter()
        .flat_map(|i| i.lines.iter())
        .map(|line| line.x)
        .fold(x, f64::min);
    let right = descendants
        .iter()
        .flat_map(|i| i.lines.iter())
        .map(|line| line.x + line.width)
        .fold(flow.x + flow.cursor, f64::max);
    items[index].x = left;
    items[index].width = (right - left).max(0.0);
    items[index].height =
        (flow.y + flow.line - y).max(node.style.line_height.used(node.style.font_size));
    Ok(())
}
/** Wrapping measures exact runs; whitespace collapses across adjacent inline elements. */
pub(super) fn text(
    context: &mut LayoutContext<'_>,
    source: &str,
    node: &Node,
    flow: &mut Flow,
) -> EngineResult<Vec<PaintLine>> {
    context.charge(source.chars().count())?;
    let mut lines: Vec<PaintLine> = Vec::new();
    let font = node.style.font_size;
    let bold = node.style.bold;
    let space = if source.split_whitespace().next().is_some() {
        context.measure(" ", font, bold)?
    } else {
        0.0
    };
    for word in source.split_whitespace() {
        let chunks = super::wrap::lines(word, flow.width.max(1.0), font, bold, context)?;
        let count = chunks.len();
        for (index, chunk) in chunks.into_iter().enumerate() {
            let gap = if flow.cursor > 0.0 { space } else { 0.0 };
            if flow.cursor > 0.0 && flow.cursor + gap + chunk.width > flow.width {
                flow.newline();
            }
            let gap = if flow.cursor > 0.0 { space } else { 0.0 };
            let x = flow.x + flow.cursor + gap;
            if let Some(line) = lines.last_mut().filter(|line| line.y == flow.y) {
                context.charge(chunk.text.chars().count())?;
                if gap > 0.0 {
                    line.text.push(' ');
                }
                line.text.push_str(&chunk.text);
                line.width += gap + chunk.width;
            } else {
                lines.push(PaintLine {
                    text: chunk.text,
                    x,
                    y: flow.y,
                    width: chunk.width,
                });
            }
            flow.cursor += gap + chunk.width;
            flow.line = flow.line.max(node.style.line_height.used(font));
            flow.bottom = flow.bottom.max(flow.y + flow.line);
            if index + 1 < count {
                flow.newline();
            }
        }
    }
    Ok(lines)
}
/** A containing block aligns whole inline lines together, including separately targetable links. */
pub(super) fn align(
    context: &mut LayoutContext<'_>,
    owner: &Node,
    items: &mut [DisplayItem],
    x: f64,
    width: f64,
) -> EngineResult<()> {
    if !["center", "right"].contains(&owner.style.align.as_str()) {
        return Ok(());
    }
    let eligible = items
        .iter()
        .map(|item| inline_descendant(context.document, item.local_id, owner.local_id))
        .collect::<Vec<_>>();
    let mut extents = std::collections::BTreeMap::<u64, (f64, f64)>::new();
    for (index, item) in items.iter().enumerate() {
        if !eligible[index] {
            continue;
        }
        for line in &item.lines {
            context.charge(1)?;
            let extent = extents
                .entry(line.y.to_bits())
                .or_insert((line.x, line.x + line.width));
            extent.0 = extent.0.min(line.x);
            extent.1 = extent.1.max(line.x + line.width);
        }
    }
    for (index, item) in items.iter_mut().enumerate() {
        if !eligible[index] {
            continue;
        }
        for line in &mut item.lines {
            let (left, right) = extents[&line.y.to_bits()];
            let free = (width - (right - left)).max(0.0);
            line.x += x - left
                + if owner.style.align == "center" {
                    free / 2.0
                } else {
                    free
                };
        }
    }
    refresh_inline_bounds(context, items, &eligible)
}
/** Reverse source order unions descendant paint into inline ancestors without widening block boxes. */
fn refresh_inline_bounds(
    context: &mut LayoutContext<'_>,
    items: &mut [DisplayItem],
    eligible: &[bool],
) -> EngineResult<()> {
    let mut extents = std::collections::BTreeMap::<usize, (f64, f64)>::new();
    for (index, item) in items.iter_mut().enumerate().rev() {
        if !eligible[index] || index == 0 {
            continue;
        }
        context.charge(item.lines.len() + 1)?;
        for line in &item.lines {
            add_extent(&mut extents, item.local_id, (line.x, line.x + line.width));
        }
        let Some((left, right)) = extents.remove(&item.local_id) else {
            continue;
        };
        item.x = left;
        item.width = (right - left).max(0.0);
        if let Some(parent) = context.node(item.local_id)?.parent {
            add_extent(&mut extents, parent, (left, right));
        }
    }
    Ok(())
}
/** Exact painted extents combine direct text and descendant runs under one stable identity. */
fn add_extent(
    extents: &mut std::collections::BTreeMap<usize, (f64, f64)>,
    id: usize,
    bounds: (f64, f64),
) {
    let extent = extents.entry(id).or_insert(bounds);
    extent.0 = extent.0.min(bounds.0);
    extent.1 = extent.1.max(bounds.1);
}
/** Nested block formatting already aligns its own descendants and must not be translated again. */
fn inline_descendant(document: &Document, mut id: usize, owner: usize) -> bool {
    while id != owner {
        let Some(node) = document.nodes.iter().find(|node| node.local_id == id) else {
            return false;
        };
        if !node.style.inline {
            return false;
        }
        let Some(parent) = node.parent else {
            return false;
        };
        id = parent;
    }
    true
}
