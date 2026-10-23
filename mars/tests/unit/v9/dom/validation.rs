/** Public projected-DOM validation rules use the shared real-document fixture. */
use super::*;

/** Host identities are located recursively only to forge otherwise well-formed private snapshots. */
fn host_paths(value: &serde_json::Value, path: Vec<String>, found: &mut Vec<Vec<String>>) {
    if value.get("type").and_then(serde_json::Value::as_str) == Some("host") {
        found.push(path);
    } else if let Some(object) = value.as_object() {
        for (key, child) in object {
            let mut next = path.clone();
            next.push(key.clone());
            host_paths(child, next, found);
        }
    } else if let Some(array) = value.as_array() {
        for (index, child) in array.iter().enumerate() {
            let mut next = path.clone();
            next.push(index.to_string());
            host_paths(child, next, found);
        }
    }
}

/** Globals, heap properties, closure cells, receivers and bound arguments all validate document identity. */
#[test]
fn hostile_host_references_in_every_retained_arena_fail_before_collection() {
    let source = "const element=document.getElementById('out');const box={element:element};function make(){let captured=element;return ()=>captured;}const retained=make();function f(x){return x;}const bound=f.bind(element,element);f.extra=element;const maker={create:function(){return ()=>this;}};const lexical=maker.create.call(element);document.getElementById('button').addEventListener('click',()=>{});";
    let document = page();
    let original = serde_json::to_value(load(source, &document).state).unwrap();
    let mut paths = Vec::new();
    host_paths(&original, Vec::new(), &mut paths);
    assert!(paths.len() >= 7, "{paths:?}");
    for path in paths {
        let mut wire = original.clone();
        let mut value = &mut wire;
        for key in &path {
            value = if value.is_array() {
                &mut value[key.parse::<usize>().unwrap()]
            } else {
                &mut value[key]
            };
        }
        value["reference"]["document"] = json!("forged-page");
        let state: VmState = serde_json::from_value(wire).unwrap();
        assert_eq!(
            state.validate_document(&document).unwrap_err().code,
            "invalid_request",
            "{path:?}"
        );
    }
}

/** Retained events must name an existing target and precede their document's monotonic event counter. */
#[test]
fn hostile_event_records_and_event_counters_are_rejected() {
    let source = "let saved=null;document.getElementById('button').addEventListener('click',event=>{saved=event;});";
    let document = page();
    let original =
        serde_json::to_value(click(source, load(source, &document).state, &document).state)
            .unwrap();
    let event_id = original["dom"]["events"]
        .as_object()
        .unwrap()
        .keys()
        .next()
        .unwrap()
        .clone();
    for case in 0..3 {
        let mut wire = original.clone();
        match case {
            0 => wire["dom"]["events"][&event_id]["target"] = json!(999),
            1 => wire["dom"]["next_event"] = json!(0),
            _ => {
                wire["dom"]["events"].as_object_mut().unwrap().clear();
            }
        }
        let state: VmState = serde_json::from_value(wire).unwrap();
        assert_eq!(
            state.validate_document(&document).unwrap_err().code,
            "invalid_request",
            "case {case}"
        );
    }
}

/** Unretained event records are reclaimed while intentionally saved event facades obey their own cap. */
#[test]
fn event_records_are_collected_or_bounded_according_to_reachability() {
    let source = "document.getElementById('button').addEventListener('click',()=>{});";
    let document = page();
    let mut state = load(source, &document).state;
    for _ in 0..mars::constants::JS_EVENT_RECORDS + 2 {
        state = click(source, state, &document).state;
    }
    let wire = serde_json::to_value(state).unwrap();
    assert!(wire["dom"]["events"].as_object().unwrap().is_empty());
    let source = "const saved=[];let count=0;document.getElementById('button').addEventListener('click',event=>{saved[count]=event;count++;});";
    let mut state = load(source, &document).state;
    for _ in 0..mars::constants::JS_EVENT_RECORDS {
        state = click(source, state, &document).state;
    }
    assert_eq!(
        click_document_scripts(
            &[source.into()],
            document.get_by_id("button").unwrap(),
            state,
            document,
            StorageState::default()
        )
        .unwrap_err()
        .code,
        "limit_exceeded"
    );
}

/** A callback failure discards earlier mutations to closures, registrations, storage and text together. */
#[test]
fn late_callback_failure_rolls_back_the_whole_candidate_and_registration_changes() {
    let source = "const b=document.getElementById('button');let count=0;function late(){}b.addEventListener('click',()=>{count++;b.addEventListener('click',late);window.localStorage.setItem('count',count);document.getElementById('out').textContent=count;missing();});";
    let document = page();
    let loaded = load(source, &document);
    let original = serde_json::to_value(&loaded.state).unwrap();
    for _ in 0..2 {
        assert!(
            click_document_scripts(
                &[source.into()],
                document.get_by_id("button").unwrap(),
                loaded.state.clone(),
                document.clone(),
                loaded.storage.clone()
            )
            .is_err()
        );
        assert_eq!(serde_json::to_value(&loaded.state).unwrap(), original);
        assert!(loaded.storage.local.is_empty());
    }
}

/** Private state cannot cross page identity boundaries, including zero-script documents. */
#[test]
fn document_identity_and_unknown_host_references_are_rejected_before_execution() {
    let source = "const element=document.getElementById('out');document.getElementById('button').addEventListener('click',()=>{});";
    let document = page();
    let loaded = load(source, &document);
    let mut different = document.clone();
    different.identity = "page-b".into();
    assert_eq!(
        loaded.state.validate_document(&different).unwrap_err().code,
        "invalid_request"
    );
    let original = serde_json::to_value(&loaded.state).unwrap();
    for value in [
        json!({"type":"host","reference":{"document":"page-b","kind":{"kind":"element","local_id":1}}}),
        json!({"type":"host","reference":{"document":"page-a","kind":{"kind":"element","local_id":999}}}),
        json!({"type":"host","reference":{"document":"page-a","kind":{"kind":"event","id":999}}}),
    ] {
        let mut wire = original.clone();
        wire["globals"]["element"]["value"] = value;
        let state: VmState = serde_json::from_value(wire).unwrap();
        assert_eq!(
            state.validate_document(&document).unwrap_err().code,
            "invalid_request"
        );
    }
    let loaded = load_document_scripts(&[], document.clone(), StorageState::default()).unwrap();
    loaded.state.validate_document(&document).unwrap();
}

/** Invalid listener identities, targets and callbacks cannot be rescued by later graph collection. */
#[test]
fn hostile_listener_records_and_monotonic_counters_fail_closed() {
    let source = "document.getElementById('button').addEventListener('click',()=>{});";
    let document = page();
    let original = serde_json::to_value(load(source, &document).state).unwrap();
    for case in 0..5 {
        let mut wire = original.clone();
        match case {
            0 => wire["dom"]["listeners"][0]["target"] = json!(999),
            1 => wire["dom"]["listeners"][0]["callback"] = json!(999),
            2 => wire["dom"]["next_listener"] = json!(0),
            3 => wire["dom"]["listeners"]
                .as_array_mut()
                .unwrap()
                .push(original["dom"]["listeners"][0].clone()),
            _ => wire["dom"]["identity"] = json!("page-b"),
        }
        let state: VmState = serde_json::from_value(wire).unwrap();
        assert_eq!(
            state.validate_document(&document).unwrap_err().code,
            "invalid_request",
            "case {case}"
        );
    }
}

/** Listener allocation has an independent quota even when callbacks are tiny and share one target. */
#[test]
fn listener_count_is_bounded_and_legacy_event_api_rejects_projected_states() {
    let source = format!(
        "const b=document.getElementById('button');for(let i=0;i<{};i++){{b.addEventListener('click',()=>{{}});}}",
        JS_EVENT_LISTENERS + 1
    );
    assert_eq!(
        load_document_scripts(&[source], page(), StorageState::default())
            .unwrap_err()
            .code,
        "limit_exceeded"
    );
    let source = "function run(){}";
    let loaded = load(source, &page());
    assert_eq!(
        event(source, "run()", loaded.state, &[]).unwrap_err().code,
        "invalid_request"
    );
}
