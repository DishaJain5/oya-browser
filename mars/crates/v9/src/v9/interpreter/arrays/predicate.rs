/** Source-guided selection visits a fixed index range while reading the current candidate heap. */
use super::*;
impl Interpreter {
    /** Some skips absent entries; find visits them and returns the value captured before author code. */
    pub(super) fn array_predicate(
        &mut self,
        intrinsic: Intrinsic,
        source: super::super::super::value::ObjectId,
        length: usize,
        arguments: Vec<JsValue>,
        location: SourceLocation,
    ) -> EngineResult<JsValue> {
        let callback = arguments.first().cloned().unwrap_or(JsValue::Undefined);
        self.callable_id(&callback, location)?;
        let receiver = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
        let some = matches!(intrinsic, Intrinsic::ArraySome);
        for index in 0..length {
            self.step(location)?;
            let present = self.heap.array_element(source, index)?;
            if some && present.is_none() {
                continue;
            }
            let value = present.unwrap_or(JsValue::Undefined);
            let selected = self.call_value(
                callback.clone(),
                receiver.clone(),
                vec![
                    value.clone(),
                    JsValue::Number(index as f64),
                    JsValue::Object(source),
                ],
                location,
            )?;
            if selected.truthy() {
                return Ok(if some { JsValue::Boolean(true) } else { value });
            }
        }
        Ok(if some {
            JsValue::Boolean(false)
        } else {
            JsValue::Undefined
        })
    }
}
