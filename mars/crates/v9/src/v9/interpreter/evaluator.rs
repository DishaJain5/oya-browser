/** Located expressions evaluate left to right and consume the interpreter budget. */
use super::*;
use crate::protocol::EngineResult;
use crate::v9::StorageState;
use crate::v9::operator::Operator;
use crate::v9::parser::{StorageArea, StorageMethod};
use std::collections::BTreeMap;
impl Interpreter {
    /** JavaScript logical operators preserve short-circuiting and operand values. */
    pub(super) fn expression(&mut self, expression: Expression) -> EngineResult<JsValue> {
        let location = expression.location;
        self.step(location)?;
        match expression.kind {
            ExpressionKind::Literal(value) => Ok(value),
            ExpressionKind::This => self.current_receiver(),
            ExpressionKind::Document => self.document_value(),
            ExpressionKind::Variable(name) => self.variable(&name, location),
            ExpressionKind::Input(id) => self.input_value(&id, location),
            ExpressionKind::Unary(operator, operand) => self.unary(operator, *operand, location),
            ExpressionKind::Binary(Operator::And, left, right) => {
                self.logical(*left, *right, false)
            }
            ExpressionKind::Binary(Operator::Or, left, right) => self.logical(*left, *right, true),
            ExpressionKind::Binary(operator, left, right) => {
                self.binary(operator, *left, *right, location)
            }
            ExpressionKind::Assignment(operator, target, value) => {
                self.assignment_expression(operator, *target, *value, location)
            }
            ExpressionKind::Update(operator, prefix, target) => {
                self.update_expression(operator, prefix, *target, location)
            }
            ExpressionKind::Conditional(test, yes, no) => {
                self.conditional_expression(*test, *yes, *no)
            }
            ExpressionKind::Array(values) => self.array_literal(values, location),
            ExpressionKind::Object(properties) => self.object(properties, location),
            ExpressionKind::Member(object, key) => self.member(*object, *key, location),
            ExpressionKind::Invoke(callee, arguments) => self.invoke(*callee, arguments, location),
            ExpressionKind::Function(_) => self.closure(location.byte_offset, location),
            ExpressionKind::StorageArea(_) | ExpressionKind::StorageMethod(_, _) => {
                Err(errors::unsupported(
                    location,
                    "Storage area values outside supported method calls",
                ))
            }
            ExpressionKind::StorageCall(area, method, arguments) => {
                self.storage_call(area, method, arguments, location)
            }
        }
    }

    /** Execute one exact Storage method against the candidate maps. */
    fn storage_call(
        &mut self,
        area: StorageArea,
        method: StorageMethod,
        expressions: Vec<Expression>,
        location: SourceLocation,
    ) -> EngineResult<JsValue> {
        let arguments = self.storage_arguments(expressions, location)?;
        let expected = storage_arity(method);
        if arguments.len() != expected {
            return Err(errors::execution(
                location,
                "invalid Storage method arguments",
            ));
        }
        self.apply_storage(area, method, arguments, location)
    }

    /** Storage method arguments use bounded primitive string conversion. */
    fn storage_arguments(
        &mut self,
        expressions: Vec<Expression>,
        location: SourceLocation,
    ) -> EngineResult<Vec<String>> {
        expressions
            .into_iter()
            .map(|expression| {
                self.expression(expression)
                    .and_then(|value| value.to_dom())
                    .map_err(|error| errors::located(error, location, "execute"))
            })
            .collect()
    }

    /** Candidate mutation is assigned only after both entry and byte quotas pass. */
    fn apply_storage(
        &mut self,
        area: StorageArea,
        method: StorageMethod,
        arguments: Vec<String>,
        location: SourceLocation,
    ) -> EngineResult<JsValue> {
        let current = storage_map(&self.storage, area).clone();
        let (candidate, result) = storage_change(current, method, arguments)
            .map_err(|error| errors::located(error, location, "execute"))?;
        let mut storage = self.storage.clone();
        *storage_map_mut(&mut storage, area) = candidate.clone();
        storage
            .validate()
            .map_err(|error| errors::located(error, location, "execute"))?;
        *storage_map_mut(&mut self.storage, area) = candidate;
        Ok(result)
    }

    /** Object properties evaluate in source order, with later duplicate keys overwriting earlier ones. */
    fn object(
        &mut self,
        properties: Vec<(String, Expression)>,
        location: SourceLocation,
    ) -> EngineResult<JsValue> {
        let value = self
            .heap
            .allocate(HeapObject::Object {
                properties: Default::default(),
            })
            .map_err(|error| errors::located(error, location, "execute"))?;
        let JsValue::Object(id) = value else {
            return Err(errors::execution(location, "object allocation failed"));
        };
        for (key, expression) in properties {
            let value = self.expression(expression)?;
            self.heap
                .set(id, key, value)
                .map_err(|error| errors::located(error, location, "execute"))?;
        }
        self.environment()
            .map_err(|error| errors::located(error, location, "execute"))?;
        Ok(JsValue::Object(id))
    }

    /** Member reads are own-property only and never consult prototypes or host objects. */
    fn member(
        &mut self,
        object: Expression,
        key: Expression,
        location: SourceLocation,
    ) -> EngineResult<JsValue> {
        let (receiver, key) = self.property_reference(object, key, location)?;
        self.get_property(&receiver, &key, location)
    }

    /** A logical expression evaluates its right side only when required. */
    fn logical(
        &mut self,
        left: Expression,
        right: Expression,
        disjunction: bool,
    ) -> EngineResult<JsValue> {
        let value = self.expression(left)?;
        if value.truthy() == disjunction {
            Ok(value)
        } else {
            self.expression(right)
        }
    }

    /** Binary operations preserve operand order, coercion, and source diagnostics. */
    fn binary(
        &mut self,
        operator: Operator,
        left: Expression,
        right: Expression,
        location: SourceLocation,
    ) -> EngineResult<JsValue> {
        let left = self.expression(left)?;
        let right = self.expression(right)?;
        self.value_operation(operator, left, right, location)
    }

    /** Name lookup observes the innermost initialized binding before page globals. */
    pub(super) fn variable(&self, name: &str, location: SourceLocation) -> EngineResult<JsValue> {
        if let Some(target) = self.resolve(name, location)? {
            return self.read_cell(target, name, location);
        }
        self.unresolved_identifier(name, location)
    }
    /** Supported global fallback does not invent native objects or rerun identifier resolution. */
    pub(super) fn unresolved_identifier(
        &self,
        name: &str,
        location: SourceLocation,
    ) -> EngineResult<JsValue> {
        if name == "globalThis" {
            return Ok(JsValue::Object(self.global_object));
        }
        if let Some(value) = super::environment::builtin(name) {
            return Ok(value);
        }
        if properties::protected_global(name) {
            return Err(errors::unsupported(
                location,
                "unavailable native global values",
            ));
        }
        Err(errors::execution(location, "Binding is not defined"))
    }
}

/** Storage methods accept a fixed argument count rather than generic invocation. */
fn storage_arity(method: StorageMethod) -> usize {
    match method {
        StorageMethod::GetItem | StorageMethod::RemoveItem => 1,
        StorageMethod::SetItem => 2,
        StorageMethod::Clear => 0,
    }
}

/** Select one candidate area without exposing a mutable map to page code. */
fn storage_map(state: &StorageState, area: StorageArea) -> &BTreeMap<String, String> {
    match area {
        StorageArea::Local => &state.local,
        StorageArea::Session => &state.session,
    }
}

/** Coordinator-owned candidate area selection stays explicit. */
fn storage_map_mut(state: &mut StorageState, area: StorageArea) -> &mut BTreeMap<String, String> {
    match area {
        StorageArea::Local => &mut state.local,
        StorageArea::Session => &mut state.session,
    }
}

/** Apply Storage string semantics to a private candidate copy. */
fn storage_change(
    mut map: BTreeMap<String, String>,
    method: StorageMethod,
    arguments: Vec<String>,
) -> EngineResult<(BTreeMap<String, String>, JsValue)> {
    let result = match method {
        StorageMethod::GetItem => match map.get(&arguments[0]) {
            Some(value) => JsValue::string(value)?,
            None => JsValue::Null,
        },
        StorageMethod::SetItem => {
            map.insert(arguments[0].clone(), arguments[1].clone());
            JsValue::Undefined
        }
        StorageMethod::RemoveItem => {
            map.remove(&arguments[0]);
            JsValue::Undefined
        }
        StorageMethod::Clear => {
            map.clear();
            JsValue::Undefined
        }
    };
    Ok((map, result))
}
