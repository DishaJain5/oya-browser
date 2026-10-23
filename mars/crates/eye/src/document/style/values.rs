/** Shared literal lengths, box values and colors remain finite and bounded before native rendering. */
use super::unsupported;
use crate::constants::{CSS_MAX_SCALAR, CSS_ROOT_FONT_SIZE};
use crate::protocol::EngineResult;
/** Supported lengths are finite positive CSS pixels or font-relative values. */
pub(super) fn length(value: &str, font: f64) -> EngineResult<f64> {
    let value = value.trim();
    if value == "auto" {
        return Ok(0.0);
    }
    if let Some(value) = value.strip_suffix("rem") {
        return Ok(number(value)? * CSS_ROOT_FONT_SIZE);
    }
    if let Some(value) = value.strip_suffix("em") {
        return Ok(number(value)? * font);
    }
    number(value.strip_suffix("px").unwrap_or(value))
}
/** Resource-safe numeric ranges exclude negative placement and extreme native geometry. */
pub(super) fn number(value: &str) -> EngineResult<f64> {
    value
        .parse::<f64>()
        .ok()
        .filter(|n| n.is_finite() && (0.0..=CSS_MAX_SCALAR).contains(n))
        .ok_or_else(unsupported)
}
/** Native color parsing accepts only these bounded literal forms. */
pub(super) fn color(value: &str) -> EngineResult<String> {
    if [
        "black",
        "white",
        "red",
        "green",
        "blue",
        "gray",
        "grey",
        "transparent",
        "navy",
        "teal",
    ]
    .contains(&value)
        || (value.starts_with('#')
            && [4, 7].contains(&value.len())
            && value[1..].bytes().all(|b| b.is_ascii_hexdigit()))
    {
        Ok(value.into())
    } else {
        Err(unsupported())
    }
}

/** Viewport-relative lengths use the visible page context supplied to this style transaction. */
pub(super) fn contextual_length(
    value: &str,
    font: f64,
    context: crate::document::RenderContext,
) -> EngineResult<f64> {
    if let Some(value) = value.strip_suffix("vh") {
        return Ok(number(value)? * context.height / 100.0);
    }
    length(value, font)
}
/** Both color branches are validated even when only one participates in the used scheme. */
pub(super) fn contextual_color(
    value: &str,
    scheme: crate::document::Appearance,
) -> EngineResult<String> {
    if let Some(pair) = value
        .strip_prefix("light-dark(")
        .and_then(|value| value.strip_suffix(')'))
    {
        let (light, dark) = pair.split_once(',').ok_or_else(unsupported)?;
        let light = color(light.trim())?;
        let dark = color(dark.trim())?;
        return Ok(if scheme == crate::document::Appearance::Light {
            light
        } else {
            dark
        });
    }
    color(value)
}
