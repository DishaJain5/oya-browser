/** Grammar cases keep ellipsis local to array elements and preserve ordinary dot scanning. */
use super::*;

/** Empty, mixed, repeated and nested spreads retain source order and legal trailing commas. */
#[test]
fn array_spread_composes_supported_elements_and_trailing_commas() {
    assert_eq!(
        result("const a=[1,2];", "[...[],0,...a,...[...[3]],4,].length"),
        "5"
    );
    assert_eq!(result("const a=[1,2];", "[...a,...a,][3]"), "2");
    assert_eq!(result("", "[...[],].length"), "0");
}

/** Assignment, conditional and arrow invocation operands reuse assignment-expression grammar. */
#[test]
fn spread_operands_use_assignment_conditional_and_arrow_expressions() {
    assert_eq!(result("let a=[];const b=[...a=[3,4]];", "a[0]+b[1]"), "7");
    assert_eq!(result("", "[...true?[2]:[8]][0]"), "2");
    assert_eq!(result("", "[...(()=>[5])()][0]"), "5");
}

/** Ellipsis recognition does not consume numeric-dot or ordinary member tokens. */
#[test]
fn numeric_and_member_dots_keep_existing_semantics() {
    assert_eq!(
        result(
            "const a={value:[1]};",
            "[.5,...a.value,1.5][0]+a.value.length"
        ),
        "1.5"
    );
    assert_eq!(result("", "[...'.5'][1]"), "5");
}

/** Missing operands report the ellipsis instead of an unrelated closing token. */
#[test]
fn missing_spread_operands_are_located_at_the_ellipsis() {
    for source in [
        "const a=[...];",
        "const a=[...,1];",
        "const a=[...",
        "const a=[......[]];",
    ] {
        let error = execute(source).unwrap_err();
        assert_eq!(error.code, "script_syntax_error", "{source}");
        assert_eq!(error.phase.as_deref(), Some("parse"));
        assert!(
            error
                .message
                .contains("Expected an expression after array spread")
        );
        assert_eq!(
            error.location.unwrap().byte_offset,
            source.find("...").unwrap()
        );
    }
}

/** Malformed dot sequences and literal holes cannot acquire accidental array-spread behavior. */
#[test]
fn malformed_ellipsis_separators_and_elisions_remain_syntax_errors() {
    for source in [
        "const a=[..[]];",
        "const a=[....[]];",
        "const a=[...[] 1];",
        "const a=[,];",
        "const a=[1,,2];",
        "const a=[...[], ,1];",
    ] {
        assert_eq!(
            execute(source).unwrap_err().code,
            "script_syntax_error",
            "{source}"
        );
    }
}

/** Shared ellipsis scanning must not admit unrelated spread or rest grammar. */
#[test]
fn call_object_and_parameter_spread_remain_explicitly_unsupported() {
    for (source, feature) in [
        ("function f(a){}f(...[]);", "call argument spread"),
        ("const a={...{}};", "object spread"),
        ("function f(...a){}", "rest parameters"),
        ("const f=(...a)=>a;", "rest parameters"),
        ("const f=(a,...b)=>a;", "rest parameters"),
    ] {
        let error = execute(source).unwrap_err();
        assert_eq!(error.code, "unsupported_feature", "{source}");
        assert_eq!(error.phase.as_deref(), Some("parse"));
        assert!(
            error.message.contains(feature),
            "{source}: {}",
            error.message
        );
    }
}
