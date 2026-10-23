/** Literal expansion copies bounded owned values without generic iterators or author callbacks. */
use super::*;
use crate::{
    constants::VM_ARRAY_LENGTH,
    v9::{JsString, parser::ArrayElement, value::ObjectId},
};

impl Interpreter {
    /** Retain existing target-allocation timing, then finish each operand before the next one. */
    pub(super) fn array_literal(
        &mut self,
        elements: Vec<ArrayElement>,
        location: SourceLocation,
    ) -> EngineResult<JsValue> {
        let array = self
            .heap
            .allocate_array(0)
            .map_err(|error| errors::located(error, location, "execute"))?;
        let JsValue::Object(target) = array else {
            return Err(errors::execution(location, "array allocation failed"));
        };
        let mut next = 0;
        for element in elements {
            self.array_literal_element(target, &mut next, element)?;
        }
        Ok(array)
    }

    /** Ordinary expression failures keep their inner location instead of the enclosing ellipsis. */
    fn array_literal_element(
        &mut self,
        target: ObjectId,
        next: &mut usize,
        element: ArrayElement,
    ) -> EngineResult<()> {
        let location = element.location();
        let spread = matches!(&element, ArrayElement::Spread { .. });
        let value = self.expression(element.into_operand())?;
        if spread {
            self.array_spread(target, next, value, location)
        } else {
            self.array_append_preflight(*next, value.string_units(), location)?;
            self.array_append(target, next, value, location)
        }
    }

    /** Only owned array brands and primitive strings enter the callback-free expansion path. */
    fn array_spread(
        &mut self,
        target: ObjectId,
        next: &mut usize,
        value: JsValue,
        location: SourceLocation,
    ) -> EngineResult<()> {
        match value {
            JsValue::Object(source) => {
                if !self
                    .heap
                    .is_array(source)
                    .map_err(|error| errors::located(error, location, "execute"))?
                {
                    return Err(unsupported_iteration(location));
                }
                self.array_spread_owned(target, next, source, location)
            }
            JsValue::String(string) => self.array_spread_string(target, next, string, location),
            JsValue::Function(_) | JsValue::Host(_) => Err(unsupported_iteration(location)),
            _ => Err(errors::execution(
                location,
                "The array spread value is not iterable",
            )),
        }
    }

    /** Length is captured after operand effects; hook-free copying cannot interleave author writes. */
    fn array_spread_owned(
        &mut self,
        target: ObjectId,
        next: &mut usize,
        source: ObjectId,
        location: SourceLocation,
    ) -> EngineResult<()> {
        let length = self
            .heap
            .array_length(source)
            .map_err(|error| errors::located(error, location, "execute"))?;
        for index in 0..length {
            self.step(location)?;
            let value = self
                .heap
                .array_element_ref(source, index)
                .map_err(|error| errors::located(error, location, "execute"))?;
            self.array_append_preflight(*next, value.map_or(0, JsValue::string_units), location)?;
            let value = value.cloned().unwrap_or(JsValue::Undefined);
            self.array_append(target, next, value, location)?;
        }
        Ok(())
    }

    /** Borrow code-point slices and charge before creating each one- or two-unit string. */
    fn array_spread_string(
        &mut self,
        target: ObjectId,
        next: &mut usize,
        string: JsString,
        location: SourceLocation,
    ) -> EngineResult<()> {
        for units in string.code_points() {
            self.step(location)?;
            self.array_append_preflight(*next, units.len(), location)?;
            let value = JsString::from_units(units.to_vec())
                .map(JsValue::String)
                .map_err(|error| errors::located(error, location, "execute"))?;
            self.array_append(target, next, value, location)?;
        }
        Ok(())
    }

    /** Independent limits reject growth before any emitted payload allocation or clone. */
    fn array_append_preflight(
        &self,
        next: usize,
        string_units: usize,
        location: SourceLocation,
    ) -> EngineResult<()> {
        let check = || {
            self.environment()?;
            if next >= VM_ARRAY_LENGTH {
                return Err(EngineError::limit("V9 array length", VM_ARRAY_LENGTH));
            }
            if self.heap.property_count() + self.graph.property_count() >= VM_PROPERTIES {
                return Err(EngineError::limit("V9 page properties", VM_PROPERTIES));
            }
            if self.environment_units().saturating_add(string_units) > JS_ENVIRONMENT_UNITS {
                return Err(EngineError::limit(
                    "V9 environment UTF-16 units",
                    JS_ENVIRONMENT_UNITS,
                ));
            }
            Ok(())
        };
        check().map_err(|error| errors::located(error, location, "execute"))
    }

    /** Dense stores materialize holes as present undefined; postchecks defend the same aggregate. */
    fn array_append(
        &mut self,
        target: ObjectId,
        next: &mut usize,
        value: JsValue,
        location: SourceLocation,
    ) -> EngineResult<()> {
        self.heap
            .set(target, next.to_string(), value)
            .map_err(|error| errors::located(error, location, "execute"))?;
        self.environment()
            .map_err(|error| errors::located(error, location, "execute"))?;
        *next += 1;
        Ok(())
    }
}

/** Runtime iteration limits must not inherit the parser-only unsupported helper's phase. */
fn unsupported_iteration(location: SourceLocation) -> EngineError {
    errors::located(
        EngineError::new(
            "unsupported_feature",
            "Iteration for this value type is not supported by array spread.",
            "Use an owned array or a primitive string; no script changes were applied.",
        ),
        location,
        "execute",
    )
}
