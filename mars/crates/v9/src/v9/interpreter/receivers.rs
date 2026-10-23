/** Receiver conversion follows the callee's source mode without leaking shell or caller authority. */
use super::*;
use crate::v9::{parser::FunctionKind, registry::Code};
impl Interpreter {
    /** Blocks inherit the nearest invocation receiver; classic-script this is always the page global. */
    pub(super) fn current_receiver(&self) -> EngineResult<JsValue> {
        let mut cursor = self.locals.last().copied();
        while let Some(id) = cursor {
            let environment = self.graph.get(id)?;
            if let Some(receiver) = &environment.receiver {
                return Ok(receiver.clone());
            }
            cursor = environment.parent;
        }
        Ok(JsValue::Object(self.global_object))
    }
    /** Arrows ignore call-site receivers; sloppy ordinary functions convert only supported values. */
    pub(super) fn call_receiver(
        &self,
        lexical_this: Option<JsValue>,
        code: &Code,
        receiver: JsValue,
        location: SourceLocation,
    ) -> EngineResult<JsValue> {
        if code.kind == FunctionKind::Arrow {
            return lexical_this.ok_or_else(super::super::registry::invalid);
        }
        if code.strict {
            return Ok(receiver);
        }
        match receiver {
            JsValue::Undefined | JsValue::Null => Ok(JsValue::Object(self.global_object)),
            JsValue::Object(_) | JsValue::Function(_) | JsValue::Host(_) => Ok(receiver),
            _ => Err(errors::unsupported(location, "primitive receiver boxing")),
        }
    }
}
