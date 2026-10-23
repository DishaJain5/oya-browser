/** Source schemas and iterative edge checks reject forged lexical graphs before any source runs. */
use super::*;
use crate::v9::{
    registry::{Registry, ScopeKind},
    value::Heap,
};
use std::collections::BTreeSet;
impl Graph {
    /** Every record is checked, including unreachable hostile input that collection would later discard. */
    pub fn validate(
        &self,
        registry: &Registry,
        heap: &Heap,
        roots: impl Iterator<Item = JsValue>,
    ) -> EngineResult<()> {
        self.bounds()?;
        for id in 0..crate::constants::VM_FUNCTION_INTRINSICS {
            let function = self.functions.get(&id).ok_or_else(invalid)?;
            if !matches!(&function.kind, Callable::Intrinsic { intrinsic } if intrinsic.id() == id)
            {
                return Err(invalid());
            }
        }
        for value in roots.chain(heap.all_references().cloned()) {
            self.value(&value, heap)?;
        }
        for (id, env) in &self.environments {
            self.environment_schema(*id, env, registry)?;
            if env.receiver.is_some() != (env.scope_id.kind == ScopeKind::Function) {
                return Err(invalid());
            }
            if let Some(value) = &env.receiver {
                self.value(value, heap)?;
            }
            for cell in env.bindings.values() {
                if let Some(value) = &cell.value {
                    self.value(value, heap)?;
                }
            }
        }
        for (id, function) in &self.functions {
            self.validate_properties(*id, function)?;
            for value in function.references() {
                self.value(&value, heap)?;
            }
            self.callable_schema(*id, function, registry)?;
        }
        Ok(())
    }
    /** A context may retain a TDZ cell, but its name and mutability always come from parsed source. */
    fn environment_schema(
        &self,
        id: Id,
        env: &Environment,
        registry: &Registry,
    ) -> EngineResult<()> {
        let schema = registry.scopes.get(&env.scope_id).ok_or_else(invalid)?;
        if env.bindings.len() != schema.bindings.len()
            || schema.bindings.iter().any(|(name, kind)| {
                !env.bindings.get(name).is_some_and(|cell| {
                    cell.kind == *kind
                        && (*kind != crate::v9::BindingKind::Var || cell.value.is_some())
                })
            })
        {
            return Err(invalid());
        }
        self.parent(env.parent, schema.parent)?;
        let mut seen = BTreeSet::new();
        let mut cursor = Some(id);
        while let Some(next) = cursor {
            if !seen.insert(next) || seen.len() > VM_ENVIRONMENTS {
                return Err(invalid());
            }
            cursor = self.get(next)?.parent;
        }
        if env.scope_id.kind == ScopeKind::Named {
            self.private_name(id, env)?;
        }
        Ok(())
    }
    /** Lexical parents must correspond to the exact defining source scope, not the caller. */
    fn parent(&self, actual: Option<Id>, expected: Option<ScopeId>) -> EngineResult<()> {
        match (actual, expected) {
            (None, None) => Ok(()),
            (Some(id), Some(scope)) if self.get(id)?.scope_id == scope => Ok(()),
            _ => Err(invalid()),
        }
    }
    /** A named expression's private cell points back to that same closure and source site. */
    fn private_name(&self, id: Id, env: &Environment) -> EngineResult<()> {
        let value = env
            .bindings
            .values()
            .next()
            .and_then(|cell| cell.value.as_ref())
            .ok_or_else(invalid)?;
        let JsValue::Function(function) = value else {
            return Err(invalid());
        };
        let function = self.functions.get(function).ok_or_else(invalid)?;
        if !matches!(&function.kind, Callable::Closure { code_id, environment, .. } if *code_id == env.scope_id.site && *environment == Some(id))
        {
            return Err(invalid());
        }
        Ok(())
    }
    /** Values never infer ownership merely because their ID fits the arena's numeric type. */
    fn value(&self, value: &JsValue, heap: &Heap) -> EngineResult<()> {
        match value {
            JsValue::Object(id) if !heap.contains(*id) => Err(invalid()),
            JsValue::Function(id) if !self.functions.contains_key(id) => Err(invalid()),
            _ => Ok(()),
        }
    }
    /** Source closures and canonical intrinsic identities are checked independently from mutable data. */
    fn callable_schema(
        &self,
        id: Id,
        function: &Function,
        registry: &Registry,
    ) -> EngineResult<()> {
        match &function.kind {
            Callable::Closure {
                code_id,
                environment,
                lexical_this,
            } => {
                let code = registry.codes.get(code_id).ok_or_else(invalid)?;
                if lexical_this.is_some() != (code.kind == crate::v9::parser::FunctionKind::Arrow) {
                    return Err(invalid());
                }
                self.name(id, registry)?;
                self.parent(*environment, code.capture)
            }
            Callable::Intrinsic { intrinsic } if intrinsic.id() == id => Ok(()),
            Callable::Intrinsic { .. } => Err(invalid()),
            Callable::Bound {
                target,
                arguments,
                name,
                length,
                ..
            } => {
                if *target >= id {
                    return Err(invalid());
                }
                self.bound_chain(*target, arguments.len(), 1)?;
                let expected_length = self
                    .length(*target, registry)?
                    .saturating_sub(arguments.len());
                let expected_name = crate::v9::JsString::from_utf8(&format!(
                    "bound {}",
                    self.name(*target, registry)?.to_dom()?
                ))?;
                if *length != expected_length || *name != expected_name {
                    return Err(invalid());
                }
                Ok(())
            }
        }
    }
}
