/** Flat closure/context arenas preserve identity, shared lexical cells and bounded private serialization. */
mod callable;
mod collect;
mod edges;
mod intrinsic;
mod properties;
mod validate;
use super::{
    Globals, JsValue,
    registry::{ScopeId, invalid},
};
use crate::{
    constants::{VM_BINDINGS, VM_ENVIRONMENTS, VM_FUNCTIONS},
    protocol::{EngineError, EngineResult},
};
pub(super) use callable::{Callable, Intrinsic};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
/** Arena IDs are monotonic; reclamation never aliases an older function or lexical environment. */
pub(super) type Id = u32;
/** A closure pairs source-owned code with its defining lexical context. */
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Function {
    /// Parsed source, rather than wire input, owns the executable body.
    pub kind: Callable,
    /// Page-authored own data stays separate from source-owned name and arity.
    pub properties: BTreeMap<String, JsValue>,
}
/** A popped environment survives only while a reachable closure or lexical descendant retains it. */
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Environment {
    /// Source metadata pins the exact cell names and mutability.
    pub scope_id: ScopeId,
    /// Lexical links preserve defining lookup after the active stack changes.
    pub parent: Option<Id>,
    /// Shared cells make sibling closures observe the same assignments.
    pub bindings: Globals,
    /// Only invocation scopes own a receiver; blocks and private names inherit through parents.
    pub receiver: Option<JsValue>,
}
/** All edges are IDs; value cycles therefore have finite wire size and deterministic tracing. */
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Graph {
    /// Records outlive stack entries only while traced roots retain them.
    pub environments: BTreeMap<Id, Environment>,
    /// Stable IDs preserve equality through private service messages.
    pub functions: BTreeMap<Id, Function>,
    /// Collection never reuses an old environment's identity.
    next_environment: Id,
    /// Cumulative allocations are independent of the live function cap.
    next_function: Id,
}
impl Graph {
    /** Per-request live allocations cannot outgrow the context or captured-cell bounds. */
    pub fn environment(
        &mut self,
        scope_id: ScopeId,
        parent: Option<Id>,
        bindings: Globals,
    ) -> EngineResult<Id> {
        if self.environments.len() >= VM_ENVIRONMENTS {
            return Err(EngineError::limit("V9 environments", VM_ENVIRONMENTS));
        }
        if self.binding_count() + bindings.len() > VM_BINDINGS {
            return Err(EngineError::limit("V9 captured bindings", VM_BINDINGS));
        }
        let id = self.next_environment;
        self.next_environment = id.checked_add(1).ok_or_else(invalid)?;
        self.environments.insert(
            id,
            Environment {
                scope_id,
                parent,
                bindings,
                receiver: None,
            },
        );
        Ok(id)
    }
    /** Code is never copied into closure values or supplied by the serialized graph. */
    pub fn function(
        &mut self,
        code_id: usize,
        environment: Option<Id>,
        lexical_this: Option<JsValue>,
    ) -> EngineResult<JsValue> {
        if self.functions.len() >= VM_FUNCTIONS {
            return Err(EngineError::limit("V9 functions", VM_FUNCTIONS));
        }
        let id = self.next_function;
        self.next_function = id.checked_add(1).ok_or_else(invalid)?;
        self.functions.insert(
            id,
            Function {
                kind: Callable::Closure {
                    code_id,
                    environment,
                    lexical_this,
                },
                properties: BTreeMap::new(),
            },
        );
        Ok(JsValue::Function(id))
    }
    /** Schema-owned lookups fail closed for every serialized reference. */
    pub fn get(&self, id: Id) -> EngineResult<&Environment> {
        self.environments.get(&id).ok_or_else(invalid)
    }
    /** Mutable cells stay in the same record while sibling closures observe their shared updates. */
    pub fn get_mut(&mut self, id: Id) -> EngineResult<&mut Environment> {
        self.environments.get_mut(&id).ok_or_else(invalid)
    }
    /** Every retained string counts once, including cells in contexts no longer active on the stack. */
    pub fn string_units(&self) -> usize {
        let cells: usize = self
            .environments
            .values()
            .flat_map(|env| env.bindings.values())
            .filter_map(|cell| cell.value.as_ref())
            .map(JsValue::string_units)
            .sum();
        cells
            + self
                .environments
                .values()
                .filter_map(|env| env.receiver.as_ref())
                .map(JsValue::string_units)
                .sum::<usize>()
            + self
                .functions
                .values()
                .map(Function::string_units)
                .sum::<usize>()
    }
    /** Cell count is independent of whether values happen to be strings or undefined. */
    fn binding_count(&self) -> usize {
        self.environments
            .values()
            .map(|env| env.bindings.len())
            .sum::<usize>()
            + self
                .functions
                .values()
                .map(|function| match &function.kind {
                    Callable::Bound { arguments, .. } => arguments.len(),
                    _ => 0,
                })
                .sum::<usize>()
    }
    /** Monotonic counters may exceed live caps after collection, but cannot precede any live identity. */
    pub fn bounds(&self) -> EngineResult<()> {
        if self.environments.len() > VM_ENVIRONMENTS
            || self.functions.len() > VM_FUNCTIONS
            || self.binding_count() > VM_BINDINGS
            || self.property_count() > crate::constants::VM_PROPERTIES
        {
            return Err(EngineError::limit(
                "V9 function/environment graph",
                VM_BINDINGS,
            ));
        }
        if self
            .environments
            .keys()
            .any(|id| *id >= self.next_environment)
            || self.functions.keys().any(|id| *id >= self.next_function)
        {
            return Err(invalid());
        }
        Ok(())
    }
}
