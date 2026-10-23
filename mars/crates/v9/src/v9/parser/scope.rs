/** Early declaration conflicts are checked in every lexical scope before any source executes. */
use super::*;
use std::collections::{BTreeMap, BTreeSet};
/** Script vars belong to the object environment while direct lexical names own their separate scope. */
pub(super) fn validate(statements: &[Statement]) -> EngineResult<()> {
    validate_scope(statements, &[])
}
/** Parameters, direct functions and recursively declared vars may redeclare one shared function cell. */
fn declaration_names(statements: &[Statement], parameters: &[String]) -> BTreeSet<String> {
    let mut names = var_declarations(statements)
        .into_keys()
        .collect::<BTreeSet<_>>();
    names.extend(parameters.iter().cloned());
    names.extend(
        statements
            .iter()
            .filter_map(|statement| match &statement.kind {
                StatementKind::Function(name, _) => Some(name.clone()),
                _ => None,
            }),
    );
    names
}
/** A lexical name conflicts with descendant vars but sibling block names remain independent. */
fn validate_scope(statements: &[Statement], parameters: &[String]) -> EngineResult<()> {
    let declared = declaration_names(statements, parameters);
    let mut lexical = BTreeMap::new();
    for statement in statements {
        if let StatementKind::Binding(name, ..) = &statement.kind
            && (declared.contains(name) || lexical.insert(name, statement.location).is_some())
        {
            return Err(conflict(statement.location));
        }
        children(statement)?;
    }
    if declared.len() + lexical.len() > STATEMENTS {
        return Err(errors::located(
            EngineError::limit("V9 scope bindings", STATEMENTS),
            statements
                .first()
                .map(|item| item.location)
                .unwrap_or_default(),
            "parse",
        ));
    }
    Ok(())
}
/** Ordinary implicit arguments cannot become a fabricated undefined cell through a var declaration. */
pub(super) fn validate_function(literal: &FunctionLiteral) -> EngineResult<()> {
    validate_scope(&literal.body, &literal.parameters)?;
    if literal.kind == FunctionKind::Ordinary
        && !literal.strict
        && !explicit_arguments(literal)
        && let Some(location) = var_declarations(&literal.body).get("arguments")
    {
        return Err(errors::unsupported(
            *location,
            "implicit arguments objects with var redeclarations",
        ));
    }
    Ok(())
}
/** Parameter and direct-function replacements already supply the explicit arguments binding's value. */
fn explicit_arguments(literal: &FunctionLiteral) -> bool {
    literal.parameters.iter().any(|name| name == "arguments")
        || literal.body.iter().any(|statement| matches!(&statement.kind, StatementKind::Function(name, _) if name == "arguments"))
}
/** Never-entered blocks and never-called functions participate in the same early error checking. */
fn children(statement: &Statement) -> EngineResult<()> {
    match &statement.kind {
        StatementKind::Block(body) => validate_scope(body, &[]),
        StatementKind::Function(_, literal) => validate_function(literal),
        StatementKind::If(_, yes, no) => {
            children(yes)?;
            if let Some(no) = no {
                children(no)?;
            }
            Ok(())
        }
        StatementKind::While(_, body) => children(body),
        StatementKind::For(loop_) => validate_for(loop_),
        _ => Ok(()),
    }
}
/** For-let headers reject var declarations passing through their lexical scope and allow inner lexical shadowing. */
fn validate_for(loop_: &ForLoop) -> EngineResult<()> {
    if let Some(Statement {
        kind: StatementKind::Binding(name, ..),
        location,
    }) = loop_.initializer.as_deref()
        && var_declarations(std::slice::from_ref(loop_.body.as_ref())).contains_key(name)
    {
        return Err(conflict(*location));
    }
    children(&loop_.body)
}
/** Conflict errors identify the declaration without copying source or author values into diagnostics. */
fn conflict(location: SourceLocation) -> EngineError {
    errors::syntax(location, "Binding has already been declared in this scope")
}
