/** Closed-value loose equality follows reviewed primary branches; provenance and limits are in UPSTREAM.md. */
use super::JsValue;
use crate::protocol::{EngineError, EngineResult};

impl JsValue {
    /// Compare primitives and stable identities, refusing the genuine ToPrimitive step instead of fabricating it.
    pub fn abstract_equal(&self, other: &Self) -> EngineResult<bool> {
        use JsValue::*;
        match (self, other) {
            (Null, Undefined) | (Undefined, Null) => Ok(true),
            (Number(_), String(_)) | (String(_), Number(_)) => {
                Ok(self.to_number() == other.to_number())
            }
            (Boolean(_), Boolean(_)) => Ok(self.strict_equal(other)),
            (Boolean(_), _) => Number(self.to_number()).abstract_equal(other),
            (_, Boolean(_)) => self.abstract_equal(&Number(other.to_number())),
            (left, right) if requires_conversion(left, right) => Err(EngineError::unsupported(
                "Object-to-primitive conversion is not supported for this comparison.",
            )),
            // Other closed-type pairs need no conversion; unequal language types are false.
            _ => Ok(self.strict_equal(other)),
        }
    }
}

/** Only String/Number against a language Object needs coercion after the one bounded Boolean normalization. */
fn requires_conversion(left: &JsValue, right: &JsValue) -> bool {
    (object_like(left) && matches!(right, JsValue::String(_) | JsValue::Number(_)))
        || (object_like(right) && matches!(left, JsValue::String(_) | JsValue::Number(_)))
}

/** Distinct internal object arenas share a language category, but strict identity retains their brands. */
fn object_like(value: &JsValue) -> bool {
    matches!(
        value,
        JsValue::Object(_) | JsValue::Function(_) | JsValue::Host(_)
    )
}
