/** Implicit punctuation consumes no new source tokens or statement records and raises no budgets. */
use super::*;
use mars::constants::{EXPRESSION_DEPTH, SCRIPT_BYTES, STATEMENT_DEPTH, STATEMENTS};

/** Semicolonless source remains subject to the exact existing byte admission boundary. */
#[test]
fn implicit_termination_preserves_the_source_byte_limit() {
    let prefix = "var n=1/*";
    let source = format!("{prefix}{}*/", "x".repeat(SCRIPT_BYTES - prefix.len() - 2));
    assert_eq!(source.len(), SCRIPT_BYTES);
    load(&source, &[]).unwrap();
    let error = load(&format!("{source} "), &[]).unwrap_err();
    assert_eq!(error.code, "limit_exceeded");
    assert!(error.message.contains("script bytes"));
}

/** Exactly the same number of real statements fits with implicit and explicit punctuation. */
#[test]
fn inserted_terminators_do_not_consume_statement_budget() {
    for separator in ["\n", ";"] {
        let source = std::iter::repeat_n("0", STATEMENTS)
            .collect::<Vec<_>>()
            .join(separator);
        load(&source, &[]).unwrap();
        let error = load(&format!("{source}{separator}0"), &[]).unwrap_err();
        assert_eq!(error.code, "limit_exceeded");
        assert!(error.message.contains("statements"));
    }
}

/** End-token accounting remains distinct for single-source scanning and ordered-script admission. */
#[test]
fn implicit_eof_adds_no_program_token_and_keeps_aggregate_end_accounting() {
    let elements = std::iter::repeat_n("1", 508).collect::<Vec<_>>().join(",");
    let source = format!("function f(){{return[{elements}]}}");
    load(&source, &[]).unwrap();
    let error = load_scripts(std::slice::from_ref(&source), &[]).unwrap_err();
    assert_eq!(error.code, "limit_exceeded");
    assert!(error.message.contains("tokens"));
    let error = load(&format!("{source};"), &[]).unwrap_err();
    assert_eq!(error.code, "limit_exceeded");
    assert_eq!(error.phase.as_deref(), Some("scan"));
}

/** Existing handler insertion still charges its optional semicolon against the scanner token cap. */
#[test]
fn handlers_preserve_their_existing_optional_semicolon_token_accounting() {
    let source = "function f(value){}";
    let near = std::iter::repeat_n("1", 509).collect::<Vec<_>>().join(",");
    event(
        source,
        &format!("f([{near}])"),
        load(source, &[]).unwrap().state,
        &[],
    )
    .unwrap();
    let full = std::iter::repeat_n("1", 510).collect::<Vec<_>>().join(",");
    let error = event(
        source,
        &format!("f([{full}])"),
        load(source, &[]).unwrap().state,
        &[],
    )
    .unwrap_err();
    assert_eq!(error.code, "limit_exceeded");
    assert!(error.message.contains("tokens"));
    assert_eq!(error.phase.as_deref(), Some("scan"));
    assert_eq!(error.input_kind.as_deref(), Some("handler"));
}

/** Bare-return and block insertion cannot bypass existing recursive expression admission. */
#[test]
fn implicit_termination_preserves_expression_and_statement_depth_limits() {
    let expression = format!(
        "var n={}1{}",
        "(".repeat(EXPRESSION_DEPTH),
        ")".repeat(EXPRESSION_DEPTH)
    );
    let error = load(&expression, &[]).unwrap_err();
    assert_eq!(error.code, "limit_exceeded");
    assert!(error.message.contains("expression depth"));
    let nested = format!(
        "{}0{}",
        "{".repeat(STATEMENT_DEPTH),
        "}".repeat(STATEMENT_DEPTH)
    );
    let error = load(&nested, &[]).unwrap_err();
    assert_eq!(error.code, "limit_exceeded");
    assert!(error.message.contains("statement depth"));
}
