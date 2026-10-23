/** Flexible child placement shares intrinsic measurements, exact border boxes and one work budget. */
use super::{
    DisplayItem,
    flex::{self, FlexItem},
    flow::Flow,
    sizing::{BorderBox, LayoutContext, content_size, horizontal_edges, specified_content},
};
use crate::{
    constants::LAYOUT_EXTENT,
    document::{AlignItems, FlexBasis, FlexDirection, FlexWrap, JustifyContent, Node},
    protocol::{EngineError, EngineResult},
};

/** Direct text and table formatting remain explicit unsupported boundaries for this slice. */
pub(super) fn children(
    context: &mut LayoutContext<'_>,
    node: &Node,
    flow: &mut Flow,
    output: &mut Vec<DisplayItem>,
    definite_height: Option<f64>,
) -> EngineResult<()> {
    let children = context.children(node.local_id);
    if !node.text.trim().is_empty()
        || children
            .iter()
            .any(|child| child.tag == "#text" || child.tag == "table")
    {
        return Err(EngineError::unsupported(
            "Flex containers require wrapped text children and cannot directly contain tables.",
        ));
    }
    if node.style.flex_direction == FlexDirection::Column {
        return super::flex_column::column(context, node, &children, flow, output, definite_height);
    }
    row(context, node, &children, flow, output)
}

/** Source-defined row bases and constraints use content units, including automatic minimums. */
fn row_item(context: &mut LayoutContext<'_>, node: &Node, inner: f64) -> EngineResult<FlexItem> {
    let intrinsic = context.intrinsic_widths(node.local_id)?;
    let specified = specified_content(node, inner);
    let base = match node.style.flex_basis {
        FlexBasis::Auto => specified.unwrap_or(intrinsic.max_content),
        FlexBasis::Pixels(value) => content_size(node, value, true),
        FlexBasis::Percent(value) => content_size(node, inner * value / 100.0, true),
    };
    let max = node
        .style
        .max_width
        .map(|value| content_size(node, value, true))
        .unwrap_or(LAYOUT_EXTENT);
    let minimum = node
        .style
        .min_width
        .map(|value| content_size(node, value, true))
        .unwrap_or(
            intrinsic
                .min_content
                .min(specified.unwrap_or(LAYOUT_EXTENT))
                .min(max),
        );
    let maximum = max.max(minimum);
    Ok(FlexItem {
        local_id: node.local_id,
        base_content: base,
        hypothetical_content: base.max(minimum).min(maximum),
        min_content: minimum,
        max_content: maximum,
        margin_start: node.style.margin[3],
        margin_end: node.style.margin[1],
        main_edges: horizontal_edges(node),
        grow: node.style.flex_grow,
        shrink: node.style.flex_shrink,
    })
}

/** Wrapping precedes flexible-length resolution and each line measures actual assigned widths. */
fn row(
    context: &mut LayoutContext<'_>,
    parent: &Node,
    children: &[&Node],
    flow: &mut Flow,
    output: &mut Vec<DisplayItem>,
) -> EngineResult<()> {
    let mut records = Vec::new();
    for child in children {
        records.push(row_item(context, child, flow.width)?);
    }
    let lines = flex::break_lines(
        &records,
        flow.width,
        parent.style.column_gap,
        parent.style.flex_wrap == FlexWrap::Wrap,
    );
    let mut y = flow.y;
    for (line_index, range) in lines.iter().enumerate() {
        let records = &records[range.clone()];
        context.charge(
            records
                .len()
                .saturating_mul(records.len().saturating_add(1)),
        )?;
        let sizes = flex::resolve_line(records, flow.width, parent.style.column_gap)?;
        let heights = measure_line(context, records, &sizes)?;
        let cross = heights
            .iter()
            .enumerate()
            .map(|(index, height)| {
                let child = children[range.start + index];
                height + child.style.margin[0] + child.style.margin[2]
            })
            .fold(0.0, f64::max);
        place_row_line(
            context,
            parent,
            RowLine {
                records,
                sizes: &sizes,
                heights: &heights,
                y,
                cross,
            },
            flow,
            output,
        )?;
        y += cross;
        if line_index + 1 < lines.len() {
            y += parent.style.row_gap;
        }
    }
    flow.y = y;
    flow.bottom = flow.bottom.max(y);
    Ok(())
}

/** Dry-run and final passes share memoization; measurements include validation rows. */
fn measure_line(
    context: &mut LayoutContext<'_>,
    records: &[FlexItem],
    sizes: &[f64],
) -> EngineResult<Vec<f64>> {
    records
        .iter()
        .zip(sizes)
        .map(|(record, size)| context.measure_height(record.local_id, size + record.main_edges))
        .collect()
}

/** Justification applies only leftover nonnegative space after flexing and authored margins. */
pub(super) fn justification(kind: JustifyContent, free: f64, count: usize) -> (f64, f64) {
    let free = free.max(0.0);
    match kind {
        JustifyContent::Start => (0.0, 0.0),
        JustifyContent::Center => (free / 2.0, 0.0),
        JustifyContent::End => (free, 0.0),
        JustifyContent::SpaceBetween if count > 1 => (0.0, free / (count - 1) as f64),
        JustifyContent::SpaceBetween => (0.0, 0.0),
    }
}

/** Measured records and their shared cross extent form one placement input. */
struct RowLine<'a> {
    /** Source-order flex records for this line. */
    records: &'a [FlexItem],
    /** Resolved content widths corresponding to the records. */
    sizes: &'a [f64],
    /** Measured border heights at those exact widths. */
    heights: &'a [f64],
    /** Shared line origin after preceding rows and gaps. */
    y: f64,
    /** Largest outer height used for alignment and stretching. */
    cross: f64,
}

/** Allocated cross-axis stretch does not rerun the main-axis width algorithm. */
fn place_row_line(
    context: &mut LayoutContext<'_>,
    parent: &Node,
    line: RowLine<'_>,
    flow: &Flow,
    output: &mut Vec<DisplayItem>,
) -> EngineResult<()> {
    let RowLine {
        records,
        sizes,
        heights,
        y,
        cross,
    } = line;
    let used = records
        .iter()
        .zip(sizes)
        .map(|(record, size)| record.outer(*size))
        .sum::<f64>()
        + parent.style.column_gap * records.len().saturating_sub(1) as f64;
    let (offset, between) = justification(
        parent.style.justify_content,
        flow.width - used,
        records.len(),
    );
    let mut x = flow.x + offset;
    for (index, record) in records.iter().enumerate() {
        let child = context.node(record.local_id)?;
        x += record.margin_start;
        let free_cross =
            (cross - heights[index] - child.style.margin[0] - child.style.margin[2]).max(0.0);
        let offset = match parent.style.align_items {
            AlignItems::Center => free_cross / 2.0,
            AlignItems::End => free_cross,
            _ => 0.0,
        };
        let height = (parent.style.align_items == AlignItems::Stretch)
            .then_some((cross - child.style.margin[0] - child.style.margin[2]).max(0.0));
        context.layout_at(
            record.local_id,
            BorderBox {
                x,
                y: y + child.style.margin[0] + offset,
                width: sizes[index] + record.main_edges,
                height,
            },
            output,
        )?;
        x += sizes[index]
            + record.main_edges
            + record.margin_end
            + parent.style.column_gap
            + between;
    }
    Ok(())
}
