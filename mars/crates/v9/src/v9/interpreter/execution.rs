/** Structured completion preserves lexical cleanup and atomic effect proposals. */
use super::*;
impl Interpreter {
    /** A list stops at abrupt completion, leaving cleanup to its owning scope. */
    pub(super) fn statements(&mut self, body: Vec<Statement>) -> EngineResult<Completion> {
        for statement in body {
            let result = self.statement(statement)?;
            if !matches!(result, Completion::Normal) {
                return Ok(result);
            }
        }
        Ok(Completion::Normal)
    }
    /** Declarations were instantiated before execution; statements share one work budget. */
    pub(super) fn statement(&mut self, statement: Statement) -> EngineResult<Completion> {
        self.step(statement.location)?;
        match statement.kind {
            StatementKind::Binding(name, kind, value) => {
                self.binding(name, kind, value, statement.location)?
            }
            StatementKind::Var(declarations) => self.var_initializers(declarations)?,
            StatementKind::Set(id, value) => self.set_text(id, value)?,
            StatementKind::Call(name, args) => {
                self.call(&name, args, statement.location)?;
            }
            StatementKind::Expression(value) => {
                self.expression(value)?;
            }
            StatementKind::Return(value) => {
                return Ok(Completion::Return(
                    value
                        .map(|value| self.expression(value))
                        .unwrap_or(Ok(JsValue::Undefined))?,
                ));
            }
            StatementKind::Block(body) => return self.block(body, statement.location),
            StatementKind::If(test, yes, no) => return self.branch(test, *yes, no.map(|no| *no)),
            StatementKind::While(test, body) => return self.loop_while(test, *body),
            StatementKind::For(loop_) => return self.loop_for(loop_, statement.location),
            StatementKind::Break => return Ok(Completion::Break),
            StatementKind::Continue => return Ok(Completion::Continue),
            StatementKind::Function(..) | StatementKind::Empty => {}
        }
        Ok(Completion::Normal)
    }
    /** Missing var initializers preserve hoisted values; present ones assign in source order. */
    fn var_initializers(
        &mut self,
        declarations: Vec<super::super::parser::VarDeclarator>,
    ) -> EngineResult<()> {
        for declaration in declarations {
            self.step(declaration.location)?;
            if let Some(initializer) = declaration.initializer {
                self.assign(declaration.name, initializer, declaration.location)?;
            }
        }
        Ok(())
    }
    /** Popping removes active ownership, not records captured by reachable closures. */
    fn block(
        &mut self,
        body: Vec<Statement>,
        location: SourceLocation,
    ) -> EngineResult<Completion> {
        let base = self.locals.len();
        self.enter_scope(ScopeId {
            site: location.byte_offset,
            kind: ScopeKind::Block,
        })
        .map_err(|error| errors::located(error, location, "execute"))?;
        let result = self.statements(body);
        self.locals.truncate(base);
        result
    }
    /** Only the selected arm evaluates callbacks or mutations. */
    fn branch(
        &mut self,
        test: Expression,
        yes: Statement,
        no: Option<Statement>,
    ) -> EngineResult<Completion> {
        if self.expression(test)?.truthy() {
            self.statement(yes)
        } else if let Some(no) = no {
            self.statement(no)
        } else {
            Ok(Completion::Normal)
        }
    }
    /** Empty loops remain bounded independently of their source condition. */
    fn loop_while(&mut self, test: Expression, body: Statement) -> EngineResult<Completion> {
        loop {
            self.step(test.location)?;
            if !self.expression(test.clone())?.truthy() {
                return Ok(Completion::Normal);
            }
            match self.statement(body.clone())? {
                Completion::Normal | Completion::Continue => {}
                Completion::Break => return Ok(Completion::Normal),
                returned @ Completion::Return(_) => return Ok(returned),
            }
        }
    }
    /** Text proposals await document and authority validation before publication. */
    fn set_text(&mut self, id: String, value: Expression) -> EngineResult<()> {
        let location = value.location;
        let text = self
            .expression(value)?
            .to_dom()
            .map_err(|error| errors::located(error, location, "dom"))?;
        if text.len() > TEXT_BYTES {
            return Err(errors::located(
                EngineError::limit("DOM text UTF-8 bytes", TEXT_BYTES),
                location,
                "dom",
            ));
        }
        if self.changes.len() >= STATEMENTS {
            return Err(errors::located(
                EngineError::limit("V9 changes", STATEMENTS),
                location,
                "execute",
            ));
        }
        self.changes.push(TextChange { html_id: id, text });
        Ok(())
    }
}
