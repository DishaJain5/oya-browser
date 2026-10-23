/** Var grammar and one shared source walker preserve declaration-scope ownership across all engine boundaries. */
use super::*;
use std::collections::BTreeMap;
impl Parser {
    /** Each simple declarator consumes a declaration slot, including repeated names without initializers. */
    pub(super) fn var_list(&mut self, for_header: bool) -> EngineResult<Vec<VarDeclarator>> {
        let mut declarations = Vec::new();
        loop {
            if !declarations.is_empty() {
                self.statement_budget()?;
            }
            declarations.push(self.var_declarator(for_header)?);
            if self.peek().kind != Kind::Symbol(',') {
                return Ok(declarations);
            }
            self.next();
        }
    }
    /** Optional initialization remains explicit so redeclarations never manufacture undefined stores. */
    fn var_declarator(&mut self, for_header: bool) -> EngineResult<VarDeclarator> {
        let location = self.peek().location;
        let name = self.identifier()?;
        if for_header {
            self.reject_for_each()?;
        }
        let initializer = if self.peek().kind == Kind::Symbol('=') {
            self.next();
            let mut value = self.expression(0, 0)?;
            Self::infer_function_name(&mut value, &name);
            Some(value)
        } else {
            None
        };
        Ok(VarDeclarator {
            name,
            initializer,
            location,
        })
    }
}
/** Hoisting, source schemas and snapshot validation share this exact function-boundary-aware collector. */
pub(in crate::v9) fn var_declarations(
    statements: &[Statement],
) -> BTreeMap<String, SourceLocation> {
    let mut declarations = BTreeMap::new();
    let mut work = statements.iter().collect::<Vec<_>>();
    while let Some(statement) = work.pop() {
        collect_statement(statement, &mut work, &mut declarations);
    }
    declarations
}
/** Expressions cannot contribute declarations to their enclosing scope, even when they contain functions. */
fn collect_statement<'a>(
    statement: &'a Statement,
    work: &mut Vec<&'a Statement>,
    declarations: &mut BTreeMap<String, SourceLocation>,
) {
    match &statement.kind {
        StatementKind::Var(items) => {
            for item in items {
                declarations
                    .entry(item.name.clone())
                    .or_insert(item.location);
            }
        }
        StatementKind::Block(body) => work.extend(body),
        StatementKind::If(_, yes, no) => {
            work.push(yes);
            work.extend(no.as_deref());
        }
        StatementKind::While(_, body) => work.push(body),
        StatementKind::For(loop_) => {
            work.extend(loop_.initializer.as_deref());
            work.push(&loop_.body);
        }
        _ => {}
    }
}
