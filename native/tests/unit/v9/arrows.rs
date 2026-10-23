/** Newly authored arrow cases verify source-guided grammar and shared lexical closure execution. */
use oya_native::{
    constants::{EXPRESSION_DEPTH, FUNCTION_NESTING, JS_STRING_UNITS},
    v9::{VmState, event, execute, load},
};
use serde_json::json;
/** An ordinary document proposal exposes expression results without a private evaluation endpoint. */
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
mod grammar;

/** Persisted arrows share captured cells and separate factories, with no initializer replay during events. */
#[test]
fn serialized_arrow_controllers_share_cells_and_keep_factories_independent() {
    let source = "function make(){let n=0;return {inc:()=>{n=n+1;},read:()=>n};}const a=make();const b=make();function run(){a.inc();a.inc();b.inc();document.getElementById('out').textContent=a.read()+' '+b.read();}";
    let mut state = load(source, &[]).unwrap().state;
    for expected in ["2 1", "4 2", "6 3"] {
        let persisted: VmState =
            serde_json::from_value(serde_json::to_value(state).unwrap()).unwrap();
        let executed = event(source, "run()", persisted, &[]).unwrap();
        assert_eq!(executed.changes[0].text, expected);
        state = executed.state;
    }
}
/** Header copies retain distinct values when arrow closures escape through continue. */
#[test]
fn arrows_keep_iteration_cells_and_lexical_tdz_const_rules() {
    let source = "const saved=[];for(let i=0;i<3;i=i+1){saved[i]=()=>i;continue;}";
    assert_eq!(
        result(source, "saved[0]()+' '+saved[1]()+' '+saved[2]()"),
        "0 1 2"
    );
    let source =
        "function make(){return ()=>pending;let pending=1;}const f=make();function run(){f();}";
    assert!(
        event(source, "run()", load(source, &[]).unwrap().state, &[])
            .unwrap_err()
            .message
            .contains("before initialization")
    );
    assert!(
        execute("const f=()=>{const x=1;x=2;};f();")
            .unwrap_err()
            .message
            .contains("const binding")
    );
}
/** Arrow blocks have their own return/loop targets and permit ordinary direct nested declarations. */
#[test]
fn arrow_contexts_do_not_inherit_enclosing_loop_targets() {
    for source in [
        "while(false){const f=()=>{break;};}",
        "while(false){const f=()=>{continue;};}",
        "const f=()=>{return 1;};return 2;",
    ] {
        assert_eq!(execute(source).unwrap_err().code, "script_syntax_error");
    }
    assert_eq!(
        result(
            "const f=()=>{return helper();function helper(){return 4;}};",
            "f()"
        ),
        "4"
    );
    assert_eq!(
        result("let n=0;while(n<1){const f=()=>1;n=n+f();break;}", "n"),
        "1"
    );
}
/** Explicit arguments bindings behave lexically while implicit argument objects remain unavailable. */
#[test]
fn arrows_do_not_synthesize_implicit_arguments() {
    assert_eq!(
        result(
            "function make(arguments){return ()=>arguments;}const f=make(7);",
            "f()"
        ),
        "7"
    );
    assert_eq!(result("const f=(arguments)=>arguments+1;", "f(7)"), "8");
    assert_eq!(
        result("const arguments=3;const f=()=>arguments;", "f()"),
        "3"
    );
    for source in [
        "function make(){return ()=>arguments;}make()();",
        "const arguments=3;function make(){return ()=>arguments;}make()();",
        "let arguments=3;function make(){return ()=>{arguments=4;};}make()();",
    ] {
        assert_eq!(execute(source).unwrap_err().code, "unsupported_feature");
    }
    assert_eq!(
        result(
            "function make(){let arguments=3;return ()=>arguments;}const f=make();",
            "f()"
        ),
        "3"
    );
    assert!(
        execute("function f(){arguments=missing;}f();")
            .unwrap_err()
            .message
            .contains("Binding is not defined")
    );
    assert_eq!(execute("function caller(){let arguments=3;return global();}const global=()=>arguments;caller();").unwrap_err().code,"script_execution_error");
}
/** Arrow code stays source-owned, including inline-handler refusal and unknown wire-kind rejection. */
#[test]
fn handlers_and_private_wire_cannot_introduce_arrow_code_or_kind() {
    let source = "const f=()=>1;function run(){f();}";
    let state = load(source, &[]).unwrap().state;
    for handler in ["run(x=>x)", "(()=>1)()"] {
        assert_eq!(
            event(source, handler, state.clone(), &[]).unwrap_err().code,
            "unsupported_feature"
        );
    }
    let mut wire = serde_json::to_value(state).unwrap();
    let id = wire["graph"]["functions"]
        .as_object()
        .unwrap()
        .keys()
        .next()
        .unwrap()
        .clone();
    wire["graph"]["functions"][id]["kind"] = json!("ordinary");
    assert!(serde_json::from_value::<VmState>(wire).is_err());
}
/** Synthetic concise returns cannot bypass statement counts or nesting/allocation limits. */
#[test]
fn concise_arrows_and_parenthesized_heads_remain_bounded() {
    let nesting = format!("let f={}1;", "x=>".repeat(FUNCTION_NESTING + 1));
    assert_eq!(execute(&nesting).unwrap_err().code, "limit_exceeded");
    let grouping = format!(
        "let f={}x=>x{};",
        "(".repeat(EXPRESSION_DEPTH),
        ")".repeat(EXPRESSION_DEPTH)
    );
    assert_eq!(execute(&grouping).unwrap_err().code, "limit_exceeded");
    let many = (0..33)
        .map(|n| format!("const f{n}=()=>1;"))
        .collect::<String>();
    assert_eq!(execute(&many).unwrap_err().code, "limit_exceeded");
    let source = format!(
        "let s='{}';{}",
        "x".repeat(JS_STRING_UNITS),
        "for(let t=s;;){break;}".repeat(4)
    );
    assert_eq!(execute(&source).unwrap_err().code, "limit_exceeded");
}
/** Short-lived arrow/object/environment cycles are collected and failures preserve the original graph. */
#[test]
fn arrows_reclaim_cycles_and_discard_failed_mutations() {
    let source = "function make(){let n=0;const box={n:0};return fail=>{n=n+1;box.n=box.n+1;document.getElementById('out').textContent=n+' '+box.n;if(fail){missing();}};}const f=make();function run(){const box={};box.f=()=>box;f(false);}function bad(){f(true);}";
    let mut state = load(source, &[]).unwrap().state;
    assert!(event(source, "bad()", state.clone(), &[]).is_err());
    for expected in 1..=600 {
        let result = event(source, "run()", state, &[]).unwrap();
        assert_eq!(result.changes[0].text, format!("{expected} {expected}"));
        state = result.state;
    }
    let wire = serde_json::to_value(state).unwrap();
    assert_eq!(wire["heap"]["objects"].as_object().unwrap().len(), 2);
    assert!(wire["graph"]["environments"].as_object().unwrap().len() < 8);
}
