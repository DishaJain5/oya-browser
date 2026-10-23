/** The supported font shorthand resets its longhands before cascade selection, following Blink ConsumeFont. */
use super::{
    LineHeight, unsupported,
    values::{length, number},
};
use crate::protocol::EngineResult;
/** Optional weight precedes the required size, optional slash advance and native family list. */
pub(super) fn shorthand(source: &str) -> EngineResult<Vec<(String, String)>> {
    let separated = source.replace('/', " / ");
    let tokens = separated.split_whitespace().collect::<Vec<_>>();
    let size = tokens
        .iter()
        .position(|token| ["px", "em", "rem"].iter().any(|unit| token.ends_with(unit)))
        .ok_or_else(unsupported)?;
    let mut weight = "normal";
    for token in &tokens[..size] {
        match *token {
            "normal" => {}
            "bold" | "400" | "500" | "600" | "700" | "800" | "900" if weight == "normal" => {
                weight = token
            }
            _ => return Err(unsupported()),
        }
    }
    length(tokens[size], crate::constants::CSS_ROOT_FONT_SIZE)?;
    let (advance, family_start) = if tokens.get(size + 1) == Some(&"/") {
        (*tokens.get(size + 2).ok_or_else(unsupported)?, size + 3)
    } else {
        ("normal", size + 1)
    };
    let family = tokens
        .get(family_start..)
        .filter(|parts| !parts.is_empty())
        .ok_or_else(unsupported)?
        .join(" ");
    family_supported(&family)?;
    Ok(vec![
        ("font-weight".into(), weight.into()),
        ("font-style".into(), "normal".into()),
        ("font-size".into(), tokens[size].into()),
        ("line-height".into(), advance.into()),
        ("font-family".into(), family),
    ])
}
/** Native measurement and drawing already share the system sans-serif strategy. */
pub(super) fn family_supported(source: &str) -> EngineResult<()> {
    if source
        .split(',')
        .all(|name| ["system-ui", "sans-serif"].contains(&name.trim()))
    {
        Ok(())
    } else {
        Err(unsupported())
    }
}
/** Percentages compute once; unitless factors remain relative through inheritance. */
pub(super) fn line_height(value: &str, font: f64) -> EngineResult<LineHeight> {
    if value == "normal" {
        return Ok(LineHeight::Normal);
    }
    if let Some(percent) = value.strip_suffix('%') {
        return Ok(LineHeight::Length(font * number(percent)? / 100.0));
    }
    if let Ok(factor) = number(value) {
        return Ok(LineHeight::Number(factor));
    }
    Ok(LineHeight::Length(length(value, font)?))
}
