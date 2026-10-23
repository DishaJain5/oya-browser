/** Numeric tokens are consumed completely before validating literal grammar. */
use super::super::value::literal;
use super::*;
/** Literal scanning validates whole numeric tokens rather than accepting a numeric prefix. */
pub(super) fn number(source: &mut Source<'_>) -> EngineResult<Kind> {
    let start = source.location();
    let rest = source.rest();
    let prefixed = ["0x", "0X", "0o", "0O", "0b", "0B"]
        .iter()
        .any(|prefix| rest.starts_with(prefix));
    if prefixed {
        source.next();
        source.next();
        while source
            .peek()
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            source.next();
        }
    } else {
        while source
            .peek()
            .is_some_and(|c| c.is_ascii_digit() || c == '_')
        {
            source.next();
        }
        if source.peek() == Some('.') {
            source.next();
            while source
                .peek()
                .is_some_and(|c| c.is_ascii_digit() || c == '_')
            {
                source.next();
            }
        }
        if source.peek().is_some_and(|c| c == 'e' || c == 'E') {
            source.next();
            if source.peek().is_some_and(|c| c == '+' || c == '-') {
                source.next();
            }
            while source
                .peek()
                .is_some_and(|c| c.is_ascii_digit() || c == '_')
            {
                source.next();
            }
        }
        if source.peek() == Some('n') {
            source.next();
        }
    }
    let length = source.location().byte_offset - start.byte_offset;
    let value = &rest[..length];
    if value.ends_with('n') {
        return Err(errors::unsupported(start, "BigInt literals"));
    }
    if source
        .peek()
        .is_some_and(|c| identifier_start(c) || c == '\\' || c.is_ascii_digit())
    {
        return Err(errors::syntax(start, "invalid numeric suffix"));
    }
    if !prefixed
        && value.len() > 1
        && value.starts_with('0')
        && value.as_bytes()[1].is_ascii_digit()
    {
        return Err(errors::unsupported(
            start,
            "legacy leading-zero numeric literals",
        ));
    }
    Ok(Kind::Number(literal(value).map_err(|_| {
        errors::syntax(start, "invalid numeric literal")
    })?))
}
