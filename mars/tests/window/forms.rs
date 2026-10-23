/** Actual native form widgets must retain the engine's flex geometry, values and guarded submission. */
use super::*;
#[path = "validation.rs"]
mod validation;

/** Small floating-point differences are permitted without hiding a pixel of displaced widget geometry. */
const GEOMETRY_TOLERANCE: f64 = 0.00001;

/** Semantic lookup avoids depending on parser-local integer identities. */
fn node<'a>(page: &'a Value, id: &str) -> &'a Value {
    page["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["html_id"] == id)
        .unwrap()
}

/** Every native frame must agree with its current agent-visible display item. */
fn assert_frames(window: &mut Window, wide: bool) -> Value {
    let page = window.call("page.inspect", json!({}));
    let layout = window.call("page.layout", json!({}));
    let state = window.call("window.inspect", json!({}));
    for id in [
        "user", "pass", "region", "notes", "remember", "standard", "plus", "submit",
    ] {
        let item = item(&layout, &page, id);
        let frame = state["native_frames"]
            .as_array()
            .unwrap()
            .iter()
            .find(|frame| frame["html_id"] == id)
            .unwrap();
        for key in ["x", "y", "width"] {
            close(frame[key].as_f64().unwrap(), item[key].as_f64().unwrap());
        }
        close(
            frame["height"].as_f64().unwrap(),
            item["control_height"]
                .as_f64()
                .unwrap_or_else(|| item["height"].as_f64().unwrap()),
        );
    }
    assert_columns(&layout, &page, wide);
    page
}

/** Expected allocation follows the designed 620px panel and the actual viewport, without an HTML renderer substitute. */
fn assert_columns(layout: &Value, page: &Value, wide: bool) {
    let user = item(layout, page, "user");
    let pass = item(layout, page, "pass");
    close(
        user["width"].as_f64().unwrap(),
        if wide { 289.0 } else { 550.0 },
    );
    if wide {
        close(user["y"].as_f64().unwrap(), pass["y"].as_f64().unwrap());
        close(
            pass["x"].as_f64().unwrap() - user["x"].as_f64().unwrap(),
            305.0,
        );
    } else {
        close(user["x"].as_f64().unwrap(), pass["x"].as_f64().unwrap());
        assert!(
            pass["y"].as_f64().unwrap()
                >= user["y"].as_f64().unwrap() + user["height"].as_f64().unwrap()
        );
    }
}

/** Display geometry names the same opaque target returned by inspection. */
fn item<'a>(layout: &'a Value, page: &Value, id: &str) -> &'a Value {
    layout["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["node_id"] == node(page, id)["node_id"])
        .unwrap()
}

/** A common tolerance produces useful failure output for actual versus expected platform geometry. */
fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < GEOMETRY_TOLERANCE,
        "actual {actual}, expected {expected}"
    );
}

/** Resize waits for the native callback and owned layout generation rather than assuming synchronous AppKit delivery. */
fn resize(window: &mut Window, width: usize) -> Value {
    window.call(
        "window.resize",
        json!({"width":width,"height":760,"control_epoch":1}),
    );
    for _ in 0..20 {
        let page = window.call("page.inspect", json!({}));
        if page["viewport"]["width"].as_f64() == Some(width as f64) {
            return page;
        }
        thread::sleep(Duration::from_millis(25));
    }
    panic!("native viewport did not reach {width}");
}

/** Local fields remain literal edits through the native public mutation API. */
fn type_field(window: &mut Window, id: &str, text: &str) {
    let page = window.call("page.inspect", json!({}));
    let mut params = target(&page, id);
    params["text"] = json!(text);
    window.call("page.type", params);
}

/** Actual wide/narrow frames, validation, input retention and cookie-backed submission share one native journey. */
#[test]
#[ignore = "opens the actual native macOS form window and captures flex/control geometry"]
fn native_flex_form_reflows_widgets_and_completes_the_session() {
    let mut window = Window::start();
    let capabilities = window.call("runtime.capabilities", json!({}));
    window.pids = capabilities["engines"]
        .as_array()
        .unwrap()
        .iter()
        .map(|engine| engine["pid"].as_u64().unwrap() as u32)
        .collect();
    let url = format!(
        "{}/forms",
        capabilities["demo_url"]
            .as_str()
            .unwrap()
            .trim_end_matches('/')
    );
    let page = window.call("page.navigate", json!({"url":url,"control_epoch":1}));
    let identity = page["page_id"].clone();
    resize(&mut window, 1100);
    let page = assert_frames(&mut window, true);
    window.capture();
    let error = window.request("page.click", target(&page, "submit"));
    assert_eq!(error["error"]["code"], "validation_error");
    let page = assert_frames(&mut window, true);
    assert_eq!(page["page_id"], identity);
    let layout = window.call("page.layout", json!({}));
    assert!(
        !item(&layout, &page, "user")["validation"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let capture = window.capture();
    validation::assert_ink(&mut window, &capture);
    for (id, value) in [
        ("user", "Ada"),
        ("pass", "synthetic-secret"),
        ("notes", "First line\nSecond line"),
    ] {
        type_field(&mut window, id, value);
    }
    let page = assert_frames(&mut window, true);
    assert_eq!(node(&page, "user")["value"], "Ada");
    assert!(node(&page, "pass").get("value").is_none());
    window.capture();
    let generation = page["layout_generation"].as_u64().unwrap();
    resize(&mut window, 640);
    let narrow = assert_frames(&mut window, false);
    assert_eq!(narrow["page_id"], identity);
    assert_eq!(
        node(&narrow, "user")["node_id"],
        node(&page, "user")["node_id"]
    );
    assert_eq!(node(&narrow, "user")["value"], "Ada");
    assert_eq!(node(&narrow, "notes")["value"], "First line\nSecond line");
    assert!(narrow["layout_generation"].as_u64().unwrap() > generation);
    window.capture();
    window.call(
        "window.appearance",
        json!({"appearance":"dark","control_epoch":1}),
    );
    assert_frames(&mut window, false);
    window.capture();
    window.call(
        "window.appearance",
        json!({"appearance":"light","control_epoch":1}),
    );
    let layout = window.call("page.layout", json!({}));
    let y = item(&layout, &narrow, "submit")["y"].as_f64().unwrap();
    window.call(
        "page.scroll",
        json!({"page_id":identity,"expected_revision":narrow["revision"],"y":y,"control_epoch":1}),
    );
    window.capture();
    let page = window.call("page.inspect", json!({}));
    let result = window.call("page.click", target(&page, "submit"));
    assert!(result["url"].as_str().unwrap().ends_with("/account"));
    assert!(
        result["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|node| node["text"] == "Your synthetic session is active.")
    );
    window.capture();
}
