/** Reference comparisons use stable language identities before considering unsupported conversion. */
use super::*;
use mars::{
    document::{parse, script_document},
    v9::{HostKind, HostRef, JsValue, StorageState, load_document_scripts},
};

/** Equal aliases, separate allocations and cross-brand references never compare by their contents. */
#[test]
fn object_array_function_and_intrinsic_comparisons_use_identity() {
    let source = "const object={};const alias=object;const other={};const array=[];const arrayAlias=array;function fn(){}const bound=fn.bind(null);const split='a'.split;";
    let expression = "(object==alias)+' '+(object==other)+' '+(array==arrayAlias)+' '+(array==[])+' '+(fn==fn)+' '+(fn==bound)+' '+(split=='b'.split)+' '+(object==array)+' '+(object==fn)+' '+(array==fn)";
    assert_eq!(
        result(source, expression),
        "true false true false true false true false false false"
    );
}

/** Nullish comparisons against every reference kind terminate without conversion in either operand order. */
#[test]
fn nullish_reference_fast_paths_do_not_invoke_conversion_methods() {
    let source = "var calls=0;const object={valueOf:function(){calls++;return 0;},toString:function(){calls++;return '';}};const array=[];function fn(){}";
    for reference in ["object", "array", "fn"] {
        assert_eq!(
            result(
                source,
                &format!(
                    "({reference}==null)+' '+(null=={reference})+' '+({reference}==undefined)+' '+(undefined=={reference})+' '+calls"
                )
            ),
            "false false false false 0"
        );
    }
}

/** Real document and element lookups retain identity without implicit native calls or authority. */
#[test]
fn projected_hosts_compare_aliases_brands_and_nullish_values() {
    let document = script_document(
        &parse("<p id=out></p><p id=other></p>").unwrap(),
        "equality-page",
    )
    .unwrap();
    let source = "const out=document.getElementById('out');const other=document.getElementById('other');out.textContent=(out==document.getElementById('out'))+' '+(out==other)+' '+(out==document)+' '+(out=={})+' '+(out==function(){})+' '+(out==null)+' '+(undefined==out)+' '+(document==document);";
    let executed =
        load_document_scripts(&[source.into()], document, StorageState::default()).unwrap();
    assert_eq!(
        executed.changes[0].text,
        "true false false false false false false true"
    );
}

/** Equal numeric IDs in distinct arenas or host brands cannot manufacture a shared identity. */
#[test]
fn public_value_fast_paths_keep_arena_document_and_host_brand_identity_separate() {
    let object = JsValue::Object(7);
    let function = JsValue::Function(7);
    let element = host("page-a", HostKind::Element { local_id: 7 });
    let event = host("page-a", HostKind::Event { id: 7 });
    for (left, right) in [
        (&object, &function),
        (&object, &element),
        (&function, &event),
        (&element, &event),
    ] {
        assert!(!left.abstract_equal(right).unwrap());
        assert!(!right.abstract_equal(left).unwrap());
    }
    assert!(element.abstract_equal(&element.clone()).unwrap());
    assert!(
        !element
            .abstract_equal(&host("page-b", HostKind::Element { local_id: 7 }))
            .unwrap()
    );
}

/** Host identities remain plain scoped data; this test does not project or grant them native access. */
fn host(document: &str, kind: HostKind) -> JsValue {
    JsValue::Host(HostRef {
        document: document.into(),
        kind,
    })
}

/** Retained object and callable aliases preserve their comparison meaning across private serialization. */
#[test]
fn serialized_event_state_retains_object_and_callable_aliases() {
    let source = "const object={};const alias=object;function fn(){}const bound=fn.bind(null);const saved=bound;function check(){document.getElementById('out').textContent=(object==alias)+' '+(bound==saved)+' '+(fn!=bound);}";
    let state =
        serde_json::from_value(serde_json::to_value(load(source, &[]).unwrap().state).unwrap())
            .unwrap();
    let executed = mars::v9::event(source, "check()", state, &[]).unwrap();
    assert_eq!(executed.changes[0].text, "true true true");
}
