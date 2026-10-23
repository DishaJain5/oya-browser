/** Array storage preserves canonical indices, sparse presence and independent bounded allocation. */
use super::*;

/** Array indices use canonical decimal spelling and exclude the reserved length property. */
pub(super) fn array_index(key: &str) -> Option<usize> {
    let index = key.parse::<u32>().ok()?;
    (index.to_string() == key && index != u32::MAX).then_some(index as usize)
}

/** Reads return undefined for absent own properties and holes. */
pub(super) fn array_get(
    elements: &[Option<JsValue>],
    properties: &BTreeMap<String, JsValue>,
    length: usize,
    key: &str,
) -> EngineResult<JsValue> {
    if key == "length" {
        return Ok(JsValue::Number(length as f64));
    }
    Ok(array_index(key)
        .and_then(|index| elements.get(index))
        .and_then(Option::as_ref)
        .or_else(|| properties.get(key))
        .cloned()
        .unwrap_or(JsValue::Undefined))
}

/** Array writes accept bounded indices and writable length updates only. */
pub(super) fn set_array(
    elements: &mut Vec<Option<JsValue>>,
    properties: &mut BTreeMap<String, JsValue>,
    length: &mut usize,
    key: String,
    value: JsValue,
) -> EngineResult<()> {
    if key == "length" {
        return set_array_length(elements, length, value);
    }
    if let Some(index) = array_index(&key) {
        if index >= VM_ARRAY_LENGTH {
            return Err(EngineError::limit("V9 array length", VM_ARRAY_LENGTH));
        }
        elements.resize_with(elements.len().max(index + 1), || None);
        elements[index] = Some(value);
        *length = (*length).max(index + 1);
        return Ok(());
    }
    properties.insert(key, value);
    Ok(())
}

/** Length is bounded to prevent a tiny write from requesting unbounded sparse iteration. */
fn set_array_length(
    elements: &mut Vec<Option<JsValue>>,
    length: &mut usize,
    value: JsValue,
) -> EngineResult<()> {
    if matches!(value, JsValue::Host(_)) {
        return Err(EngineError::unsupported(
            "Host-to-primitive conversion is not implemented yet.",
        ));
    }
    let number = value.to_number();
    if !number.is_finite()
        || number.fract() != 0.0
        || number < 0.0
        || number as usize > VM_ARRAY_LENGTH
    {
        return Err(EngineError::invalid("Invalid bounded array length."));
    }
    *length = number as usize;
    elements.resize_with(*length, || None);
    Ok(())
}

impl Heap {
    /** Brand checks distinguish a valid nonarray from a dangling private reference. */
    pub fn is_array(&self, id: ObjectId) -> EngineResult<bool> {
        self.objects
            .get(&id)
            .map(|object| matches!(object, HeapObject::Array { .. }))
            .ok_or_else(invalid_reference)
    }
    /** Borrowing permits quota checks before copying string or host payloads. */
    pub fn array_element_ref(&self, id: ObjectId, index: usize) -> EngineResult<Option<&JsValue>> {
        match self.objects.get(&id) {
            Some(HeapObject::Array { elements, .. }) => {
                Ok(elements.get(index).and_then(Option::as_ref))
            }
            Some(_) => Err(EngineError::unsupported(
                "Array methods require an actual owned array.",
            )),
            None => Err(invalid_reference()),
        }
    }
    /** Only owned arrays inherit array algorithms; ordinary objects cannot masquerade through a length property. */
    pub fn array_length(&self, id: ObjectId) -> EngineResult<usize> {
        match self.objects.get(&id) {
            Some(HeapObject::Array { length, .. }) => Ok(*length),
            Some(_) => Err(EngineError::unsupported(
                "Array methods require an actual owned array.",
            )),
            None => Err(invalid_reference()),
        }
    }
    /** Presence is observed afresh before each callback without holding a heap borrow through author code. */
    pub fn array_element(&self, id: ObjectId, index: usize) -> EngineResult<Option<JsValue>> {
        match self.objects.get(&id) {
            Some(HeapObject::Array { elements, .. }) => Ok(elements.get(index).cloned().flatten()),
            Some(_) => Err(EngineError::unsupported(
                "Array methods require an actual owned array.",
            )),
            None => Err(invalid_reference()),
        }
    }
    /** Hole-filled results retain snapshot length without treating holes as present undefined entries. */
    pub fn allocate_array(&mut self, length: usize) -> EngineResult<JsValue> {
        if length > VM_ARRAY_LENGTH {
            return Err(EngineError::limit("V9 array length", VM_ARRAY_LENGTH));
        }
        self.allocate(HeapObject::Array {
            elements: vec![None; length],
            properties: BTreeMap::new(),
            length,
        })
    }
}
