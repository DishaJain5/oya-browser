/** Bounded arrow-head recognition preserves assignment precedence without speculative reparsing. */
use super::*;

/** The token cap bounds both the matching table and the transient nesting stack. */
pub(super) fn parentheses(tokens: &[Token]) -> Vec<Option<usize>> {
    let mut matches = vec![None; tokens.len()];
    let mut opens = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        match token.kind {
            Kind::Symbol('(') => opens.push(index),
            Kind::Symbol(')') => {
                if let Some(open) = opens.pop() {
                    matches[open] = Some(index);
                }
            }
            _ => {}
        }
    }
    matches
}
impl Parser {
    /** Only a potential complete head can claim arrow parsing at assignment precedence. */
    pub(super) fn arrow_position(&self) -> Option<usize> {
        self.arrow_at(self.position)
    }
    /** Precomputed closing indices make repeated grouping lookahead constant-time. */
    fn arrow_at(&self, index: usize) -> Option<usize> {
        let next = match self.tokens.get(index)?.kind {
            Kind::Name(_) => index + 1,
            Kind::Symbol('(') => self.parentheses.get(index).copied().flatten()? + 1,
            _ => return None,
        };
        self.tokens
            .get(next)
            .is_some_and(|token| token.kind == Kind::Arrow)
            .then_some(next)
    }
    /** Async heads must not be accepted as ordinary arrows with invented scheduling semantics. */
    pub(super) fn reject_async_arrow(&self) -> EngineResult<()> {
        if self.peek().kind == Kind::Name("async".into())
            && self.arrow_at(self.position + 1).is_some()
        {
            return Err(errors::unsupported(self.peek().location, "async arrows"));
        }
        Ok(())
    }
    /** No line terminator, including one hidden in a comment, can separate a head from its arrow. */
    pub(super) fn arrow_expression(
        &mut self,
        arrow: usize,
        nesting: usize,
    ) -> EngineResult<Expression> {
        let location = self.peek().location;
        if self.tokens[arrow].line_terminator_before {
            return Err(errors::syntax(
                self.tokens[arrow].location,
                "A line break is not allowed before =>",
            ));
        }
        let parameters = self.arrow_parameters()?;
        if self.next().kind != Kind::Arrow {
            return Err(errors::syntax(
                self.peek().location,
                "Invalid arrow function parameters",
            ));
        }
        let (body, strict) = self.function_context(|parser| parser.arrow_body(nesting))?;
        let literal = FunctionLiteral {
            kind: FunctionKind::Arrow,
            strict,
            name: None,
            display_name: String::new(),
            parameters,
            body,
        };
        super::scope::validate_function(&literal)?;
        super::strict::function(&literal, location)?;
        self.node(ExpressionKind::Function(Box::new(literal)), location, 1)
    }
    /** A bare identifier and a parenthesized simple list own exactly their own delimiters. */
    fn arrow_parameters(&mut self) -> EngineResult<Vec<String>> {
        if matches!(self.peek().kind, Kind::Name(_)) {
            return Ok(vec![self.identifier()?]);
        }
        self.symbol('(')?;
        let mut parameters = Vec::new();
        while self.peek().kind != Kind::Symbol(')') {
            self.arrow_parameter(&mut parameters)?;
            if self.peek().kind != Kind::Symbol(',') {
                break;
            }
            self.next();
        }
        self.symbol(')')?;
        Ok(parameters)
    }
    /** Duplicate names are early errors; richer binding patterns remain explicitly unimplemented. */
    fn arrow_parameter(&mut self, parameters: &mut Vec<String>) -> EngineResult<()> {
        let location = self.peek().location;
        if matches!(self.peek().kind, Kind::Symbol('[' | '{')) {
            return Err(errors::unsupported(
                location,
                "destructured arrow parameters",
            ));
        }
        if matches!(self.peek().kind, Kind::Spread | Kind::Symbol('.')) {
            return Err(errors::unsupported(location, "rest parameters"));
        }
        if !matches!(self.peek().kind, Kind::Name(_)) {
            return Err(errors::syntax(
                location,
                "Invalid arrow function parameters",
            ));
        }
        let name = self.identifier()?;
        if parameters.contains(&name) {
            return Err(errors::syntax(
                location,
                "Duplicate parameter name in arrow function",
            ));
        }
        if self.peek().kind == Kind::Symbol('=') {
            return Err(errors::unsupported(
                self.peek().location,
                "default arrow parameters",
            ));
        }
        if !matches!(self.peek().kind, Kind::Symbol(',' | ')')) {
            return Err(errors::syntax(
                self.peek().location,
                "Invalid arrow function parameters",
            ));
        }
        parameters.push(name);
        if parameters.len() > crate::constants::FUNCTION_PARAMETERS {
            return Err(self.limit(
                "V9 function parameters",
                crate::constants::FUNCTION_PARAMETERS,
            ));
        }
        Ok(())
    }
    /** Concise bodies use an ordinary located return and charge its statement allocation. */
    fn arrow_body(&mut self, nesting: usize) -> EngineResult<Vec<Statement>> {
        if self.peek().kind == Kind::Symbol('{') {
            return self.function_block();
        }
        self.statement_budget()?;
        let location = self.peek().location;
        let value = self.expression(0, nesting + 1)?;
        Ok(vec![Statement {
            kind: StatementKind::Return(Some(value)),
            location,
        }])
    }
}
