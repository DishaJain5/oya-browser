/** Primitive string reads and array method identities stay separate from page-global property ownership. */
use super::*;
impl Interpreter {
    /** String indices expose one raw UTF-16 unit while methods retain shared callable identities. */
    pub(super) fn string_property(
        &self,
        value: &super::super::JsString,
        key: &str,
        location: SourceLocation,
    ) -> EngineResult<JsValue> {
        if key.encode_utf16().count() > crate::constants::VM_PROPERTY_KEY_UNITS {
            return Err(errors::located(
                EngineError::limit(
                    "V9 property key UTF-16 units",
                    crate::constants::VM_PROPERTY_KEY_UNITS,
                ),
                location,
                "execute",
            ));
        }
        match key {
            "length" => return Ok(JsValue::Number(value.len() as f64)),
            "trim" => return Ok(JsValue::Function(crate::constants::VM_STRING_TRIM_ID)),
            "split" => return Ok(JsValue::Function(crate::constants::VM_STRING_SPLIT_ID)),
            _ => {}
        }
        if unsupported_string_property(key) {
            return Err(errors::unsupported(
                location,
                "String methods and reflection beyond trim, split, length and indexed reads",
            ));
        }
        Ok(key
            .parse::<usize>()
            .ok()
            .filter(|index| index.to_string() == key)
            .and_then(|index| value.as_units().get(index))
            .map(|unit| super::super::JsString::from_units(vec![*unit]).map(JsValue::String))
            .transpose()?
            .unwrap_or(JsValue::Undefined))
    }
}
/** Only actual arrays inherit these bounded helpers, after explicit own-property shadowing. */
pub(super) fn array_method(key: &str) -> Option<u32> {
    match key {
        "map" => Some(crate::constants::VM_ARRAY_MAP_ID),
        "filter" => Some(crate::constants::VM_ARRAY_FILTER_ID),
        "join" => Some(crate::constants::VM_ARRAY_JOIN_ID),
        "some" => Some(crate::constants::VM_ARRAY_SOME_ID),
        "find" => Some(crate::constants::VM_ARRAY_FIND_ID),
        _ => None,
    }
}
/** Known standard operations must not appear implemented merely because an absent property returns undefined. */
fn unsupported_string_property(key: &str) -> bool {
    [
        "at",
        "charAt",
        "charCodeAt",
        "codePointAt",
        "concat",
        "constructor",
        "endsWith",
        "includes",
        "indexOf",
        "isWellFormed",
        "lastIndexOf",
        "localeCompare",
        "match",
        "matchAll",
        "normalize",
        "padEnd",
        "padStart",
        "repeat",
        "replace",
        "replaceAll",
        "search",
        "slice",
        "startsWith",
        "substring",
        "substr",
        "toLocaleLowerCase",
        "toLocaleUpperCase",
        "toLowerCase",
        "toUpperCase",
        "toWellFormed",
        "trimEnd",
        "trimStart",
        "trimLeft",
        "trimRight",
        "toString",
        "valueOf",
        "__proto__",
        "prototype",
        "hasOwnProperty",
        "isPrototypeOf",
        "propertyIsEnumerable",
        "__defineGetter__",
        "__defineSetter__",
        "__lookupGetter__",
        "__lookupSetter__",
    ]
    .contains(&key)
}
