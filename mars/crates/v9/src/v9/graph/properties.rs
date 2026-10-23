/** Function own data and the canonical prototype form one bounded property lookup strategy. */
use super::*;
use crate::{constants::*, v9::registry::Registry};
impl Graph {
    /** Own expandos precede metadata and shared prototype methods, preserving explicit shadowing. */
    pub fn get_property(&self, id: Id, key: &str, registry: &Registry) -> EngineResult<JsValue> {
        validate_key(key)?;
        let function = self.functions.get(&id).ok_or_else(invalid)?;
        if let Some(value) = function.properties.get(key) {
            return Ok(value.clone());
        }
        match key {
            "name" => return self.name(id, registry).map(JsValue::String),
            "length" => {
                return self
                    .length(id, registry)
                    .map(|length| JsValue::Number(length as f64));
            }
            "prototype" if id == VM_FUNCTION_CONSTRUCTOR_ID => {
                return Ok(JsValue::Function(VM_FUNCTION_PROTOTYPE_ID));
            }
            _ => {}
        }
        unavailable(key)?;
        Ok(self
            .functions
            .get(&VM_FUNCTION_PROTOTYPE_ID)
            .ok_or_else(invalid)?
            .properties
            .get(key)
            .cloned()
            .unwrap_or(JsValue::Undefined))
    }
    /** Readonly metadata ignores sloppy assignments and rejects strict assignments before any mutation. */
    pub fn set_property(
        &mut self,
        id: Id,
        key: String,
        value: JsValue,
        _registry: &Registry,
        strict: bool,
    ) -> EngineResult<()> {
        validate_key(&key)?;
        if !self.functions.contains_key(&id) {
            return Err(invalid());
        }
        if key == "name"
            || key == "length"
            || (key == "prototype" && id == VM_FUNCTION_CONSTRUCTOR_ID)
        {
            return if strict {
                Err(EngineError::new(
                    "script_execution_error",
                    "Cannot assign to readonly function metadata.",
                    "Use a different own data property.",
                ))
            } else {
                Ok(())
            };
        }
        unavailable(&key)?;
        if !self.functions[&id].properties.contains_key(&key)
            && self.property_count() >= VM_PROPERTIES
        {
            return Err(EngineError::limit("V9 function properties", VM_PROPERTIES));
        }
        self.functions
            .get_mut(&id)
            .ok_or_else(invalid)?
            .properties
            .insert(key, value);
        Ok(())
    }
    /** Saved own data cannot override immutable source metadata or claim unavailable reflection support. */
    pub(super) fn validate_properties(&self, id: Id, function: &Function) -> EngineResult<()> {
        for key in function.properties.keys() {
            validate_key(key)?;
            if key == "name" || key == "length" || key == "prototype" || unavailable(key).is_err() {
                return Err(invalid());
            }
        }
        if id < VM_FUNCTION_INTRINSICS
            && !matches!(&function.kind, Callable::Intrinsic { intrinsic } if intrinsic.id() == id)
        {
            return Err(invalid());
        }
        Ok(())
    }
}
/** Property names use the same UTF-16 ceiling on functions as on ordinary objects. */
fn validate_key(key: &str) -> EngineResult<()> {
    if key.encode_utf16().count() > VM_PROPERTY_KEY_UNITS {
        return Err(EngineError::limit(
            "V9 property key UTF-16 units",
            VM_PROPERTY_KEY_UNITS,
        ));
    }
    Ok(())
}
/** Unsupported reflection remains an error rather than a fabricated prototype or host capability. */
fn unavailable(key: &str) -> EngineResult<()> {
    if [
        "prototype",
        "__proto__",
        "caller",
        "arguments",
        "toString",
        "valueOf",
        "hasOwnProperty",
        "isPrototypeOf",
        "propertyIsEnumerable",
        "__defineGetter__",
        "__defineSetter__",
        "__lookupGetter__",
        "__lookupSetter__",
    ]
    .contains(&key)
    {
        return Err(EngineError::unsupported(
            "Function reflection and arbitrary prototype operations are not implemented yet.",
        ));
    }
    Ok(())
}
