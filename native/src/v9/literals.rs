/** V8 ScanString/ScanEscape-guided UTF-16 decoding; see THIRD_PARTY_NOTICES.md. */
use super::{
    errors,
    source::{Source, line_terminator},
    value::JsString,
};
use crate::{
    constants::JS_STRING_UNITS,
    protocol::{EngineError, EngineResult, SourceLocation},
};

/** String literals preserve surrogate units until an explicit document boundary. */
pub(super) fn string(
    source: &mut Source<'_>,
    quote: char,
    start: SourceLocation,
) -> EngineResult<JsString> {
    let mut units = Vec::new();
    while let Some(character) = source.next() {
        if character == quote {
            return JsString::from_units(units);
        }
        if matches!(character, '\n' | '\r') {
            return Err(errors::syntax(start, "unescaped line break in string"));
        }
        if character == '\\' {
            escape(source, &mut units, start)?;
        } else {
            push(character as u32, &mut units);
        }
        if units.len() > JS_STRING_UNITS {
            return Err(errors::located(
                EngineError::limit("JavaScript string UTF-16 units", JS_STRING_UNITS),
                start,
                "scan",
            ));
        }
    }
    Err(errors::syntax(start, "unterminated string literal"))
}
/** Non-escape characters are identity escapes; line continuations contribute no code units. */
fn escape(
    source: &mut Source<'_>,
    units: &mut Vec<u16>,
    start: SourceLocation,
) -> EngineResult<()> {
    let character = source
        .next()
        .ok_or_else(|| errors::syntax(start, "unterminated escape"))?;
    if line_terminator(character) {
        if character == '\r' && source.peek() == Some('\n') {
            source.next();
        }
        return Ok(());
    }
    let value = match character {
        'b' => 8,
        'f' => 12,
        'n' => 10,
        'r' => 13,
        't' => 9,
        'v' => 11,
        'x' => hex(source, 2, start)?,
        'u' => unicode(source, start)?,
        '0' if !source.peek().is_some_and(|c| c.is_ascii_digit()) => 0,
        '0'..='9' => {
            return Err(errors::unsupported(
                start,
                "legacy octal or non-octal decimal string escapes",
            ));
        }
        other => other as u32,
    };
    push(value, units);
    Ok(())
}
/** Fixed-width escapes require every promised hexadecimal digit. */
fn hex(source: &mut Source<'_>, count: usize, start: SourceLocation) -> EngineResult<u32> {
    let mut value = 0;
    for _ in 0..count {
        let digit = source
            .next()
            .and_then(|c| c.to_digit(16))
            .ok_or_else(|| errors::syntax(start, "invalid hexadecimal escape"))?;
        value = value * 16 + digit;
    }
    Ok(value)
}
/** Braced code points use a checked bound, including surrogate code points. */
fn unicode(source: &mut Source<'_>, start: SourceLocation) -> EngineResult<u32> {
    if source.peek() != Some('{') {
        return hex(source, 4, start);
    }
    source.next();
    let mut value = 0_u32;
    let mut digits = 0;
    while source.peek() != Some('}') {
        let digit = source
            .next()
            .and_then(|c| c.to_digit(16))
            .ok_or_else(|| errors::syntax(start, "invalid Unicode escape"))?;
        value = value
            .checked_mul(16)
            .and_then(|n| n.checked_add(digit))
            .filter(|n| *n <= 0x10ffff)
            .ok_or_else(|| errors::syntax(start, "Unicode escape exceeds the code-point range"))?;
        digits += 1;
    }
    source.next();
    if digits == 0 {
        return Err(errors::syntax(start, "empty Unicode escape"));
    }
    Ok(value)
}
/** Direct code-unit insertion retains lone surrogates instead of using Rust char conversion. */
fn push(value: u32, units: &mut Vec<u16>) {
    if value <= 0xffff {
        units.push(value as u16);
    } else {
        let value = value - 0x10000;
        units.push(0xd800 + (value >> 10) as u16);
        units.push(0xdc00 + (value & 0x3ff) as u16);
    }
}
