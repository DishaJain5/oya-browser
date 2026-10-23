/** Column layout distributes only actual definite space and otherwise derives height from content. */
use super::{
    DisplayItem,
    flex::{self, FlexItem},
    flex_layout::justification,
    flow::Flow,
    sizing::{BorderBox, LayoutContext, border_width, content_size, vertical_edges},
};
use crate::{
    constants::LAYOUT_EXTENT,
    document::{AlignItems, FlexBasis, FlexWrap, Node},
    protocol::{EngineError, EngineResult},
};

/** Cross widths use intrinsic size unless automatic stretch has a definite containing width. */
fn cross_width(
    context: &mut LayoutContext<'_>,
    parent: &Node,
    child: &Node,
    inner: f64,
) -> EngineResult<f64> {
    let intrinsic = context.intrinsic_widths(child.local_id)?;
    let available = (inner - child.style.margin[1] - child.style.margin[3]).max(0.0);
    let natural = if parent.style.align_items == AlignItems::Stretch && child.style.width.is_none()
    {
        None
    } else {
        Some(intrinsic.max_content)
    };
    Ok(border_width(child, inner, available, natural))
}

/** A percentage basis resolves only when an actually assigned main extent is definite. */
fn item(
    context: &mut LayoutContext<'_>,
    child: &Node,
    width: f64,
    definite_height: Option<f64>,
) -> EngineResult<FlexItem> {
    let natural = context.measure_height(child.local_id, width)?;
    let natural_content = (natural - vertical_edges(child)).max(0.0);
    let base = match child.style.flex_basis {
        FlexBasis::Pixels(value) => content_size(child, value, false),
        FlexBasis::Percent(value) if definite_height.is_some() => content_size(
            child,
            definite_height.unwrap_or_default() * value / 100.0,
            false,
        ),
        _ => natural_content,
    };
    Ok(FlexItem {
        local_id: child.local_id,
        base_content: base,
        hypothetical_content: base.max(natural_content),
        min_content: natural_content,
        max_content: LAYOUT_EXTENT,
        margin_start: child.style.margin[0],
        margin_end: child.style.margin[2],
        main_edges: vertical_edges(child),
        grow: child.style.flex_grow,
        shrink: child.style.flex_shrink,
    })
}

/** Auto main extent adds only child outer sizes and between-item row gaps. */
pub(super) fn column(
    context: &mut LayoutContext<'_>,
    parent: &Node,
    children: &[&Node],
    flow: &mut Flow,
    output: &mut Vec<DisplayItem>,
    definite_height: Option<f64>,
) -> EngineResult<()> {
    if parent.style.flex_wrap == FlexWrap::Wrap {
        return Err(EngineError::unsupported(
            "Column wrapping is not supported.",
        ));
    }
    let mut records = Vec::new();
    let mut widths = Vec::new();
    for child in children {
        let width = cross_width(context, parent, child, flow.width)?;
        widths.push(width);
        records.push(item(context, child, width, definite_height)?);
    }
    let intrinsic = records
        .iter()
        .map(|record| record.outer(record.hypothetical_content))
        .sum::<f64>()
        + parent.style.row_gap * records.len().saturating_sub(1) as f64;
    let extent = definite_height.unwrap_or(intrinsic);
    context.charge(
        records
            .len()
            .saturating_mul(records.len().saturating_add(1)),
    )?;
    let sizes = flex::resolve_line(&records, extent, parent.style.row_gap)?;
    let used = records
        .iter()
        .zip(&sizes)
        .map(|(record, size)| record.outer(*size))
        .sum::<f64>()
        + parent.style.row_gap * records.len().saturating_sub(1) as f64;
    let (offset, between) =
        justification(parent.style.justify_content, extent - used, records.len());
    let mut y = flow.y + offset;
    for (index, child) in children.iter().enumerate() {
        let free =
            (flow.width - widths[index] - child.style.margin[1] - child.style.margin[3]).max(0.0);
        let offset = match parent.style.align_items {
            AlignItems::Center => free / 2.0,
            AlignItems::End => free,
            _ => 0.0,
        };
        y += child.style.margin[0];
        context.layout_at(
            child.local_id,
            BorderBox {
                x: flow.x + child.style.margin[3] + offset,
                y,
                width: widths[index],
                height: Some(sizes[index] + records[index].main_edges),
            },
            output,
        )?;
        y += sizes[index] + records[index].main_edges + child.style.margin[2];
        if index + 1 < children.len() {
            y += parent.style.row_gap + between;
        }
    }
    flow.y += extent;
    flow.bottom = flow.bottom.max(flow.y.max(y));
    Ok(())
}
