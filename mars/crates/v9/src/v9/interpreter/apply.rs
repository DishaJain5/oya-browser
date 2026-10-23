/** Apply constructs one bounded array-like argument list in observable property order. */
use super::*;
use crate::constants::VM_MAX_SAFE_LENGTH;
impl Interpreter {
    /** Target callability is checked before any array-like length conversion or indexed access. */
    pub(super) fn function_apply(
        &mut self,
        target: JsValue,
        arguments: Vec<JsValue>,
        location: SourceLocation,
    ) -> EngineResult<JsValue> {
        self.callable_id(&target, location)?;
        let mut arguments = arguments.into_iter();
        let receiver = arguments.next().unwrap_or(JsValue::Undefined);
        let list = arguments.next().unwrap_or(JsValue::Undefined);
        let arguments = self.array_like_arguments(list, location)?;
        self.call_value(target, receiver, arguments, location)
    }
    /** Nullish lists are empty; supported object and function records use the ordinary property path. */
    fn array_like_arguments(
        &mut self,
        list: JsValue,
        location: SourceLocation,
    ) -> EngineResult<Vec<JsValue>> {
        if matches!(list, JsValue::Undefined | JsValue::Null) {
            return Ok(Vec::new());
        }
        if !matches!(
            list,
            JsValue::Object(_) | JsValue::Function(_) | JsValue::Host(_)
        ) {
            return Err(errors::execution(
                location,
                "The apply argument list must be an object, null or undefined",
            ));
        }
        self.step(location)?;
        let length = self.get_property(&list, "length", location)?;
        let length = self.array_like_length(length, location)?;
        self.argument_count(length, location)?;
        self.read_argument_indices(&list, length, location)
    }
    /** Object conversion remains unavailable rather than silently treating object lengths as NaN. */
    fn array_like_length(&self, value: JsValue, location: SourceLocation) -> EngineResult<usize> {
        if matches!(
            value,
            JsValue::Object(_) | JsValue::Function(_) | JsValue::Host(_)
        ) {
            return Err(errors::unsupported(
                location,
                "object-to-primitive conversion of apply length",
            ));
        }
        let number = value.to_number();
        Ok(if number.is_nan() || number <= 0.0 {
            0
        } else {
            number.min(VM_MAX_SAFE_LENGTH).floor() as usize
        })
    }
    /** Each index is read once in increasing order and holes remain explicit undefined values. */
    fn read_argument_indices(
        &mut self,
        list: &JsValue,
        length: usize,
        location: SourceLocation,
    ) -> EngineResult<Vec<JsValue>> {
        let mut arguments = Vec::with_capacity(length);
        for index in 0..length {
            self.step(location)?;
            arguments.push(self.get_property(list, &index.to_string(), location)?);
        }
        Ok(arguments)
    }
}
