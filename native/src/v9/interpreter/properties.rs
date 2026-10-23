/** One page-owned property path keeps call references, global cells and heap writes coherent. */
use super::*;
impl Interpreter {
    /** Computed key expressions run before null checks; key conversion follows the coercibility check. */
    pub(super) fn property_reference(
        &mut self,
        base: Expression,
        key: Expression,
        location: SourceLocation,
    ) -> EngineResult<(JsValue, String)> {
        let receiver = self.expression(base)?;
        let key = self.expression(key)?;
        self.property_base(&receiver, location)?;
        let key = key
            .to_js_string()
            .and_then(|key| key.to_dom())
            .map_err(|error| errors::located(error, location, "execute"))?;
        Ok((receiver, key))
    }
    /** Object and callable identities expose only bounded page-owned data and supported intrinsics. */
    fn property_base(&self, receiver: &JsValue, location: SourceLocation) -> EngineResult<()> {
        match receiver {
            JsValue::Object(_) | JsValue::Function(_) => Ok(()),
            JsValue::Undefined => Err(errors::execution(
                location,
                "Cannot read a property of undefined",
            )),
            JsValue::Null => Err(errors::execution(
                location,
                "Cannot read a property of null",
            )),
            _ => Err(errors::unsupported(location, "primitive wrappers")),
        }
    }
    /** Root functions have exactly one cell, even when accessed through the global receiver. */
    pub(super) fn get_property(
        &self,
        receiver: &JsValue,
        key: &str,
        location: SourceLocation,
    ) -> EngineResult<JsValue> {
        if let JsValue::Function(id) = receiver {
            return self
                .graph
                .get_property(*id, key, &self.registry)
                .map_err(|error| errors::located(error, location, "execute"));
        }
        let JsValue::Object(id) = receiver else {
            return Err(super::super::registry::invalid());
        };
        if *id == self.global_object {
            if self.functions.contains_key(key) {
                return self.read_cell(environment::Cell::Function, key, location);
            }
            if key == "globalThis" {
                return Ok(receiver.clone());
            }
            if key == "Function" && !self.heap.has(*id, key) {
                return Ok(JsValue::Function(
                    crate::constants::VM_FUNCTION_CONSTRUCTOR_ID,
                ));
            }
            if let Some(value) = environment::builtin(key) {
                return Ok(value);
            }
            if protected_global(key) {
                return Err(errors::unsupported(location, "global host properties"));
            }
        }
        self.heap
            .get(*id, key)
            .map_err(|error| errors::located(error, location, "execute"))
    }
    /** All global aliases route to their source-owned cell; other page data uses bounded own properties. */
    pub(super) fn set_property(
        &mut self,
        receiver: &JsValue,
        key: String,
        value: JsValue,
        location: SourceLocation,
    ) -> EngineResult<()> {
        if let JsValue::Function(id) = receiver {
            self.graph
                .set_property(*id, key, value, &self.registry, self.strict)
                .map_err(|error| errors::located(error, location, "execute"))?;
            return self
                .environment()
                .map_err(|error| errors::located(error, location, "execute"));
        }
        let JsValue::Object(id) = receiver else {
            return Err(super::super::registry::invalid());
        };
        if *id == self.global_object && self.functions.contains_key(&key) {
            self.functions.get_mut(&key).expect("root cell").value = Some(value);
        } else {
            if *id == self.global_object && protected_global(&key) {
                return Err(errors::unsupported(
                    location,
                    "writes to protected global properties",
                ));
            }
            self.heap
                .set(*id, key, value)
                .map_err(|error| errors::located(error, location, "execute"))?;
        }
        self.environment()
            .map_err(|error| errors::located(error, location, "execute"))
    }
}
/** These names are explicit capability boundaries, never ambient native objects or writable aliases. */
pub(super) fn protected_global(key: &str) -> bool {
    PROTECTED_GLOBALS.contains(&key)
}
/** Property access and snapshot validation share the exact protected name set. */
pub(super) const PROTECTED_GLOBALS: &[&str] = &[
    "globalThis",
    "undefined",
    "NaN",
    "Infinity",
    "window",
    "document",
    "fetch",
    "eval",
    "require",
    "process",
    "localStorage",
    "sessionStorage",
];
