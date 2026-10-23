/** Saved closure graphs preserve cycles and reject hostile identities before publication. */
use super::closures::next;
use oya_native::v9::{event, load};
use serde_json::json;

/** Boundary tracing reclaims unreachable value cycles without limiting cumulative events to arena caps. */
#[test]
fn unreachable_cycles_and_temporary_callbacks_are_reclaimed_between_events() {
    let source = "function discard(){const box={};const f=function(){return box;};box.f=f;}function run(){discard();let f=function(){return 1;};f();}";
    let mut state = load(source, &[]).unwrap().state;
    for _ in 0..600 {
        state = next(source, state).state;
    }
    let wire = serde_json::to_value(state).unwrap();
    assert_eq!(wire["heap"]["objects"].as_object().unwrap().len(), 1);
    assert_eq!(wire["graph"]["environments"].as_object().unwrap().len(), 0);
    assert_eq!(wire["graph"]["functions"].as_object().unwrap().len(), 7);
    assert!(wire["heap"]["next_id"].as_u64().unwrap() > 256);
    assert!(wire["graph"]["next_function"].as_u64().unwrap() > 512);
}
/** Genuine object/function/environment cycles remain reachable and finite on the private wire. */
#[test]
fn reachable_mixed_cycles_round_trip_and_keep_identity() {
    let source = "function make(){const box={};const f=function(){return box;};box.f=f;return box;}const box=make();function run(){document.getElementById('out').textContent=box.f()===box;}";
    let state = load(source, &[]).unwrap().state;
    assert_eq!(next(source, state).changes[0].text, "true");
}
/** A source function's retained local schema is used to reject forged references before execution. */
#[test]
fn forged_graph_edges_and_binding_schemas_fail_closed() {
    let source = "function make(){let n=0;return function(){n=n+1;return n;};}const f=make();function run(){f();}";
    let original = serde_json::to_value(load(source, &[]).unwrap().state).unwrap();
    let env = original["graph"]["environments"]
        .as_object()
        .unwrap()
        .keys()
        .next()
        .unwrap()
        .clone();
    let function = original["graph"]["functions"]
        .as_object()
        .unwrap()
        .iter()
        .find(|(_, record)| record["kind"]["kind"] == "closure")
        .unwrap()
        .0
        .clone();
    for case in 0..9 {
        let mut wire = original.clone();
        match case {
            0 => wire["globals"]["f"]["value"] = json!({"type":"function","id":999}),
            1 => wire["graph"]["functions"][&function]["kind"]["code_id"] = json!(999999),
            2 => wire["graph"]["functions"][&function]["kind"]["environment"] = json!(999),
            3 => wire["graph"]["environments"][&env]["bindings"]["n"]["kind"] = json!("const"),
            4 => wire["graph"]["environments"][&env]["parent"] = json!(env.parse::<u32>().unwrap()),
            5 => wire["graph"]["environments"][&env]["scope_id"]["site"] = json!(999999),
            6 => {
                wire["graph"]["environments"][&env]["bindings"]["n"]["value"] =
                    json!({"type":"object","id":999})
            }
            7 => wire["functions"]["run"]["kind"] = json!("const"),
            _ => wire["graph"]["next_function"] = json!(0),
        }
        let state = serde_json::from_value(wire).unwrap();
        assert_eq!(
            event(source, "run()", state, &[]).unwrap_err().code,
            "invalid_request",
            "case {case}"
        );
    }
}
/** Unreachable hostile records must pass validation even when tracing would sweep them away. */
#[test]
fn unreachable_and_private_self_corruption_is_not_silently_collected() {
    let source = "const f=function self(){return self;};function run(){}";
    let original = serde_json::to_value(load(source, &[]).unwrap().state).unwrap();
    let env = original["graph"]["environments"]
        .as_object()
        .unwrap()
        .keys()
        .next()
        .unwrap()
        .clone();
    let mut wire = original.clone();
    wire["graph"]["environments"][&env]["bindings"]["self"]["value"] = json!({"type":"undefined"});
    assert_eq!(
        event(source, "run()", serde_json::from_value(wire).unwrap(), &[])
            .unwrap_err()
            .code,
        "invalid_request"
    );
    let mut wire = original;
    wire["graph"]["functions"]["999"] = json!({"kind":{"kind":"closure","code_id":9999,"environment":null,"lexical_this":null},"properties":{}});
    wire["graph"]["next_function"] = json!(1000);
    assert_eq!(
        event(source, "run()", serde_json::from_value(wire).unwrap(), &[])
            .unwrap_err()
            .code,
        "invalid_request"
    );
}
