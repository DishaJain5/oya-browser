/** Missing ToPrimitive remains an explicit located failure and never leaks a partially changed candidate. */
use super::*;
use mars::{
    document::{parse, script_document},
    v9::{StorageState, click_document_scripts, load_document_scripts, load_scripts},
};

/** Number, String and Boolean comparisons require genuine conversion for all supported reference brands. */
#[test]
fn conversion_required_comparisons_refuse_both_orders_and_both_operators() {
    for reference in ["({})", "[]", "(function(){})"] {
        for primitive in ["0", "''", "false", "true"] {
            for operator in ["==", "!="] {
                for expression in [
                    format!("{reference}{operator}{primitive}"),
                    format!("{primitive}{operator}{reference}"),
                ] {
                    let error = execute(&format!("const result={expression};")).unwrap_err();
                    assert_eq!(error.code, "unsupported_feature", "{expression}");
                    assert_eq!(error.phase.as_deref(), Some("execute"));
                    assert!(error.location.is_some());
                }
            }
        }
    }
}

/** A host reference is not a numeric fallback, while its nullish fast path stays supported. */
#[test]
fn projected_host_conversion_is_located_and_does_not_echo_page_values() {
    for expression in [
        "document==0",
        "''!=document",
        "false==document.getElementById('out')",
    ] {
        let document = script_document(
            &parse("<p id=out>private marker</p>").unwrap(),
            "equality-page",
        )
        .unwrap();
        let error = load_document_scripts(
            &[format!("const result={expression};")],
            document,
            StorageState::default(),
        )
        .unwrap_err();
        assert_eq!(error.code, "unsupported_feature");
        assert_eq!(error.phase.as_deref(), Some("execute"));
        assert_eq!(error.location.unwrap().script_index, Some(1));
        assert!(!error.message.contains("private marker"));
    }
}

/** Comparison failure points to the binary token in the original ordered source entry. */
#[test]
fn ordered_script_conversion_errors_retain_exact_operator_coordinates() {
    let first = "const object={};";
    let second = "\nconst result = object == 1;";
    let error = load_scripts(&[first.into(), second.into()], &[]).unwrap_err();
    let location = error.location.unwrap();
    assert_eq!(location.script_index, Some(2));
    assert_eq!(
        location.byte_offset,
        first.len() + 1 + second.find("==").unwrap()
    );
    assert_eq!((location.line, location.column), (2, 23));
    assert_eq!(error.phase.as_deref(), Some("execute"));
}

/** Malformed longer spellings cannot be split into a supported equality plus a store or unary operation. */
#[test]
fn malformed_equality_remains_rejected_even_in_unreachable_source() {
    for expression in ["1====1", "1!===1", "1== =1", "1!= =1"] {
        assert!(
            execute(&format!("if(false){{const value={expression};}}")).is_err(),
            "{expression}"
        );
    }
    assert_eq!(result("if(false){const value={}==0;}", "'kept'"), "kept");
}

/** Failed coercion discards globals, heap, closures, Storage and DOM proposals; a valid later click still works. */
#[test]
fn failed_event_conversion_preserves_committed_state_and_allows_recovery() {
    let document = script_document(&parse("<p id=out>initial</p><button id=bad type=button onclick='fail()'>Fail</button><button id=good type=button onclick='good()'>Good</button>").unwrap(), "equality-page").unwrap();
    let source = "let count=0;const box={count:0};function make(){let value=0;return ()=>++value;}const next=make();function fail(){count=9;box.count=9;next();window.localStorage.setItem('changed','yes');document.getElementById('out').textContent='partial';const object={valueOf:function(){missing();}};const bad=object==0;}function good(){count++;document.getElementById('out').textContent=count+' '+box.count+' '+next()+' '+window.localStorage.getItem('changed');}";
    let sources = [source.into()];
    let loaded =
        load_document_scripts(&sources, document.clone(), StorageState::default()).unwrap();
    let before = serde_json::to_value(&loaded.state).unwrap();
    let error = click_document_scripts(
        &sources,
        document.get_by_id("bad").unwrap(),
        loaded.state.clone(),
        document.clone(),
        loaded.storage.clone(),
    )
    .unwrap_err();
    assert_eq!(error.code, "unsupported_feature");
    assert_eq!(serde_json::to_value(&loaded.state).unwrap(), before);
    assert!(loaded.storage.local.is_empty());
    let recovered = click_document_scripts(
        &sources,
        document.get_by_id("good").unwrap(),
        loaded.state,
        document,
        loaded.storage,
    )
    .unwrap();
    assert_eq!(recovered.changes[0].text, "1 0 1 null");
    assert!(recovered.storage.local.is_empty());
}
