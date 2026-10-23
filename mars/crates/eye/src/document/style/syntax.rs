/** Bounded stylesheet grammar expands declarations before cascade resolution. */
use super::{
    selectors::{specificity, validate_selector},
    unsupported,
};
use crate::{
    constants::STYLE_BYTES,
    protocol::{EngineError, EngineResult},
};
/** The parser never evaluates URLs, variables or host callbacks. */
pub(super) struct Rule {
    pub selector: String,
    pub declarations: Vec<(String, String, bool)>,
    pub specificity: usize,
    pub order: usize,
}

/** At-rules and unresolved nested syntax cannot silently expose hidden controls. */
pub(super) fn parse_sheet(sheet: &str, rules: &mut Vec<Rule>) -> EngineResult<()> {
    if sheet.len() > STYLE_BYTES {
        return Err(EngineError::limit("stylesheet bytes", STYLE_BYTES));
    }
    let mut source = String::new();
    let mut rest = sheet;
    while let Some(start) = rest.find("/*") {
        source.push_str(&rest[..start]);
        let end = rest[start + 2..].find("*/").ok_or_else(unsupported)? + start + 4;
        rest = &rest[end..];
    }
    source.push_str(rest);
    let mut rest = source.trim();
    while !rest.is_empty() {
        let (selectors, tail) = rest.split_once('{').ok_or_else(unsupported)?;
        let (body, tail) = tail.split_once('}').ok_or_else(unsupported)?;
        if selectors.contains('@') || body.contains(['{', '}']) {
            return Err(unsupported());
        }
        let declarations = declarations(body)?;
        for selector in selectors.split(',') {
            let selector = selector.trim();
            validate_selector(selector)?;
            rules.push(Rule {
                selector: selector.into(),
                declarations: declarations.clone(),
                specificity: specificity(selector),
                order: rules.len(),
            });
            if rules.len() > crate::constants::NODES {
                return Err(EngineError::limit("CSS rules", crate::constants::NODES));
            }
        }
        rest = tail.trim();
    }
    Ok(())
}
/** Simple declaration grammar refuses unresolved values instead of guessing layout. */
pub(super) fn declarations(source: &str) -> EngineResult<Vec<(String, String, bool)>> {
    let mut declarations = Vec::new();
    for declaration in source.split(';').filter(|s| !s.trim().is_empty()) {
        let (key, value) = declaration.split_once(':').ok_or_else(unsupported)?;
        let key = key.trim().to_ascii_lowercase();
        let mut value = value.trim().to_ascii_lowercase();
        let important = value.ends_with("!important");
        if important {
            value.truncate(value.len() - "!important".len());
            value = value.trim().into();
        }
        if key.is_empty() || value.is_empty() || value.contains(['{', '}', '!']) {
            return Err(unsupported());
        }
        declarations.extend(super::shorthand::expand(key, value, important)?);
    }
    Ok(declarations)
}
