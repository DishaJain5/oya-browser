/** Separate programs, restricted handlers and original coordinates retain their existing public seams. */
use super::*;

/** Every ordered classic source finishes at its own EOF before the next source is parsed. */
#[test]
fn separate_scripts_finish_independently_and_execute_in_source_order() {
    let sources = vec![
        "var n=1".into(),
        "n=n+2".into(),
        "document.getElementById('result').textContent=n".into(),
    ];
    assert_eq!(load_scripts(&sources, &[]).unwrap().changes[0].text, "3");
    let sources = vec![
        "var n=1".into(),
        "(n=4)".into(),
        "document.getElementById('result').textContent=n".into(),
    ];
    assert_eq!(load_scripts(&sources, &[]).unwrap().changes[0].text, "4");
}

/** Later-source failures preserve their own source coordinates rather than the preceding EOF. */
#[test]
fn later_script_separator_errors_preserve_utf16_and_source_indices() {
    let source = "let label='😀' let n=2";
    let sources = vec!["var first=1".into(), source.into()];
    let error = load_scripts(&sources, &[]).unwrap_err();
    assert_eq!(error.code, "script_syntax_error");
    assert_eq!(error.input_kind.as_deref(), Some("script"));
    let location = error.location.unwrap();
    assert_eq!(location.script_index, Some(2));
    assert_eq!(
        (location.byte_offset, location.line, location.column),
        (sources[0].len() + 1 + source.find("let n").unwrap(), 1, 16)
    );
}

/** CRLF counts once and astral literal content consumes two UTF-16 columns on its source line. */
#[test]
fn separator_errors_retain_original_byte_offsets_and_utf16_columns() {
    let source = "// note\r\nlet label='😀' let n=2";
    let location = syntax(source).location.unwrap();
    assert_eq!(
        (location.byte_offset, location.line, location.column),
        (source.find("let n").unwrap(), 2, 16)
    );
}

/** Parsing still covers never-called functions and never-executed branches before any candidate runs. */
#[test]
fn unreachable_and_later_sources_are_still_completely_validated() {
    for source in [
        "function unused(){return\nlet n=}",
        "if(false){let n=1 let m=2}",
    ] {
        syntax(source);
    }
    let sources = vec![
        "document.getElementById('result').textContent='candidate'".into(),
        "function unused(){return\nlet n=}".into(),
    ];
    let error = load_scripts(&sources, &[]).unwrap_err();
    assert_eq!(error.code, "script_syntax_error");
    assert_eq!(error.location.unwrap().script_index, Some(2));
}

/** The unchanged handler policy admits named calls with its already supported optional punctuation. */
#[test]
fn named_handlers_keep_trailing_comment_and_line_terminator_behavior() {
    let source = "function f(){document.getElementById('result').textContent='done'}";
    for handler in [
        "f()",
        "f();",
        "f\n()",
        "f() // note",
        "f() /* note */",
        "/*😀*/\r\nf() // note\u{2028}",
    ] {
        let state = load(source, &[]).unwrap().state;
        assert_eq!(
            event(source, handler, state, &[]).unwrap().changes[0].text,
            "done"
        );
    }
}

/** Program insertion does not widen event handlers into general source evaluation. */
#[test]
fn handlers_remain_one_direct_named_call() {
    let source = "function f(){}";
    for handler in ["f()\nf()", "let n=1", "(()=>f())()", "f.call(null)"] {
        let error = event(source, handler, load(source, &[]).unwrap().state, &[]).unwrap_err();
        assert_eq!(error.code, "unsupported_feature", "{handler}");
        assert_eq!(error.input_kind.as_deref(), Some("handler"));
    }
}

/** Handler failures keep handler coordinates and never expose page-script locations instead. */
#[test]
fn handler_errors_keep_their_original_utf16_location_and_input_kind() {
    let source = "function f(){}";
    let handler = "/*😀*/\r\nmissing() // note";
    let error = event(source, handler, load(source, &[]).unwrap().state, &[]).unwrap_err();
    let location = error.location.unwrap();
    assert_eq!(error.input_kind.as_deref(), Some("handler"));
    assert_eq!(
        (location.byte_offset, location.line, location.column),
        ("/*😀*/\r\n".len(), 2, 1)
    );
}

/** New punctuation handling cannot admit currently unsupported statement forms in dead code. */
#[test]
fn unsupported_statement_boundaries_remain_explicit() {
    for source in [
        "if(false){throw\n1}",
        "if(false){do{}while(false)}",
        "if(false){class A{}}",
    ] {
        let error = load(source, &[]).unwrap_err();
        assert_eq!(error.code, "unsupported_feature", "{source}");
        assert_eq!(error.phase.as_deref(), Some("parse"));
    }
}
