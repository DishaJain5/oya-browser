/** Ordinary expression operators preserve references, bounded evaluation and transactional native state. */
use oya_native::{
    constants::EXPRESSION_DEPTH,
    v9::{JsValue, VmState, event, execute, load},
};

/** Native text proposals expose public expression behavior without a privileged evaluation API. */
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

/** Assignments associate rightward and retain their resulting value inside arguments and initializers. */
#[test]
fn assignment_expressions_are_right_associative_and_return_the_stored_value() {
    let source = "var a,b;function identity(value){return value;}const assigned=identity(a=b=4);const extra=(a+=2);";
    assert_eq!(result(source, "assigned+' '+a+' '+b+' '+extra"), "4 6 4 6");
    assert_eq!(
        result(
            "var named;const value=(named=function(a){});",
            "named.name+' '+value.length"
        ),
        "named 1"
    );
}

/** Conditional arms accept assignment and arrow expressions while only the selected arm executes. */
#[test]
fn conditional_precedence_supports_lazy_assignment_and_arrow_branches() {
    let source = "var a=0,b=0;const selected=true?a=1:b=2;const callback=false?x=>x+1:x=>x+2;const nested=false?1:true?2:3;const lazy=true?7:missing();";
    assert_eq!(
        result(
            source,
            "selected+' '+a+' '+b+' '+callback(3)+' '+nested+' '+lazy"
        ),
        "1 1 0 5 2 7"
    );
}

/** A compound retains its base and key before the RHS can replace the variable holding its receiver. */
#[test]
fn compound_references_evaluate_base_and_key_once_and_keep_the_original_receiver() {
    let source = "var order='',a={n:2},b={n:100};const original=a;function base(){order+='B';return a;}function key(){order+='K';return 'n';}function rhs(){order+='R';a=b;return 3;}const value=(base()[key()]+=rhs());";
    assert_eq!(
        result(source, "value+' '+original.n+' '+a.n+' '+order"),
        "5 5 100 BKR"
    );
}

/** Compound arithmetic reads the old property before RHS mutation and performs one final store. */
#[test]
fn compound_reads_its_old_value_before_running_the_rhs() {
    let source = "const box={n:2};function rhs(){box.n=40;return 3;}const value=(box.n+=rhs());";
    assert_eq!(result(source, "value+' '+box.n"), "5 5");
    assert_eq!(
        result(
            "var n=8;n+=2;n-=3;n*=4;n/=2;n%=5;var text='a';text+=1;",
            "n+' '+text"
        ),
        "4 a1"
    );
}

/** Updates convert supported primitives once and return old or new Numbers according to their position. */
#[test]
fn prefix_and_postfix_updates_use_numeric_results_for_primitive_values() {
    let source = "var text='2',truth=true,nil=null,absent;const old=text++;const fresh=++text;const beforeTruth=truth++;const beforeNil=nil--;const beforeAbsent=absent++;";
    assert_eq!(
        result(
            source,
            "old+' '+fresh+' '+typeof text+' '+beforeTruth+' '+truth+' '+beforeNil+' '+nil+' '+(beforeAbsent===beforeAbsent)+' '+typeof absent"
        ),
        "2 4 number 1 2 0 -1 false number"
    );
}

/** Postfix return values retain IEEE negative zero even though the stored value advances to one. */
#[test]
fn postfix_preserves_negative_zero_in_the_returned_number() {
    let wire = serde_json::to_value(
        load("var value=-0;var previous=value++;", &[])
            .unwrap()
            .state,
    )
    .unwrap();
    assert_eq!(
        wire["functions"]["previous"]["value"],
        serde_json::to_value(JsValue::Number(-0.0)).unwrap()
    );
    assert_eq!(
        wire["functions"]["value"]["value"],
        serde_json::to_value(JsValue::Number(1.0)).unwrap()
    );
}

/** Objects and functions cannot silently become NaN when an operator requires primitive conversion. */
#[test]
fn arithmetic_compounds_and_updates_refuse_unsupported_object_conversion() {
    for source in [
        "var x={};x++;",
        "var x=function(){};++x;",
        "var x=1;x+={};",
        "var x=1;x*=function(){};",
    ] {
        assert_eq!(
            execute(source).unwrap_err().code,
            "unsupported_feature",
            "{source}"
        );
    }
}

/** Plain stores defer TDZ/const checks until RHS evaluation while compounds must read their old value first. */
#[test]
fn plain_and_compound_assignment_preserve_reference_and_store_error_order() {
    assert!(
        execute("x=missing();let x;")
            .unwrap_err()
            .message
            .contains("Binding is not defined")
    );
    assert!(
        execute("x+=missing();let x;")
            .unwrap_err()
            .message
            .contains("before initialization")
    );
    assert!(
        execute("const x=1;x+=missing();")
            .unwrap_err()
            .message
            .contains("Binding is not defined")
    );
    assert!(
        execute("const x=1;x++;")
            .unwrap_err()
            .message
            .contains("const binding")
    );
    assert!(
        execute("const x=1;x+=2;")
            .unwrap_err()
            .message
            .contains("const binding")
    );
    assert_eq!(
        execute("'use strict';missing=1;").unwrap_err().code,
        "script_execution_error"
    );
    assert_eq!(
        execute("const f=function self(){self=1;};f();")
            .unwrap_err()
            .code,
        "unsupported_feature"
    );
    assert_eq!(
        execute("'use strict';const value=(missing=(globalThis.missing=1));")
            .unwrap_err()
            .code,
        "script_execution_error"
    );
    assert_eq!(
        result(
            "const value=(fresh=(globalThis.fresh=2));",
            "value+' '+fresh"
        ),
        "2 2"
    );
}

/** Updates and compounds reuse function metadata readonly behavior under the executing source mode. */
#[test]
fn function_metadata_stores_preserve_sloppy_results_and_strict_refusals() {
    let source = "function f(a){}const old=f.length++;const sum=(f.length+=2);";
    assert_eq!(result(source, "old+' '+sum+' '+f.length"), "1 3 1");
    assert_eq!(
        execute("'use strict';function f(a){}f.length++;")
            .unwrap_err()
            .code,
        "script_execution_error"
    );
    for source in [
        "'use strict';if(false){eval+=1;}",
        "'use strict';function f(){arguments++;}",
    ] {
        assert_eq!(execute(source).unwrap_err().code, "script_syntax_error");
    }
    assert_eq!(
        result(
            "'use strict';const box={eval:1,arguments:2};box.eval++;box.arguments+=2;",
            "box.eval+' '+box.arguments"
        ),
        "2 4"
    );
}

/** Typeof suppresses only an ordinary unresolved identifier, preserving TDZ and unavailable host boundaries. */
#[test]
fn typeof_distinguishes_missing_identifiers_tdz_primitives_and_owned_callables() {
    assert_eq!(
        result(
            "const fn=()=>1;const box={};",
            "typeof missing+' '+typeof (missing)+' '+typeof undefined+' '+typeof null+' '+typeof true+' '+typeof 1+' '+typeof 'x'+' '+typeof box+' '+typeof fn"
        ),
        "undefined undefined undefined object boolean number string object function"
    );
    assert!(
        execute("const observed=typeof x;let x;")
            .unwrap_err()
            .message
            .contains("before initialization")
    );
    assert!(
        execute("const observed=typeof missing.value;")
            .unwrap_err()
            .message
            .contains("Binding is not defined")
    );
    for source in [
        "const x=typeof process;",
        "const x=typeof fetch;",
        "function f(){return typeof arguments;}f();",
    ] {
        assert_eq!(
            execute(source).unwrap_err().code,
            "unsupported_feature",
            "{source}"
        );
    }
    assert!(execute("const x=typeof null.value;").is_err());
    assert_eq!(
        result(
            "const process=1;function f(arguments){return typeof arguments;}",
            "typeof process+' '+f(1)"
        ),
        "number number"
    );
}

/** Void executes its operand once and discards objects/functions without demanding primitive conversion. */
#[test]
fn void_evaluates_once_and_returns_undefined_for_any_supported_value() {
    let source =
        "var n=0;const discarded=void(n++);const object=void {};const callback=void function(){};";
    assert_eq!(
        result(
            source,
            "n+' '+(discarded===undefined)+' '+(object===undefined)+' '+(callback===undefined)"
        ),
        "1 true true true"
    );
    assert!(
        execute("const value=void missing;")
            .unwrap_err()
            .message
            .contains("Binding is not defined")
    );
}

/** Postfix compares its operator with the last target token, including comment-hidden line terminators. */
#[test]
fn postfix_obeys_line_breaks_and_accepts_multiline_members_ending_on_the_operator_line() {
    for source in [
        "var n=0;n\n++;",
        "var n=0;n/*\n*/++;",
        "const box={n:0};box['n']\n++;",
    ] {
        assert_eq!(
            execute(source).unwrap_err().code,
            "script_syntax_error",
            "{source}"
        );
    }
    assert_eq!(
        result(
            "const box={n:2};const old=box[\n'n'\n]++;var n=0;++\nn;",
            "old+' '+box.n+' '+n"
        ),
        "2 3 1"
    );
}

/** Function sites in either conditional arm and assignment RHS remain source-owned through service reconstruction. */
#[test]
fn conditional_and_assignment_function_sites_survive_serialized_events() {
    let source = "function pick(flag){return flag?function(){return 1;}:()=>2;}const yes=pick(true);const no=pick(false);function run(){var assigned;assigned=function(){return 3;};document.getElementById('out').textContent=yes()+' '+no()+' '+assigned();}";
    let state: VmState =
        serde_json::from_value(serde_json::to_value(load(source, &[]).unwrap().state).unwrap())
            .unwrap();
    assert_eq!(
        event(source, "run()", state, &[]).unwrap().changes[0].text,
        "1 2 3"
    );
}

/** Captured cells, global aliases, array slots and function properties use the same prepared-reference stores. */
#[test]
fn updates_and_compounds_preserve_every_supported_reference_kind_across_events() {
    let source = "var n=0;const array=[0];function holder(){}holder.n=0;function make(){let count=0;return ()=>++count;}const next=make();function run(){++n;this.n+=1;array[0]+=2;holder.n++;document.getElementById('out').textContent=n+' '+array[0]+' '+holder.n+' '+next();}";
    let mut state = load(source, &[]).unwrap().state;
    for expected in ["2 2 1 1", "4 4 2 2", "6 6 3 3"] {
        let executed = event(source, "run()", state, &[]).unwrap();
        assert_eq!(executed.changes[0].text, expected);
        state = executed.state;
    }
}

/** Calls include argument trees in their nesting bounds rather than resetting expression depth at each call. */
#[test]
fn assignment_conditional_and_nested_argument_chains_share_expression_depth_limits() {
    for expression in [
        format!("{}1", "a=".repeat(EXPRESSION_DEPTH)),
        format!("{}1", "false?0:".repeat(EXPRESSION_DEPTH)),
        format!(
            "{}1{}",
            "f(".repeat(EXPRESSION_DEPTH),
            ")".repeat(EXPRESSION_DEPTH)
        ),
    ] {
        assert_eq!(
            execute(&format!("var a;const f=x=>x;const value={expression};"))
                .unwrap_err()
                .code,
            "limit_exceeded"
        );
    }
}

/** Ordinary operators cannot turn inline handlers or native DOM fields into a general authority surface. */
#[test]
fn handlers_and_native_dom_writes_keep_their_existing_restricted_grammar() {
    let source = "var count=0;function run(){count++;}";
    let state = load(source, &[]).unwrap().state;
    for handler in ["count++", "count=1", "count+=1", "this.run()"] {
        assert!(
            event(source, handler, state.clone(), &[]).is_err(),
            "{handler}"
        );
    }
    for source in [
        "document.getElementById('out').textContent+='x';",
        "document.getElementById('out').textContent++;",
    ] {
        assert!(execute(source).is_err(), "{source}");
    }
}

/** Unsupported targets and longer operator families cannot be reinterpreted as shorter supported expressions. */
#[test]
fn invalid_reference_targets_and_unsupported_operator_families_fail_before_execution() {
    for source in [
        "1=2;",
        "const f=()=>1;f()++;",
        "var a=1,b=2;(true?a:b)+=1;",
        "var a=1;a**=2;",
        "var a=1;a&&=2;",
        "var a=1;a||=2;",
        "var a=1;a??=2;",
        "var a=1;a<<=2;",
    ] {
        assert!(execute(source).is_err(), "{source}");
    }
}
