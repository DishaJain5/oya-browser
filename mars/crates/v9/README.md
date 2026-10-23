<!-- V9 is an independently selectable, source-owned Rust engine package. -->
# V9

V9 is Mars's bounded Rust JavaScript runtime. Use it without launching the Mars desktop app, Eye, Relay or sibling service processes. This development engine implements a documented subset, not full V8 or JavaScript compatibility.

```toml
[dependencies]
mars-v9 = { path = "/path/to/agentchrome/mars/crates/v9" }
```

```rust
use mars_v9::{load, JsValue};

let execution = load("const count = [...'A😀B'].length", &[])?;
assert_eq!(execution.state.globals()["count"].value, Some(JsValue::Number(3.0)));
# Ok::<(), mars_v9::protocol::EngineError>(())
```

Run the executable example from the repository root:

```sh
cargo run --manifest-path mars/Cargo.toml -p mars-v9 --example evaluate
```

`load` accepts source and explicit field inputs, returning a candidate execution with owned VM state, storage and text proposals. `load_document_scripts` accepts the inert `document::ScriptDocument` projection from Mars Core. A projection grants no native browser authority; the caller must validate and commit returned proposals through its own host boundary.

The root facade exports the same operations as `mars_v9::v9`. Strings retain UTF-16 units, including unmatched surrogates; `JsString::code_points` borrows code-point slices while UTF-8 document conversion explicitly rejects unmatched surrogates. Source, work, frame, heap and string bounds remain enforced. Unsupported syntax or host operations return typed errors rather than falling back to another JavaScript engine.

See [Mars's supported features](../../README.md), [upstream research](../../UPSTREAM.md) and [third-party notices](../../THIRD_PARTY_NOTICES.md) for precise compatibility and source-attribution limits. The only engine dependency is Mars Core's shared contracts; V9 also uses the existing pinned serde, serde_json and ryu-js libraries. This package is an unpublished path dependency with no API stability guarantee.
