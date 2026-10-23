/** Typed ECMAScript scalar values retain UTF-16 and binary64 through private service messages. */
mod equality;
mod heap;
mod host;
mod number;
mod string;
mod wire;
use crate::{
    constants::JS_STRING_UNITS,
    protocol::{EngineError, EngineResult},
};
pub(super) use heap::{Heap, HeapObject, ObjectId};
pub use host::{HostKind, HostRef};
pub(super) use number::{literal, to_uint32};
pub use string::CodePointCursor;

/// JavaScript strings can retain lone surrogate code units without invalid Rust UTF-8.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JsString(Vec<u16>);
impl JsString {
    /// Explicit DOM inputs are UTF-8, converted once at the VM boundary.
    pub fn from_utf8(value: &str) -> EngineResult<Self> {
        Self::from_units(value.encode_utf16().collect())
    }
    /// Limits apply before a literal or service-supplied string enters an environment.
    pub fn from_units(units: Vec<u16>) -> EngineResult<Self> {
        if units.len() > JS_STRING_UNITS {
            return Err(EngineError::limit(
                "JavaScript string UTF-16 units",
                JS_STRING_UNITS,
            ));
        }
        Ok(Self(units))
    }
    /// Count code units rather than Rust Unicode scalar values or UTF-8 bytes.
    pub fn len(&self) -> usize {
        self.0.len()
    }
    /// Environment validation can count an empty string without decoding it.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    /// DOMString support is incomplete; fail explicitly rather than silently replacing surrogates.
    pub fn to_dom(&self) -> EngineResult<String> {
        String::from_utf16(&self.0).map_err(|_|EngineError::unsupported("The UTF-8 document boundary cannot represent an unpaired JavaScript surrogate yet."))
    }
    /** Concatenation checks the code-unit budget before reserving output. */
    fn concat(&self, other: &Self) -> EngineResult<Self> {
        if self.len() + other.len() > JS_STRING_UNITS {
            return Err(EngineError::limit(
                "JavaScript string UTF-16 units",
                JS_STRING_UNITS,
            ));
        }
        let mut units = Vec::with_capacity(self.len() + other.len());
        units.extend_from_slice(&self.0);
        units.extend_from_slice(&other.0);
        Ok(Self(units))
    }
}
/// Primitive values retain their language type without exposing any host objects.
#[derive(Clone, Debug, PartialEq)]
pub enum JsValue {
    Number(f64),
    String(JsString),
    Boolean(bool),
    Null,
    Undefined,
    Object(ObjectId),
    Function(u32),
    /// Request-bound document identities carry data without native authority.
    Host(HostRef),
}
impl JsValue {
    /// Input strings remain literal values; entering one never evaluates source.
    pub fn string(value: &str) -> EngineResult<Self> {
        Ok(Self::String(JsString::from_utf8(value)?))
    }
    /// Number conversion uses validated ECMAScript StringNumericLiteral syntax.
    pub fn to_number(&self) -> f64 {
        match self {
            Self::Number(value) => *value,
            Self::Boolean(value) => {
                if *value {
                    1.0
                } else {
                    0.0
                }
            }
            Self::Null => 0.0,
            Self::Undefined => f64::NAN,
            Self::Object(_) | Self::Function(_) | Self::Host(_) => f64::NAN,
            Self::String(value) => value
                .to_dom()
                .map(|s| number::string_number(&s))
                .unwrap_or(f64::NAN),
        }
    }
    /// Numeric rendering uses the pinned ECMA-compatible primitive rather than Rust formatting.
    pub fn to_js_string(&self) -> EngineResult<JsString> {
        match self {
            Self::String(value) => Ok(value.clone()),
            Self::Number(value) => JsString::from_utf8(ryu_js::Buffer::new().format(*value)),
            Self::Boolean(true) => JsString::from_utf8("true"),
            Self::Boolean(false) => JsString::from_utf8("false"),
            Self::Null => JsString::from_utf8("null"),
            Self::Undefined => JsString::from_utf8("undefined"),
            Self::Object(_) => Err(EngineError::unsupported(
                "Object conversion requires JavaScript ToPrimitive, which is not implemented yet.",
            )),
            Self::Host(_) => Err(EngineError::unsupported(
                "Host-to-primitive conversion is not implemented yet.",
            )),
            Self::Function(_) => Err(EngineError::unsupported(
                "Function-to-primitive conversion is not implemented yet.",
            )),
        }
    }
    /// Text proposals are validated before any document publication.
    pub fn to_dom(&self) -> EngineResult<String> {
        self.to_js_string()?.to_dom()
    }
    /// Numeric and string operands retain their type for JavaScript addition.
    pub fn add(&self, other: &Self) -> EngineResult<Self> {
        if matches!(self, Self::String(_)) || matches!(other, Self::String(_)) {
            Ok(Self::String(
                self.to_js_string()?.concat(&other.to_js_string()?)?,
            ))
        } else {
            Ok(Self::Number(self.to_number() + other.to_number()))
        }
    }
    /// Aggregate environments charge strings, independently of their serialized JSON size.
    pub fn string_units(&self) -> usize {
        match self {
            Self::String(value) => value.len(),
            Self::Host(value) => value.document.encode_utf16().count(),
            _ => 0,
        }
    }
    /// Truthiness never converts a string to a number or invokes native code.
    pub fn truthy(&self) -> bool {
        match self {
            Self::Undefined | Self::Null => false,
            Self::Boolean(value) => *value,
            Self::Number(value) => *value != 0.0 && !value.is_nan(),
            Self::String(value) => !value.is_empty(),
            Self::Object(_) | Self::Function(_) | Self::Host(_) => true,
        }
    }
    /// Strict equality compares matching primitive types, treating NaN and signed zero as JavaScript does.
    pub fn strict_equal(&self, other: &Self) -> bool {
        self == other
    }
    /// Two strings compare raw UTF-16 units; all other supported primitive pairs compare numerically.
    pub fn compare(&self, other: &Self) -> Option<std::cmp::Ordering> {
        if let (Self::String(left), Self::String(right)) = (self, other) {
            Some(left.0.cmp(&right.0))
        } else {
            self.to_number().partial_cmp(&other.to_number())
        }
    }
}
