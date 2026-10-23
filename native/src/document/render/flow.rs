/** Owned block and inline formatting lays out native controls and literal shaped text. */
use super::{
    BLOCK_GAP, CONTENT_WIDTH, CONTROL_HEIGHT, DisplayItem, MARGIN, PaintLine, TABLE_MIN_ROW_HEIGHT,
    TABLE_PADDING,
};
use crate::{
    document::{Document, Node},
    platform::measure,
};

/** Each containing block owns one inline cursor and measured line height. */
struct Flow {
    x: f64,
    y: f64,
    width: f64,
    cursor: f64,
    line: f64,
    bottom: f64,
}
impl Flow {
    /** A block starts at its content origin with no invented glyph width. */
    fn new(x: f64, y: f64, width: f64) -> Self {
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
    fn newline(&mut self) {
        if self.cursor > 0.0 || self.line > 0.0 {
            self.y += self.line;
            self.bottom = self.bottom.max(self.y);
        }
        self.cursor = 0.0;
        self.line = 0.0;
    }
}
/** Top-level children share a virtual body formatting context. */
pub(super) fn layout(document: &Document, viewport: f64) -> (Vec<DisplayItem>, f64) {
    let mut items = Vec::new();
    let mut flow = Flow::new(MARGIN, MARGIN, (viewport - MARGIN * 2.0).min(CONTENT_WIDTH));
    for node in document.nodes.iter().filter(|n| n.parent.is_none()) {
        node_layout(document, node, &mut flow, &mut items);
    }
    flow.newline();
    (items, flow.bottom.max(flow.y))
}
/** Every subtree is laid out once and hidden subtrees contribute no native target. */
fn node_layout(document: &Document, node: &Node, flow: &mut Flow, items: &mut Vec<DisplayItem>) {
    if node.hidden() {
        return;
    }
    if node.tag == "br" {
        flow.line = flow.line.max(node.style.font_size * 1.5);
        flow.newline();
        return;
    }
    if node.tag == "img" {
        if node.style.inline {
            inline_image(node, flow, items);
        } else {
            block_image(node, flow, items);
        }
        return;
    }
    if node.tag == "table" {
        table(document, node, flow, items);
        return;
    }
    if ["input", "button", "select", "textarea"].contains(&node.tag.as_str()) {
        control(document, node, flow, items);
        return;
    }
    if node.style.inline {
        inline(document, node, flow, items);
        return;
    }
    flow.newline();
    let style = &node.style;
    let x = flow.x + style.margin[3];
    let y = flow.y + style.margin[0];
    let width = width(
        node,
        (flow.width - style.margin[1] - style.margin[3]).max(1.0),
    );
    let mut inner = Flow::new(
        x + style.padding[3] + style.border,
        y + style.padding[0] + style.border,
        (width - style.padding[1] - style.padding[3] - style.border * 2.0).max(1.0),
    );
    let index = items.len();
    items.push(box_item(node, x, y, width, 0.0, Vec::new()));
    let lines = text(&node.text, node, &mut inner);
    items[index].lines = lines;
    for child in document
        .nodes
        .iter()
        .filter(|n| n.parent == Some(node.local_id))
    {
        node_layout(document, child, &mut inner, items);
    }
    inner.newline();
    let height =
        (inner.bottom - y + style.padding[2] + style.border).max(if node.text.is_empty() {
            0.0
        } else {
            node.style.font_size * 1.5
        });
    items[index].height = height;
    flow.y = y + height + style.margin[2] + BLOCK_GAP;
    flow.bottom = flow.bottom.max(flow.y);
}
/** Tables use a bounded grid, while cell descendants retain their normal engine layout. */
fn table(document: &Document, node: &Node, flow: &mut Flow, items: &mut Vec<DisplayItem>) {
    flow.newline();
    let table_index = items.len();
    items.push(box_item(node, flow.x, flow.y, flow.width, 0.0, Vec::new()));
    for caption in document
        .nodes
        .iter()
        .filter(|child| child.parent == Some(node.local_id) && child.tag == "caption")
    {
        node_layout(document, caption, flow, items);
    }
    let rows = table_rows(document, node.local_id);
    let widths = column_widths(document, &rows, flow.width);
    for row in rows {
        table_row(document, row, flow, items, &widths);
    }
    items[table_index].height = flow.y - items[table_index].y;
    flow.y += BLOCK_GAP;
    flow.bottom = flow.bottom.max(flow.y);
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
fn column_widths(document: &Document, rows: &[&Node], available: f64) -> Vec<f64> {
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
            widths[column] = widths[column].max(cell_preferred(document, cell, available));
        }
    }
    fit_widths(&mut widths, available);
    widths
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
    document: &Document,
    row: &Node,
    flow: &mut Flow,
    items: &mut Vec<DisplayItem>,
    widths: &[f64],
) {
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
        height = height.max(table_cell(document, cell, x, y, widths[column], items));
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
}
/** Cell content wraps within its column and contributes its child controls to row height. */
fn table_cell(
    document: &Document,
    cell: &Node,
    x: f64,
    y: f64,
    width: f64,
    items: &mut Vec<DisplayItem>,
) -> f64 {
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
    items[start].lines = text(&cell.text, cell, &mut inner);
    for child in document
        .nodes
        .iter()
        .filter(|child| child.parent == Some(cell.local_id))
    {
        node_layout(document, child, &mut inner, items);
    }
    inner.newline();
    let height = (inner.bottom - y + TABLE_PADDING).max(TABLE_MIN_ROW_HEIGHT);
    items[start].border = 1.0;
    items[start].border_color = Some("#a0a0a0".into());
    height
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
fn cell_preferred(document: &Document, cell: &Node, available: f64) -> f64 {
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
    let text_width = measure(&content, cell.style.font_size, cell.style.bold);
    let control_width = document
        .nodes
        .iter()
        .filter(|node| {
            has_ancestor(document, node, cell.local_id)
                && !node.hidden()
                && ["input", "button", "select", "textarea"].contains(&node.tag.as_str())
        })
        .map(|node| {
            if node.tag == "button"
                || (node.tag == "input" && ["submit", "button"].contains(&node.input_type()))
            {
                (measure(&node.text, node.style.font_size, node.style.bold) + 32.0).max(120.0)
            } else {
                480.0
            }
        })
        .fold(0.0, f64::max);
    (text_width.max(control_width) + TABLE_PADDING * 2.0).min(available)
}
/** Parent traversal distinguishes this table's rows from unrelated nested page content. */
fn has_ancestor(document: &Document, node: &Node, ancestor: usize) -> bool {
    let mut parent = node.parent;
    while let Some(id) = parent {
        if id == ancestor {
            return true;
        }
        parent = document.nodes[id].parent;
    }
    false
}
/** Intrinsic image sizes preserve aspect ratio and clamp to the containing flow. */
fn block_image(node: &Node, flow: &mut Flow, items: &mut Vec<DisplayItem>) {
    flow.newline();
    if node.image_size.is_none() {
        let lines = text(&node.text, node, flow);
        let width = lines
            .iter()
            .map(|line| line.x + line.width)
            .fold(flow.x, f64::max)
            - flow.x;
        let height = flow.line.max(node.style.font_size * 1.5);
        items.push(box_item(node, flow.x, flow.y, width, height, lines));
        flow.y += height + BLOCK_GAP;
        flow.newline();
        return;
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
}
/** Intrinsic sizing and HTML dimensions share clamping in either flow mode. */
fn image_size(node: &Node, available: f64) -> (f64, f64) {
    let (intrinsic_width, intrinsic_height) = node.image_size.unwrap_or((1, 1));
    let available = available.max(1.0);
    let authored_width = node.style.width.or_else(|| dimension(node, "width"));
    let authored_height = dimension(node, "height");
    let natural_width = authored_width
        .unwrap_or(f64::from(intrinsic_width))
        .min(available)
        .max(1.0);
    let natural_height = authored_height
        .unwrap_or_else(|| {
            f64::from(intrinsic_height) * natural_width / f64::from(intrinsic_width.max(1))
        })
        .max(1.0);
    (natural_width, natural_height)
}
/** Inline images occupy replaced-content geometry in the same line as adjacent text. */
fn inline_image(node: &Node, flow: &mut Flow, items: &mut Vec<DisplayItem>) {
    let (width, height) = image_size(node, flow.width);
    if node.image_size.is_none() {
        return inline_alt(node, flow, items);
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
}
/** Failed inline images remain readable text without dropping adjacent content. */
fn inline_alt(node: &Node, flow: &mut Flow, items: &mut Vec<DisplayItem>) {
    let start = flow.cursor;
    let y = flow.y;
    let lines = text(&node.text, node, flow);
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
        (flow.y + flow.line - y).max(node.style.font_size * 1.5),
        lines,
    ));
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
/** Inline descendants preserve their own paint and exact hit geometry. */
fn inline(document: &Document, node: &Node, flow: &mut Flow, items: &mut Vec<DisplayItem>) {
    let index = items.len();
    let y = flow.y;
    let x = flow.x + flow.cursor;
    let lines = text(&node.text, node, flow);
    items.push(box_item(node, x, y, 0.0, 0.0, lines));
    for child in document
        .nodes
        .iter()
        .filter(|n| n.parent == Some(node.local_id))
    {
        node_layout(document, child, flow, items);
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
    items[index].height = (flow.y + flow.line - y).max(node.style.font_size * 1.5);
}
/** Wrapping measures exact runs; whitespace collapses across adjacent inline elements. */
fn text(source: &str, node: &Node, flow: &mut Flow) -> Vec<PaintLine> {
    let mut lines: Vec<PaintLine> = Vec::new();
    let font = node.style.font_size;
    let bold = node.style.bold;
    for word in source.split_whitespace() {
        let chunks = super::wrap::lines(word, flow.width, font, bold);
        for chunk in chunks {
            let gap = if flow.cursor > 0.0 {
                measure(" ", font, bold)
            } else {
                0.0
            };
            let width = measure(&chunk, font, bold);
            if flow.cursor > 0.0 && flow.cursor + gap + width > flow.width {
                flow.newline();
            }
            let gap = if flow.cursor > 0.0 {
                measure(" ", font, bold)
            } else {
                0.0
            };
            let x = flow.x + flow.cursor + gap;
            if let Some(line) = lines.last_mut().filter(|line| line.y == flow.y) {
                line.text.push(' ');
                line.text.push_str(&chunk);
                line.width = measure(&line.text, font, bold);
            } else {
                lines.push(PaintLine {
                    text: chunk,
                    x,
                    y: flow.y,
                    width,
                });
            }
            flow.cursor += gap + width;
            flow.line = flow.line.max(font * 1.5);
            flow.bottom = flow.bottom.max(flow.y + flow.line);
        }
    }
    lines
}
/** Native controls occupy engine boxes; AppKit never lays out their surrounding HTML. */
fn control(document: &Document, node: &Node, flow: &mut Flow, items: &mut Vec<DisplayItem>) {
    flow.newline();
    let style = &node.style;
    let x = flow.x + style.margin[3];
    let y = flow.y + style.margin[0];
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
        (measure(&title, style.font_size, style.bold) + 32.0).max(120.0)
    } else {
        480.0
    };
    let width = if style.width.is_some() {
        width(node, available)
    } else {
        available.min(natural)
    };
    let mut local = Flow::new(x + 8.0, y + 4.0, (width - 16.0).max(1.0));
    let lines = if button {
        text(&title, node, &mut local)
    } else {
        Vec::new()
    };
    let height = if node.tag == "textarea" {
        (style.font_size * 1.5 * 3.0).max(CONTROL_HEIGHT)
    } else {
        CONTROL_HEIGHT.max(local.bottom - y + 4.0)
    };
    let mut item = box_item(node, x, y, width, height, lines);
    item.control_height = Some(height);
    if let Some(error) = &node.validation {
        let mut error_flow = Flow::new(x, y + height + 2.0, width);
        item.validation = text(error, node, &mut error_flow);
        item.height += error_flow.bottom - (y + height);
    }
    let total_height = item.height;
    items.push(item);
    flow.y = y + total_height + style.margin[2] + BLOCK_GAP;
    flow.bottom = flow.bottom.max(flow.y);
}
/** Widths are clamped to the containing block to preserve usable narrow controls. */
fn width(node: &Node, available: f64) -> f64 {
    node.style
        .width
        .map(|w| {
            if node.style.percent_width {
                available * w / 100.0
            } else {
                w
            }
        })
        .unwrap_or(available)
        .min(node.style.max_width.unwrap_or(available))
        .clamp(1.0, available.max(1.0))
}
/** One representation supplies both agent bounds and the native paint path. */
fn box_item(
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
        lines,
        color: node.style.color.clone(),
        background: node.style.background.clone(),
        border: node.style.border,
        border_color: node.style.border_color.clone(),
        validation: Vec::new(),
        control_height: None,
    }
}
