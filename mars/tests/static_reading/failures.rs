/** Failed bounded candidates preserve the current context and committed page. */
use super::*;

/** Larger static capacity does not grant an unbounded measurement budget or publish partial document output. */
#[test]
fn exhausted_layout_work_keeps_previous_page_and_context() {
    let good = fixture("<p id=out>Committed</p>");
    let paragraph = "word ".repeat(3000);
    let bad = fixture(&format!("<p>{paragraph}</p><p>{paragraph}</p>"));
    let mut runtime = runtime(&[&good.url, &bad.url]);
    runtime
        .set_render_context(RenderContext {
            width: 640.0,
            height: 400.0,
            appearance: Appearance::Dark,
        })
        .unwrap();
    let page = call(&mut runtime, "page.navigate", json!({"url":good.url})).unwrap();
    let context = runtime.snapshot().render_context;
    let error = call(&mut runtime, "page.navigate", json!({"url":bad.url})).unwrap_err();
    assert_eq!(error.code, "limit_exceeded");
    assert!(error.message.contains("layout work"), "{}", error.message);
    assert!(error.previous_page_retained);
    assert_eq!(runtime.snapshot().render_context, context);
    assert_eq!(call(&mut runtime, "page.inspect", json!({})).unwrap(), page);
}

/** An unsupported required script is never skipped to make a styled document appear committed. */
#[test]
fn synthetic_unsupported_script_keeps_the_previous_page() {
    let good = fixture("<p id=out>Committed</p>");
    let bad = fixture(
        "<style>body{font:16px/1.6 system-ui}</style><p>Candidate</p><script>unsupported();</script>",
    );
    let mut runtime = runtime(&[&good.url, &bad.url]);
    let page = call(&mut runtime, "page.navigate", json!({"url":good.url})).unwrap();
    let error = call(&mut runtime, "page.navigate", json!({"url":bad.url})).unwrap_err();
    assert!(error.previous_page_retained);
    assert_eq!(call(&mut runtime, "page.inspect", json!({})).unwrap(), page);
}
