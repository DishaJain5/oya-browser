/** Tool results duplicate complete native JSON in text and structured form without adapter success prose. */
use crate::protocol::{EngineError, EngineResult};
use serde_json::{Value, json};
/** Native failures remain tool failures; malformed native success values are internal protocol faults. */
pub fn tool_result(result: EngineResult<Value>) -> EngineResult<Value> {
    let (value, is_error) = match result {
        Ok(value) if value.is_object() => (value, false),
        Ok(_) => return Err(internal()),
        Err(error) => (
            json!({"error":serde_json::to_value(error).map_err(|_|internal())?}),
            true,
        ),
    };
    let text = serde_json::to_string(&value).map_err(|_| internal())?;
    Ok(
        json!({"content":[{"type":"text","text":text}],"structuredContent":value,"isError":is_error}),
    )
}
/** A fixed diagnostic avoids leaking malformed service payloads into adapter-generated errors. */
fn internal() -> EngineError {
    EngineError::new(
        "internal_error",
        "The native tool response is invalid.",
        "Start a new connection; no operation was replayed.",
    )
}
