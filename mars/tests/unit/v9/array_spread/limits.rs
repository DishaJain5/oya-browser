/** Expansion accounts for the source, destination and callable graph before copying each value. */
use super::*;

/** Canonical function properties are real shared capacity, not removable test metadata. */
fn graph_properties(wire: &Value) -> usize {
    wire["graph"]["functions"]
        .as_object()
        .unwrap()
        .values()
        .map(|record| record["properties"].as_object().unwrap().len())
        .sum()
}

/** A sparse source uses no element properties but its dense result still shares canonical capacity. */
#[test]
fn sparse_materialization_admits_actual_property_boundary_then_refuses_growth() {
    let baseline = serde_json::to_value(load("", &[]).unwrap().state).unwrap();
    let available = VM_PROPERTIES - graph_properties(&baseline);
    assert!(
        available < VM_ARRAY_LENGTH,
        "canonical graph records must remain accounted"
    );
    assert_eq!(
        result(
            &format!("const a=[];a.length={available};const b=[...a];"),
            "b.length"
        ),
        available.to_string()
    );
    for length in [available + 1, VM_ARRAY_LENGTH] {
        let source = format!("const a=[];a.length={length};const b=[...a];");
        let error = execute(&source).unwrap_err();
        assert_eq!(error.code, "limit_exceeded");
        assert!(
            error.message.contains("V9 page properties"),
            "{}",
            error.message
        );
        assert_eq!(error.phase.as_deref(), Some("execute"));
        assert_eq!(
            error.location.unwrap().byte_offset,
            source.find("...").unwrap()
        );
    }
}

/** Dense source entries and destination entries consume the same quota rather than separate caps. */
#[test]
fn source_and_destination_entries_share_page_capacity() {
    let source = "const a=[];function run(){const b=[...a];document.getElementById('out').textContent=b.length;}";
    let original = serde_json::to_value(load(source, &[]).unwrap().state).unwrap();
    let array = original["globals"]["a"]["value"]["id"]
        .as_u64()
        .unwrap()
        .to_string();
    let admitted = (VM_PROPERTIES - graph_properties(&original)) / 2;
    for count in [admitted, admitted + 1] {
        let mut wire = original.clone();
        wire["heap"]["objects"][&array]["length"] = json!(count);
        wire["heap"]["objects"][&array]["elements"] =
            json!(vec![json!({"type":"undefined"}); count]);
        let executed = event(source, "run()", serde_json::from_value(wire).unwrap(), &[]);
        if count == admitted {
            assert_eq!(executed.unwrap().changes[0].text, count.to_string());
        } else {
            let error = executed.unwrap_err();
            assert_eq!(error.code, "limit_exceeded");
            assert_eq!(
                error.location.unwrap().byte_offset,
                source.find("...").unwrap()
            );
        }
    }
}

/** User function expandos are included alongside canonical and heap own properties. */
#[test]
fn function_own_properties_reduce_spread_destination_capacity() {
    let source = "const a=[];function run(){const b=[...a];}";
    let mut wire = serde_json::to_value(load(source, &[]).unwrap().state).unwrap();
    let array = wire["globals"]["a"]["value"]["id"]
        .as_u64()
        .unwrap()
        .to_string();
    let function = wire["functions"]["run"]["value"]["id"]
        .as_u64()
        .unwrap()
        .to_string();
    let remaining = VM_PROPERTIES - graph_properties(&wire);
    let properties = wire["graph"]["functions"][&function]["properties"]
        .as_object_mut()
        .unwrap();
    for index in 0..remaining {
        properties.insert(format!("extra{index}"), json!({"type":"undefined"}));
    }
    wire["heap"]["objects"][&array]["length"] = json!(1);
    wire["heap"]["objects"][&array]["elements"] = json!([null]);
    let error = event(source, "run()", serde_json::from_value(wire).unwrap(), &[]).unwrap_err();
    assert_eq!(error.code, "limit_exceeded");
    assert!(error.message.contains("V9 page properties"));
    assert_eq!(
        error.location.unwrap().byte_offset,
        source.find("...").unwrap()
    );
}

/** Retained duplicate strings are refused before the next emitted clone can grow the candidate. */
#[test]
fn string_duplication_enforces_environment_capacity_at_the_ellipsis() {
    let text = "x".repeat(JS_STRING_UNITS);
    let source = format!("const s='{text}';const a=[s,s,s];const b=[...a,...a];");
    let error = execute(&source).unwrap_err();
    assert_eq!(error.code, "limit_exceeded");
    assert!(error.message.contains("V9 environment UTF-16 units"));
    assert_eq!(error.phase.as_deref(), Some("execute"));
    assert_eq!(
        error.location.unwrap().byte_offset,
        source.rfind("...").unwrap()
    );
}

/** Existing exhausted heap policy allocates the literal target before evaluating its first operand. */
#[test]
fn destination_heap_exhaustion_precedes_operand_evaluation() {
    let source = "function run(){const b=[...missing()];}";
    let mut wire = serde_json::to_value(load(source, &[]).unwrap().state).unwrap();
    let objects = wire["heap"]["objects"].as_object_mut().unwrap();
    for id in 1..VM_OBJECTS {
        objects.insert(id.to_string(), json!({"kind":"object","properties":{}}));
    }
    wire["heap"]["next_id"] = json!(VM_OBJECTS);
    let error = event(source, "run()", serde_json::from_value(wire).unwrap(), &[]).unwrap_err();
    assert_eq!(error.code, "limit_exceeded");
    assert!(error.message.contains("V9 heap objects"));
    assert_eq!(
        error.location.unwrap().byte_offset,
        source.find('[').unwrap()
    );
}

/** Prior work and later expansion share one decreasing allowance; no next operand gets to run. */
#[test]
fn expansion_uses_the_existing_request_operation_budget() {
    let source = "let i=0;while(i<180){i++;}const a=[];a.length=1000;const b=[...a,missing];";
    let error = execute(source).unwrap_err();
    assert_eq!(error.code, "limit_exceeded");
    assert!(error.message.contains("operation limit"));
    assert_eq!(error.phase.as_deref(), Some("execute"));
    assert_eq!(
        error.location.unwrap().byte_offset,
        source.find("...").unwrap()
    );
}
