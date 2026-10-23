/** V8 scanner-guided bounded lexical rules; upstream mapping and notices live beside this crate. */
mod number;
use super::{
    errors, literals,
    operator::{AssignmentOperator, Operator, UpdateOperator},
    source::{Source, line_terminator, whitespace},
    value::JsString,
};
use crate::{
    constants::{SCRIPT_BYTES, TOKENS},
    protocol::{EngineError, EngineResult, SourceLocation},
};
use number::number;

/** Tokens retain locations independently of the value or identifier they carry. */
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Token {
    pub kind: Kind,
    pub location: SourceLocation,
    /// Decoded equality cannot make an escaped string a strict-mode directive.
    pub exact_strict: bool,
}
/** Numeric/string literals retain JavaScript types through AST construction. */
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Kind {
    String(JsString),
    Number(f64),
    Name(String),
    Operator(Operator),
    Assignment(AssignmentOperator),
    Update(UpdateOperator),
    Symbol(char),
    /** Arrow heads are assignment-level grammar rather than binary operators. */
    Arrow,
    End,
}
/** Scanning consumes the complete bounded script before any evaluation can begin. */
pub(super) fn tokenize(source: &str) -> EngineResult<Vec<Token>> {
    tokenize_with_limit(source, SCRIPT_BYTES)
}
/** One scanner applies identical lexical rules under an operation-specific source bound. */
fn tokenize_with_limit(source: &str, limit: usize) -> EngineResult<Vec<Token>> {
    if source.len() > limit {
        return Err(EngineError::limit("script bytes", limit));
    }
    let mut source = Source::new(source);
    let mut tokens = Vec::new();
    while skip(&mut source)? {
        if tokens.len() >= TOKENS {
            return Err(errors::located(
                EngineError::limit("V9 tokens", TOKENS),
                source.location(),
                "scan",
            ));
        }
        let location = source.location();
        let exact_strict = source.rest().starts_with("\"use strict\"")
            || source.rest().starts_with("'use strict'");
        let kind = token(&mut source).map_err(|mut error| {
            error.phase = Some("scan".into());
            errors::located(error, location, "scan")
        })?;
        tokens.push(Token {
            kind,
            location,
            exact_strict,
        });
    }
    tokens.push(Token {
        kind: Kind::End,
        location: source.location(),
        exact_strict: false,
    });
    Ok(tokens)
}
/** Insert the handler's optional semicolon as a token so trailing comments cannot swallow it. */
pub(super) fn tokenize_handler(source: &str) -> EngineResult<Vec<Token>> {
    let mut tokens = tokenize(source)?;
    let end = tokens.pop().expect("the scanner always appends End");
    if !tokens
        .last()
        .is_some_and(|token| token.kind == Kind::Symbol(';'))
    {
        if tokens.len() >= TOKENS {
            return Err(errors::located(
                EngineError::limit("V9 tokens", TOKENS),
                end.location,
                "scan",
            ));
        }
        tokens.push(Token {
            kind: Kind::Symbol(';'),
            location: end.location,
            exact_strict: false,
        });
    }
    tokens.push(end);
    Ok(tokens)
}
/** Comments terminate on JavaScript line terminators, not Rust's broader Unicode whitespace. */
fn skip(source: &mut Source<'_>) -> EngineResult<bool> {
    loop {
        if source.peek().is_some_and(whitespace) {
            source.next();
            continue;
        }
        if source.rest().starts_with("//") {
            while source.peek().is_some_and(|c| !line_terminator(c)) {
                source.next();
            }
            continue;
        }
        if source.rest().starts_with("/*") {
            let start = source.location();
            source.next();
            source.next();
            while !source.rest().starts_with("*/") {
                if source.next().is_none() {
                    return Err(errors::located(
                        errors::syntax(start, "unterminated block comment"),
                        start,
                        "scan",
                    ));
                }
            }
            source.next();
            source.next();
            continue;
        }
        return Ok(source.peek().is_some());
    }
}
/** Longest-token checks retain supported operators without splitting unsupported prefixes. */
fn token(source: &mut Source<'_>) -> EngineResult<Kind> {
    let start = source.location();
    let character = source.peek().expect("scanner cursor is nonempty");
    if character == '\'' || character == '"' {
        source.next();
        return Ok(Kind::String(literals::string(source, character, start)?));
    }
    if character.is_ascii_digit()
        || character == '.'
            && source
                .rest()
                .chars()
                .nth(1)
                .is_some_and(|c| c.is_ascii_digit())
    {
        return number(source);
    }
    if identifier_start(character) {
        let mut name = String::new();
        while source
            .peek()
            .is_some_and(|c| identifier_start(c) || c.is_ascii_digit())
        {
            name.push(source.next().unwrap());
        }
        return Ok(Kind::Name(name));
    }
    if source.rest().starts_with("=>") {
        source.next();
        source.next();
        return Ok(Kind::Arrow);
    }
    if ["**", "&&=", "||=", "??", "?.", "<<", ">>", "&=", "|="]
        .iter()
        .any(|op| source.rest().starts_with(op))
    {
        return Err(errors::unsupported(start, "this operator"));
    }
    if let Some((spelling, kind)) = mutation_token(source.rest()) {
        for _ in spelling.chars() {
            source.next();
        }
        return Ok(kind);
    }
    if (source.rest().starts_with("==") && !source.rest().starts_with("==="))
        || (source.rest().starts_with("!=") && !source.rest().starts_with("!=="))
    {
        return Err(errors::unsupported(start, "coercive equality"));
    }
    if let Some((spelling, operator)) = Operator::scan(source.rest()) {
        for _ in spelling.chars() {
            source.next();
        }
        return Ok(Kind::Operator(operator));
    }
    if ".()=;{}[],:?".contains(character) {
        source.next();
        return Ok(Kind::Symbol(character));
    }
    let feature = match character {
        '`' => "template literals",
        '\\' => "escaped identifiers",
        '<' | '>' | '!' | '&' | '|' | '?' | ':' => "this expression operator",
        _ => "this token or non-ASCII identifier",
    };
    Err(errors::unsupported(start, feature))
}
/** Count and compound tokens are recognized only after rejecting every longer unsupported prefix. */
fn mutation_token(source: &str) -> Option<(&'static str, Kind)> {
    [
        ("++", Kind::Update(UpdateOperator::Increment)),
        ("--", Kind::Update(UpdateOperator::Decrement)),
        ("+=", Kind::Assignment(AssignmentOperator::Add)),
        ("-=", Kind::Assignment(AssignmentOperator::Subtract)),
        ("*=", Kind::Assignment(AssignmentOperator::Multiply)),
        ("/=", Kind::Assignment(AssignmentOperator::Divide)),
        ("%=", Kind::Assignment(AssignmentOperator::Remainder)),
    ]
    .into_iter()
    .find(|(spelling, _)| source.starts_with(spelling))
}
/** ASCII support is a disclosed first-port limit, not a Unicode compatibility claim. */
fn identifier_start(character: char) -> bool {
    character.is_ascii_alphabetic() || character == '_' || character == '$'
}
