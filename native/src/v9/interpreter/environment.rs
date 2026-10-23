/** Shared lexical resolution walks defining context links rather than the dynamic call stack. */
use super::*;
/** Assignment retains a stable cell identity before evaluating a potentially mutating RHS. */
#[derive(Clone, Copy)]
pub(super) enum Cell {
    Global,
    Function,
    GlobalProperty,
    BuiltinFunction,
    Environment(Id),
}
impl Interpreter {
    /** An implicit arguments boundary blocks outer cells before ordinary global fallback. */
    pub(super) fn resolve(
        &self,
        name: &str,
        location: SourceLocation,
    ) -> EngineResult<Option<Cell>> {
        let mut cursor = self.locals.last().copied();
        while let Some(id) = cursor {
            let environment = self.graph.get(id)?;
            if environment.bindings.contains_key(name) {
                return Ok(Some(Cell::Environment(id)));
            }
            if name == "arguments"
                && environment.scope_id.kind == ScopeKind::Function
                && self
                    .registry
                    .codes
                    .get(&environment.scope_id.site)
                    .is_some_and(|code| code.kind == super::super::parser::FunctionKind::Ordinary)
            {
                return Err(errors::unsupported(location, "implicit arguments objects"));
            }
            cursor = environment.parent;
        }
        Ok(if self.globals.contains_key(name) {
            Some(Cell::Global)
        } else if self.functions.contains_key(name) {
            Some(Cell::Function)
        } else if self.heap.has(self.global_object, name) {
            Some(Cell::GlobalProperty)
        } else if name == "Function" {
            Some(Cell::BuiltinFunction)
        } else {
            None
        })
    }
    /** The nearest uninitialized cell blocks both outer scopes and builtin fallback. */
    pub(super) fn read_cell(
        &self,
        target: Cell,
        name: &str,
        location: SourceLocation,
    ) -> EngineResult<JsValue> {
        if matches!(target, Cell::BuiltinFunction) {
            return Ok(JsValue::Function(
                crate::constants::VM_FUNCTION_CONSTRUCTOR_ID,
            ));
        }
        if matches!(target, Cell::GlobalProperty) {
            return self
                .heap
                .get(self.global_object, name)
                .map_err(|error| errors::located(error, location, "execute"));
        }
        let cell = match target {
            Cell::Global => &self.globals[name],
            Cell::Function => &self.functions[name],
            Cell::Environment(id) => &self.graph.get(id)?.bindings[name],
            Cell::GlobalProperty => unreachable!("global property handled before cell lookup"),
            Cell::BuiltinFunction => unreachable!("builtin function handled before cell lookup"),
        };
        cell.value.clone().ok_or_else(|| {
            errors::execution(
                location,
                "Cannot access a lexical binding before initialization",
            )
        })
    }
    /** Source-owned cells are never removed while a statement retains their identity. */
    pub(super) fn cell_mut(&mut self, target: Cell, name: &str) -> &mut Binding {
        match target {
            Cell::Global => self.globals.get_mut(name),
            Cell::Function => self.functions.get_mut(name),
            Cell::Environment(id) => self
                .graph
                .environments
                .get_mut(&id)
                .and_then(|env| env.bindings.get_mut(name)),
            Cell::GlobalProperty | Cell::BuiltinFunction => None,
        }
        .expect("validated lexical identity")
    }
    /** Declaration cells begin at undefined while lexical cells retain their temporal dead zone. */
    pub(super) fn new_scope(&mut self, scope: ScopeId, parent: Option<Id>) -> EngineResult<Id> {
        let schema = self
            .registry
            .scopes
            .get(&scope)
            .ok_or_else(super::super::registry::invalid)?;
        let bindings = schema
            .bindings
            .iter()
            .map(|(name, kind)| {
                (
                    name.clone(),
                    Binding {
                        kind: *kind,
                        value: (*kind == BindingKind::Var).then_some(JsValue::Undefined),
                    },
                )
            })
            .collect();
        self.graph.environment(scope, parent, bindings)
    }
    /** Ordinary nested blocks retain the active defining environment even after the block exits. */
    pub(super) fn enter_scope(&mut self, scope: ScopeId) -> EngineResult<Id> {
        let id = self.new_scope(scope, self.locals.last().copied())?;
        self.locals.push(id);
        Ok(id)
    }
    /** Declaration execution initializes exactly the cell instantiated for the current lexical list. */
    pub(super) fn binding(
        &mut self,
        name: String,
        _kind: BindingKind,
        expression: Expression,
        location: SourceLocation,
    ) -> EngineResult<()> {
        let value = self.expression(expression)?;
        let target = if let Some(id) = self.locals.last().copied() {
            Cell::Environment(id)
        } else {
            Cell::Global
        };
        self.cell_mut(target, &name).value = Some(value);
        self.environment()
            .map_err(|error| errors::located(error, location, "execute"))
    }
    /** LHS resolution precedes RHS evaluation; TDZ and immutable-store checks follow it. */
    pub(super) fn assign(
        &mut self,
        name: String,
        expression: Expression,
        location: SourceLocation,
    ) -> EngineResult<()> {
        let reference = self.identifier_reference(name, location);
        let value = self.expression(expression)?;
        self.write_reference(reference, value)
    }
}
/** Builtins are fallback values, never hidden declarations in captured contexts. */
pub(super) fn builtin(name: &str) -> Option<JsValue> {
    match name {
        "undefined" => Some(JsValue::Undefined),
        "NaN" => Some(JsValue::Number(f64::NAN)),
        "Infinity" => Some(JsValue::Number(f64::INFINITY)),
        _ => None,
    }
}
