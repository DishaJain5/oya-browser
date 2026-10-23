/** Canonical methods retain conversion order and receiver rules through ordinary call adaptation. */
use super::*;
/** Generic supported primitive receivers convert while nullish receivers fail before other conversions. */
#[test]
fn generic_string_methods_convert_supported_primitive_receivers() {
    assert_eq!(
        result("const trim=''.trim;", "trim.call(123)+' '+trim.call(true)"),
        "123 true"
    );
    assert_eq!(
        result(
            "const split=''.split;const values=split.call(123,2);",
            "values.length+' '+values[0]+' '+values[1]"
        ),
        "2 1 3"
    );
    for source in [
        "''.trim.call(null);",
        "''.trim.call(undefined);",
        "''.split.call(null,{},{});",
    ] {
        assert_eq!(
            execute(source).unwrap_err().code,
            "script_execution_error",
            "{source}"
        );
    }
}
/** Limit conversion precedes separator conversion, and separator conversion precedes zero-limit success. */
#[test]
fn split_conversion_order_matches_pinned_v8() {
    let error = execute("'abc'.split({},{});").unwrap_err();
    assert_eq!(error.code, "unsupported_feature");
    assert!(
        error.message.contains("object-to-primitive conversion"),
        "{}",
        error.message
    );
    for source in [
        "'abc'.split({},0);",
        "'abc'.split(undefined,{});",
        "''.split.call({},',',{});",
    ] {
        assert_eq!(
            execute(source).unwrap_err().code,
            "unsupported_feature",
            "{source}"
        );
    }
}
/** Separator primitives use JavaScript rendering and undefined retains the omitted-separator shortcut. */
#[test]
fn split_separator_primitives_use_javascript_string_conversion() {
    assert_eq!(
        result(
            "const values='anullb'.split(null);",
            "values[0]+' '+values[1]"
        ),
        "a b"
    );
    assert_eq!(
        result(
            "const values='atrueb'.split(true);",
            "values[0]+' '+values[1]"
        ),
        "a b"
    );
    assert_eq!(
        result("const values='a2b'.split(2);", "values[0]+' '+values[1]"),
        "a b"
    );
}
/** Canonical methods remain true callable values with fixed metadata and ordinary adapters. */
#[test]
fn extracted_and_bound_string_methods_keep_identity_and_metadata() {
    assert_eq!(
        result(
            "const trim=''.trim;const bound=trim.bind(' a ');",
            "bound()+' '+trim.name+' '+trim.length+' '+''.split.length"
        ),
        "a trim 0 2"
    );
    assert_eq!(
        result("", "'a'.trim==='b'.trim&&'a'.split==='b'.split"),
        "true"
    );
    assert_eq!(
        result(
            "const split=''.split;const values=split.apply('a,b',[',']);",
            "values[1]"
        ),
        "b"
    );
}
/** Unsupported coercion never silently renders objects, functions or host references. */
#[test]
fn object_function_and_host_coercion_fail_explicitly() {
    for source in [
        "''.trim.call({});",
        "''.trim.call(function(){});",
        "'abc'.split(function(){});",
        "'abc'.split(',',function(){});",
    ] {
        assert_eq!(
            execute(source).unwrap_err().code,
            "unsupported_feature",
            "{source}"
        );
    }
    use mars::{
        document::ScriptDocument,
        v9::{StorageState, load_document_scripts},
    };
    assert_eq!(
        load_document_scripts(
            &["''.trim.call(document);".into()],
            ScriptDocument {
                identity: "strings".into(),
                nodes: Vec::new()
            },
            StorageState::default()
        )
        .unwrap_err()
        .code,
        "unsupported_feature"
    );
}
