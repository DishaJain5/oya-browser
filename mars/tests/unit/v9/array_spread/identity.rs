/** Shallow expansion retains exact scalar bits and all owned graph identities across events. */
use super::*;

/** Object/function cycles and bound callbacks survive copying, serialization and finish-only collection. */
#[test]
fn copied_cycles_closures_and_bound_callbacks_keep_identity_across_events() {
    let source = "function make(){const box={n:7};const read=()=>box;box.read=read;return box;}const box=make();function plus(n){return this.n+n;}const bound=plus.bind(box,2);const a=[box,box.read,bound];const b=[...a];function run(){document.getElementById('out').textContent=(b!==a)+' '+(b[0]===box)+' '+(b[1]()===box)+' '+(b[0].read===b[1])+' '+b[2]();}";
    let state = round_trip(load(source, &[]).unwrap().state);
    assert_eq!(
        event(source, "run()", state, &[]).unwrap().changes[0].text,
        "true true true true 9"
    );
}

/** Numbers keep their wire bits and strings keep unpaired units without a UTF-8 conversion. */
#[test]
fn scalar_bits_and_lone_surrogate_strings_are_shallow_copied_exactly() {
    let source = "const a=[-0,NaN,'\\ud800'];const b=[...a];function run(){document.getElementById('out').textContent=(1/b[0])+' '+(b[1]!==b[1])+' '+b[2].length;}";
    let loaded = load(source, &[]).unwrap();
    let wire = serde_json::to_value(&loaded.state).unwrap();
    let array = wire["globals"]["b"]["value"]["id"]
        .as_u64()
        .unwrap()
        .to_string();
    let elements = &wire["heap"]["objects"][&array]["elements"];
    assert_eq!(
        elements[0],
        json!({"type":"number","bits":format!("{:016x}", (-0.0f64).to_bits())})
    );
    assert_eq!(elements[2], json!({"type":"string","units":[0xd800]}));
    assert_eq!(
        event(source, "run()", round_trip(loaded.state), &[])
            .unwrap()
            .changes[0]
            .text,
        "-Infinity true 1"
    );
}

/** Invalid source records cannot be rescued by a spread followed by collection. */
#[test]
fn forged_sparse_source_references_are_rejected_before_event_execution() {
    let source = "const a=[1];function run(){const b=[...a];}";
    let original = serde_json::to_value(load(source, &[]).unwrap().state).unwrap();
    let array = original["globals"]["a"]["value"]["id"]
        .as_u64()
        .unwrap()
        .to_string();
    let mut wire = original;
    wire["heap"]["objects"][&array]["elements"][0] = json!({"type":"object","id":99999});
    let error = event(source, "run()", serde_json::from_value(wire).unwrap(), &[]).unwrap_err();
    assert_eq!(error.code, "invalid_request");
}

/** Registry traversal retains closures defined inside a spread operand after its activation exits. */
#[test]
fn nested_spread_operand_closures_retain_their_source_owned_capture() {
    let source = "function make(){let n=4;return [...[()=>{n++;return n;}]];}const a=[...make()];function run(){document.getElementById('out').textContent=a[0]();}";
    let mut state = load(source, &[]).unwrap().state;
    for expected in ["5", "6"] {
        let executed = event(source, "run()", round_trip(state), &[]).unwrap();
        assert_eq!(executed.changes[0].text, expected);
        state = executed.state;
    }
}

/** Host values remain exact document identities as array elements, but hosts are not iterable. */
#[test]
fn copied_host_references_keep_identity_and_host_iteration_is_explicitly_unsupported() {
    use mars::{
        document::{parse, script_document},
        v9::{StorageState, click_document_scripts, load_document_scripts},
    };
    let document = script_document(
        &parse("<p id=out>initial</p><button id=button type=button>Click</button>").unwrap(),
        "spread-hosts",
    )
    .unwrap();
    let source = "const e=document.getElementById('out');const a=[document,e];const b=[...a];document.getElementById('button').addEventListener('click',()=>{e.textContent=(b[0]===document)+' '+(b[1]===e);});";
    let loaded =
        load_document_scripts(&[source.into()], document.clone(), StorageState::default()).unwrap();
    let executed = click_document_scripts(
        &[source.into()],
        document.get_by_id("button").unwrap(),
        round_trip(loaded.state.clone()),
        document.clone(),
        StorageState::default(),
    )
    .unwrap();
    assert_eq!(executed.changes[0].text, "true true");
    let invalid = "const a=[...document];";
    let error = load_document_scripts(&[invalid.into()], document.clone(), StorageState::default())
        .unwrap_err();
    assert_eq!(error.code, "unsupported_feature");
    assert_eq!(error.phase.as_deref(), Some("execute"));
    assert_eq!(
        error.location.unwrap().byte_offset,
        invalid.find("...").unwrap()
    );
    let mut wire = serde_json::to_value(loaded.state).unwrap();
    let array = wire["globals"]["b"]["value"]["id"]
        .as_u64()
        .unwrap()
        .to_string();
    wire["heap"]["objects"][&array]["elements"][1]["reference"]["document"] = json!("forged-page");
    let forged: VmState = serde_json::from_value(wire).unwrap();
    assert_eq!(
        forged.validate_document(&document).unwrap_err().code,
        "invalid_request"
    );
}
