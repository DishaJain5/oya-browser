/** Oya's bounded V9 interpreter owns execution and receives only explicit input data. */
mod binding;
mod errors;
mod graph;
mod interpreter;
mod lexer;
mod literals;
mod operator;
mod parser;
mod registry;
mod source;
mod value;
use crate::document::TextChange;
use crate::{
    constants::{VM_PROPERTIES, VM_STATE_BYTES},
    protocol::{EngineError, EngineResult},
};
pub use binding::{Binding, BindingKind, Globals};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use value::{Heap, ObjectId};
pub use value::{JsString, JsValue};

/// The committed page owns lexical cells and their shared object arena together.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VmState {
    /// Script-global let/const cells remain distinct from page-global object properties.
    globals: Globals,
    /// Arena identity preserves aliases without recursive wire values.
    heap: Heap,
    /// Script-global var/function cells share the global object's lookup and survive native events.
    functions: Globals,
    /// Captured cells and functions use flat identities rather than recursive serialized values.
    graph: graph::Graph,
    /// The page-global receiver is data in this heap, never a native Window or agent handle.
    global_object: Option<ObjectId>,
}
impl VmState {
    /// Expose committed lexical values for inspection without exposing heap internals.
    pub fn globals(&self) -> &Globals {
        &self.globals
    }
    /// Build a candidate scalar state; event execution validates it before use.
    pub fn from_globals(globals: Globals) -> Self {
        Self {
            globals,
            heap: Heap::default(),
            functions: Globals::new(),
            graph: graph::Graph::default(),
            global_object: None,
        }
    }
    /// Validate object references and keep the private service snapshot below half a frame.
    pub(super) fn validate(&self) -> EngineResult<()> {
        use crate::constants::{JS_ENVIRONMENT_UNITS, STATEMENTS};
        if self.globals.len() > STATEMENTS {
            return Err(EngineError::limit("V9 variables", STATEMENTS));
        }
        if self.functions.len() > STATEMENTS {
            return Err(EngineError::limit(
                "V9 global object declarations",
                STATEMENTS,
            ));
        }
        if self
            .globals
            .values()
            .any(|cell| cell.kind == BindingKind::Var)
            || self
                .functions
                .values()
                .any(|cell| cell.kind != BindingKind::Var || cell.value.is_none())
        {
            return Err(EngineError::invalid("Invalid V9 global declaration cells."));
        }
        let units = self
            .globals
            .values()
            .chain(self.functions.values())
            .filter_map(|cell| cell.value.as_ref())
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
        self.heap.validate(
            self.globals
                .values()
                .chain(self.functions.values())
                .filter_map(|binding| binding.value.clone()),
        )?;
        if let Some(id) = self.global_object {
            if id != crate::constants::VM_GLOBAL_OBJECT_ID {
                return Err(EngineError::invalid("Invalid V9 global receiver identity."));
            }
            self.heap.validate_global(id)?;
        }
        self.graph.bounds()?;
        if self.heap.property_count() + self.graph.property_count() > VM_PROPERTIES {
            return Err(EngineError::limit(
                "V9 aggregate own properties",
                VM_PROPERTIES,
            ));
        }
        let bytes = serde_json::to_vec(self)
            .map_err(|_| EngineError::invalid("Cannot encode V9 state."))?
            .len();
        if bytes > VM_STATE_BYTES {
            return Err(EngineError::limit("V9 state bytes", VM_STATE_BYTES));
        }
        Ok(())
    }
}

/// Only input IDs and literal values cross into V9, never native agent handles.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InputValue {
    /// Source lookup key within the exact candidate document.
    pub html_id: String,
    /// Bounded current field text.
    pub value: String,
}

/// Explicit VM state lets stateless service requests preserve load-time globals.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Execution {
    /// Document engine validates all proposals before publication.
    pub changes: Vec<TextChange>,
    /// Globals and heap commit atomically with document proposals.
    pub state: VmState,
    /// Storage proposals remain candidates until the coordinator commits the page.
    pub storage: StorageState,
}

/** Only string keys and values cross the V9 storage boundary. */
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StorageState {
    /// Origin-scoped localStorage candidate.
    pub local: BTreeMap<String, String>,
    /// Tab and origin-scoped sessionStorage candidate.
    pub session: BTreeMap<String, String>,
}

impl StorageState {
    /// Enforce independent entry and byte budgets before a candidate enters or leaves V9.
    pub fn validate(&self) -> EngineResult<()> {
        validate_storage_area(&self.local)?;
        validate_storage_area(&self.session)
    }
}

/** Storage limits apply to UTF-8 key/value bytes and never count profile encoding overhead. */
fn validate_storage_area(area: &BTreeMap<String, String>) -> EngineResult<()> {
    use crate::constants::{
        JS_STRING_UNITS, STORAGE_AREA_BYTES, STORAGE_ENTRIES, STORAGE_VALUE_BYTES,
    };
    let bytes = area
        .iter()
        .map(|(key, value)| key.len().saturating_add(value.len()))
        .sum::<usize>();
    if area.len() > STORAGE_ENTRIES
        || area.iter().any(|(key, value)| {
            key.len() > STORAGE_VALUE_BYTES
                || value.len() > STORAGE_VALUE_BYTES
                || key.encode_utf16().count() > JS_STRING_UNITS
                || value.encode_utf16().count() > JS_STRING_UNITS
        })
        || bytes > STORAGE_AREA_BYTES
    {
        return Err(EngineError::limit(
            "Web Storage area bytes",
            STORAGE_AREA_BYTES,
        ));
    }
    Ok(())
}

/// Compatibility entry point for existing source-only headless callers.
pub fn execute(source: &str) -> EngineResult<Vec<TextChange>> {
    Ok(load(source, &[])?.changes)
}

/// Load source exactly once against candidate input values and return its globals.
pub fn load(source: &str, inputs: &[InputValue]) -> EngineResult<Execution> {
    let program = parser::parse(lexer::tokenize(source)?)?;
    interpreter::load(program, inputs)
}

/// Load ordered classic scripts with shared state and source-order declaration visibility.
pub fn load_scripts(sources: &[String], inputs: &[InputValue]) -> EngineResult<Execution> {
    load_scripts_with_storage(sources, inputs, StorageState::default())
}

/** Load scripts against isolated coordinator-owned storage candidates. */
pub fn load_scripts_with_storage(
    sources: &[String],
    inputs: &[InputValue],
    storage: StorageState,
) -> EngineResult<Execution> {
    let spans = script_spans(sources)?;
    let result = parse_scripts(sources)
        .and_then(|programs| interpreter::load_scripts(programs, inputs, storage));
    result.map_err(|error| script_error(error, &spans))
}

/// A button's inline handler can only call a named author function.
pub fn event(
    source: &str,
    handler: &str,
    state: VmState,
    inputs: &[InputValue],
) -> EngineResult<Execution> {
    let program = parser::parse(lexer::tokenize(source)?)?;
    let calls = lexer::tokenize_handler(handler)
        .and_then(parser::parse_handler)
        .map_err(|mut error| {
            error.input_kind = Some("handler".into());
            error
        })?;
    if calls.len() != 1 || !matches!(&calls[0].kind, parser::StatementKind::Call(..)) {
        let mut error = errors::unsupported(
            calls
                .first()
                .map(|call| call.location)
                .unwrap_or(crate::protocol::SourceLocation {
                    byte_offset: 0,
                    line: 1,
                    column: 1,
                    script_index: None,
                }),
            "button handlers other than a named call",
        );
        error.input_kind = Some("handler".into());
        return Err(error);
    }
    interpreter::event(program, calls, state, inputs)
}

/// Run a named event against globals produced by the same ordered page scripts.
pub fn event_scripts(
    sources: &[String],
    handler: &str,
    state: VmState,
    inputs: &[InputValue],
) -> EngineResult<Execution> {
    event_scripts_with_storage(sources, handler, state, inputs, StorageState::default())
}

/** Run one event against isolated storage copies supplied by the coordinator. */
pub fn event_scripts_with_storage(
    sources: &[String],
    handler: &str,
    state: VmState,
    inputs: &[InputValue],
    storage: StorageState,
) -> EngineResult<Execution> {
    let spans = script_spans(sources)?;
    let result = parse_scripts(sources)
        .and_then(|programs| run_script_event(programs, handler, state, inputs, storage));
    result.map_err(|error| script_error(error, &spans))
}

/** Join scripts with recorded byte ranges so diagnostics can identify their source entry. */
fn script_spans(sources: &[String]) -> EngineResult<Vec<(usize, usize)>> {
    use crate::constants::{SCRIPT_BYTES, SCRIPT_COUNT, SCRIPT_TOTAL_BYTES};
    if sources.len() > SCRIPT_COUNT {
        return Err(EngineError::limit("classic scripts", SCRIPT_COUNT));
    }
    let mut total = 0usize;
    let mut source_total = 0usize;
    let mut spans = Vec::with_capacity(sources.len());
    for source in sources {
        if source.len() > SCRIPT_BYTES {
            return Err(EngineError::limit("script bytes", SCRIPT_BYTES));
        }
        let start = total;
        total = total.saturating_add(source.len());
        source_total = source_total.saturating_add(source.len());
        spans.push((start, total));
        total = total.saturating_add(1);
    }
    if source_total > SCRIPT_TOTAL_BYTES {
        return Err(EngineError::limit("page script bytes", SCRIPT_TOTAL_BYTES));
    }
    Ok(spans)
}

/** Each source gets its own parser program while diagnostics keep page-relative offsets. */
fn parse_scripts(sources: &[String]) -> EngineResult<Vec<parser::Program>> {
    let spans = script_spans(sources)?;
    let mut programs = Vec::with_capacity(sources.len());
    let mut token_count = 0usize;
    for (index, (source, (start, _))) in sources.iter().zip(&spans).enumerate() {
        let mut tokens = lexer::tokenize(source).map_err(|mut error| {
            if let Some(location) = error.location.as_deref_mut() {
                location.byte_offset += start;
                location.script_index = Some(index + 1);
            }
            if error.location.is_none() {
                error.location = Some(Box::new(crate::protocol::SourceLocation {
                    script_index: Some(index + 1),
                    ..Default::default()
                }));
            }
            error
        })?;
        token_count = token_count.saturating_add(tokens.len());
        if token_count > crate::constants::TOKENS {
            return Err(EngineError::limit("V9 tokens", crate::constants::TOKENS));
        }
        for token in &mut tokens {
            token.location.byte_offset += start;
        }
        programs.push(parser::parse(tokens)?);
    }
    Ok(programs)
}

/** The scanner byte offset maps failures back to a one-based document script index. */
fn script_error(mut error: EngineError, spans: &[(usize, usize)]) -> EngineError {
    if error.input_kind.as_deref() != Some("handler")
        && let Some(location) = error.location.as_deref()
    {
        let index = spans
            .iter()
            .position(|(start, end)| location.byte_offset >= *start && location.byte_offset <= *end)
            .map(|index| index + 1);
        if let Some(location) = error.location.as_deref_mut() {
            location.script_index = index;
        }
    }
    error
}

/** Events share the exact named-call validation used by the source-only API. */
fn run_script_event(
    programs: Vec<parser::Program>,
    handler: &str,
    state: VmState,
    inputs: &[InputValue],
    storage: StorageState,
) -> EngineResult<Execution> {
    let calls = lexer::tokenize_handler(handler)
        .and_then(parser::parse_handler)
        .map_err(|mut error| {
            error.input_kind = Some("handler".into());
            error
        })?;
    if calls.len() != 1 || !matches!(&calls[0].kind, parser::StatementKind::Call(..)) {
        let mut error = errors::unsupported(
            calls.first().map(|call| call.location).unwrap_or_default(),
            "button handlers other than a named call",
        );
        error.input_kind = Some("handler".into());
        return Err(error);
    }
    interpreter::event_scripts(programs, calls, state, inputs, storage)
}
