/** Real services reject stale inline input reads without publishing earlier argument side effects. */
use super::*;

/** Missing or unavailable first-match values roll back DOM, revision, globals and Storage together. */
#[test]
fn inline_input_failure_rolls_back_earlier_argument_effects() {
    for (nodes, handler, action) in [
        (
            "<div id=parent><input id=name value=Ada></div>",
            "receive(remove(),document.getElementById('name').value)",
            "document.getElementById('parent').textContent='removed';",
        ),
        (
            "<p id=name>first</p><input id=name value=Ada>",
            "receive(remove(),document.getElementById('name').value)",
            "",
        ),
    ] {
        let body = format!(
            "{nodes}<p id=out>initial</p><button id=button type=button onclick=\"{handler}\">Bad</button><button id=probe type=button onclick='probe()'>Probe</button><script>var count=0;function remove(){{count++;window.localStorage.setItem('count',count);{action}}}function receive(a,b){{document.getElementById('out').textContent=b;}}function probe(){{document.getElementById('out').textContent=count+' '+window.localStorage.getItem('count');}}</script>"
        );
        let fixture = fixture(&body);
        let mut runtime = runtime(&fixture.url);
        let page = call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
        assert!(call(&mut runtime, "page.click", target(&page, "button")).is_err());
        let inspected = call(&mut runtime, "page.inspect", json!({})).unwrap();
        assert_eq!(inspected["revision"], page["revision"]);
        assert_eq!(inspected["nodes"], page["nodes"]);
        let probed = call(&mut runtime, "page.click", target(&inspected, "probe")).unwrap();
        assert_eq!(text(&probed, "out"), "0 null");
    }
}

/** Native editing supplies current values to an ordinary inline argument without changing its handler contract. */
#[test]
fn inline_input_arguments_use_current_native_edits() {
    let body = "<input id=name value=Ada><p id=out>initial</p><button id=button type=button onclick=\"receive(document.getElementById('name').value)\">Read</button><script>function receive(value){document.getElementById('out').textContent=value;}</script>";
    let fixture = fixture(body);
    let mut runtime = runtime(&fixture.url);
    let mut page = call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    for value in ["Lin", "Rust 🐙", ""] {
        let mut params = target(&page, "name");
        params["text"] = json!(value);
        call(&mut runtime, "page.type", params).unwrap();
        page = call(&mut runtime, "page.inspect", json!({})).unwrap();
        page = call(&mut runtime, "page.click", target(&page, "button")).unwrap();
        assert_eq!(text(&page, "out"), value);
    }
}
