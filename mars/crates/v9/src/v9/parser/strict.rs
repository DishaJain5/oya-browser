/** Source-owned language modes enforce early errors even in never-entered function bodies. */
use super::*;

/** Strict bindings and stores cannot use the scanner's supported restricted identifiers. */
pub(super) fn name(value: &str, location: SourceLocation) -> EngineResult<()> {
    if [
        "eval",
        "arguments",
        "implements",
        "interface",
        "package",
        "private",
        "protected",
        "public",
        "static",
        "yield",
    ]
    .contains(&value)
    {
        return Err(errors::syntax(
            location,
            "Invalid identifier in strict mode",
        ));
    }
    Ok(())
}
/** An own directive is discovered after parameters, so their validation must wait for the body. */
pub(super) fn function(literal: &FunctionLiteral, location: SourceLocation) -> EngineResult<()> {
    if literal.strict {
        for parameter in literal.parameters.iter().chain(literal.name.iter()) {
            name(parameter, location)?;
        }
    }
    validate(&literal.body, literal.strict)
}
/** Every supported statement participates, including unreachable branches and for headers. */
pub(super) fn validate(body: &[Statement], strict: bool) -> EngineResult<()> {
    for statement in body {
        check(statement, strict)?;
    }
    Ok(())
}
/** Nested function modes belong to their defining source rather than the caller. */
fn check(statement: &Statement, strict: bool) -> EngineResult<()> {
    let location = statement.location;
    match &statement.kind {
        StatementKind::Var(declarations) => {
            for declaration in declarations {
                if strict {
                    name(&declaration.name, declaration.location)?;
                }
                if let Some(value) = &declaration.initializer {
                    expression(value, strict)?;
                }
            }
            Ok(())
        }
        StatementKind::Binding(binding, _, value) => {
            if strict {
                name(binding, location)?;
            }
            expression(value, strict)
        }
        StatementKind::Function(binding, literal) => {
            if strict || literal.strict {
                name(binding, location)?;
            }
            function(literal, location)
        }
        StatementKind::Expression(value)
        | StatementKind::Set(_, value)
        | StatementKind::Return(Some(value)) => expression(value, strict),
        StatementKind::Block(body) => validate(body, strict),
        StatementKind::If(test, yes, no) => {
            expression(test, strict)?;
            check(yes, strict)?;
            if let Some(no) = no {
                check(no, strict)?;
            }
            Ok(())
        }
        StatementKind::While(test, body) => {
            expression(test, strict)?;
            check(body, strict)
        }
        StatementKind::For(loop_) => {
            for clause in [loop_.initializer.as_deref(), loop_.update.as_deref()]
                .into_iter()
                .flatten()
            {
                check(clause, strict)?;
            }
            if let Some(test) = &loop_.condition {
                expression(test, strict)?;
            }
            check(&loop_.body, strict)
        }
        StatementKind::Call(binding, args) => {
            if strict && binding != "eval" && binding != "arguments" {
                name(binding, location)?;
            }
            expressions(args, strict)
        }
        _ => Ok(()),
    }
}
/** Expression children retain mode without confusing property names with identifier references. */
fn expression(value: &Expression, strict: bool) -> EngineResult<()> {
    match &value.kind {
        ExpressionKind::Function(literal) => function(literal, value.location),
        ExpressionKind::Variable(binding)
            if strict && binding != "eval" && binding != "arguments" =>
        {
            name(binding, value.location)
        }
        ExpressionKind::Unary(_, value) => expression(value, strict),
        ExpressionKind::Assignment(_, target, value) => {
            store_target(target, strict)?;
            expression(value, strict)
        }
        ExpressionKind::Update(_, _, target) => store_target(target, strict),
        ExpressionKind::Conditional(test, yes, no) => {
            expression(test, strict)?;
            expression(yes, strict)?;
            expression(no, strict)
        }
        ExpressionKind::Binary(_, left, right) | ExpressionKind::Member(left, right) => {
            expression(left, strict)?;
            expression(right, strict)
        }
        ExpressionKind::Invoke(callee, args) => {
            expression(callee, strict)?;
            expressions(args, strict)
        }
        ExpressionKind::Array(elements) => {
            for element in elements {
                expression(element.operand(), strict)?;
            }
            Ok(())
        }
        ExpressionKind::StorageCall(_, _, args) => expressions(args, strict),
        ExpressionKind::Object(properties) => {
            for (_, value) in properties {
                expression(value, strict)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}
/** Identifier stores have extra strict restrictions; property names remain ordinary data keys. */
fn store_target(target: &Expression, strict: bool) -> EngineResult<()> {
    if strict && let ExpressionKind::Variable(binding) = &target.kind {
        name(binding, target.location)?;
    }
    expression(target, strict)
}
/** Arguments and elements use the same bounded AST walk. */
fn expressions(values: &[Expression], strict: bool) -> EngineResult<()> {
    for value in values {
        expression(value, strict)?;
    }
    Ok(())
}
