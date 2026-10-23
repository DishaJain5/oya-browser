/** Shared sizing caches and work limits keep intrinsic, dry-run and final layout bounded. */
use super::validation::bounded;
use super::{DisplayItem, flow};
use crate::{
    constants::{LAYOUT_EXTENT, LAYOUT_WORK},
    document::{BoxSizing, Document, FlexDirection, FlexWrap, Node},
    platform::measure,
    protocol::{EngineError, EngineResult},
};
use std::collections::BTreeMap;

/** Assigned border boxes never reinterpret an item's authored width. */
#[derive(Clone, Copy, Debug)]
pub(super) struct BorderBox {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: Option<f64>,
}
/** Intrinsic widths exclude the node's own decorations and margins. */
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct IntrinsicWidths {
    pub min_content: f64,
    pub max_content: f64,
}
/** One request shares every measurement and recursive visit budget. */
pub(super) struct LayoutContext<'a> {
    pub document: &'a Document,
    work: usize,
    intrinsic: BTreeMap<usize, IntrinsicWidths>,
    heights: BTreeMap<(usize, u64), f64>,
    /** Cached widths are bounded by the same work needed to create each key. */
    measured: BTreeMap<(String, u64, bool), f64>,
    /** Completed word wrapping is reusable across dry runs with identical font and containing width. */
    wrapped: BTreeMap<(u64, u64, bool), BTreeMap<String, Vec<super::wrap::MeasuredText>>>,
}
impl<'a> LayoutContext<'a> {
    /** Validate identities before any recursive layout, including direct Rust callers. */
    pub fn new(document: &'a Document) -> EngineResult<Self> {
        super::validation::tree(document)?;
        Ok(Self {
            document,
            work: LAYOUT_WORK,
            intrinsic: BTreeMap::new(),
            heights: BTreeMap::new(),
            measured: BTreeMap::new(),
            wrapped: BTreeMap::new(),
        })
    }
    /** Shared work charges include repeated measurement rather than resetting in a dry run. */
    pub fn charge(&mut self, amount: usize) -> EngineResult<()> {
        self.work = self
            .work
            .checked_sub(amount)
            .ok_or_else(|| EngineError::limit("layout work", LAYOUT_WORK))?;
        Ok(())
    }
    /** Charge scans and native shaping before either can exceed the shared request allowance. */
    pub fn measure(&mut self, text: &str, font: f64, bold: bool) -> EngineResult<f64> {
        let scalars = text.chars().count();
        self.charge(scalars + 1)?;
        let key = (text.to_owned(), font.to_bits(), bold);
        if let Some(width) = self.measured.get(&key) {
            return Ok(*width);
        }
        self.charge(scalars + 1)?;
        let width = measure(text, font, bold);
        bounded(width)?;
        self.measured.insert(key, width);
        Ok(width)
    }
    /** A borrowed source lookup avoids rebuilding scalar boundaries and repeated native prefix probes. */
    pub fn wrapped(
        &mut self,
        text: &str,
        width: f64,
        font: f64,
        bold: bool,
    ) -> EngineResult<Option<Vec<super::wrap::MeasuredText>>> {
        let key = (width.to_bits(), font.to_bits(), bold);
        self.charge(text.chars().count() + 1)?;
        Ok(self
            .wrapped
            .get(&key)
            .and_then(|runs| runs.get(text))
            .cloned())
    }
    /** Cached text and run copies are admitted against the same finite request allowance. */
    pub fn remember_wrapped(
        &mut self,
        text: &str,
        width: f64,
        font: f64,
        bold: bool,
        lines: &[super::wrap::MeasuredText],
    ) -> EngineResult<()> {
        self.charge(text.chars().count() * 2 + 1)?;
        let key = (width.to_bits(), font.to_bits(), bold);
        self.wrapped
            .entry(key)
            .or_default()
            .insert(text.to_owned(), lines.to_vec());
        Ok(())
    }
    /** All lookup uses stable identities, never vector positions after descendant replacement. */
    pub fn node(&self, local: usize) -> EngineResult<&'a Node> {
        self.document
            .nodes
            .iter()
            .find(|node| node.local_id == local)
            .ok_or_else(|| EngineError::invalid("Invalid layout node identity."))
    }
    /** Source-order children share their parent's already validated tree depth. */
    pub fn children(&self, local: usize) -> Vec<&'a Node> {
        self.document
            .nodes
            .iter()
            .filter(|node| node.parent == Some(local) && !node.hidden())
            .collect()
    }
    /** Intrinsic measurement is memoized per stable node for the entire request. */
    pub fn intrinsic_widths(&mut self, local: usize) -> EngineResult<IntrinsicWidths> {
        if let Some(value) = self.intrinsic.get(&local) {
            return Ok(*value);
        }
        self.charge(1)?;
        let node = self.node(local)?;
        let mut size = text_intrinsic(self, node)?;
        let children = self.children(local);
        let mut sum = IntrinsicWidths::default();
        for child in &children {
            let value = self.intrinsic_widths(child.local_id)?;
            let contribution = intrinsic_contribution(node, child, value);
            let extra = horizontal_edges(child) + child.style.margin[1] + child.style.margin[3];
            let minimum = contribution.min_content;
            let max = contribution.max_content;
            size.min_content = size.min_content.max(minimum + extra);
            size.max_content = size.max_content.max(max + extra);
            sum.min_content += minimum + extra;
            sum.max_content += max + extra;
        }
        if node.style.display_flex && node.style.flex_direction == FlexDirection::Row {
            let gaps = node.style.column_gap * children.len().saturating_sub(1) as f64;
            size.max_content = size.max_content.max(sum.max_content + gaps);
            if node.style.flex_wrap == FlexWrap::NoWrap {
                size.min_content = size.min_content.max(sum.min_content + gaps);
            }
        } else if children.iter().all(|child| child.style.inline) {
            size.max_content = size.max_content.max(sum.max_content);
        }
        bounded(size.min_content)?;
        bounded(size.max_content)?;
        self.intrinsic.insert(local, size);
        Ok(size)
    }
    /** Dry runs reuse the same algorithm and budget, including controls and nested flex. */
    pub fn measure_height(&mut self, local: usize, width: f64) -> EngineResult<f64> {
        if let Some(height) = self.heights.get(&(local, width.to_bits())) {
            return Ok(*height);
        }
        let mut items = Vec::new();
        let height = self.layout_at(
            local,
            BorderBox {
                x: 0.0,
                y: 0.0,
                width,
                height: None,
            },
            &mut items,
        )?;
        self.heights.insert((local, width.to_bits()), height);
        Ok(height)
    }
    /** Final placement consumes allocated border dimensions directly. */
    pub fn layout_at(
        &mut self,
        local: usize,
        bounds: BorderBox,
        items: &mut Vec<DisplayItem>,
    ) -> EngineResult<f64> {
        let node = self.node(local)?;
        let start = items.len();
        let mut target = flow::Flow::new(bounds.x, bounds.y, bounds.width);
        flow::node_layout(self, node, &mut target, items, Some(bounds))?;
        Ok(items.get(start).map(|item| item.height).unwrap_or(0.0))
    }
}
/** Decorations are always excluded from flexible content sizes. */
pub(super) fn horizontal_edges(node: &Node) -> f64 {
    node.style.padding[1] + node.style.padding[3] + node.style.border * 2.0
}
/** Auto-height columns use the actual block-axis border/padding extent. */
pub(super) fn vertical_edges(node: &Node) -> f64 {
    node.style.padding[0] + node.style.padding[2] + node.style.border * 2.0
}
/** CSS box sizing applies identically to authored width, flex basis and width constraints. */
pub(super) fn content_size(node: &Node, value: f64, horizontal: bool) -> f64 {
    if node.style.box_sizing == BoxSizing::BorderBox {
        (value
            - if horizontal {
                horizontal_edges(node)
            } else {
                vertical_edges(node)
            })
        .max(0.0)
    } else {
        value
    }
}
/** Percent widths resolve against the definite containing content extent. */
pub(super) fn specified_content(node: &Node, available: f64) -> Option<f64> {
    node.style.width.map(|width| {
        content_size(
            node,
            if node.style.percent_width {
                available * width / 100.0
            } else {
                width
            },
            true,
        )
    })
}
/** Auto blocks fill the available border box; specified sizes can overflow their containing block. */
pub(super) fn border_width(
    node: &Node,
    containing: f64,
    available: f64,
    natural: Option<f64>,
) -> f64 {
    let edges = horizontal_edges(node);
    let min = node
        .style
        .min_width
        .map(|width| content_size(node, width, true))
        .unwrap_or(0.0);
    let max = node
        .style
        .max_width
        .map(|width| content_size(node, width, true))
        .unwrap_or(LAYOUT_EXTENT)
        .max(min);
    let content = specified_content(node, containing).unwrap_or_else(|| {
        natural
            .unwrap_or((available - edges).max(0.0))
            .min((available - edges).max(0.0))
    });
    content.max(min).min(max) + edges
}
/** Native sizing preferences retain the established undecorated control dimensions. */
fn text_intrinsic(context: &mut LayoutContext<'_>, node: &Node) -> EngineResult<IntrinsicWidths> {
    let font = node.style.font_size;
    let bold = node.style.bold;
    if node.tag == "img" {
        let width = match node.image_size {
            Some(size) => f64::from(size.0),
            None => context.measure(&node.text, font, bold)?,
        };
        return Ok(IntrinsicWidths {
            min_content: width,
            max_content: width,
        });
    }
    if ["input", "textarea", "select"].contains(&node.tag.as_str())
        && !(node.tag == "input" && ["submit", "button"].contains(&node.input_type()))
    {
        return Ok(IntrinsicWidths {
            min_content: 480.0,
            max_content: 480.0,
        });
    }
    if node.tag == "button"
        || (node.tag == "input" && ["submit", "button"].contains(&node.input_type()))
    {
        let title = if node.tag == "input" {
            if node.value.is_empty() {
                "Submit"
            } else {
                &node.value
            }
        } else {
            &node.text
        };
        let value = (context.measure(title, font, bold)? + 32.0).max(120.0);
        return Ok(IntrinsicWidths {
            min_content: value,
            max_content: value,
        });
    }
    context.charge(node.text.chars().count())?;
    let words = node.text.split_whitespace().collect::<Vec<_>>();
    let mut minimum: f64 = 0.0;
    for word in &words {
        minimum = minimum.max(context.measure(word, font, bold)?);
    }
    let maximum = if words.len() <= 1 {
        minimum
    } else {
        context.measure(&words.join(" "), font, bold)?
    };
    Ok(IntrinsicWidths {
        min_content: minimum,
        max_content: maximum,
    })
}

/** Intrinsic contributions clamp child constraints before adding decorations and margins. */
fn intrinsic_contribution(parent: &Node, child: &Node, raw: IntrinsicWidths) -> IntrinsicWidths {
    let preferred = (!child.style.percent_width)
        .then(|| specified_content(child, 0.0))
        .flatten();
    let minimum = child
        .style
        .min_width
        .map(|value| content_size(child, value, true))
        .unwrap_or(0.0);
    let maximum = child
        .style
        .max_width
        .map(|value| content_size(child, value, true))
        .unwrap_or(LAYOUT_EXTENT)
        .max(minimum);
    let clamp = |value: f64| value.max(minimum).min(maximum);
    let mut contribution = IntrinsicWidths {
        min_content: clamp(preferred.unwrap_or(raw.min_content)),
        max_content: clamp(preferred.unwrap_or(raw.max_content)),
    };
    if parent.style.display_flex && parent.style.flex_direction == FlexDirection::Row {
        row_basis_contribution(parent, child, raw, preferred, &mut contribution);
    }
    contribution
}

/** Pinned Blink row intrinsic rules preserve definite bases that zero flex factors cannot move. */
fn row_basis_contribution(
    parent: &Node,
    child: &Node,
    raw: IntrinsicWidths,
    preferred: Option<f64>,
    contribution: &mut IntrinsicWidths,
) {
    let basis = match child.style.flex_basis {
        crate::document::FlexBasis::Pixels(value) => Some(content_size(child, value, true)),
        crate::document::FlexBasis::Auto => preferred,
        crate::document::FlexBasis::Percent(_) => None,
    };
    let Some(base) = basis else {
        return;
    };
    let maximum = child
        .style
        .max_width
        .map(|value| content_size(child, value, true))
        .unwrap_or(LAYOUT_EXTENT);
    let minimum = child
        .style
        .min_width
        .map(|value| content_size(child, value, true))
        .unwrap_or(
            raw.min_content
                .min(preferred.unwrap_or(LAYOUT_EXTENT))
                .min(maximum),
        );
    let hypothetical = base.max(minimum).min(maximum.max(minimum));
    let cant_move = |value: f64| {
        (value > base && child.style.flex_grow == 0.0)
            || (value < base && child.style.flex_shrink == 0.0)
    };
    if parent.style.flex_wrap == FlexWrap::NoWrap && cant_move(contribution.min_content) {
        contribution.min_content = hypothetical;
    }
    if cant_move(contribution.max_content) {
        contribution.max_content = hypothetical;
    }
    contribution.max_content = contribution.max_content.max(contribution.min_content);
}
