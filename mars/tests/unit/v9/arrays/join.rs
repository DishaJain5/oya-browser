/** Join converts supported primitives while preserving separators for every snapshot position. */
use super::*;

/** Holes and nullish values contribute empty fields; other primitives use ECMAScript formatting. */
#[test]
fn join_preserves_sparse_nullish_fields_and_formats_supported_primitives() {
    let source = "const a=[];a.length=6;a[1]=undefined;a[2]=null;a[3]=true;a[4]=7;a[5]='Rust 🐙';";
    assert_eq!(result(source, "a.join()"), ",,,true,7,Rust 🐙");
    assert_eq!(result("", "[1,2].join(null)"), "1null2");
    assert_eq!(result("", "[1,2].join('')"), "12");
}

/** Separator conversion occurs even when no positions will be visited. */
#[test]
fn empty_array_join_still_validates_separator_conversion() {
    assert_eq!(result("", "[].join(undefined)"), "");
    assert_eq!(
        execute("[].join({});").unwrap_err().code,
        "unsupported_feature"
    );
}

/** General ToPrimitive, nested-array conversion and function serialization stay explicitly unavailable. */
#[test]
fn join_refuses_object_function_and_nested_array_elements() {
    for source in ["[{}].join();", "[[1]].join();", "[()=>1].join();"] {
        assert_eq!(
            execute(source).unwrap_err().code,
            "unsupported_feature",
            "{source}"
        );
    }
}

/** A joined value may reach the exact UTF-16 cap, while the next append fails before growing it further. */
#[test]
fn join_checks_utf16_output_budget_before_appending() {
    let value = "x".repeat(JS_STRING_UNITS / 2);
    assert_eq!(
        result(&format!("const s='{value}';"), "[s,s].join('')").len(),
        JS_STRING_UNITS
    );
    assert_eq!(
        execute(&format!("const s='{value}';[s,s].join(',');"))
            .unwrap_err()
            .code,
        "limit_exceeded"
    );
}
