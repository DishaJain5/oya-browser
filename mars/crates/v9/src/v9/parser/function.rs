/** Function grammar restores outer parse contexts so nested declarations cannot inherit loop targets. */
use super::*;
impl Parser {
    /** An optional expression-only self name belongs to its own private runtime environment. */
    pub(super) fn function_expression(
        &mut self,
        location: SourceLocation,
    ) -> EngineResult<Expression> {
        if self.peek().kind == Kind::Operator(super::super::operator::Operator::Multiply) {
            return Err(errors::unsupported(location, "generator functions"));
        }
        let name = if matches!(self.peek().kind, Kind::Name(_)) {
            Some(self.identifier()?)
        } else {
            None
        };
        let literal = self.function_body(name)?;
        self.node(ExpressionKind::Function(Box::new(literal)), location, 1)
    }
    /** Ordinary functions and arrows share context cleanup and lexical validation. */
    pub(super) fn function_body(&mut self, name: Option<String>) -> EngineResult<FunctionLiteral> {
        self.symbol('(')?;
        let parameters = self.parameters()?;
        self.symbol(')')?;
        let (body, strict) = self.function_context(|parser| parser.function_block())?;
        let literal = FunctionLiteral {
            kind: FunctionKind::Ordinary,
            strict,
            display_name: name.clone().unwrap_or_default(),
            name,
            parameters,
            body,
        };
        super::scope::validate_function(&literal)?;
        super::strict::function(&literal, self.peek().location)?;
        Ok(literal)
    }
    /** V8's direct identifier/property inference names anonymous code without adding a self binding. */
    pub(super) fn infer_function_name(value: &mut Expression, name: &str) {
        if let ExpressionKind::Function(literal) = &mut value.kind
            && literal.name.is_none()
        {
            literal.display_name = name.into();
        }
    }
    /** Restore nesting, loop targets and declaration position even when a function body fails. */
    pub(super) fn function_context<T>(
        &mut self,
        parse: impl FnOnce(&mut Self) -> EngineResult<T>,
    ) -> EngineResult<(T, bool)> {
        if !self.allow_functions {
            return Err(errors::unsupported(
                self.peek().location,
                "function literals in inline handlers",
            ));
        }
        if self.function_nesting >= crate::constants::FUNCTION_NESTING {
            return Err(self.limit("V9 function nesting", crate::constants::FUNCTION_NESTING));
        }
        let context = (
            self.in_function,
            self.function_body,
            self.loops,
            self.strict,
        );
        self.in_function = true;
        self.function_body = Some(self.depth + 1);
        self.loops = 0;
        self.function_nesting += 1;
        let result = parse(self).map(|body| (body, self.strict));
        self.function_nesting -= 1;
        (
            self.in_function,
            self.function_body,
            self.loops,
            self.strict,
        ) = context;
        result
    }
}
