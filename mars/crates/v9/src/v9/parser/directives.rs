/** Only program and function statement lists can raise their source-owned language mode. */
use super::*;
impl Parser {
    /** A complete list gives lexical validation all declarations, including unreachable ones. */
    pub(super) fn block(&mut self) -> EngineResult<Vec<Statement>> {
        self.symbol('{')?;
        let statements = self.statement_list(Kind::Symbol('}'), false)?;
        self.symbol('}')?;
        Ok(statements)
    }
    /** Only script and function lists have directive prologues; ordinary blocks do not. */
    pub(super) fn function_block(&mut self) -> EngineResult<Vec<Statement>> {
        self.symbol('{')?;
        let statements = self.statement_list(Kind::Symbol('}'), true)?;
        self.symbol('}')?;
        Ok(statements)
    }
    /** Exact raw spelling and a direct literal statement jointly activate strictness. */
    pub(super) fn statement_list(
        &mut self,
        end: Kind,
        mut prologue: bool,
    ) -> EngineResult<Vec<Statement>> {
        let mut statements = Vec::new();
        while self.peek().kind != end {
            if self.peek().kind == Kind::End {
                return Err(errors::syntax(
                    self.peek().location,
                    "expected closing brace",
                ));
            }
            let token = self.peek().clone();
            let statement = self.statement(true)?;
            prologue &= matches!(token.kind, Kind::String(_))
                && matches!(
                    &statement.kind,
                    StatementKind::Expression(Expression {
                        kind: ExpressionKind::Literal(super::super::JsValue::String(_)),
                        ..
                    })
                );
            self.strict |= prologue && token.exact_strict;
            statements.push(statement);
        }
        Ok(statements)
    }
}
