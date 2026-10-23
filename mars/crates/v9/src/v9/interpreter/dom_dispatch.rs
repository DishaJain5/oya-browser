/** Projected load and click execution share one VM candidate, budget and finish-only collector. */
use super::*;
use crate::{
    constants::JS_EVENT_RECORDS,
    document::ScriptDocument,
    v9::{
        dom::{DomState, EventRecord},
        value::HostKind,
    },
};
/** Empty-script pages still receive their own validated host realm. */
pub(in crate::v9) fn load_document(
    programs: Vec<Program>,
    document: ScriptDocument,
    storage: StorageState,
) -> EngineResult<Execution> {
    document.validate()?;
    let registry = Registry::build(&programs)?;
    let state = VmState {
        dom: Some(DomState::new(document.identity.clone())),
        ..VmState::default()
    };
    let mut vm = Interpreter::new_document(registry, state, &[], storage, Some(document))?;
    for program in programs {
        vm.strict = program.strict;
        vm.instantiate_script(&program)?;
        vm.statements(program.statements)?;
    }
    vm.finish()
}
/** Rebuild source metadata and validate every incoming record before any inline or listener work. */
pub(in crate::v9) fn click_document(
    programs: Vec<Program>,
    target: usize,
    state: VmState,
    document: ScriptDocument,
    storage: StorageState,
) -> EngineResult<Execution> {
    state.validate_document(&document)?;
    let registry = Registry::build(&programs)?;
    declarations::validate_state(&programs, &state, &registry)?;
    let node = document
        .nodes
        .iter()
        .find(|node| node.local_id == target)
        .ok_or_else(|| EngineError::invalid("Invalid projected click target."))?;
    let handler = node.inline_handler.clone();
    if !node.click_listener && handler.is_none() {
        return Err(EngineError::unsupported(
            "This projected click target is unsupported.",
        ));
    }
    let mut vm = Interpreter::new_document(registry, state, &[], storage, Some(document))?;
    vm.dispatch_click(target, handler.as_deref())?;
    vm.finish()
}
impl Interpreter {
    /** Snapshot IDs precede inline work; removed records are skipped and additions wait for another click. */
    fn dispatch_click(&mut self, target: usize, handler: Option<&str>) -> EngineResult<()> {
        let snapshot = self
            .dom
            .as_ref()
            .expect("DOM")
            .listeners
            .iter()
            .filter(|listener| listener.target == target)
            .map(|listener| listener.id)
            .collect::<Vec<_>>();
        if let Some(handler) = handler {
            self.inline_click(handler)?;
        }
        if snapshot.is_empty() {
            return Ok(());
        }
        let event = self.allocate_event(target)?;
        for registration in snapshot {
            self.step(SourceLocation::default())?;
            let listener = self
                .dom
                .as_ref()
                .expect("DOM")
                .listeners
                .iter()
                .find(|listener| listener.id == registration)
                .cloned();
            if let Some(listener) = listener {
                self.active_event = Some(event);
                let identity = self.dom.as_ref().expect("DOM").identity.clone();
                let receiver = self.host_value(HostKind::Element { local_id: target }, &identity);
                let argument = self.host_value(HostKind::Event { id: event }, &identity);
                let result = self.call_value(
                    JsValue::Function(listener.callback),
                    receiver,
                    vec![argument],
                    SourceLocation::default(),
                );
                self.active_event = None;
                result?;
            }
        }
        Ok(())
    }
    /** Inline calls retain their existing named-call receiver and handler diagnostics. */
    fn inline_click(&mut self, handler: &str) -> EngineResult<()> {
        let result = (|| {
            let calls = super::super::lexer::tokenize_handler(handler)
                .and_then(super::super::parser::parse_handler)?;
            if calls.len() != 1 || !matches!(&calls[0].kind, StatementKind::Call(..)) {
                return Err(errors::unsupported(
                    calls.first().map(|call| call.location).unwrap_or_default(),
                    "button handlers other than a named call",
                ));
            }
            for call in calls {
                self.statement(call)?;
            }
            Ok(())
        })();
        result.map_err(|mut error| {
            error.input_kind = Some("handler".into());
            error
        })
    }
    /** Event allocation is capped before mutation and uses monotonic, never-recycled identity. */
    fn allocate_event(&mut self, target: usize) -> EngineResult<u32> {
        self.step(SourceLocation::default())?;
        let dom = self.dom.as_mut().expect("DOM");
        if dom.events.len() >= JS_EVENT_RECORDS {
            return Err(EngineError::limit("V9 event records", JS_EVENT_RECORDS));
        }
        let id = dom.next_event;
        dom.next_event = id
            .checked_add(1)
            .ok_or_else(|| EngineError::limit("V9 event identity", JS_EVENT_RECORDS))?;
        dom.events.insert(
            id,
            EventRecord {
                target,
                default_prevented: false,
            },
        );
        Ok(id)
    }
}
