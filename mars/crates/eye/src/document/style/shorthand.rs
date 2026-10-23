/** Shorthands expand at their original declaration position before independent cascade winners are chosen. */
use super::{flex, unsupported, values::number};
use crate::protocol::EngineResult;
/** Expanded longhands retain one declaration's exact importance, specificity and order. */
pub(super) fn expand(
    key: String,
    value: String,
    important: bool,
) -> EngineResult<Vec<(String, String, bool)>> {
    let values = match key.as_str() {
        "flex" => flex_values(&value)?,
        "font" => super::font::shorthand(&value)?,
        "gap" => gap_values(&value)?,
        "margin" | "padding" => box_values(&key, &value)?,
        _ => vec![(key, value)],
    };
    Ok(values
        .into_iter()
        .map(|(key, value)| (key, value, important))
        .collect())
}
/** Omitted shorthand basis is zero percent; an explicit third numeric zero is zero pixels. */
fn flex_values(value: &str) -> EngineResult<Vec<(String, String)>> {
    let (grow, shrink, basis) = match value {
        "none" => ("0".into(), "0".into(), "auto".into()),
        "initial" => ("0".into(), "1".into(), "auto".into()),
        _ => parse_flex(value)?,
    };
    Ok(vec![
        ("flex-grow".into(), grow),
        ("flex-shrink".into(), shrink),
        ("flex-basis".into(), basis),
    ])
}
/** Blink's factor-first ambiguity rule admits a basis before both factors or after their completed pair. */
fn parse_flex(value: &str) -> EngineResult<(String, String, String)> {
    let (mut grow, mut shrink, mut basis) = (None, None, None);
    for token in value.split_whitespace() {
        if let Ok(factor) = number(token) {
            if grow.is_none() {
                grow = Some(token.to_owned());
            } else if shrink.is_none() {
                shrink = Some(token.to_owned());
            } else if basis.is_none() && factor == 0.0 {
                basis = Some("0px".into());
            } else {
                return Err(unsupported());
            }
        } else if basis.is_none() {
            flex::basis(token)?;
            if grow.is_some() && shrink.is_none() {
                shrink = Some("1".into());
            }
            basis = Some(token.to_owned());
        } else {
            return Err(unsupported());
        }
    }
    if grow.is_none() && shrink.is_none() && basis.is_none() {
        return Err(unsupported());
    }
    Ok((
        grow.unwrap_or_else(|| "1".into()),
        shrink.unwrap_or_else(|| "1".into()),
        basis.unwrap_or_else(|| "0%".into()),
    ))
}
/** A single gap sets both axes while two values retain row-then-column order. */
fn gap_values(value: &str) -> EngineResult<Vec<(String, String)>> {
    let values = value.split_whitespace().collect::<Vec<_>>();
    let (row, column) = match values.as_slice() {
        [all] => (*all, *all),
        [row, column] => (*row, *column),
        _ => return Err(unsupported()),
    };
    flex::pixels(row)?;
    flex::pixels(column)?;
    Ok(vec![
        ("row-gap".into(), row.into()),
        ("column-gap".into(), column.into()),
    ])
}
/** Existing clockwise margin/padding values stay textual until the winning font size is known. */
fn box_values(key: &str, value: &str) -> EngineResult<Vec<(String, String)>> {
    let values = value.split_whitespace().collect::<Vec<_>>();
    let expanded = match values.as_slice() {
        [a] => [*a; 4],
        [a, b] => [*a, *b, *a, *b],
        [a, b, c] => [*a, *b, *c, *b],
        [a, b, c, d] => [*a, *b, *c, *d],
        _ => return Err(unsupported()),
    };
    Ok(["top", "right", "bottom", "left"]
        .into_iter()
        .zip(expanded)
        .map(|(side, value)| (format!("{key}-{side}"), value.into()))
        .collect())
}
