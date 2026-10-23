/** Source-ordered root declarations validate event snapshots without replaying page initialization. */
use super::declaration_schema::{extend, lexical, object_names};
use super::*;
/** Missing, extra, kind-tampered or uninitialized root cells are refused before any callback. */
pub(super) fn validate_state(
    programs: &[Program],
    state: &VmState,
    registry: &Registry,
) -> EngineResult<()> {
    state.validate()?;
    let global = state
        .global_object
        .ok_or_else(super::super::registry::invalid)?;
    let (mut globals, mut functions) = (Globals::new(), BTreeMap::new());
    for program in programs {
        extend(&globals, &functions, program)?;
        globals.extend(lexical(program));
        functions.extend(object_names(program));
    }
    if globals.len() != state.globals.len()
        || globals.iter().any(|(name, cell)| {
            !state
                .globals
                .get(name)
                .is_some_and(|actual| actual.kind == cell.kind && actual.value.is_some())
        })
    {
        return Err(EngineError::invalid(
            "V9 global bindings do not match the loaded declarations.",
        ));
    }
    if functions.len() != state.functions.len()
        || functions.keys().any(|name| {
            !state
                .functions
                .get(name)
                .is_some_and(|cell| cell.kind == BindingKind::Var && cell.value.is_some())
        })
    {
        return Err(EngineError::invalid(
            "V9 global object declarations do not match the loaded declarations.",
        ));
    }
    for name in functions
        .keys()
        .map(String::as_str)
        .chain(properties::PROTECTED_GLOBALS.iter().copied())
    {
        if state.heap.has(global, name) {
            return Err(super::super::registry::invalid());
        }
    }

    state.graph.validate(
        registry,
        &state.heap,
        state
            .globals
            .values()
            .chain(state.functions.values())
            .filter_map(|cell| cell.value.clone()),
    )
}
impl Interpreter {
    /** Each script installs only its own declarations after earlier scripts have completed. */
    pub(super) fn instantiate_script(&mut self, program: &[Statement]) -> EngineResult<()> {
        extend(
            &self.globals,
            &self
                .functions
                .keys()
                .map(|name| (name.clone(), SourceLocation::default()))
                .collect(),
            program,
        )?;
        self.globals.extend(lexical(program));
        if self.globals.len() > STATEMENTS {
            return Err(errors::located(
                EngineError::limit("V9 variables", STATEMENTS),
                program
                    .first()
                    .map(|statement| statement.location)
                    .unwrap_or_default(),
                "execute",
            ));
        }
        self.instantiate_objects(program)?;
        self.hoist(program, true)
    }
    /** New object cells preserve prior properties or builtins; redeclarations retain their existing value. */
    fn instantiate_objects(&mut self, program: &[Statement]) -> EngineResult<()> {
        for (name, location) in object_names(program) {
            if self.functions.contains_key(&name) {
                continue;
            }
            if self.functions.len() >= STATEMENTS {
                return Err(errors::located(
                    EngineError::limit("V9 global object declarations", STATEMENTS),
                    location,
                    "execute",
                ));
            }
            let value = self.get_property(&JsValue::Object(self.global_object), &name, location)?;
            if self.heap.has(self.global_object, &name) {
                self.heap.remove(self.global_object, &name)?;
            }
            self.functions.insert(
                name,
                Binding {
                    kind: BindingKind::Var,
                    value: Some(value),
                },
            );
        }
        self.environment()
    }
    /** Nested declarations are initialized after parameters and share their activation's cells. */
    pub(super) fn hoist(&mut self, program: &[Statement], root: bool) -> EngineResult<()> {
        for statement in program {
            if let StatementKind::Function(name, ..) = &statement.kind {
                let value = self.closure(statement.location.byte_offset, statement.location)?;
                if root {
                    if self.heap.has(self.global_object, name) {
                        self.heap.remove(self.global_object, name)?;
                    }
                    self.functions.insert(
                        name.clone(),
                        Binding {
                            kind: BindingKind::Var,
                            value: Some(value),
                        },
                    );
                } else {
                    let id = *self.locals.last().expect("function activation");
                    self.graph
                        .get_mut(id)?
                        .bindings
                        .get_mut(name)
                        .expect("source declaration")
                        .value = Some(value);
                }
            }
        }
        Ok(())
    }
}
