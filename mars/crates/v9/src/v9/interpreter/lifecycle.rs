/** Candidate initialization and finish-only collection keep projected state isolated from publication. */
use super::*;
impl Interpreter {
    /** Initialization has no ambient authority, host callbacks or mutable shared VM state. */
    pub(super) fn new(
        registry: Registry,
        state: VmState,
        inputs: &[InputValue],
        storage: StorageState,
    ) -> EngineResult<Self> {
        state.validate_legacy()?;
        Self::new_document(registry, state, inputs, storage, None)
    }
    /** Projected requests retain only a candidate projection during synchronous execution. */
    pub(super) fn new_document(
        registry: Registry,
        mut state: VmState,
        inputs: &[InputValue],
        storage: StorageState,
        document: Option<crate::document::ScriptDocument>,
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
            inputs: if let Some(document) = &document {
                Self::input_values(
                    &document
                        .nodes
                        .iter()
                        .filter_map(|node| {
                            node.value.as_ref().map(|value| InputValue {
                                html_id: node.html_id.clone(),
                                value: value.clone(),
                            })
                        })
                        .collect::<Vec<_>>(),
                )?
            } else {
                Self::input_values(inputs)?
            },
            changes: Vec::new(),
            remaining: OPERATIONS,
            storage,
            dom: state.dom,
            document,
            active_event: None,
        };
        vm.environment()?;
        Ok(vm)
    }
    /** Reclamation runs only after evaluator temporaries and active frames have disappeared. */
    pub(super) fn finish(mut self) -> EngineResult<Execution> {
        let mut roots = self
            .globals
            .values()
            .chain(self.functions.values())
            .filter_map(|cell| cell.value.clone())
            .collect::<Vec<_>>();
        roots.push(JsValue::Object(self.global_object));
        if let Some(dom) = &self.dom {
            roots.extend(
                dom.listeners
                    .iter()
                    .map(|listener| JsValue::Function(listener.callback)),
            );
        }
        self.graph.collect(&mut self.heap, roots.into_iter())?;
        self.environment()?;
        self.storage.validate()?;
        let mut state = VmState {
            globals: self.globals,
            functions: self.functions,
            heap: self.heap,
            graph: self.graph,
            global_object: Some(self.global_object),
            dom: self.dom,
        };
        if state.dom.is_some() {
            let event_ids = state
                .values()
                .into_iter()
                .filter_map(|value| match value {
                    JsValue::Host(crate::v9::HostRef {
                        kind: crate::v9::HostKind::Event { id },
                        ..
                    }) => Some(id),
                    _ => None,
                })
                .collect::<std::collections::BTreeSet<_>>();
            if let Some(dom) = state.dom.as_mut() {
                dom.events.retain(|id, _| event_ids.contains(id));
            }
            state.validate()?;
            state.validate_document(self.document.as_ref().expect("projected document")).map_err(|error| {
                if error.code == "invalid_request" { EngineError::unsupported("Retaining references or listeners to removed elements is not supported.") } else {error}
            })?;
        }
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
}
