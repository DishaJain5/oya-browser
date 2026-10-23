/** Nested sizing probes consume one shared budget instead of multiplying recursive layout work. */
use mars::{
    constants::TEXT_BYTES,
    document::{compute_styles, parse, render},
};

/** Repeated measurements use real bounded text and nested auto-height columns. */
fn nested_columns(levels: usize) -> String {
    let mut html = "<div style='display:flex;flex-direction:column'>".repeat(levels);
    let word = "a".repeat(TEXT_BYTES);
    html.push_str(&format!("<p>{word}</p><p>{word}</p>"));
    html.push_str(&"</div>".repeat(levels));
    html
}

/** Deep valid trees fail with the work limit before unbounded repeated native shaping. */
#[test]
fn nested_flex_measurement_and_final_passes_share_the_same_work_budget() {
    let document = compute_styles(&parse(&nested_columns(30)).unwrap(), &[]).unwrap();
    let error = render(&document, 640.0).unwrap_err();
    assert_eq!(error.code, "limit_exceeded");
    assert!(error.message.contains("layout work"), "{error:?}");
}

/** A smaller equivalent workload still renders and retains both leaf elements. */
#[test]
fn bounded_nested_flex_measurement_keeps_real_leaf_content() {
    let document = compute_styles(&parse(&nested_columns(3)).unwrap(), &[]).unwrap();
    let display = render(&document, 640.0).unwrap();
    assert_eq!(
        display.items.iter().filter(|item| item.tag == "p").count(),
        2
    );
    assert!(display.height > 0.0);
}
