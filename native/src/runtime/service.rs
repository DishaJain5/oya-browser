/** Typed private operations keep engine data separate from native agent authority. */
use crate::document::{self, Document, TextChange};
use crate::protocol::{
    EngineError, EngineResult, Request, Response, decode, encode, parameters, read_frame,
    write_message,
};
use crate::{network, v9};
use serde::Deserialize;
use serde_json::{Value, json};
use std::io::{self, BufReader};

/** Private dispatch never shares the public agent operation table. */
type Handler = fn(Value) -> EngineResult<Value>;

/// Ownership-pipe EOF and failed writes terminate services without background descendants.
pub fn run_service(name: &str) -> EngineResult<()> {
    let _owner = super::lifetime::OwnerWatch::start()?;
    if !["network", "document", "v9"].contains(&name) {
        return Err(EngineError::invalid("Unknown engine service."));
    }
    let mut input = BufReader::new(io::stdin().lock());
    let mut output = io::stdout().lock();
    let mut prepared: Option<(u64, network::Prepared)> = None;
    let mut sequence = 0_u64;
    while let Some(frame) = read_frame(&mut input)? {
        let request: Request = decode(&frame)?;
        request.validate()?;
        let response = Response::new(
            Some(request.id),
            if name == "network" && request.op == "network.prepare" {
                prepared.take();
                sequence = sequence.saturating_add(1);
                prepare(request.params).map(|connection| {
                    prepared = Some((sequence, connection));
                    json!({"token":sequence})
                })
            } else if name == "network" && request.op == "network.transmit" {
                let input: EngineResult<Transmit> = parameters(request.params);
                input
                    .and_then(|input| {
                        prepared
                            .take()
                            .filter(|(token, _)| *token == input.token)
                            .ok_or_else(|| {
                                EngineError::invalid(
                                    "Prepared request token expired or was consumed.",
                                )
                            })
                    })
                    .and_then(|(_, connection)| connection.transmit())
                    .and_then(serialize)
            } else {
                dispatch(name, &request.op, request.params)
            },
        );
        let response = match encode(&response) {
            Ok(_) => response,
            Err(error) => Response::new(response.id, Err(error)),
        };
        write_message(&mut output, &response)?;
    }
    Ok(())
}

/** A correlated readiness response proves the child speaks this exact protocol. */
fn dispatch(name: &str, op: &str, params: Value) -> EngineResult<Value> {
    if op == "service.ready" {
        let _: Empty = parameters(params)?;
        return Ok(json!({"service":name,"ready":true}));
    }
    let handlers: &[(&str, &str, Handler)] = &[
        ("network", "network.fetch", fetch),
        ("network", "network.web_fetch", web_fetch),
        ("document", "document.parse", parse),
        ("document", "document.apply", apply),
        ("document", "document.layout", layout),
        ("document", "document.render", render),
        ("document", "document.value", value),
        ("document", "document.styles", styles),
        ("v9", "v9.execute", execute),
        ("v9", "v9.load", load),
        ("v9", "v9.event", event),
    ];
    let handler = handlers
        .iter()
        .find(|(service, operation, _)| *service == name && *operation == op)
        .map(|(_, _, handler)| handler)
        .ok_or_else(|| EngineError::invalid("Unknown private service operation."))?;
    handler(params)
}

/** Empty schemas reject unrecognized fields. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Empty {}
/** Only the network owner receives an outbound URL. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fetch {
    /** Literal loopback URL. */
    url: String,
}
/** Fixture grants originate at startup, never at a public page operation. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WebFetch {
    /** Normalized outbound URL. */
    url: String,
    /** Exact owned origins. */
    fixture_origins: Vec<String>,
}
/** Preparation has no authority to transmit HTTP application bytes. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Prepare {
    request: network::WebRequest,
    fixture_origins: Vec<String>,
    remaining_ms: u64,
}
/** A one-use private token is not reachable through the agent front door. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Transmit {
    token: u64,
}
/** Stateful preparation remains isolated to the one owned network process. */
fn prepare(params: Value) -> EngineResult<network::Prepared> {
    let input: Prepare = parameters(params)?;
    network::Prepared::connect(input.request, &input.fixture_origins, input.remaining_ms)
}
/** Source carries no coordinator capabilities. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Parse {
    /** Bounded HTML. */
    source: String,
}
/** Candidate mutation is atomic over explicit input data. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Apply {
    /** Candidate snapshot. */
    document: Document,
    /** Inert proposals. */
    changes: Vec<TextChange>,
}
/** Layout never resolves a hidden page. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    /** Explicit snapshot. */
    document: Document,
}
/** One point viewport feeds both painting and inspection. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Render {
    /** Exact snapshot. */
    document: Document,
    /** Logical-point width. */
    width: f64,
}
/** CSS sources have already passed resource acquisition and byte limits. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Styles {
    document: Document,
    sheets: Vec<String>,
}
/** Style visibility and paint metadata come from the same owned cascade. */
fn styles(params: Value) -> EngineResult<Value> {
    let input: Styles = parameters(params)?;
    serialize(document::compute_styles(&input.document, &input.sheets)?)
}
/** Input editing does not reinterpret its value as source. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SetValue {
    /** Exact snapshot. */
    document: Document,
    /** Native local identity. */
    local_id: usize,
    /** Literal value. */
    value: String,
}
/** Legacy V9 source-only execution. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Execute {
    /** Bounded source. */
    source: String,
}
/** Page loading receives only bounded input values. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Load {
    /** Ordered page programs share one VM candidate and execution budget. */
    sources: Vec<String>,
    /** Explicit DOM reads. */
    inputs: Vec<v9::InputValue>,
    /** Candidate Storage maps supplied by the coordinator for this origin and tab. */
    #[serde(default)]
    storage: v9::StorageState,
}
/** Function execution receives committed string globals, not host objects. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Event {
    /** Original ordered page programs. */
    sources: Vec<String>,
    /** One supported named call. */
    handler: String,
    /** Committed globals. */
    state: v9::VmState,
    /** Current DOM input values. */
    inputs: Vec<v9::InputValue>,
    /** Candidate Storage maps supplied by the coordinator for this origin and tab. */
    #[serde(default)]
    storage: v9::StorageState,
}

/** Headless policy stays literal-loopback only. */
fn fetch(params: Value) -> EngineResult<Value> {
    Ok(json!({"source":network::fetch(&parameters::<Fetch>(params)?.url)?}))
}
/** Desktop policy validates every exact destination inside this process. */
fn web_fetch(params: Value) -> EngineResult<Value> {
    let input: WebFetch = parameters(params)?;
    serialize(network::fetch_web(&input.url, &input.fixture_origins)?)
}
/** Markup semantics are owned by the document service. */
fn parse(params: Value) -> EngineResult<Value> {
    serialize(document::parse(&parameters::<Parse>(params)?.source)?)
}
/** All proposed targets are checked before any mutation escapes. */
fn apply(params: Value) -> EngineResult<Value> {
    let input: Apply = parameters(params)?;
    serialize(document::apply(&input.document, &input.changes)?)
}
/** Legacy abstract geometry remains available to headless clients. */
fn layout(params: Value) -> EngineResult<Value> {
    serialize(document::layout(&parameters::<Snapshot>(params)?.document))
}
/** Native point layout is computed once, independently of the window. */
fn render(params: Value) -> EngineResult<Value> {
    let input: Render = parameters(params)?;
    serialize(document::render(&input.document, input.width)?)
}
/** Type actions and human input use the same literal-value validator. */
fn value(params: Value) -> EngineResult<Value> {
    let input: SetValue = parameters(params)?;
    serialize(document::set_value(
        &input.document,
        input.local_id,
        &input.value,
    )?)
}
/** Legacy execution delegates to the same owned interpreter. */
fn execute(params: Value) -> EngineResult<Value> {
    serialize(v9::execute(&parameters::<Execute>(params)?.source)?)
}
/** Load-time globals become explicit serializable VM state. */
fn load(params: Value) -> EngineResult<Value> {
    let input: Load = parameters(params)?;
    serialize(v9::load_scripts_with_storage(
        &input.sources,
        &input.inputs,
        input.storage,
    )?)
}
/** No event can obtain an agent transport or native OS callback. */
fn event(params: Value) -> EngineResult<Value> {
    let input: Event = parameters(params)?;
    serialize(v9::event_scripts_with_storage(
        &input.sources,
        &input.handler,
        input.state,
        &input.inputs,
        input.storage,
    )?)
}
/** Serialization failures are explicit rather than successful null values. */
fn serialize(value: impl serde::Serialize) -> EngineResult<Value> {
    serde_json::to_value(value).map_err(|_| EngineError::invalid("Cannot serialize engine result."))
}
