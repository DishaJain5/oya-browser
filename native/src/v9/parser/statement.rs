/** Control grammar retains the restricted DOM and named-handler seams around ordinary expression statements. */
use super::*;
impl Parser {
    /** Declarations and control forms keep dedicated productions; every other statement uses expression grammar. */
    pub(super) fn statement_kind(&mut self, list_item: bool) -> EngineResult<Statement> {
        let location = self.peek().location;
        let kind = match &self.peek().kind {
            Kind::Symbol('{') => StatementKind::Block(self.block()?),
            Kind::Symbol(';') => {
                self.next();
                StatementKind::Empty
            }
            Kind::Name(name) if statement_keyword(name) && self.arrow_position().is_none() => {
                self.named_statement(list_item)?
            }
            _ => self.expression_statement()?,
        };
        Ok(Statement { kind, location })
    }
    /** Known statement keywords cannot become accidental variable reads or dynamic calls. */
    fn named_statement(&mut self, list_item: bool) -> EngineResult<StatementKind> {
        let location = self.peek().location;
        let name = self.name()?;
        match name.as_str() {
            "let" | "const" if !list_item => Err(errors::syntax(
                location,
                "lexical declarations require a statement list",
            )),
            "let" => self.binding(BindingKind::Let),
            "const" => self.binding(BindingKind::Const),
            "var" => {
                let declarations = self.var_list(false)?;
                self.symbol(';')?;
                Ok(StatementKind::Var(declarations))
            }
            "function" => self.function(list_item),
            "document" => self.assignment(),
            "if" => self.conditional(),
            "while" => self.while_loop(),
            "for" => self.for_loop(),
            "break" | "continue" => self.jump(&name, location),
            "return" if self.in_function => self.return_statement(),
            "return" => Err(errors::syntax(location, "Illegal return statement")),
            _ => Err(errors::unsupported(location, "this statement form")),
        }
    }
    /** Only a direct identifier call becomes the handler-compatible statement variant. */
    fn expression_statement(&mut self) -> EngineResult<StatementKind> {
        let direct_name = match &self.peek().kind {
            Kind::Name(name)
                if self
                    .tokens
                    .get(self.position + 1)
                    .is_some_and(|token| token.kind == Kind::Symbol('(')) =>
            {
                Some(name.clone())
            }
            _ => None,
        };
        let expression = self.expression(0, 0)?;
        self.symbol(';')?;
        if let ExpressionKind::Invoke(callee, arguments) = &expression.kind
            && let ExpressionKind::Variable(name) = &callee.kind
            && direct_name.as_ref() == Some(name)
        {
            return Ok(StatementKind::Call(name.clone(), arguments.clone()));
        }
        Ok(StatementKind::Expression(expression))
    }
    /** Root and direct function-body declarations have hoisted cells; block declarations remain unsupported. */
    fn function(&mut self, list_item: bool) -> EngineResult<StatementKind> {
        if !list_item
            || !(self.depth == 1 && !self.in_function || self.function_body == Some(self.depth))
        {
            return Err(errors::unsupported(
                self.peek().location,
                "block function declarations",
            ));
        }
        let name = self.identifier()?;
        let mut literal = self.function_body(None)?;
        literal.display_name = name.clone();
        if literal.strict {
            super::strict::name(&name, self.peek().location)?;
        }
        Ok(StatementKind::Function(name, Box::new(literal)))
    }
    /** Simple unique parameters bind into one bounded call frame. */
    pub(super) fn parameters(&mut self) -> EngineResult<Vec<String>> {
        let mut parameters = Vec::new();
        while self.peek().kind != Kind::Symbol(')') {
            let location = self.peek().location;
            let name = self.identifier()?;
            if parameters.contains(&name) {
                return Err(errors::syntax(location, "duplicate function parameter"));
            }
            parameters.push(name);
            if parameters.len() > crate::constants::FUNCTION_PARAMETERS {
                return Err(errors::located(
                    EngineError::limit(
                        "V9 function parameters",
                        crate::constants::FUNCTION_PARAMETERS,
                    ),
                    location,
                    "parse",
                ));
            }
            if self.peek().kind != Kind::Symbol(',') {
                break;
            }
            self.next();
        }
        Ok(parameters)
    }
    /** Calls accept bounded positional arguments and are parsed in source order. */
    pub(super) fn arguments(&mut self, nesting: usize) -> EngineResult<Vec<Expression>> {
        let mut arguments = Vec::new();
        while self.peek().kind != Kind::Symbol(')') {
            if arguments.len() >= crate::constants::FUNCTION_PARAMETERS {
                return Err(errors::located(
                    EngineError::limit(
                        "V9 function arguments",
                        crate::constants::FUNCTION_PARAMETERS,
                    ),
                    self.peek().location,
                    "parse",
                ));
            }
            arguments.push(self.expression(0, nesting + 1)?);
            if self.peek().kind != Kind::Symbol(',') {
                break;
            }
            self.next();
        }
        Ok(arguments)
    }
    /** Returns propagate as function completion and cannot escape the current function. */
    fn return_statement(&mut self) -> EngineResult<StatementKind> {
        let value = if self.peek().kind == Kind::Symbol(';') {
            None
        } else {
            Some(self.expression(0, 0)?)
        };
        self.symbol(';')?;
        Ok(StatementKind::Return(value))
    }
    /** Recursive body parsing naturally binds an else to the nearest unmatched if. */
    fn conditional(&mut self) -> EngineResult<StatementKind> {
        let condition = self.condition()?;
        let yes = Box::new(self.statement(false)?);
        let no = if self.peek().kind == Kind::Name("else".into()) {
            self.next();
            Some(Box::new(self.statement(false)?))
        } else {
            None
        };
        Ok(StatementKind::If(condition, yes, no))
    }
    /** Loop targets exist only while parsing the body and never cross function boundaries. */
    fn while_loop(&mut self) -> EngineResult<StatementKind> {
        let condition = self.condition()?;
        self.loops += 1;
        let body = self.statement(false);
        self.loops -= 1;
        Ok(StatementKind::While(condition, Box::new(body?)))
    }
    /** Both supported branch forms require an explicitly parenthesized test. */
    fn condition(&mut self) -> EngineResult<Expression> {
        self.symbol('(')?;
        let expression = self.expression(0, 0)?;
        self.symbol(')')?;
        Ok(expression)
    }
    /** Illegal jumps fail at parse time even inside an unreachable branch. */
    fn jump(&mut self, name: &str, location: SourceLocation) -> EngineResult<StatementKind> {
        if self.loops == 0 {
            return Err(errors::syntax(
                location,
                if name == "break" {
                    "Illegal break statement"
                } else {
                    "Illegal continue statement"
                },
            ));
        }
        if matches!(self.peek().kind, Kind::Name(_)) {
            return Err(errors::unsupported(
                self.peek().location,
                "labeled control flow",
            ));
        }
        self.symbol(';')?;
        Ok(if name == "break" {
            StatementKind::Break
        } else {
            StatementKind::Continue
        })
    }
}

/** Statement keywords stay separate from expression operators and page-owned identifiers. */
fn statement_keyword(name: &str) -> bool {
    [
        "let", "const", "var", "function", "document", "if", "while", "for", "break", "continue",
        "return", "throw", "class", "import", "export", "try", "async", "do", "switch",
    ]
    .contains(&name)
}
