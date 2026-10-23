/** Public projected-DOM lookup rules use the shared real-document fixture. */
use super::*;

/** Surviving registration or event roots cannot silently retain a host target removed by text replacement. */
#[test]
fn descendant_removal_refuses_detached_listener_and_retained_event_targets() {
    let document =
        document("<div id=parent><button id=button type=button>Click</button></div><p id=out></p>");
    for source in [
        "document.getElementById('button').addEventListener('click',()=>{});document.getElementById('parent').textContent='removed';",
        "let saved=null;function remove(e){saved=e;this.removeEventListener('click',remove);document.getElementById('parent').textContent='removed';}document.getElementById('button').addEventListener('click',remove);",
    ] {
        let loaded =
            load_document_scripts(&[source.into()], document.clone(), StorageState::default());
        let error = match loaded {
            Err(error) => error,
            Ok(loaded) => click_document_scripts(
                &[source.into()],
                document.get_by_id("button").unwrap(),
                loaded.state,
                document.clone(),
                StorageState::default(),
            )
            .unwrap_err(),
        };
        assert_eq!(error.code, "unsupported_feature");
    }
}

/** Lookup uses actual author identifiers, primitive DOMString conversion and stable host identities. */
#[test]
fn document_lookup_converts_primitives_and_returns_null_for_missing_or_empty_ids() {
    let document = document(
        "<p id=out></p><p id=7></p><p id=undefined></p><p id=true></p><p id=same>first</p><p id=same>second</p>",
    );
    let executed = load(
        "const lookup=document.getElementById;const first=lookup.call(document,'same');document.getElementById('out').textContent=(first===document.getElementById('same'))+' '+(document.getElementById('')===null)+' '+(document.getElementById('absent')===null)+' '+(document.getElementById(7)!==null)+' '+(document.getElementById()!==null)+' '+(document.getElementById(true)!==null);",
        &document,
    );
    assert_eq!(executed.changes[0].text, "true true true true true true");
}

/** Retained input references read the current candidate value instead of the load-time string. */
#[test]
fn cached_input_references_observe_current_nonsecret_field_values() {
    let source = "const field=document.getElementById('name');const out=document.getElementById('out');document.getElementById('button').addEventListener('click',()=>{out.textContent=field.value||'world';});";
    let mut document = page();
    let mut state = load(source, &document).state;
    for name in ["Lin", "Rust 🐙", ""] {
        document
            .nodes
            .iter_mut()
            .find(|node| node.html_id == "name")
            .unwrap()
            .value = Some(name.into());
        let executed = click(source, state, &document);
        assert_eq!(
            executed.changes[0].text,
            if name.is_empty() { "world" } else { name }
        );
        state = executed.state;
    }
}

/** Secret access is refused rather than returning a fake empty value or leaking protected bytes. */
#[test]
fn protected_field_values_and_unsupported_dom_properties_fail_explicitly() {
    for (html, source) in [
        (
            "<input id=secret type=password value=private>",
            "const value=document.getElementById('secret').value;",
        ),
        (
            "<input id=secret type=hidden value=private>",
            "const value=document.getElementById('secret').value;",
        ),
        (
            "<p id=out>text</p>",
            "const value=document.getElementById('out').innerHTML;",
        ),
    ] {
        let error =
            load_document_scripts(&[source.into()], document(html), StorageState::default())
                .unwrap_err();
        assert_eq!(error.code, "unsupported_feature");
        assert!(!error.message.contains("private"));
    }
}

/** Every canonical host method validates its receiver even when detached through ordinary callable adapters. */
#[test]
fn detached_host_methods_require_the_matching_document_element_or_event_brand() {
    for source in [
        "const lookup=document.getElementById;lookup('out');",
        "document.getElementById.call({},'out');",
        "const b=document.getElementById('button');b.addEventListener.call(document,'click',()=>{});",
    ] {
        assert_eq!(
            load_document_scripts(&[source.into()], page(), StorageState::default())
                .unwrap_err()
                .code,
            "script_execution_error"
        );
    }
}

/** Detachment becomes visible immediately; unsupported retained detached references cannot publish a candidate. */
#[test]
fn descendant_removal_updates_lookup_and_refuses_retained_detached_host_state() {
    let document = document(
        "<div id=parent><p id=child>nested</p></div><p id=out></p><button id=button type=button>Click</button>",
    );
    let good = load(
        "document.getElementById('parent').textContent='new';document.getElementById('out').textContent=document.getElementById('child')===null;",
        &document,
    );
    assert_eq!(good.changes[1].text, "true");
    for retained in [
        "const retained=document.getElementById('child');",
        "const box={child:document.getElementById('child')};",
    ] {
        let source = format!("{retained}document.getElementById('parent').textContent='new';");
        assert_eq!(
            load_document_scripts(&[source], document.clone(), StorageState::default())
                .unwrap_err()
                .code,
            "unsupported_feature"
        );
    }
}
