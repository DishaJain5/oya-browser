/** Supported flex longhands preserve definite pixels, percentages and automatic main sizes. */
use super::{
    AlignItems, BoxSizing, ComputedStyle, FlexBasis, FlexDirection, FlexWrap, JustifyContent,
    unsupported, values::number,
};
use crate::protocol::EngineResult;
/** New flex values are explicit; unrelated properties remain owned by the legacy declaration strategy. */
pub(super) fn apply(style: &mut ComputedStyle, key: &str, value: &str) -> EngineResult<bool> {
    match key {
        "box-sizing" => {
            style.box_sizing = match value {
                "content-box" => BoxSizing::ContentBox,
                "border-box" => BoxSizing::BorderBox,
                _ => return Err(unsupported()),
            }
        }
        "flex-direction" => {
            style.flex_direction = match value {
                "row" => FlexDirection::Row,
                "column" => FlexDirection::Column,
                _ => return Err(unsupported()),
            }
        }
        "flex-wrap" => {
            style.flex_wrap = match value {
                "nowrap" => FlexWrap::NoWrap,
                "wrap" => FlexWrap::Wrap,
                _ => return Err(unsupported()),
            }
        }
        "flex-basis" => style.flex_basis = basis(value)?,
        "flex-grow" => style.flex_grow = number(value)?,
        "flex-shrink" => style.flex_shrink = number(value)?,
        "row-gap" => style.row_gap = pixels(value)?,
        "column-gap" => style.column_gap = pixels(value)?,
        "justify-content" => {
            style.justify_content = match value {
                "start" | "flex-start" => JustifyContent::Start,
                "center" => JustifyContent::Center,
                "end" | "flex-end" => JustifyContent::End,
                "space-between" => JustifyContent::SpaceBetween,
                _ => return Err(unsupported()),
            }
        }
        "align-items" => {
            style.align_items = match value {
                "start" | "flex-start" => AlignItems::Start,
                "center" => AlignItems::Center,
                "end" | "flex-end" => AlignItems::End,
                "stretch" => AlignItems::Stretch,
                _ => return Err(unsupported()),
            }
        }
        "min-width" => {
            style.min_width = if value == "auto" {
                None
            } else {
                Some(pixels(value)?)
            }
        }
        _ => return Ok(false),
    }
    Ok(true)
}
/** Basis percentages remain unresolved until layout knows whether the main extent is definite. */
pub(super) fn basis(value: &str) -> EngineResult<FlexBasis> {
    if value == "auto" {
        return Ok(FlexBasis::Auto);
    }
    if let Some(value) = value.strip_suffix('%') {
        return number(value).map(FlexBasis::Percent);
    }
    pixels(value).map(FlexBasis::Pixels)
}
/** New structural lengths accept pixels or unitless zero; unsupported units cannot guess layout. */
pub(super) fn pixels(value: &str) -> EngineResult<f64> {
    if let Some(value) = value.strip_suffix("px") {
        return number(value);
    }
    match number(value)? {
        0.0 => Ok(0.0),
        _ => Err(unsupported()),
    }
}
