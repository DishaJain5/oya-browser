/** A bounded arena preserves object identity across isolated service requests. */
mod array;
use super::JsValue;
use crate::{
    constants::{VM_ARRAY_LENGTH, VM_OBJECTS, VM_PROPERTIES, VM_PROPERTY_KEY_UNITS},
    protocol::{EngineError, EngineResult},
};
use array::{array_get, array_index, set_array};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/** Arena indices remain stable while cloned page candidates are evaluated. */
pub type ObjectId = u32;

/** Records contain only own data properties; prototype behavior is not exposed. */
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum HeapObject {
    Object {
        properties: BTreeMap<String, JsValue>,
    },
    Array {
        elements: Vec<Option<JsValue>>,
        properties: BTreeMap<String, JsValue>,
        length: usize,
    },
}

/** The page owns one bounded heap and IDs are never reused within its lifetime. */
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Heap {
    objects: BTreeMap<ObjectId, HeapObject>,
    next_id: ObjectId,
}

impl Heap {
    /** Array and object allocation share the same monotonically increasing arena. */
    pub fn allocate(&mut self, object: HeapObject) -> EngineResult<JsValue> {
        if self.objects.len() >= VM_OBJECTS {
            return Err(EngineError::limit("V9 heap objects", VM_OBJECTS));
        }
        let id = self.next_id;
        self.next_id = id
            .checked_add(1)
            .ok_or_else(|| EngineError::limit("V9 heap identity", VM_OBJECTS))?;
        self.objects.insert(id, object);
        self.validate(std::iter::empty())?;
        Ok(JsValue::Object(id))
    }

    /** Property reads include array length and sparse indexed elements. */
    pub fn get(&self, id: ObjectId, key: &str) -> EngineResult<JsValue> {
        validate_key(key)?;
        let object = self.objects.get(&id).ok_or_else(invalid_reference)?;
        match object {
            HeapObject::Object { properties } => {
                Ok(properties.get(key).cloned().unwrap_or(JsValue::Undefined))
            }
            HeapObject::Array {
                elements,
                properties,
                length,
            } => array_get(elements, properties, *length, key),
        }
    }

    /** Writes mutate only a candidate heap and enforce aggregate own-property limits. */
    pub fn set(&mut self, id: ObjectId, key: String, value: JsValue) -> EngineResult<()> {
        validate_key(&key)?;
        let existing = self.objects.get(&id).ok_or_else(invalid_reference)?;
        let is_new = !has_key(existing, &key);
        if is_new && self.property_count() >= VM_PROPERTIES {
            return Err(EngineError::limit("V9 heap properties", VM_PROPERTIES));
        }
        let object = self.objects.get_mut(&id).ok_or_else(invalid_reference)?;
        match object {
            HeapObject::Object { properties } => {
                properties.insert(key, value);
            }
            HeapObject::Array {
                elements,
                properties,
                length,
            } => set_array(elements, properties, length, key, value)?,
        }
        Ok(())
    }

    /** Service-supplied heap state is checked before any evaluation begins. */
    pub fn validate(&self, roots: impl Iterator<Item = JsValue>) -> EngineResult<()> {
        if self.objects.len() > VM_OBJECTS
            || self.property_count() > VM_PROPERTIES
            || self.objects.keys().any(|id| *id >= self.next_id)
        {
            return Err(EngineError::limit("V9 heap state", VM_OBJECTS));
        }
        for root in roots {
            self.validate_value(&root)?;
        }
        for object in self.objects.values() {
            validate_object(object)?;
            for value in values(object) {
                self.validate_value(value)?;
            }
        }
        Ok(())
    }

    /** Strings held by object properties count toward the same page environment budget. */
    pub fn string_units(&self) -> usize {
        self.objects
            .values()
            .flat_map(values)
            .map(JsValue::string_units)
            .sum()
    }
    /** Closure tracing follows object edges without exposing mutable heap records to callers. */
    pub fn references(&self, id: ObjectId) -> EngineResult<Vec<JsValue>> {
        Ok(values(self.objects.get(&id).ok_or_else(invalid_reference)?)
            .cloned()
            .collect())
    }
    /** Graph validation includes functions held by every object, even unreachable input records. */
    pub fn all_references(&self) -> impl Iterator<Item = &JsValue> {
        self.objects.values().flat_map(values)
    }
    /** Identity validation does not infer ownership from the numerical range alone. */
    pub fn contains(&self, id: ObjectId) -> bool {
        self.objects.contains_key(&id)
    }
    /** The canonical script-global receiver cannot be a forged array or missing record. */
    pub fn validate_global(&self, id: ObjectId) -> EngineResult<()> {
        if !matches!(self.objects.get(&id), Some(HeapObject::Object { .. })) {
            return Err(invalid_reference());
        }
        Ok(())
    }
    /** Own-property presence distinguishes a missing global name from an undefined value. */
    pub fn has(&self, id: ObjectId, key: &str) -> bool {
        self.objects
            .get(&id)
            .is_some_and(|object| has_key(object, key))
    }
    /** Global declaration instantiation replaces an earlier configurable page-authored expando. */
    pub fn remove(&mut self, id: ObjectId, key: &str) -> EngineResult<()> {
        let Some(HeapObject::Object { properties }) = self.objects.get_mut(&id) else {
            return Err(invalid_reference());
        };
        properties.remove(key);
        Ok(())
    }
    /** Request-boundary collection preserves surviving IDs and never reuses retired ones. */
    pub fn sweep(&mut self, reachable: &std::collections::BTreeSet<ObjectId>) {
        self.objects.retain(|id, _| reachable.contains(id));
    }

    /** Identity references must point at an owned arena record. */
    fn validate_value(&self, value: &JsValue) -> EngineResult<()> {
        if let JsValue::Object(id) = value
            && !self.objects.contains_key(id)
        {
            return Err(invalid_reference());
        }
        Ok(())
    }

    /** Property counts include named and indexed records without counting array length. */
    pub fn property_count(&self) -> usize {
        self.objects.values().map(count_properties).sum()
    }
}

/** Property-key size is measured in UTF-16 code units as in the VM string model. */
fn validate_key(key: &str) -> EngineResult<()> {
    if key.encode_utf16().count() > VM_PROPERTY_KEY_UNITS {
        return Err(EngineError::limit(
            "V9 property key UTF-16 units",
            VM_PROPERTY_KEY_UNITS,
        ));
    }
    Ok(())
}

/** Object and array property counts share one page-wide cap. */
fn count_properties(object: &HeapObject) -> usize {
    match object {
        HeapObject::Object { properties } => properties.len(),
        HeapObject::Array {
            elements,
            properties,
            ..
        } => elements.iter().flatten().count() + properties.len(),
    }
}

/** Replacing an existing array element or named property does not consume another slot. */
fn has_key(object: &HeapObject, key: &str) -> bool {
    match object {
        HeapObject::Object { properties } => properties.contains_key(key),
        HeapObject::Array {
            elements,
            properties,
            ..
        } => {
            key == "length"
                || array_index(key)
                    .is_some_and(|index| elements.get(index).is_some_and(Option::is_some))
                || properties.contains_key(key)
        }
    }
}

/** Every nested reference is checked, including values held by sparse array elements. */
fn values(object: &HeapObject) -> Box<dyn Iterator<Item = &JsValue> + '_> {
    match object {
        HeapObject::Object { properties } => Box::new(properties.values()),
        HeapObject::Array {
            elements,
            properties,
            ..
        } => Box::new(elements.iter().flatten().chain(properties.values())),
    }
}

/** Deserialized records obey the same key, array and element limits as fresh allocations. */
fn validate_object(object: &HeapObject) -> EngineResult<()> {
    match object {
        HeapObject::Object { properties } => validate_keys(properties),
        HeapObject::Array {
            elements,
            properties,
            length,
        } => {
            if *length > VM_ARRAY_LENGTH
                || elements.len() != *length
                || properties
                    .keys()
                    .any(|key| key == "length" || array_index(key).is_some())
            {
                return Err(EngineError::invalid("Invalid bounded array record."));
            }
            validate_keys(properties)
        }
    }
}

/** Wire property names must fit the per-key UTF-16 limit. */
fn validate_keys(properties: &BTreeMap<String, JsValue>) -> EngineResult<()> {
    for key in properties.keys() {
        validate_key(key)?;
    }
    Ok(())
}

/** Dangling object references fail closed at the V9 boundary. */
fn invalid_reference() -> EngineError {
    EngineError::invalid("Invalid V9 object reference.")
}
