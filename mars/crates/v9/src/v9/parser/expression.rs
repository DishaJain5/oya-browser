/** Located expression parsing follows the pinned V8 precedence and primary grammar. */
use super::super::{operator::Operator, value::JsValue};
use super::*;
use crate::constants::{EXPRESSION_DEPTH, TOKENS};
impl Parser {
    /** Precedence climbing keeps binary operations left associative. */
    pub(super) fn expression(&mut self, minimum: u8, nesting: usize) -> EngineResult<Expression> {
        self.check_depth(nesting)?;
        if minimum == 0 {
            self.reject_async_arrow()?;
            if let Some(arrow) = self.arrow_position() {
                return self.arrow_expression(arrow, nesting);
            }
            return self.assignment_expression(nesting);
        }
        let mut left = self.unary(nesting + 1)?;
        while let Kind::Operator(operator) = self.peek().kind {
            let precedence = operator.precedence();
            if precedence == 0 || precedence < minimum {
                break;
            }
            let location = self.next().location;
            let right = self.expression(precedence + 1, nesting + 1)?;
            let depth = left.depth.max(right.depth) + 1;
            left = self.node(
                ExpressionKind::Binary(operator, Box::new(left), Box::new(right)),
                location,
                depth,
            )?;
        }
        Ok(left)
    }

    /** Unary operators bind tighter than multiplication and member access. */
    fn unary(&mut self, nesting: usize) -> EngineResult<Expression> {
        self.check_depth(nesting)?;
        if let Kind::Update(operator) = self.peek().kind {
            let location = self.next().location;
            let operand = self.unary(nesting + 1)?;
            return self.update_expression(operator, true, operand, location);
        }
        let operator = match &self.peek().kind {
            Kind::Operator(operator @ (Operator::Add | Operator::Subtract | Operator::Not)) => {
                Some(*operator)
            }
            Kind::Name(name) if name == "typeof" => Some(Operator::TypeOf),
            Kind::Name(name) if name == "void" => Some(Operator::Void),
            _ => None,
        };
        if let Some(operator) = operator {
            let location = self.next().location;
            let operand = self.unary(nesting + 1)?;
            let depth = operand.depth + 1;
            return self.node(
                ExpressionKind::Unary(operator, Box::new(operand)),
                location,
                depth,
            );
        }
        let value = self.primary(nesting)?;
        self.postfix_update(value)
    }

    /** Primary values feed bounded own-property reads and calls of supported function values. */
    fn primary(&mut self, nesting: usize) -> EngineResult<Expression> {
        let token = self.next();
        let base = match token.kind {
            Kind::Name(name) if name == "this" => {
                self.node(ExpressionKind::This, token.location, 1)?
            }
            Kind::Name(name) if name == "function" => self.function_expression(token.location)?,
            Kind::String(value) => self.node(
                ExpressionKind::Literal(JsValue::String(value)),
                token.location,
                1,
            )?,
            Kind::Number(value) => self.node(
                ExpressionKind::Literal(JsValue::Number(value)),
                token.location,
                1,
            )?,
            Kind::Name(name) if name == "true" || name == "false" => self.node(
                ExpressionKind::Literal(JsValue::Boolean(name == "true")),
                token.location,
                1,
            )?,
            Kind::Name(name) if name == "null" => {
                self.node(ExpressionKind::Literal(JsValue::Null), token.location, 1)?
            }
            Kind::Name(name) if name == "document" && self.document_mode => {
                self.node(ExpressionKind::Document, token.location, 1)?
            }
            Kind::Name(name) if name == "document" => {
                if self.peek().kind != Kind::Symbol('.') {
                    return Err(errors::unsupported(
                        token.location,
                        "native document objects",
                    ));
                }
                return self.dom_input(token.location);
            }
            Kind::Name(name) if name == "window" => {
                if self.peek().kind != Kind::Symbol('.') {
                    return Err(errors::unsupported(token.location, "native window objects"));
                }
                self.storage_area(token.location)?
            }
            Kind::Name(name) if unsupported_primary(&name) => {
                return Err(errors::unsupported(
                    token.location,
                    "this value or unary expression",
                ));
            }
            Kind::Name(name) if reserved(&name) => {
                return Err(errors::syntax(
                    token.location,
                    "unexpected keyword in expression",
                ));
            }
            Kind::Name(name) => self.node(ExpressionKind::Variable(name), token.location, 1)?,
            Kind::Symbol('(') => {
                let expression = self.expression(0, nesting + 1)?;
                self.symbol(')')?;
                expression
            }
            Kind::Symbol('[') => self.array(token.location, nesting)?,
            Kind::Symbol('{') => self.object(token.location, nesting)?,
            Kind::Operator(Operator::Divide) => {
                return Err(errors::unsupported(
                    token.location,
                    "regular expression literals",
                ));
            }
            _ => return Err(errors::syntax(token.location, "expected an expression")),
        };
        self.postfix(base, nesting)
    }

    /** The document remains a restricted native receiver rather than a JS host object. */
    fn dom_input(&mut self, location: SourceLocation) -> EngineResult<Expression> {
        let id = self.target()?;
        self.symbol('.')?;
        self.exact_name("value")?;
        self.node(ExpressionKind::Input(id), location, 1)
    }

    /** Window exposes only coordinator-bound Storage areas in this V9 subset. */
    pub(super) fn storage_area(&mut self, location: SourceLocation) -> EngineResult<Expression> {
        self.symbol('.')?;
        let area = match self.name()?.as_str() {
            "localStorage" => StorageArea::Local,
            "sessionStorage" => StorageArea::Session,
            _ => return Err(errors::unsupported(location, "window host properties")),
        };
        self.node(ExpressionKind::StorageArea(area), location, 1)
    }

    /** Dot/bracket reads and calls retain their source order without exposing native reflection. */
    pub(super) fn postfix(
        &mut self,
        mut base: Expression,
        nesting: usize,
    ) -> EngineResult<Expression> {
        loop {
            let location = self.peek().location;
            match self.peek().kind {
                Kind::Symbol('.') => base = self.dot_member(base, location)?,
                Kind::Symbol('[') => base = self.computed_member(base, location, nesting)?,
                Kind::Symbol('(') => base = self.call_expression(base, location, nesting)?,
                _ => return Ok(base),
            }
        }
    }

    /** Dot properties are own-data reads represented with the ordinary key path. */
    fn dot_member(
        &mut self,
        base: Expression,
        location: SourceLocation,
    ) -> EngineResult<Expression> {
        self.next();
        let name = self.name()?;
        if let ExpressionKind::StorageArea(area) = base.kind {
            let method = storage_method(&name).ok_or_else(|| {
                errors::unsupported(location, "Storage methods other than the supported four")
            })?;
            return self.node(
                ExpressionKind::StorageMethod(area, method),
                location,
                base.depth + 1,
            );
        }
        let key = self.node(
            ExpressionKind::Literal(JsValue::string(&name)?),
            location,
            1,
        )?;
        self.member(base, key, location)
    }

    /** Computed reads evaluate their key as an expression at runtime. */
    fn computed_member(
        &mut self,
        base: Expression,
        location: SourceLocation,
        nesting: usize,
    ) -> EngineResult<Expression> {
        self.next();
        let key = self.expression(0, nesting + 1)?;
        self.symbol(']')?;
        self.member(base, key, location)
    }

    /** Ordinary callee expressions can invoke owned functions; Storage retains its fixed method seam. */
    fn call_expression(
        &mut self,
        base: Expression,
        location: SourceLocation,
        nesting: usize,
    ) -> EngineResult<Expression> {
        if let ExpressionKind::StorageMethod(area, method) = base.kind {
            self.next();
            let args = self.arguments(nesting)?;
            self.symbol(')')?;
            let depth = args
                .iter()
                .map(|value| value.depth)
                .max()
                .unwrap_or(0)
                .max(base.depth)
                + 1;
            return self.node(
                ExpressionKind::StorageCall(area, method, args),
                location,
                depth,
            );
        }
        if let ExpressionKind::Variable(name) = &base.kind
            && ["fetch", "eval", "require", "process", "window"].contains(&name.as_str())
        {
            return Err(errors::unsupported(location, "host and dynamic-code calls"));
        }
        self.next();
        let args = self.arguments(nesting)?;
        self.symbol(')')?;
        let depth = args
            .iter()
            .map(|value| value.depth)
            .max()
            .unwrap_or(0)
            .max(base.depth)
            + 1;
        self.node(
            ExpressionKind::Invoke(Box::new(base), args),
            location,
            depth,
        )
    }

    /** Array elements retain ellipsis locations without admitting literal elisions. */
    fn array(&mut self, location: SourceLocation, nesting: usize) -> EngineResult<Expression> {
        let mut values = Vec::new();
        while self.peek().kind != Kind::Symbol(']') {
            values.push(self.array_element(nesting)?);
            if self.peek().kind != Kind::Symbol(',') {
                break;
            }
            self.next();
            if self.peek().kind == Kind::Symbol(']') {
                break;
            }
        }
        self.symbol(']')?;
        let depth = values
            .iter()
            .map(|value| value.operand().depth)
            .max()
            .unwrap_or(0)
            + 1;
        self.node(ExpressionKind::Array(values), location, depth)
    }

    /** Spread operands use the ordinary assignment grammar and its existing node/depth limits. */
    fn array_element(&mut self, nesting: usize) -> EngineResult<ArrayElement> {
        if self.peek().kind != Kind::Spread {
            return Ok(ArrayElement::Value(self.expression(0, nesting + 1)?));
        }
        let location = self.next().location;
        if matches!(
            self.peek().kind,
            Kind::Symbol(',' | ']') | Kind::End | Kind::Spread
        ) {
            return Err(errors::syntax(
                location,
                "Expected an expression after array spread",
            ));
        }
        let operand = self.expression(0, nesting + 1)?;
        Ok(ArrayElement::Spread { operand, location })
    }

    /** Object literals accept identifier or string keys and own data values only. */
    fn object(&mut self, location: SourceLocation, nesting: usize) -> EngineResult<Expression> {
        let mut properties = Vec::new();
        while self.peek().kind != Kind::Symbol('}') {
            let (key, key_location) = self.property_key()?;
            self.symbol(':')?;
            let mut value = self.expression(0, nesting + 1)?;
            Self::infer_function_name(&mut value, &key);
            properties.push((key, value));
            if self.peek().kind != Kind::Symbol(',') {
                break;
            }
            self.next();
            if self.peek().kind == Kind::Symbol('}') {
                break;
            }
            if properties.len() >= TOKENS {
                return Err(errors::located(
                    EngineError::limit("V9 object properties", TOKENS),
                    key_location,
                    "parse",
                ));
            }
        }
        self.symbol('}')?;
        let depth = properties
            .iter()
            .map(|(_, value)| value.depth)
            .max()
            .unwrap_or(0)
            + 1;
        self.node(ExpressionKind::Object(properties), location, depth)
    }

    /** Property keys reject computed syntax and preserve decoded source strings. */
    fn property_key(&mut self) -> EngineResult<(String, SourceLocation)> {
        let token = self.next();
        let key = match token.kind {
            Kind::Spread => return Err(errors::unsupported(token.location, "object spread")),
            Kind::Name(name) => name,
            Kind::String(value) => value
                .to_dom()
                .map_err(|error| errors::located(error, token.location, "parse"))?,
            Kind::Number(value) => JsValue::Number(value).to_dom()?,
            _ => {
                return Err(errors::unsupported(
                    token.location,
                    "computed object-literal keys",
                ));
            }
        };
        Ok((key, token.location))
    }

    /** Member nodes retain the deepest child for parser and evaluator quotas. */
    fn member(
        &mut self,
        base: Expression,
        key: Expression,
        location: SourceLocation,
    ) -> EngineResult<Expression> {
        let depth = base.depth.max(key.depth) + 1;
        self.node(
            ExpressionKind::Member(Box::new(base), Box::new(key)),
            location,
            depth,
        )
    }

    /** AST allocation checks actual depth and aggregate node count before construction. */
    pub(super) fn node(
        &mut self,
        kind: ExpressionKind,
        location: SourceLocation,
        depth: usize,
    ) -> EngineResult<Expression> {
        self.check_depth(depth)?;
        self.nodes += 1;
        if self.nodes > TOKENS {
            return Err(errors::located(
                EngineError::limit("V9 AST nodes", TOKENS),
                location,
                "parse",
            ));
        }
        Ok(Expression {
            kind,
            location,
            depth,
        })
    }

    /** Syntax recursion and AST depth share one named limit. */
    fn check_depth(&self, depth: usize) -> EngineResult<()> {
        if depth > EXPRESSION_DEPTH {
            return Err(errors::located(
                EngineError::limit("V9 expression depth", EXPRESSION_DEPTH),
                self.peek().location,
                "parse",
            ));
        }
        Ok(())
    }
}

/** Resolve only standard Storage methods; generic object dispatch stays unsupported. */
fn storage_method(name: &str) -> Option<StorageMethod> {
    match name {
        "getItem" => Some(StorageMethod::GetItem),
        "setItem" => Some(StorageMethod::SetItem),
        "removeItem" => Some(StorageMethod::RemoveItem),
        "clear" => Some(StorageMethod::Clear),
        _ => None,
    }
}

/** Unsupported primary forms fail explicitly instead of becoming global lookups. */
fn unsupported_primary(name: &str) -> bool {
    ["new", "await", "yield", "delete"].contains(&name)
}
