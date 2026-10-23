/** Standard-for parsing preserves lexical header ownership and explicitly excludes iterator grammar. */
use super::*;
impl Parser {
    /** Header delimiters belong to this production rather than ordinary statement parsing. */
    pub(super) fn for_loop(&mut self) -> EngineResult<StatementKind> {
        if self.peek().kind == Kind::Name("await".into()) {
            return Err(errors::unsupported(self.peek().location, "for await"));
        }
        self.symbol('(')?;
        let initializer = self.for_initializer()?;
        self.for_separator(';')?;
        let condition = if self.peek().kind == Kind::Symbol(';') {
            None
        } else {
            Some(self.expression(0, 0)?)
        };
        self.for_separator(';')?;
        let update = if self.peek().kind == Kind::Symbol(')') {
            None
        } else {
            Some(Box::new(self.for_action()?))
        };
        self.for_separator(')')?;
        self.loops += 1;
        let body = self.statement(false);
        self.loops -= 1;
        Ok(StatementKind::For(ForLoop {
            initializer,
            condition,
            update,
            body: Box::new(body?),
        }))
    }
    /** Var lists retain declaration-scope ownership; lexical headers still admit one declarator. */
    fn for_initializer(&mut self) -> EngineResult<Option<Box<Statement>>> {
        if self.peek().kind == Kind::Symbol(';') {
            return Ok(None);
        }
        let location = self.peek().location;
        let kind = match &self.peek().kind {
            Kind::Name(name) if name == "var" => {
                self.next();
                self.header_budget()?;
                StatementKind::Var(self.var_list(true)?)
            }
            Kind::Name(name) if name == "let" || name == "const" => {
                let kind = if name == "let" {
                    BindingKind::Let
                } else {
                    BindingKind::Const
                };
                self.next();
                self.header_budget()?;
                self.declaration(kind, true)?
            }
            _ => return self.for_action().map(|statement| Some(Box::new(statement))),
        };
        Ok(Some(Box::new(Statement { kind, location })))
    }
    /** Expression clauses share assignment/update grammar without creating a second store implementation. */
    fn for_action(&mut self) -> EngineResult<Statement> {
        self.header_budget()?;
        let location = self.peek().location;
        let kind = StatementKind::Expression(self.expression(0, 0)?);
        Ok(Statement { kind, location })
    }
    /** Unsupported header forms fail as a whole instead of being split into partial statements. */
    fn for_separator(&mut self, expected: char) -> EngineResult<()> {
        self.reject_for_each()?;
        if self.peek().kind == Kind::Symbol(',') {
            return Err(errors::unsupported(
                self.peek().location,
                "multiple declarators and comma expressions",
            ));
        }
        self.symbol(expected)
    }
    /** These contextual names are rejected only at a header delimiter, remaining valid ordinary identifiers. */
    pub(super) fn reject_for_each(&self) -> EngineResult<()> {
        if matches!(&self.peek().kind, Kind::Name(name) if name == "in" || name == "of") {
            return Err(errors::unsupported(
                self.peek().location,
                "for-in and for-of iteration",
            ));
        }
        Ok(())
    }
    /** Both initializer and update consume the same aggregate statement allocation allowance. */
    fn header_budget(&mut self) -> EngineResult<()> {
        self.statement_budget()
    }
}
