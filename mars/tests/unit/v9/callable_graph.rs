/** Callable graph boundaries reject forged authority and preserve only genuinely reachable bound state. */
use mars::constants::VM_FUNCTION_INTRINSICS;
use mars::v9::{VmState, event, load};
use serde_json::{Value, json};

/** Every event crosses the same serialized state boundary used by the V9 service. */
fn next(source: &str, state: VmState) -> mars::v9::Execution {
    event(
        source,
        "run()",
        serde_json::from_value(serde_json::to_value(state).unwrap()).unwrap(),
        &[],
    )
    .unwrap()
}
/** Private wire records are inspected only to simulate hostile service input through public APIs. */
fn wire(source: &str) -> Value {
    serde_json::to_value(load(source, &[]).unwrap().state).unwrap()
}
/** Shape-valid hostile snapshots must fail before any script body executes. */
fn rejected(source: &str, wire: Value) {
    assert_eq!(
        event(source, "run()", serde_json::from_value(wire).unwrap(), &[])
            .unwrap_err()
            .code,
        "invalid_request"
    );
}
/** Canonical intrinsic selectors have exactly one reserved identity, including unreachable input. */
#[test]
fn canonical_intrinsic_identities_cannot_be_missing_replaced_or_duplicated() {
    let source = "function run(){}";
    let original = wire(source);
    let mut missing = original.clone();
    missing["graph"]["functions"]
        .as_object_mut()
        .unwrap()
        .remove("1");
    rejected(source, missing);
    let mut replaced = original.clone();
    replaced["graph"]["functions"]["0"]["kind"]["intrinsic"] = json!("bind");
    rejected(source, replaced);
    let mut extra = original;
    extra["graph"]["functions"]["999"] =
        json!({"kind":{"kind":"intrinsic","intrinsic":"call"},"properties":{}});
    extra["graph"]["next_function"] = json!(1000);
    rejected(source, extra);
}
/** Bound target edges cannot cycle or point outside the page's own function arena. */
#[test]
fn bound_cycles_and_dangling_owned_values_are_rejected_before_execution() {
    let source = "function f(x){return x;}const bound=f.bind({n:1},{});function run(){}";
    let original = wire(source);
    let id = original["graph"]["functions"]
        .as_object()
        .unwrap()
        .iter()
        .find(|(_, value)| value["kind"]["kind"] == "bound")
        .unwrap()
        .0
        .clone();
    for case in 0..5 {
        let mut value = original.clone();
        match case {
            0 => {
                value["graph"]["functions"][&id]["kind"]["target"] =
                    json!(id.parse::<u32>().unwrap())
            }
            1 => value["graph"]["functions"][&id]["kind"]["target"] = json!(999),
            2 => {
                value["graph"]["functions"][&id]["kind"]["receiver"] =
                    json!({"type":"object","id":999})
            }
            3 => {
                value["graph"]["functions"][&id]["kind"]["arguments"][0] =
                    json!({"type":"function","id":999})
            }
            _ => {
                value["graph"]["functions"]["1"]["properties"]["saved"] =
                    json!({"type":"object","id":999})
            }
        }
        rejected(source, value);
    }
}
/** Objects and functions retained exclusively by bound receivers or prefixes survive finish-only collection. */
#[test]
fn bound_receivers_arguments_and_targets_survive_serialized_events() {
    let source = "function make(){const receiver={n:4};const argument=()=>3;return function(cb){this.n=this.n+cb();return this.n;}.bind(receiver,argument);}const bound=make();function run(){document.getElementById('out').textContent=bound();}";
    let mut state = load(source, &[]).unwrap().state;
    for expected in ["7", "10", "13"] {
        let result = next(source, state);
        assert_eq!(result.changes[0].text, expected);
        state = result.state;
    }
}
/** Canonical roots retain mutable prototype properties and every cycle reachable through those values. */
#[test]
fn canonical_prototype_properties_keep_owned_cycles_alive() {
    let source = "function setup(){const box={n:8};const callback=()=>box.n;box.callback=callback;Function.prototype.saved=box;}setup();function run(){document.getElementById('out').textContent=Function.prototype.saved.callback();}";
    let state = load(source, &[]).unwrap().state;
    assert_eq!(next(source, state).changes[0].text, "8");
}
/** Discarded bound records do not consume a lifetime allocation quota or retain their target cycles. */
#[test]
fn unreachable_bound_records_and_expando_cycles_are_reclaimed_between_events() {
    let source = "function discard(){const f=function(){return 1;};const bound=f.bind({});bound.self=bound;f.bound=bound;}function run(){discard();}";
    let mut state = load(source, &[]).unwrap().state;
    for _ in 0..600 {
        state = next(source, state).state;
    }
    let value = serde_json::to_value(state).unwrap();
    assert_eq!(
        value["graph"]["functions"].as_object().unwrap().len(),
        VM_FUNCTION_INTRINSICS as usize + 2
    );
    assert_eq!(value["heap"]["objects"].as_object().unwrap().len(), 1);
    assert!(value["graph"]["next_function"].as_u64().unwrap() > 512);
}
