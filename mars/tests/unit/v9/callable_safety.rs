/** Public callable boundaries enforce shared work, graph, metadata and aggregate allocation limits. */
use mars::{
    constants::{JS_STRING_UNITS, VM_BOUND_HOPS, VM_CALL_ARGUMENTS},
    v9::{VmState, event, execute, load},
};
use serde_json::{Value, json};

/** Observable script results use native document proposals rather than private evaluator hooks. */
fn result(source: &str, expression: &str) -> String {
    execute(&format!(
        "{source}document.getElementById('out').textContent={expression};"
    ))
    .unwrap()
    .last()
    .unwrap()
    .text
    .clone()
}

/** Recursive adapters cannot evade the source-function invocation budget or recurse indefinitely. */
#[test]
fn recursive_call_and_self_referential_apply_lists_hit_invocation_limits() {
    for source in [
        "function recur(){return recur.call(null);}recur();",
        "const apply=Function.prototype.apply;const args=[apply];args[1]=args;apply.apply(apply,args);",
    ] {
        let error = execute(source).unwrap_err();
        assert_eq!(error.code, "limit_exceeded");
        assert!(error.message.contains("call depth"), "{}", error.message);
    }
}

/** Empty prefixes still consume the named chain allowance while iterative dispatch uses one frame. */
#[test]
fn sixteen_empty_bindings_invoke_successfully_and_the_next_binding_is_refused() {
    let source = format!(
        "let f=function(){{return 7;}};for(let i=0;i<{VM_BOUND_HOPS};i=i+1){{f=f.bind(null);}}"
    );
    assert_eq!(result(&source, "f()+' '+f.call(null)"), "7 7");
    assert_eq!(
        execute(&format!("{source}f=f.bind(null);"))
            .unwrap_err()
            .code,
        "limit_exceeded"
    );
}

/** Prefix storage and final call arguments share a cumulative limit across distinct bindings. */
#[test]
fn accumulated_bound_prefixes_share_the_final_call_argument_limit() {
    let prefix = vec!["0"; VM_CALL_ARGUMENTS - 1].join(",");
    let source =
        format!("function f(){{return 9;}}const bound=f.bind(null,{prefix}).bind(null,0);");
    assert_eq!(result(&source, "bound()"), "9");
    for suffix in ["bound(1);", "const extra=bound.bind(null,1);"] {
        assert_eq!(
            execute(&format!("{source}{suffix}")).unwrap_err().code,
            "limit_exceeded"
        );
    }
}

/** Hostile snapshots retain their structural validity so rejection must come from the event boundary. */
fn rejected(source: &str, wire: Value) {
    let state: VmState = serde_json::from_value(wire).unwrap();
    assert_eq!(
        event(source, "run()", state, &[]).unwrap_err().code,
        "invalid_request"
    );
}

/** Bound metadata is derived from its target even when a malicious record has no root. */
#[test]
fn forged_bound_names_lengths_and_readonly_own_metadata_fail_before_events() {
    let source = "function f(a,b){return a;}const bound=f.bind(null,1);function run(){}";
    let original = serde_json::to_value(load(source, &[]).unwrap().state).unwrap();
    let id = original["graph"]["functions"]
        .as_object()
        .unwrap()
        .iter()
        .find(|(_, value)| value["kind"]["kind"] == "bound")
        .unwrap()
        .0
        .clone();
    for case in 0..5 {
        let mut wire = original.clone();
        match case {
            0 => {
                wire["graph"]["functions"][&id]["kind"]["name"] =
                    json!({"type":"string","units":[120]})
            }
            1 => wire["graph"]["functions"][&id]["kind"]["length"] = json!(99),
            2 => {
                wire["graph"]["functions"][&id]["properties"]["name"] = json!({"type":"undefined"})
            }
            3 => {
                wire["graph"]["functions"][&id]["properties"]["length"] =
                    json!({"type":"undefined"})
            }
            _ => {
                let mut hidden = original["graph"]["functions"][&id].clone();
                hidden["kind"]["length"] = json!(99);
                wire["graph"]["functions"]["999"] = hidden;
                wire["graph"]["next_function"] = json!(1000);
            }
        }
        rejected(source, wire);
    }
}

/** Objects and function expandos consume one property allowance and failed events preserve earlier records. */
#[test]
fn function_and_object_properties_share_a_page_quota_across_events() {
    let source = "const box={};function f(){}let n=0;function run(){for(let i=0;i<10;i=i+1){box['a'+n]=n;f['b'+n]=n;n=n+1;}document.getElementById('out').textContent=n;}function read(){document.getElementById('out').textContent=n;}";
    let mut state = load(source, &[]).unwrap().state;
    let mut committed = String::new();
    for _ in 0..100 {
        match event(source, "run()", state.clone(), &[]) {
            Ok(executed) => {
                committed = executed.changes[0].text.clone();
                state = executed.state;
            }
            Err(error) => {
                assert_eq!(error.code, "limit_exceeded");
                assert!(error.message.contains("properties"), "{}", error.message);
                assert!(!committed.is_empty());
                assert_eq!(
                    event(source, "read()", state, &[]).unwrap().changes[0].text,
                    committed
                );
                return;
            }
        }
    }
    panic!("Function and object properties exceeded the shared page allowance.");
}

/** Prefix values cannot bypass aggregate string accounting by moving a global value into callable records. */
#[test]
fn bound_arguments_and_function_properties_share_the_environment_string_quota() {
    let source = format!(
        "const s='{}';function f(){{}}f.extra=s;",
        "x".repeat(JS_STRING_UNITS)
    );
    execute(&format!("{source}const good=f.bind(null,s,s,s,s,s);")).unwrap();
    assert_eq!(
        execute(&format!("{source}const bad=f.bind(null,s,s,s,s,s,s);"))
            .unwrap_err()
            .code,
        "limit_exceeded"
    );
}

/** Each retained bound name is a real string allocation even when prefixes are empty. */
#[test]
fn bound_name_chains_count_toward_the_aggregate_string_quota() {
    let name = "n".repeat(1000);
    let source = format!("let saved=function {name}(){{}};");
    execute(&format!(
        "{source}for(let i=0;i<14;i=i+1){{saved=saved.bind(null);}}"
    ))
    .unwrap();
    assert_eq!(
        execute(&format!(
            "{source}for(let i=0;i<16;i=i+1){{saved=saved.bind(null);}}"
        ))
        .unwrap_err()
        .code,
        "limit_exceeded"
    );
}
