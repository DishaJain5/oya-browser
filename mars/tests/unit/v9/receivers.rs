/** Receiver behavior is exercised through public load/events without a foreign JavaScript runtime. */
use mars::v9::{VmState, event, execute, load, load_scripts};
use serde_json::json;

/** Native document proposals expose values while keeping heap records private. */
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
/** Dot, computed and grouped member references retain their owning object. */
#[test]
fn method_references_keep_receivers_and_detached_calls_use_the_callee_mode() {
    let source = "const a={n:3,m:function(){return this.n;}};const b={n:7,m:a.m};";
    assert_eq!(result(source, "a.m()+' '+b['m']()+' '+(a.m)()"), "3 7 3");
    let source = "const a={m:function(){return this;},s:function(){'use strict';return this;}};const m=a.m;const s=a.s;";
    assert_eq!(
        result(
            source,
            "(m()===globalThis)+' '+(s()===undefined)+' '+(a.s()===a)"
        ),
        "true true true"
    );
    assert_eq!(
        result(
            source,
            "((false||a.s)()===undefined)+' '+((false||a.m)()===globalThis)"
        ),
        "true true"
    );
}
/** Calls select their callee before argument callbacks mutate the original property. */
#[test]
fn base_key_and_arguments_run_once_in_source_order() {
    let source = "let order='';const a={n:4,m:function(x){order=order+'C';return this.n+x;}};function base(){order=order+'B';return a;}function key(){order=order+'K';return 'm';}function argument(){order=order+'A';a.m=function(){return 99;};return 2;}const value=base()[key()](argument());";
    assert_eq!(result(source, "value+' '+order+' '+a.m()"), "6 BKAC 99");
    assert!(
        execute("const a={m:0};a.m(missing());")
            .unwrap_err()
            .message
            .contains("Binding is not defined")
    );
}
/** Computed key expressions precede coercibility failure, while arguments do not. */
#[test]
fn null_member_errors_obey_key_and_argument_order_without_disclosing_values() {
    let error = execute("const a=null;const x=a[missing()];").unwrap_err();
    assert!(error.message.contains("Binding is not defined"));
    let error = execute("const a=null;const x=a[{}];").unwrap_err();
    assert!(error.message.contains("Cannot read a property of null"));
    let error = execute("const a=undefined;a.m(missing());").unwrap_err();
    assert!(
        error
            .message
            .contains("Cannot read a property of undefined")
    );
}
/** Arrows retain the defining method receiver even when stored on another object or invoked bare. */
#[test]
fn arrows_retain_this_after_return_and_service_serialization() {
    let source = "function make(){const a={n:0,create:function(){return ()=>{this.n=this.n+1;return this.n;};}};return a.create();}const f=make();const other={f:f};function run(){document.getElementById('out').textContent=f()+' '+other.f();}";
    let mut state = load(source, &[]).unwrap().state;
    for expected in ["1 2", "3 4", "5 6"] {
        let wire = serde_json::to_value(state).unwrap();
        let executed = event(source, "run()", serde_json::from_value(wire).unwrap(), &[]).unwrap();
        assert_eq!(executed.changes[0].text, expected);
        state = executed.state;
    }
}
/** Nested ordinary functions establish their own receiver and arrows capture that receiver rather than the caller. */
#[test]
fn nested_ordinary_calls_and_top_level_arrows_keep_distinct_receivers() {
    let source = "const top=()=>this;const a={n:4,m:function(){const outer=()=>this;function inner(){'use strict';return ()=>this;}return (outer()===this)&&inner()()===undefined;}};const b={top:top};";
    assert_eq!(
        result(source, "a.m()+' '+(b.top()===globalThis)"),
        "true true"
    );
    assert_eq!(
        result("'use strict';const f=()=>this;", "f()===globalThis"),
        "true"
    );
}
/** Only exact direct prologue literals activate strict mode; unknown directives do not lower it. */
#[test]
fn directives_distinguish_raw_literals_grouping_and_late_expressions() {
    for body in [
        "'use strict';",
        "'custom';\"use strict\";",
        "'use strict';'custom';",
    ] {
        assert_eq!(
            result(
                &format!("function f(){{{body}return this;}}"),
                "f()===undefined"
            ),
            "true"
        );
    }
    for body in [
        "'use\\x20strict';",
        "('use strict');",
        "'use strict'+'';",
        ";'use strict';",
        "{'use strict';}",
    ] {
        assert_eq!(
            result(
                &format!("function f(){{{body}return this;}}"),
                "f()===globalThis"
            ),
            "true",
            "{body}"
        );
    }
    assert_eq!(
        result(
            "function outer(){'use strict';return function(){return this;};}const f=outer();",
            "f()===undefined"
        ),
        "true"
    );
    assert_eq!(
        result(
            "const f=()=>{'use strict';return function(){return this;};};const g=f();",
            "g()===undefined"
        ),
        "true"
    );
}
/** Separate classic scripts do not inherit strictness from the earlier script or a dynamic caller. */
#[test]
fn script_and_caller_modes_do_not_change_defining_source_modes() {
    let sources=vec!["'use strict';function strict(){return this;}".into(),"function sloppy(){return this;}document.getElementById('out').textContent=(strict()===undefined)+' '+(sloppy()===globalThis);".into()];
    assert_eq!(
        load_scripts(&sources, &[]).unwrap().changes[0].text,
        "true true"
    );
    assert_eq!(
        result(
            "function sloppy(){return this;}function strict(){'use strict';return sloppy();}",
            "strict()===globalThis"
        ),
        "true"
    );
}
/** Strict validation runs after an own directive is discovered and covers unreachable code and headers. */
#[test]
fn strict_restricted_names_are_early_errors() {
    for source in [
        "function f(arguments){'use strict';}",
        "const f=function eval(){'use strict';};",
        "const f=(arguments)=>{'use strict';};",
        "'use strict';if(false){let arguments=1;}",
        "'use strict';function f(){eval=1;}",
        "'use strict';for(let arguments=0;false;){break;}",
        "function f(){'use strict';function arguments(){}}",
        "'use strict';const public=1;",
    ] {
        assert_eq!(
            execute(source).unwrap_err().code,
            "script_syntax_error",
            "{source}"
        );
    }
    execute("function f(arguments){return arguments;}const eval=1;").unwrap();
}
/** Page-global properties share function cells while lexical cells stay off the object. */
#[test]
fn global_receiver_properties_and_identifier_bindings_remain_coherent() {
    let source = "let privateValue=3;function f(){return 1;}const same=this.f===f;this.f=function(){return 2;};const two=f();f=function(){return 3;};this.expando=4;expando=expando+1;";
    assert_eq!(
        result(
            source,
            "same+' '+two+' '+this.f()+' '+(this.privateValue===undefined)+' '+this.expando+' '+expando+' '+(this.globalThis===globalThis)"
        ),
        "true 2 3 true 5 5 true"
    );
    assert_eq!(result("fresh=7;", "this.fresh"), "7");
    assert_eq!(
        execute("'use strict';fresh=7;").unwrap_err().code,
        "script_execution_error"
    );
    assert_eq!(
        result(
            "const globalThis=5;",
            "globalThis+' '+(this.globalThis===this)"
        ),
        "5 true"
    );
    for source in [
        "this.process;",
        "this.globalThis=1;",
        "globalThis=1;",
        "this.window;",
    ] {
        assert_eq!(execute(source).unwrap_err().code, "unsupported_feature");
    }
}
/** Source metadata owns receiver shape; every hostile reference is checked before event execution. */
#[test]
fn forged_arrow_activation_and_global_receivers_fail_closed() {
    let source = "const box={n:1,make:function(){return ()=>this.n;}};const f=box.make();function run(){f();}";
    let original = serde_json::to_value(load(source, &[]).unwrap().state).unwrap();
    let arrow = original["graph"]["functions"]
        .as_object()
        .unwrap()
        .iter()
        .find(|(_, f)| f["kind"]["kind"] == "closure" && !f["kind"]["lexical_this"].is_null())
        .unwrap()
        .0
        .clone();
    let ordinary = original["graph"]["functions"]
        .as_object()
        .unwrap()
        .iter()
        .find(|(_, f)| f["kind"]["kind"] == "closure" && f["kind"]["lexical_this"].is_null())
        .unwrap()
        .0
        .clone();
    let env = original["graph"]["environments"]
        .as_object()
        .unwrap()
        .keys()
        .next()
        .unwrap()
        .clone();
    for case in 0..6 {
        let mut wire = original.clone();
        match case {
            0 => {
                wire["graph"]["functions"][&arrow]["kind"]["lexical_this"] =
                    json!({"type":"object","id":9999})
            }
            1 => wire["graph"]["functions"][&arrow]["kind"]["lexical_this"] = json!(null),
            2 => {
                wire["graph"]["functions"][&ordinary]["kind"]["lexical_this"] =
                    json!({"type":"undefined"})
            }
            3 => {
                wire["graph"]["environments"][&env]["receiver"] =
                    json!({"type":"function","id":9999})
            }
            4 => wire["global_object"] = json!(9999),
            _ => {
                let global = wire["global_object"].as_u64().unwrap().to_string();
                wire["heap"]["objects"][global]["properties"]["run"] = json!({"type":"undefined"});
            }
        }
        let state: VmState = serde_json::from_value(wire).unwrap();
        assert_eq!(
            event(source, "run()", state, &[]).unwrap_err().code,
            "invalid_request",
            "case {case}"
        );
    }
}
/** Receiver-only object edges remain live while discarded receiver cycles are reclaimed after each event. */
#[test]
fn receiver_only_objects_survive_and_temporary_receiver_cycles_are_reclaimed() {
    let source = "function make(){return {n:0,create:function(){return ()=>{this.n=this.n+1;return this.n;};}}.create();}const saved=make();function run(){make();document.getElementById('out').textContent=saved();}";
    let mut state = load(source, &[]).unwrap().state;
    for expected in 1..=600 {
        let executed = event(source, "run()", state, &[]).unwrap();
        assert_eq!(executed.changes[0].text, expected.to_string());
        state = executed.state;
    }
    let wire = serde_json::to_value(state).unwrap();
    assert_eq!(wire["heap"]["objects"].as_object().unwrap().len(), 2);
    assert_eq!(wire["graph"]["environments"].as_object().unwrap().len(), 2);
    assert!(wire["heap"]["next_id"].as_u64().unwrap() > 600);
}
/** Receiver captures count strings before finish-time collection can conceal peak over-allocation. */
#[test]
fn serialized_receiver_strings_share_the_environment_budget() {
    let source = "const a={create:function(){'use strict';return ()=>this;}};const f=a.create();function run(){f();}";
    let mut wire = serde_json::to_value(load(source, &[]).unwrap().state).unwrap();
    let string = json!({"type":"string","units":vec![120u16;mars::constants::JS_STRING_UNITS]});
    for function in wire["graph"]["functions"]
        .as_object_mut()
        .unwrap()
        .values_mut()
    {
        if !function["kind"]["lexical_this"].is_null() {
            function["kind"]["lexical_this"] = string.clone();
        }
    }
    let env = wire["graph"]["environments"]
        .as_object()
        .unwrap()
        .values()
        .next()
        .unwrap()
        .clone();
    let mut next = wire["graph"]["next_environment"].as_u64().unwrap();
    for _ in 0..8 {
        let mut env = env.clone();
        env["receiver"] = string.clone();
        wire["graph"]["environments"][next.to_string()] = env;
        next += 1;
    }
    wire["graph"]["next_environment"] = json!(next);
    let state = serde_json::from_value(wire).unwrap();
    assert_eq!(
        event(source, "run()", state, &[]).unwrap_err().code,
        "limit_exceeded"
    );
}
