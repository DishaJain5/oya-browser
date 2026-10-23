/** Primitive arithmetic is shared by binary, compound and count expressions without host coercion. */
use super::*;
use crate::v9::operator::Operator;
impl Interpreter {
    /** Value-level operations run only after their caller has evaluated both operands in source order. */
    pub(super) fn value_operation(
        &self,
        operator: Operator,
        left: JsValue,
        right: JsValue,
        location: SourceLocation,
    ) -> EngineResult<JsValue> {
        if matches!(
            operator,
            Operator::Add
                | Operator::Subtract
                | Operator::Multiply
                | Operator::Divide
                | Operator::Remainder
                | Operator::Less
                | Operator::LessEqual
                | Operator::Greater
                | Operator::GreaterEqual
        ) && (matches!(left, JsValue::Object(_) | JsValue::Function(_))
            || matches!(right, JsValue::Object(_) | JsValue::Function(_)))
        {
            return Err(errors::unsupported(
                location,
                "object-to-primitive conversion",
            ));
        }
        let result = match operator {
            Operator::Add => left.add(&right),
            Operator::Subtract => Ok(JsValue::Number(left.to_number() - right.to_number())),
            Operator::Multiply => Ok(JsValue::Number(left.to_number() * right.to_number())),
            Operator::Divide => Ok(JsValue::Number(left.to_number() / right.to_number())),
            Operator::Remainder => Ok(JsValue::Number(left.to_number() % right.to_number())),
            Operator::Equal => Ok(JsValue::Boolean(left.strict_equal(&right))),
            Operator::NotEqual => Ok(JsValue::Boolean(!left.strict_equal(&right))),
            Operator::Less => Ok(JsValue::Boolean(
                left.compare(&right).is_some_and(|order| order.is_lt()),
            )),
            Operator::LessEqual => Ok(JsValue::Boolean(
                left.compare(&right).is_some_and(|order| order.is_le()),
            )),
            Operator::Greater => Ok(JsValue::Boolean(
                left.compare(&right).is_some_and(|order| order.is_gt()),
            )),
            Operator::GreaterEqual => Ok(JsValue::Boolean(
                left.compare(&right).is_some_and(|order| order.is_ge()),
            )),
            Operator::And | Operator::Or | Operator::Not | Operator::TypeOf | Operator::Void => {
                Err(EngineError::invalid("Invalid internal binary operator."))
            }
        };
        result.map_err(|error| errors::located(error, location, "execute"))
    }

    /** Numeric operators reject object/function conversion rather than accepting the scalar helper's NaN fallback. */
    pub(super) fn primitive_number(
        &self,
        value: &JsValue,
        location: SourceLocation,
    ) -> EngineResult<f64> {
        if matches!(value, JsValue::Object(_) | JsValue::Function(_)) {
            return Err(errors::unsupported(
                location,
                "object-to-primitive conversion",
            ));
        }
        Ok(value.to_number())
    }
}
