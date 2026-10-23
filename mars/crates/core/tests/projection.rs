/** Shared inert DOM contracts reject malformed candidates independently of every engine. */
use mars_core::{
    constants::{DOCUMENT_DEPTH, ID_BYTES, TEXT_BYTES},
    dom::{ScriptDocument, ScriptNode},
};

/** Construct ordinary inert nodes with no native handles or capability grants. */
fn node(local_id: usize, parent: Option<usize>) -> ScriptNode {
    ScriptNode {
        local_id,
        html_id: format!("node-{local_id}"),
        parent,
        value: None,
        text_writable: true,
        click_listener: false,
        inline_handler: None,
    }
}

/** Every projection needs a bounded nonempty page identity before interpreter admission. */
#[test]
fn rejects_empty_and_oversized_projection_identities() {
    for identity in [String::new(), "x".repeat(ID_BYTES + 1)] {
        let document = ScriptDocument {
            identity,
            nodes: vec![node(0, None)],
        };
        assert_eq!(document.validate().unwrap_err().code, "invalid_request");
    }
}

/** Parent references must resolve to earlier nodes, ruling out cycles and forward references. */
#[test]
fn rejects_forward_parent_references() {
    let document = ScriptDocument {
        identity: "page".into(),
        nodes: vec![node(0, Some(1)), node(1, None)],
    };
    assert_eq!(document.validate().unwrap_err().code, "invalid_request");
}

/** Projection identities cannot alias two distinct source nodes. */
#[test]
fn rejects_duplicate_local_identities() {
    let document = ScriptDocument {
        identity: "page".into(),
        nodes: vec![node(0, None), node(0, None)],
    };
    assert_eq!(document.validate().unwrap_err().code, "invalid_request");
}

/** The last accepted ancestor chain and first rejected chain retain their existing bound. */
#[test]
fn enforces_projection_depth_at_the_shared_boundary() {
    let mut document = ScriptDocument {
        identity: "page".into(),
        nodes: (0..DOCUMENT_DEPTH)
            .map(|id| node(id, id.checked_sub(1)))
            .collect(),
    };
    document.validate().unwrap();
    document
        .nodes
        .push(node(DOCUMENT_DEPTH, Some(DOCUMENT_DEPTH - 1)));
    assert_eq!(document.validate().unwrap_err().code, "limit_exceeded");
}

/** Input values use the same UTF-8 byte budget before any engine receives the projection. */
#[test]
fn enforces_projection_input_text_at_the_shared_boundary() {
    let mut document = ScriptDocument {
        identity: "page".into(),
        nodes: vec![node(0, None)],
    };
    document.nodes[0].value = Some("x".repeat(TEXT_BYTES));
    document.validate().unwrap();
    document.nodes[0].value.as_mut().unwrap().push('x');
    assert_eq!(document.validate().unwrap_err().code, "limit_exceeded");
}

/** A candidate replacement removes descendants without recycling surviving identities. */
#[test]
fn emits_a_text_proposal_and_updates_only_the_inert_tree() {
    let mut document = ScriptDocument {
        identity: "page".into(),
        nodes: vec![node(0, None), node(1, Some(0)), node(2, None)],
    };
    document.validate().unwrap();
    let change = document.set_text(0, "replacement").unwrap();
    assert_eq!(change.html_id, "node-0");
    assert_eq!(change.text, "replacement");
    assert_eq!(
        document
            .nodes
            .iter()
            .map(|node| node.local_id)
            .collect::<Vec<_>>(),
        vec![0, 2]
    );
    document.validate().unwrap();
}
