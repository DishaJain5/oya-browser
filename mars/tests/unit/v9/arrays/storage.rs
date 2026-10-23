/** Array private records enforce canonical index ownership and survive serialized request boundaries. */
use super::*;

/** Result insertion enforces aggregate retained strings during iteration, before a large result can publish. */
#[test]
fn map_and_filter_result_insertions_enforce_the_environment_string_budget() {
    let value = "x".repeat(JS_STRING_UNITS);
    for operation in ["[0,0,0,0,0,0,0,0].map(()=>s)", "[s,s,s,s].filter(()=>true)"] {
        assert_eq!(
            execute(&format!("const s='{value}';{operation};"))
                .unwrap_err()
                .code,
            "limit_exceeded",
            "{operation}"
        );
    }
}

/** Canonical indices outside the engine's array budget cannot masquerade as named properties. */
#[test]
fn out_of_budget_canonical_array_indices_are_rejected() {
    for key in [VM_ARRAY_LENGTH.to_string(), (u32::MAX - 1).to_string()] {
        assert_eq!(
            execute(&format!("const a=[];a['{key}']=1;"))
                .unwrap_err()
                .code,
            "limit_exceeded"
        );
    }
    assert_eq!(
        result(
            "const a=[];a['4294967295']=7;a['01']=8;a['-0']=9;",
            "a.length+' '+a['4294967295']+' '+a['01']+' '+a['-0']"
        ),
        "0 7 8 9"
    );
}

/** Named-property state cannot duplicate the special length or any canonical indexed storage slot. */
#[test]
fn hostile_array_property_maps_cannot_duplicate_length_or_canonical_indices() {
    let source = "const a=[1];function run(){a.map(value=>value);}";
    let original = serde_json::to_value(load(source, &[]).unwrap().state).unwrap();
    let id = original["heap"]["objects"]
        .as_object()
        .unwrap()
        .iter()
        .find(|(_, value)| value["kind"] == "array")
        .unwrap()
        .0
        .clone();
    for key in ["length", "0", "1024", "4294967294"] {
        let mut wire = original.clone();
        wire["heap"]["objects"][&id]["properties"][key] = json!({"type":"undefined"});
        let state: VmState = serde_json::from_value(wire).unwrap();
        assert_eq!(
            event(source, "run()", state, &[]).unwrap_err().code,
            "invalid_request",
            "{key}"
        );
    }
}

/** Sparse map results and dense filters retain identity and values through ordinary service serialization. */
#[test]
fn array_results_survive_serialized_events_and_failed_callbacks_preserve_committed_state() {
    let source = "let count=0;const a=[1,2];let mapped=a.map(value=>value+1);function run(){count++;mapped=mapped.map(value=>value+1);document.getElementById('out').textContent=count+' '+mapped.join(' ');}function bad(){count++;mapped=mapped.filter(()=>{missing();});}";
    let mut state = load(source, &[]).unwrap().state;
    for (count, expected) in [(1, "1 3 4"), (2, "2 4 5")] {
        assert!(event(source, "bad()", state.clone(), &[]).is_err());
        let persisted = serde_json::from_value(serde_json::to_value(state).unwrap()).unwrap();
        let executed = event(source, "run()", persisted, &[]).unwrap();
        assert_eq!(executed.changes[0].text, expected, "{count}");
        state = executed.state;
    }
}
