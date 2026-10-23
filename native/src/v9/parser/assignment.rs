/** Assignment and conditional grammar retain targets and evaluate only the selected runtime branches. */
use super::super::operator::{AssignmentOperator, UpdateOperator};
use super::*;
impl Parser {
    /** The RHS recursively accepts assignment grammar, preserving right associativity and arrow branches. */
    pub(super) fn assignment_expression(&mut self, nesting: usize) -> EngineResult<Expression> {
        let left = self.conditional_expression(nesting)?;
        let operator = match self.peek().kind {
            Kind::Symbol('=') => AssignmentOperator::Assign,
            Kind::Assignment(operator) => operator,
            _ => return Ok(left),
        };
        let location = self.next().location;
        self.assignment_target(&left)?;
        let mut right = self.expression(0, nesting + 1)?;
        if operator == AssignmentOperator::Assign
            && let ExpressionKind::Variable(name) = &left.kind
        {
            Self::infer_function_name(&mut right, name);
        }
        let depth = left.depth.max(right.depth) + 1;
        self.node(
            ExpressionKind::Assignment(operator, Box::new(left), Box::new(right)),
            location,
            depth,
        )
    }
    /** Both conditional arms parse assignment expressions; the test keeps ordinary binary precedence. */
    fn conditional_expression(&mut self, nesting: usize) -> EngineResult<Expression> {
        let test = self.expression(1, nesting)?;
        if self.peek().kind != Kind::Symbol('?') {
            return Ok(test);
        }
        let location = self.next().location;
        let yes = self.expression(0, nesting + 1)?;
        self.symbol(':')?;
        let no = self.expression(0, nesting + 1)?;
        let depth = test.depth.max(yes.depth).max(no.depth) + 1;
        self.node(
            ExpressionKind::Conditional(Box::new(test), Box::new(yes), Box::new(no)),
            location,
            depth,
        )
    }
    /** Only source-owned identifier or property references can be assigned or updated. */
    pub(super) fn assignment_target(&self, target: &Expression) -> EngineResult<()> {
        match &target.kind {
            ExpressionKind::Variable(name) if !reserved(name) => Ok(()),
            ExpressionKind::Member(..) => Ok(()),
            _ => Err(errors::syntax(
                target.location,
                "Invalid assignment or update target",
            )),
        }
    }
    /** Postfix operators compare with the final LHS token, including a multiline computed member's bracket. */
    pub(super) fn postfix_update(&mut self, target: Expression) -> EngineResult<Expression> {
        let Kind::Update(operator) = self.peek().kind else {
            return Ok(target);
        };
        if self.peek().location.line != self.tokens[self.position - 1].location.line {
            return Ok(target);
        }
        let location = self.next().location;
        self.update_expression(operator, false, target, location)
    }
    /** Prefix and postfix updates share target validation and charge their complete operand depth. */
    pub(super) fn update_expression(
        &mut self,
        operator: UpdateOperator,
        prefix: bool,
        target: Expression,
        location: SourceLocation,
    ) -> EngineResult<Expression> {
        self.assignment_target(&target)?;
        let depth = target.depth + 1;
        self.node(
            ExpressionKind::Update(operator, prefix, Box::new(target)),
            location,
            depth,
        )
    }
}
