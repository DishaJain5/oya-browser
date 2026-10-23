/** Canonical String methods adapt primitive receivers and publish bounded array candidates. */
use super::*;
use crate::{
    constants::JS_STRING_SCAN_UNITS,
    v9::{JsString, value::to_uint32},
};
impl Interpreter {
    /** Method extraction, call, apply and bind all retain the same primitive conversion and work rules. */
    pub(super) fn string_intrinsic(
        &mut self,
        intrinsic: Intrinsic,
        receiver: JsValue,
        arguments: Vec<JsValue>,
        location: SourceLocation,
    ) -> EngineResult<JsValue> {
        let string = self.string_receiver(receiver, location)?;
        match intrinsic {
            Intrinsic::StringTrim => {
                self.string_scan(string.len(), location)?;
                string
                    .trimmed()
                    .map(JsValue::String)
                    .map_err(|error| errors::located(error, location, "execute"))
            }
            Intrinsic::StringSplit => self.string_split(string, arguments, location),
            _ => Err(EngineError::invalid("Invalid String intrinsic selector.")),
        }
    }
    /** RequireObjectCoercible precedes primitive conversion without adding unsupported wrapper objects. */
    fn string_receiver(
        &self,
        receiver: JsValue,
        location: SourceLocation,
    ) -> EngineResult<JsString> {
        if matches!(receiver, JsValue::Null | JsValue::Undefined) {
            return Err(errors::execution(
                location,
                "String method receiver must not be null or undefined",
            ));
        }
        receiver
            .to_js_string()
            .map_err(|error| errors::located(error, location, "execute"))
    }
    /** Conversion ordering is shared by direct, detached and adapted split calls. */
    fn string_split(
        &mut self,
        string: JsString,
        arguments: Vec<JsValue>,
        location: SourceLocation,
    ) -> EngineResult<JsValue> {
        let limit = match arguments.get(1) {
            None | Some(JsValue::Undefined) => u32::MAX,
            Some(value) => to_uint32(self.primitive_number(value, location)?),
        };
        let separator = match arguments.first() {
            None | Some(JsValue::Undefined) => None,
            Some(value) => Some(
                value
                    .to_js_string()
                    .map_err(|error| errors::located(error, location, "execute"))?,
            ),
        };
        if limit != 0 {
            self.string_scan(
                string.len().saturating_add(
                    separator
                        .as_ref()
                        .map_or(0, |separator| separator.len().saturating_mul(2)),
                ),
                location,
            )?;
        }
        let fields = string
            .split_literal(separator.as_ref(), limit)
            .map_err(|error| errors::located(error, location, "execute"))?;
        self.string_array(fields, location)
    }
    /** Each insertion checks the aggregate environment while the result remains rooted in this evaluator. */
    fn string_array(
        &mut self,
        fields: Vec<JsString>,
        location: SourceLocation,
    ) -> EngineResult<JsValue> {
        let result = self
            .heap
            .allocate_array(fields.len())
            .map_err(|error| errors::located(error, location, "execute"))?;
        let JsValue::Object(id) = result else {
            return Err(super::super::registry::invalid());
        };
        for (index, field) in fields.into_iter().enumerate() {
            self.step(location)?;
            self.heap
                .set(id, index.to_string(), JsValue::String(field))
                .map_err(|error| errors::located(error, location, "execute"))?;
            self.environment()
                .map_err(|error| errors::located(error, location, "execute"))?;
        }
        Ok(result)
    }
    /** Linear UTF-16 scans charge bounded chunks in addition to ordinary call and insertion work. */
    fn string_scan(&mut self, units: usize, location: SourceLocation) -> EngineResult<()> {
        for _ in 0..units.div_ceil(JS_STRING_SCAN_UNITS) {
            self.step(location)?;
        }
        Ok(())
    }
}
