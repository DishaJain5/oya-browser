/** Canonical predicates participate in closed graph validation and finish-only tracing. */
use super::*;

/** Methods share canonical identity, readonly metadata and ordinary own-property shadowing. */
#[test]
fn predicate_metadata_identity_and_shadowing_use_the_existing_callable_model() {
    assert_eq!(
        result(
            "",
            "([].some===[1].some)+' '+([].find===[1].find)+' '+[].some.name+' '+[].some.length+' '+[].find.name+' '+[].find.length+' '+[].find.bind([]).name+' '+[].find.bind([]).length"
        ),
        "true true some 1 find 1 bound find 1"
    );
    let source = "const a=[];const b=[];a.some=undefined;b.find=function(){return 8;};[].some.note=7;[].find.note=9;";
    assert_eq!(
        result(
            source,
            "(a.some===undefined)+' '+b.find()+' '+[1].some.note+' '+[1].find.note+' '+({}.some===undefined)+' '+({}.find===undefined)"
        ),
        "true 8 7 9 true true"
    );
    for source in [
        "'use strict';[].some.name='bad';",
        "'use strict';[].find.length=99;",
        "const a=[];a.some=undefined;a.some(()=>true);",
    ] {
        assert_eq!(execute(source).unwrap_err().code, "script_execution_error");
    }
}

/** The canonical roots cannot be missing, shifted, substituted, or duplicated even in unreachable records. */
#[test]
fn predicate_canonical_graph_records_reject_hostile_wire_states() {
    let source = "function run(){}";
    let original = serde_json::to_value(load(source, &[]).unwrap().state).unwrap();
    let some = VM_ARRAY_SOME_ID.to_string();
    let find = VM_ARRAY_FIND_ID.to_string();
    for case in 0..6 {
        let mut wire = original.clone();
        match case {
            0 => {
                wire["graph"]["functions"]
                    .as_object_mut()
                    .unwrap()
                    .remove(&some);
            }
            1 => {
                wire["graph"]["functions"]
                    .as_object_mut()
                    .unwrap()
                    .remove(&find);
            }
            2 => wire["graph"]["functions"][&some]["kind"]["intrinsic"] = json!("array_find"),
            3 => wire["graph"]["functions"][&find] = wire["graph"]["functions"][&some].clone(),
            4 => {
                wire["graph"]["functions"]["999"] = wire["graph"]["functions"][&find].clone();
                wire["graph"]["next_function"] = json!(1000);
            }
            _ => {
                wire["graph"]["functions"]
                    .as_object_mut()
                    .unwrap()
                    .remove(&some);
                wire["graph"]["functions"]
                    .as_object_mut()
                    .unwrap()
                    .remove(&find);
                wire["graph"]["next_function"] = json!(14);
            }
        }
        assert_eq!(
            event(source, "run()", serde_json::from_value(wire).unwrap(), &[])
                .unwrap_err()
                .code,
            "invalid_request",
            "case {case}"
        );
    }
}

/** Canonical expandos root cycles, closures and retained selected values across serialized events. */
#[test]
fn intrinsic_properties_and_selected_references_survive_collection_and_roundtrip() {
    let source = "const source=[{n:7}];const selected=source.find(()=>true);function setup(){const box={n:4};const callback=()=>box.n;box.callback=callback;[].some.saved=box;[].find.self=[].find;}setup();const test=[].some.bind(source,v=>v===selected);function run(){selected.n++;document.getElementById('out').textContent=test()+' '+selected.n+' '+[].some.saved.callback()+' '+([].find.self===[].find);}";
    let mut state = load(source, &[]).unwrap().state;
    for expected in ["true 8 4 true", "true 9 4 true"] {
        let execution = next(source, "run()", state);
        assert_eq!(execution.changes[0].text, expected);
        state = execution.state;
    }
    assert_eq!(VM_ARRAY_SOME_ID, 14);
    assert_eq!(VM_ARRAY_FIND_ID, 15);
    assert_eq!(VM_FUNCTION_INTRINSICS, 16);
}

/** Predicate calls do not allocate result arrays, even when every ordinary heap slot is occupied. */
#[test]
fn selection_succeeds_at_full_heap_capacity_while_map_cannot_allocate() {
    let source = "const a=[7];function yes(){return true;}function some(){document.getElementById('out').textContent=a.some(yes);}function find(){document.getElementById('out').textContent=a.find(yes);}function map(){a.map(yes);}";
    let mut wire = serde_json::to_value(load(source, &[]).unwrap().state).unwrap();
    let objects = wire["heap"]["objects"].as_object_mut().unwrap();
    for id in objects.len()..VM_OBJECTS {
        objects.insert(id.to_string(), json!({"kind":"object","properties":{}}));
    }
    wire["heap"]["next_id"] = json!(VM_OBJECTS);
    let state: VmState = serde_json::from_value(wire).unwrap();
    for (handler, expected) in [("some()", "true"), ("find()", "7")] {
        let execution = next(source, handler, state.clone());
        assert_eq!(execution.changes[0].text, expected);
        let after = serde_json::to_value(execution.state).unwrap();
        assert_eq!(after["heap"]["next_id"], json!(VM_OBJECTS));
    }
    assert_eq!(
        event(source, "map()", state, &[]).unwrap_err().code,
        "limit_exceeded"
    );
}
