/** Unsupported and forged private layout inputs fail before native geometry is published. */
use super::*;

/** Unsupported flex modes and unresolved values cannot silently turn into block flow. */
#[test]
fn unsupported_layout_declarations_fail_closed() {
    for css in [
        "display:inline-flex",
        "display:flex;flex-direction:row-reverse",
        "display:flex;flex-wrap:wrap-reverse",
        "display:flex;flex-direction:column;flex-wrap:wrap",
        "display:flex;gap:10%",
        "display:flex;flex-basis:calc(100px - 2px)",
        "display:flex;align-items:baseline",
    ] {
        assert_eq!(
            compute_styles(&parse(&format!("<div style='{css}'></div>")).unwrap(), &[])
                .unwrap_err()
                .code,
            "unsupported_feature",
            "{css}"
        );
    }
}

/** Mixed anonymous text runs and flex table participation require unimplemented layout models. */
#[test]
fn unsupported_flex_tree_shapes_fail_explicitly() {
    for body in [
        "<div style='display:flex'>before<div>child</div></div>",
        "<div style='display:flex'><table><tr><td>cell</td></tr></table></div>",
        "<table style='display:flex'><tr><td>cell</td></tr></table>",
    ] {
        let result = compute_styles(&parse(body).unwrap(), &[])
            .and_then(|document| render(&document, 500.0));
        assert_eq!(result.unwrap_err().code, "unsupported_feature", "{body}");
    }
}

/** Private wire data cannot bypass CSS numeric ranges and poison native geometry. */
#[test]
fn forged_computed_dimensions_are_rejected_at_the_render_boundary() {
    for (field, value) in [("width", -1.0), ("font_size", 5000.0), ("border", 99.0)] {
        let mut wire = serde_json::to_value(parse("<div></div>").unwrap()).unwrap();
        wire["nodes"][0]["style"] = serde_json::json!({field:value});
        let document: Document = serde_json::from_value(wire).unwrap();
        assert_eq!(
            render(&document, 500.0).unwrap_err().code,
            "invalid_request",
            "{field}"
        );
    }
}

/** Renderer traversal validates parent topology and unique IDs before following any edges. */
#[test]
fn malformed_private_trees_are_rejected_instead_of_looping_or_dropping_nodes() {
    let document = parse("<div><p>child</p></div>").unwrap();
    let mut cycle = document.clone();
    cycle.nodes[0].parent = Some(cycle.nodes[1].local_id);
    let mut duplicate = document.clone();
    duplicate.nodes[1].local_id = duplicate.nodes[0].local_id;
    let mut orphan = document;
    orphan.nodes[1].parent = Some(9999);
    for forged in [cycle, duplicate, orphan] {
        assert_eq!(render(&forged, 500.0).unwrap_err().code, "invalid_request");
    }
}

/** A private request cannot evade the parser's independent node-work allowance. */
#[test]
fn private_document_node_overflow_is_bounded_before_layout() {
    let mut document = parse("<div></div>").unwrap();
    let template = document.nodes[0].clone();
    document.nodes = (0..=mars::constants::NODES)
        .map(|id| {
            let mut node = template.clone();
            node.local_id = id;
            node
        })
        .collect();
    assert_eq!(render(&document, 500.0).unwrap_err().code, "limit_exceeded");
}
