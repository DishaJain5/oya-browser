/** Static table layout retains its existing grid outside flex formatting contexts. */
use super::flow::*;
/** Tables use a bounded grid, while cell descendants retain their normal engine layout. */
pub(super) fn table(
    context: &mut LayoutContext<'_>,
    node: &Node,
    flow: &mut Flow,
    items: &mut Vec<DisplayItem>,
) -> EngineResult<()> {
    let document = context.document;
    if document
        .nodes
        .iter()
        .any(|child| child.style.display_flex && has_ancestor(document, child, node.local_id))
    {
        return Err(EngineError::unsupported(
            "Flex layout inside tables is not supported.",
        ));
    }
    flow.newline();
    let table_index = items.len();
    items.push(box_item(node, flow.x, flow.y, flow.width, 0.0, Vec::new()));
    for caption in document
        .nodes
        .iter()
        .filter(|child| child.parent == Some(node.local_id) && child.tag == "caption")
    {
        node_layout(context, caption, flow, items, None)?;
    }
    let rows = table_rows(document, node.local_id);
    let widths = column_widths(context, &rows, flow.width)?;
    for row in rows {
        table_row(context, row, flow, items, &widths)?;
    }
    items[table_index].height = flow.y - items[table_index].y;
    flow.y += BLOCK_GAP;
    flow.bottom = flow.bottom.max(flow.y);
    Ok(())
}
/** Source-order traversal includes groups but skips captions already laid above the grid. */
fn table_rows(document: &Document, table: usize) -> Vec<&Node> {
    document
        .nodes
        .iter()
        .filter(|node| node.tag == "tr" && !node.hidden() && has_ancestor(document, node, table))
        .collect()
}
/** Preferred cell widths grow from measured text and stay inside available content width. */
fn column_widths(
    context: &mut LayoutContext<'_>,
    rows: &[&Node],
    available: f64,
) -> EngineResult<Vec<f64>> {
    let document = context.document;
    let count = rows
        .iter()
        .map(|row| table_cells(document, row.local_id).len())
        .max()
        .unwrap_or(1);
    let mut widths = vec![48.0_f64; count];
    for row in rows {
        for (column, cell) in table_cells(document, row.local_id).iter().enumerate() {
            if cell.hidden() {
                continue;
            }
            widths[column] = widths[column].max(cell_preferred(context, cell, available)?);
        }
    }
    fit_widths(&mut widths, available);
    Ok(widths)
}
/** Column proportions shrink together instead of overflowing the page viewport. */
fn fit_widths(widths: &mut [f64], available: f64) {
    let total = widths.iter().sum::<f64>();
    if total > available {
        widths
            .iter_mut()
            .for_each(|width| *width *= available / total);
    }
}
/** A row height is the tallest wrapped cell; all its cell boxes share that height. */
fn table_row(
    context: &mut LayoutContext<'_>,
    row: &Node,
    flow: &mut Flow,
    items: &mut Vec<DisplayItem>,
    widths: &[f64],
) -> EngineResult<()> {
    context.charge(1)?;
    let document = context.document;
    flow.newline();
    let y = flow.y;
    let start = items.len();
    items.push(box_item(
        row,
        flow.x,
        y,
        widths.iter().sum(),
        TABLE_MIN_ROW_HEIGHT,
        Vec::new(),
    ));
    let mut x = flow.x;
    let mut height: f64 = TABLE_MIN_ROW_HEIGHT;
    for (column, cell) in table_cells(document, row.local_id).iter().enumerate() {
        if cell.hidden() {
            x += widths[column];
            continue;
        }
        height = height.max(table_cell(context, cell, x, y, widths[column], items)?);
        x += widths[column];
    }
    items[start].height = height;
    for item in &mut items[start + 1..] {
        if item.tag == "th" || item.tag == "td" {
            item.height = height;
        }
    }
    flow.y = y + height + 1.0;
    flow.bottom = flow.bottom.max(flow.y);
    Ok(())
}
/** Cell content wraps within its column and contributes its child controls to row height. */
fn table_cell(
    context: &mut LayoutContext<'_>,
    cell: &Node,
    x: f64,
    y: f64,
    width: f64,
    items: &mut Vec<DisplayItem>,
) -> EngineResult<f64> {
    context.charge(1)?;
    let document = context.document;
    let start = items.len();
    items.push(box_item(
        cell,
        x,
        y,
        width,
        TABLE_MIN_ROW_HEIGHT,
        Vec::new(),
    ));
    let mut inner = Flow::new(
        x + TABLE_PADDING,
        y + TABLE_PADDING,
        (width - TABLE_PADDING * 2.0).max(1.0),
    );
    items[start].lines = super::text::text(context, &cell.text, cell, &mut inner)?;
    for child in document
        .nodes
        .iter()
        .filter(|child| child.parent == Some(cell.local_id))
    {
        node_layout(context, child, &mut inner, items, None)?;
    }
    inner.newline();
    let height = (inner.bottom - y + TABLE_PADDING).max(TABLE_MIN_ROW_HEIGHT);
    items[start].border = 1.0;
    items[start].border_color = Some("#a0a0a0".into());
    Ok(height)
}
/** Table children keep direct row and cell identities throughout grid construction. */
fn table_cells(document: &Document, row: usize) -> Vec<&Node> {
    document
        .nodes
        .iter()
        .filter(|node| node.parent == Some(row) && ["th", "td"].contains(&node.tag.as_str()))
        .collect()
}
/** Nested text and controls contribute their natural width to table column estimates. */
fn cell_preferred(
    context: &mut LayoutContext<'_>,
    cell: &Node,
    available: f64,
) -> EngineResult<f64> {
    let document = context.document;
    let mut content = cell.text.clone();
    for node in document
        .nodes
        .iter()
        .filter(|node| has_ancestor(document, node, cell.local_id) && !node.hidden())
    {
        if !node.text.is_empty() {
            content.push(' ');
            content.push_str(&node.text);
        }
    }
    let text_width = context.measure(&content, cell.style.font_size, cell.style.bold)?;
    let mut control_width: f64 = 0.0;
    for node in document.nodes.iter().filter(|node| {
        has_ancestor(document, node, cell.local_id)
            && !node.hidden()
            && ["input", "button", "select", "textarea"].contains(&node.tag.as_str())
    }) {
        let width = if node.tag == "button"
            || (node.tag == "input" && ["submit", "button"].contains(&node.input_type()))
        {
            (context.measure(&node.text, node.style.font_size, node.style.bold)? + 32.0).max(120.0)
        } else {
            480.0
        };
        control_width = control_width.max(width);
    }
    Ok((text_width.max(control_width) + TABLE_PADDING * 2.0).min(available))
}
/** Parent traversal distinguishes this table's rows from unrelated nested page content. */
fn has_ancestor(document: &Document, node: &Node, ancestor: usize) -> bool {
    let mut parent = node.parent;
    while let Some(id) = parent {
        if id == ancestor {
            return true;
        }
        parent = document
            .nodes
            .iter()
            .find(|node| node.local_id == id)
            .and_then(|node| node.parent);
    }
    false
}
