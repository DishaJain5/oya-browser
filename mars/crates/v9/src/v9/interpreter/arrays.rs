/** Bounded array methods preserve sparse iteration and ordinary callable ownership within one candidate. */
mod predicate;
use super::*;
use crate::{constants::JS_STRING_UNITS, v9::JsString};
impl Interpreter {
    /** Every direct or adapted method rejects nonarrays before allocation or author callbacks. */
    pub(super) fn array_intrinsic(
        &mut self,
        intrinsic: Intrinsic,
        receiver: JsValue,
        arguments: Vec<JsValue>,
        location: SourceLocation,
    ) -> EngineResult<JsValue> {
        let JsValue::Object(id) = receiver else {
            return Err(errors::unsupported(
                location,
                "Array methods on nonarray receivers",
            ));
        };
        let length = self
            .heap
            .array_length(id)
            .map_err(|error| errors::located(error, location, "execute"))?;
        match intrinsic {
            Intrinsic::ArrayMap | Intrinsic::ArrayFilter => {
                self.array_iterate(intrinsic, id, length, arguments, location)
            }
            Intrinsic::ArrayJoin => self.array_join(id, length, arguments, location),
            Intrinsic::ArraySome | Intrinsic::ArrayFind => {
                self.array_predicate(intrinsic, id, length, arguments, location)
            }
            _ => Err(EngineError::invalid("Invalid Array intrinsic selector.")),
        }
    }
    /** Length is fixed once; element presence and value are read immediately before each invocation. */
    fn array_iterate(
        &mut self,
        intrinsic: Intrinsic,
        source: super::super::value::ObjectId,
        length: usize,
        arguments: Vec<JsValue>,
        location: SourceLocation,
    ) -> EngineResult<JsValue> {
        let callback = arguments.first().cloned().unwrap_or(JsValue::Undefined);
        self.callable_id(&callback, location)?;
        let receiver = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
        let map = matches!(intrinsic, Intrinsic::ArrayMap);
        let result = self
            .heap
            .allocate_array(if map { length } else { 0 })
            .map_err(|error| errors::located(error, location, "execute"))?;
        let JsValue::Object(target) = result else {
            return Err(super::super::registry::invalid());
        };
        let mut next = 0;
        for index in 0..length {
            self.step(location)?;
            let Some(value) = self.heap.array_element(source, index)? else {
                continue;
            };
            let mapped = self.call_value(
                callback.clone(),
                receiver.clone(),
                vec![
                    value.clone(),
                    JsValue::Number(index as f64),
                    JsValue::Object(source),
                ],
                location,
            )?;
            if map || mapped.truthy() {
                self.heap
                    .set(
                        target,
                        if map { index } else { next }.to_string(),
                        if map { mapped } else { value },
                    )
                    .map_err(|error| errors::located(error, location, "execute"))?;
                next += 1;
                self.environment()
                    .map_err(|error| errors::located(error, location, "execute"))?;
            }
        }
        Ok(result)
    }
    /** Separator conversion precedes even an empty result; each snapshot index still emits its separator. */
    fn array_join(
        &mut self,
        source: super::super::value::ObjectId,
        length: usize,
        arguments: Vec<JsValue>,
        location: SourceLocation,
    ) -> EngineResult<JsValue> {
        let separator = match arguments.first() {
            None | Some(JsValue::Undefined) => JsString::from_utf8(","),
            Some(value) => value.to_js_string(),
        }
        .map_err(|error| errors::located(error, location, "execute"))?;
        let mut result = Vec::new();
        for index in 0..length {
            self.step(location)?;
            if index != 0 {
                append(&mut result, separator.as_units(), location)?;
            }
            if let Some(value) = self.heap.array_element(source, index)? {
                if matches!(value, JsValue::Null | JsValue::Undefined) {
                    continue;
                }
                let string = value
                    .to_js_string()
                    .map_err(|error| errors::located(error, location, "execute"))?;
                append(&mut result, string.as_units(), location)?;
            }
        }
        JsString::from_units(result)
            .map(JsValue::String)
            .map_err(|error| errors::located(error, location, "execute"))
    }
}
/** Output limits are checked before allocation grows; lone UTF-16 surrogates remain ordinary VM units. */
fn append(result: &mut Vec<u16>, units: &[u16], location: SourceLocation) -> EngineResult<()> {
    if result.len().saturating_add(units.len()) > JS_STRING_UNITS {
        return Err(errors::located(
            EngineError::limit("JavaScript string UTF-16 units", JS_STRING_UNITS),
            location,
            "execute",
        ));
    }
    result.extend_from_slice(units);
    Ok(())
}
