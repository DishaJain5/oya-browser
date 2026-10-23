/** Assignment, count and conditional expressions preserve reference identity and lazy evaluation. */
use super::*;
use crate::constants::JS_UPDATE_STEP;
use crate::v9::operator::{AssignmentOperator, Operator, UpdateOperator};
impl Interpreter {
    /** Plain assignment skips the old read; compounds snapshot it before evaluating the right-hand side. */
    pub(super) fn assignment_expression(
        &mut self,
        operator: AssignmentOperator,
        target: Expression,
        rhs: Expression,
        location: SourceLocation,
    ) -> EngineResult<JsValue> {
        let reference = self.prepare_reference(target)?;
        let old = if operator == AssignmentOperator::Assign {
            None
        } else {
            Some(self.read_reference(&reference)?)
        };
        let rhs = self.expression(rhs)?;
        let value = if let Some(old) = old {
            self.value_operation(arithmetic_operator(operator), old, rhs, location)?
        } else {
            rhs
        };
        self.write_reference(reference, value.clone())?;
        Ok(value)
    }
    /** Postfix returns the converted old Number, including its negative-zero bits, after a successful store. */
    pub(super) fn update_expression(
        &mut self,
        operator: UpdateOperator,
        prefix: bool,
        target: Expression,
        location: SourceLocation,
    ) -> EngineResult<JsValue> {
        let reference = self.prepare_reference(target)?;
        let old = self.read_reference(&reference)?;
        let old = self.primitive_number(&old, location)?;
        let value = match operator {
            UpdateOperator::Increment => old + JS_UPDATE_STEP,
            UpdateOperator::Decrement => old - JS_UPDATE_STEP,
        };
        self.write_reference(reference, JsValue::Number(value))?;
        Ok(JsValue::Number(if prefix { value } else { old }))
    }
    /** Only the selected arm runs; both arms remain source-owned for static validation and registry rebuilding. */
    pub(super) fn conditional_expression(
        &mut self,
        test: Expression,
        yes: Expression,
        no: Expression,
    ) -> EngineResult<JsValue> {
        if self.expression(test)?.truthy() {
            self.expression(yes)
        } else {
            self.expression(no)
        }
    }
}
/** Compound operators reuse the exact primitive arithmetic operation used by ordinary binary expressions. */
fn arithmetic_operator(operator: AssignmentOperator) -> Operator {
    match operator {
        AssignmentOperator::Add => Operator::Add,
        AssignmentOperator::Subtract => Operator::Subtract,
        AssignmentOperator::Multiply => Operator::Multiply,
        AssignmentOperator::Divide => Operator::Divide,
        AssignmentOperator::Remainder => Operator::Remainder,
        AssignmentOperator::Assign => unreachable!("plain assignments do not perform arithmetic"),
    }
}
