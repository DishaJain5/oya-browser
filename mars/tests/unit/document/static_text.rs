/** Static reading keeps complete source identities while byte, control and measured-work limits stay independent. */
use mars::{
    constants::{STATIC_TEXT_NODE_BYTES, STATIC_TEXT_TOTAL_BYTES, TEXT_BYTES},
    document::{
        TextChange, apply, compute_styles, parse, render, script_document, set_value,
        validate_content,
    },
};

/** A long readable paragraph remains one element with its complete original UTF-8 contents. */
#[test]
fn long_static_paragraph_keeps_complete_text_and_one_identity() {
    let text = "Readable beginning café 🐙 middle and final sentence. ".repeat(70);
    assert!(text.len() > TEXT_BYTES);
    let document = parse(&format!("<p id=paragraph>{text}</p>")).unwrap();
    assert_eq!(document.nodes.len(), 1);
    assert_eq!(document.nodes[0].local_id, 0);
    assert_eq!(document.nodes[0].text, text);
    let projection = script_document(&document, "static-page").unwrap();
    assert!(projection.nodes[0].value.is_none());
}

/** Static UTF-8 bytes have an exact per-node cap rather than a scalar-count approximation. */
#[test]
fn static_node_limit_accepts_exact_utf8_bytes_and_refuses_the_next_byte() {
    let text = "é".repeat(STATIC_TEXT_NODE_BYTES / "é".len());
    assert_eq!(
        parse(&format!("<p>{text}</p>")).unwrap().nodes[0].text,
        text
    );
    assert_eq!(
        parse(&format!("<p>{text}x</p>")).unwrap_err().code,
        "limit_exceeded"
    );
}

/** Many individually valid paragraphs cannot evade the total stored-text byte budget. */
#[test]
fn aggregate_static_limit_counts_each_stored_node_once() {
    let half = "x".repeat(STATIC_TEXT_NODE_BYTES);
    let document = parse(&format!("<p>{half}</p><p>{half}</p>")).unwrap();
    assert_eq!(
        document
            .nodes
            .iter()
            .map(|node| node.text.len())
            .sum::<usize>(),
        STATIC_TEXT_TOTAL_BYTES
    );
    assert_eq!(
        parse(&format!("<p>{half}</p><p>{half}</p><p>x</p>"))
            .unwrap_err()
            .code,
        "limit_exceeded"
    );
}

/** Comments cannot bypass accumulated element bytes or create chunk nodes to hold oversized text. */
#[test]
fn split_source_text_checks_combined_node_limit_before_append() {
    let half = "x".repeat(STATIC_TEXT_NODE_BYTES / 2);
    let document = parse(&format!("<p>{half}<!-- split -->{half}</p>")).unwrap();
    assert_eq!(document.nodes.len(), 1);
    assert_eq!(document.nodes[0].text.len(), STATIC_TEXT_NODE_BYTES);
    assert_eq!(
        parse(&format!("<p>{half}<!-- split -->{half}x</p>"))
            .unwrap_err()
            .code,
        "limit_exceeded"
    );
}

/** Numeric references can expand UTF-8 bytes and still stop at the decoded node allowance. */
#[test]
fn expanded_static_entities_obey_decoded_bytes() {
    let entities = "&#x1f419;".repeat(STATIC_TEXT_NODE_BYTES / "🐙".len());
    let document = parse(&format!("<p>{entities}</p>")).unwrap();
    assert_eq!(
        document.nodes[0].text,
        "🐙".repeat(STATIC_TEXT_NODE_BYTES / "🐙".len())
    );
    assert_eq!(
        parse(&format!("<p>{entities}x</p>")).unwrap_err().code,
        "limit_exceeded"
    );
}

/** The larger text-token decoder does not expand title, attributes or ordinary/protected controls. */
#[test]
fn titles_attributes_and_native_control_data_keep_the_editable_limit() {
    let large = "x".repeat(TEXT_BYTES + 1);
    for source in [
        format!("<title>{large}</title><p>body</p>"),
        format!("<p aria-label='{large}'>body</p>"),
        format!("<input value='{large}'>"),
        format!("<input type=password value='{large}'>"),
        format!("<textarea>{large}</textarea>"),
        format!("<button>{large}</button>"),
        format!("<select><option>{large}</option></select>"),
        format!("<button><span>{large}</span></button>"),
        format!("<label for=entry>{large}</label><input id=entry>"),
    ] {
        assert_eq!(parse(&source).unwrap_err().code, "limit_exceeded");
    }
}

/** Replacing already-long static content never raises the independent script/native-write allowance. */
#[test]
fn static_capacity_does_not_expand_text_or_value_mutations() {
    let text = "x".repeat(TEXT_BYTES + 1);
    let document = parse(&format!("<p id=long>{text}</p><input id=entry>")).unwrap();
    assert_eq!(
        apply(
            &document,
            &[TextChange {
                html_id: "long".into(),
                text: text.clone()
            }]
        )
        .unwrap_err()
        .code,
        "limit_exceeded"
    );
    assert_eq!(
        set_value(&document, 1, &text).unwrap_err().code,
        "limit_exceeded"
    );
    let mut projection = script_document(&document, "static-page").unwrap();
    assert_eq!(
        projection.set_text(0, &text).unwrap_err().code,
        "limit_exceeded"
    );
    assert_eq!(document.nodes[0].text, text);
}

/** Forged documents cannot bypass semantic validation by skipping source parsing. */
#[test]
fn forged_static_overflow_is_refused_before_style_render_or_projection() {
    let mut document = parse("<p id=paragraph>valid</p>").unwrap();
    document.nodes[0].text = "x".repeat(STATIC_TEXT_NODE_BYTES + 1);
    assert!(validate_content(&document).is_err());
    assert!(compute_styles(&document, &[]).is_err());
    assert!(render(&document, 640.0).is_err());
    assert!(script_document(&document, "static-page").is_err());
    assert!(apply(&document, &[]).is_err());
}

/** A malicious parent cycle is rejected before descendant-derived native label traversal starts. */
#[test]
fn forged_cycles_fail_before_native_label_recursion() {
    let mut document = parse("<button><span>caption</span></button>").unwrap();
    document.nodes[0].parent = Some(1);
    assert_eq!(
        validate_content(&document).unwrap_err().code,
        "invalid_request"
    );
}

/** UTF-8 and combining scalars remain complete across narrow long-word wrapping without fake nodes. */
#[test]
fn narrow_long_word_wrapping_preserves_unicode_and_source_identity() {
    let word = "e\u{301}🐙".repeat(320);
    let document = compute_styles(&parse(&format!("<p>{word}</p>")).unwrap(), &[]).unwrap();
    let display = render(&document, 280.0).unwrap();
    let item = display
        .items
        .iter()
        .find(|item| item.local_id == 0)
        .unwrap();
    assert!(item.lines.len() > 1);
    assert_eq!(
        item.lines
            .iter()
            .map(|line| line.text.as_str())
            .collect::<String>(),
        word
    );
    assert_eq!(document.nodes.len(), 1);
}

/** Exhausted native shaping work is an explicit error rather than partial display or dropped text. */
#[test]
fn excessive_measured_prefix_work_fails_without_changing_the_document() {
    let word = "W".repeat(STATIC_TEXT_NODE_BYTES);
    let document = compute_styles(
        &parse(&format!("<p style='font-size:72px'>{word}</p>")).unwrap(),
        &[],
    )
    .unwrap();
    let error = render(&document, 280.0).unwrap_err();
    assert_eq!(error.code, "limit_exceeded");
    assert!(error.message.contains("layout work"));
    assert_eq!(document.nodes[0].text, word);
}

/** A deserialized candidate must obey the same aggregate budget as independently parsed paragraphs. */
#[test]
fn forged_aggregate_overflow_cannot_enter_public_document_operations() {
    let mut document = parse("<p id=a></p><p id=b></p><p id=c></p>").unwrap();
    for node in &mut document.nodes {
        node.text = "x".repeat(STATIC_TEXT_NODE_BYTES);
    }
    assert!(validate_content(&document).is_err());
    assert!(compute_styles(&document, &[]).is_err());
    assert!(render(&document, 640.0).is_err());
    assert!(script_document(&document, "static-page").is_err());
    assert!(apply(&document, &[]).is_err());
}
