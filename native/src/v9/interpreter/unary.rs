/** Unary operators preserve typeof's unresolved-name exception without suppressing real lookup failures. */
use super::*;
use crate::v9::operator::Operator;
impl Interpreter {
    /** Typeof has a dedicated identifier lookup; void and boolean negation do not coerce object values. */
    pub(super) fn unary(
        &mut self,
        operator: Operator,
        operand: Expression,
        location: SourceLocation,
    ) -> EngineResult<JsValue> {
        if operator == Operator::TypeOf {
            return self.type_of(operand);
        }
        let value = self.expression(operand)?;
        match operator {
            Operator::Void => Ok(JsValue::Undefined),
            Operator::Not => Ok(JsValue::Boolean(!value.truthy())),
            Operator::Add => self.primitive_number(&value, location).map(JsValue::Number),
            Operator::Subtract => self
                .primitive_number(&value, location)
                .map(|value| JsValue::Number(-value)),
            _ => Err(errors::execution(location, "invalid unary operator")),
        }
    }
    /** Only an unresolved ordinary direct identifier becomes undefined; TDZ and capability errors remain visible. */
    fn type_of(&mut self, operand: Expression) -> EngineResult<JsValue> {
        let value = if let ExpressionKind::Variable(name) = operand.kind {
            self.step(operand.location)?;
            self.type_of_identifier(&name, operand.location)?
        } else {
            self.expression(operand)?
        };
        JsValue::string(match value {
            JsValue::Undefined => "undefined",
            JsValue::Null | JsValue::Object(_) => "object",
            JsValue::Function(_) => "function",
            JsValue::Number(_) => "number",
            JsValue::String(_) => "string",
            JsValue::Boolean(_) => "boolean",
        })
    }
    /** Supported fallback values are retained while native/protected names never masquerade as absent globals. */
    fn type_of_identifier(&self, name: &str, location: SourceLocation) -> EngineResult<JsValue> {
        if let Some(target) = self.resolve(name, location)? {
            return self.read_cell(target, name, location);
        }
        if name == "globalThis" || environment::builtin(name).is_some() {
            return self.unresolved_identifier(name, location);
        }
        if properties::protected_global(name) {
            return Err(errors::unsupported(
                location,
                "typeof protected native globals",
            ));
        }
        Ok(JsValue::Undefined)
    }
}
