mod apply;
mod arithmetic;
mod assignments;
mod callable;
/** Candidate execution owns lexical graphs and effects until validated request-boundary publication. */
mod calls;
mod declaration_schema;
mod declarations;
mod environment;
mod evaluator;
mod execution;
mod for_loop;
mod properties;
mod receivers;
mod references;
mod unary;
use super::{
    Binding, BindingKind, Execution, Globals, InputValue, JsValue, StorageState, VmState, errors,
    graph::{Callable, Graph, Id, Intrinsic},
    parser::{Expression, ExpressionKind, Program, Statement, StatementKind},
    registry::{Registry, ScopeId, ScopeKind},
    value::{Heap, HeapObject},
};
use crate::{
    constants::{JS_ENVIRONMENT_UNITS, NODES, OPERATIONS, STATEMENTS, TEXT_BYTES, VM_PROPERTIES},
    document::TextChange,
    protocol::{EngineError, EngineResult, SourceLocation},
};
use std::collections::BTreeMap;
/** Recursion has an independent bound even when functions retain their defining contexts. */
const CALL_DEPTH: usize = 16;
/** Loop jumps and function returns propagate through lexical cleanup. */
enum Completion {
    Normal,
    Break,
    Continue,
    Return(JsValue),
}
/** The source-only facade shares exactly the ordered-script implementation. */
pub(super) fn load(program: Program, inputs: &[InputValue]) -> EngineResult<Execution> {
    load_scripts(vec![program], inputs, StorageState::default())
}
/** Scripts instantiate and execute in source order while sharing one bounded candidate. */
pub(super) fn load_scripts(
    programs: Vec<Program>,
    inputs: &[InputValue],
    storage: StorageState,
) -> EngineResult<Execution> {
    let registry = Registry::build(&programs)?;
    let mut vm = Interpreter::new(registry, VmState::default(), inputs, storage)?;
    for program in programs {
        vm.strict = program.strict;
        vm.instantiate_script(&program)?;
        vm.statements(program.statements)?;
    }
    vm.finish()
}
/** Event state is structurally validated against the same source without replaying initializers. */
pub(super) fn event(
    program: Program,
    calls: Vec<Statement>,
    state: VmState,
    inputs: &[InputValue],
) -> EngineResult<Execution> {
    event_scripts(vec![program], calls, state, inputs, StorageState::default())
}
/** Only author-defined named handlers reach ordinary closure-aware calls. */
pub(super) fn event_scripts(
    programs: Vec<Program>,
    calls: Vec<Statement>,
    state: VmState,
    inputs: &[InputValue],
    storage: StorageState,
) -> EngineResult<Execution> {
    let registry = Registry::build(&programs)?;
    declarations::validate_state(&programs, &state, &registry)?;
    let mut vm = Interpreter::new(registry, state, inputs, storage)?;
    for call in calls {
        if let StatementKind::Call(name, _) = &call.kind {
            vm.variable(name, call.location).map_err(|mut error| {
                error.input_kind = Some("handler".into());
                error
            })?;
        }
        vm.statement(call)?;
    }
    vm.finish()
}
/** Active IDs are stack-owned; arena records outlive their activation only through traced roots. */
struct Interpreter {
    globals: Globals,
    functions: Globals,
    heap: Heap,
    global_object: super::value::ObjectId,
    graph: Graph,
    registry: Registry,
    locals: Vec<Id>,
    function_depth: usize,
    /** Language mode belongs to the executing code rather than its dynamic caller. */
    strict: bool,
    inputs: BTreeMap<String, JsValue>,
    changes: Vec<TextChange>,
    remaining: usize,
    storage: StorageState,
}
impl Interpreter {
    /** Initialization has no ambient authority, host callbacks or mutable shared VM state. */
    fn new(
        registry: Registry,
        mut state: VmState,
        inputs: &[InputValue],
        storage: StorageState,
    ) -> EngineResult<Self> {
        state.validate()?;
        let global_object = match state.global_object {
            Some(id) => id,
            None => match state.heap.allocate(HeapObject::Object {
                properties: Default::default(),
            })? {
                JsValue::Object(id) => id,
                _ => unreachable!("heap allocation returns an object"),
            },
        };
        storage.validate()?;
        if inputs.len() > NODES {
            return Err(EngineError::limit("V9 inputs", NODES));
        }
        let vm = Self {
            globals: state.globals,
            functions: state.functions,
            heap: state.heap,
            global_object,
            graph: state.graph,
            registry,
            locals: Vec::new(),
            function_depth: 0,
            strict: false,
            inputs: Self::input_values(inputs)?,
            changes: Vec::new(),
            remaining: OPERATIONS,
            storage,
        };
        vm.environment()?;
        Ok(vm)
    }
    /** Reclamation runs only after evaluator temporaries and active frames have disappeared. */
    fn finish(mut self) -> EngineResult<Execution> {
        let mut roots = self
            .globals
            .values()
            .chain(self.functions.values())
            .filter_map(|cell| cell.value.clone())
            .collect::<Vec<_>>();
        roots.push(JsValue::Object(self.global_object));
        self.graph.collect(&mut self.heap, roots.into_iter())?;
        self.environment()?;
        self.storage.validate()?;
        let state = VmState {
            globals: self.globals,
            functions: self.functions,
            heap: self.heap,
            graph: self.graph,
            global_object: Some(self.global_object),
        };
        state.validate()?;
        state.graph.validate(
            &self.registry,
            &state.heap,
            state
                .globals
                .values()
                .chain(state.functions.values())
                .filter_map(|cell| cell.value.clone()),
        )?;
        Ok(Execution {
            changes: self.changes,
            state,
            storage: self.storage,
        })
    }
    /** Protected controls are omitted by callers; supplied fields remain literal bounded strings. */
    fn input_values(inputs: &[InputValue]) -> EngineResult<BTreeMap<String, JsValue>> {
        let mut values = BTreeMap::new();
        for input in inputs {
            if input.value.len() > TEXT_BYTES {
                return Err(EngineError::limit("V9 DOM input UTF-8 bytes", TEXT_BYTES));
            }
            values
                .entry(input.html_id.clone())
                .or_insert(JsValue::string(&input.value)?);
        }
        Ok(values)
    }
    /** Peak candidate allocations remain bounded even before finish-only garbage collection. */
    fn environment(&self) -> EngineResult<()> {
        self.graph.bounds()?;
        if self.heap.property_count() + self.graph.property_count() > VM_PROPERTIES {
            return Err(EngineError::limit("V9 page properties", VM_PROPERTIES));
        }
        let units = self
            .globals
            .values()
            .chain(self.functions.values())
            .filter_map(|cell| cell.value.as_ref())
            .chain(self.inputs.values())
            .map(JsValue::string_units)
            .sum::<usize>()
            + self.heap.string_units()
            + self.graph.string_units();
        if units > JS_ENVIRONMENT_UNITS {
            return Err(EngineError::limit(
                "V9 environment UTF-16 units",
                JS_ENVIRONMENT_UNITS,
            ));
        }
        Ok(())
    }
    /** Every iteration, statement and expression shares one decreasing request work allowance. */
    fn step(&mut self, location: SourceLocation) -> EngineResult<()> {
        self.remaining = self.remaining.checked_sub(1).ok_or_else(|| {
            errors::located(
                EngineError::new(
                    "limit_exceeded",
                    "Script execution stopped: operation limit exceeded.",
                    "Reduce the script work; no script changes were applied.",
                ),
                location,
                "execute",
            )
        })?;
        Ok(())
    }
}
