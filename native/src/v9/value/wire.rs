/** Typed private values preserve number bits, UTF-16 strings and stable object IDs. */
use super::{JsString, JsValue, ObjectId};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error};
/** No arbitrary fields, native references or executable objects cross this value boundary. */
#[derive(Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum WireValue {
    Number { bits: String },
    String { units: Vec<u16> },
    Boolean { value: bool },
    Null {},
    Undefined {},
    Object { id: ObjectId },
    Function { id: u32 },
}
impl Serialize for JsValue {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let value = match self {
            Self::Number(value) => WireValue::Number {
                bits: format!(
                    "{:016x}",
                    if value.is_nan() {
                        f64::NAN.to_bits()
                    } else {
                        value.to_bits()
                    }
                ),
            },
            Self::String(value) => WireValue::String {
                units: value.0.clone(),
            },
            Self::Boolean(value) => WireValue::Boolean { value: *value },
            Self::Null => WireValue::Null {},
            Self::Undefined => WireValue::Undefined {},
            Self::Object(id) => WireValue::Object { id: *id },
            Self::Function(id) => WireValue::Function { id: *id },
        };
        value.serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for JsValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match WireValue::deserialize(deserializer)? {
            WireValue::String { units } => JsString::from_units(units)
                .map(Self::String)
                .map_err(|e| D::Error::custom(e.message)),
            WireValue::Number { bits } => decode_number::<D>(&bits),
            WireValue::Boolean { value } => Ok(Self::Boolean(value)),
            WireValue::Null {} => Ok(Self::Null),
            WireValue::Undefined {} => Ok(Self::Undefined),
            WireValue::Object { id } => Ok(Self::Object(id)),
            WireValue::Function { id } => Ok(Self::Function(id)),
        }
    }
}
/** Validate before conversion, canonicalizing only NaN payloads. */
fn decode_number<'de, D: Deserializer<'de>>(bits: &str) -> Result<JsValue, D::Error> {
    if bits.len() != 16 || !bits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(D::Error::custom(
            "Number bits must contain exactly sixteen hexadecimal digits.",
        ));
    }
    let bits = u64::from_str_radix(bits, 16).map_err(D::Error::custom)?;
    let value = f64::from_bits(bits);
    Ok(JsValue::Number(if value.is_nan() {
        f64::NAN
    } else {
        value
    }))
}
