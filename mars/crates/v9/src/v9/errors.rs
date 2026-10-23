/** Static reasons and structured locations prevent source/secret leakage in VM errors. */
use crate::protocol::{EngineError, SourceLocation};
/** Malformed supported grammar is distinct from recognized unimplemented JavaScript. */
pub(super) fn syntax(location: SourceLocation, reason: &str) -> EngineError {
    located(
        EngineError::new(
            "script_syntax_error",
            format!(
                "Script syntax error at line {}, column {}: {reason}.",
                location.line, location.column
            ),
            "Correct the script; no script changes were applied.",
        ),
        location,
        "parse",
    )
}
/** Recognized features fail explicitly rather than acquiring accidental subset semantics. */
pub(super) fn unsupported(location: SourceLocation, feature: &str) -> EngineError {
    located(
        EngineError::unsupported(format!(
            "This JavaScript feature is not supported yet: {feature}. Line {}, column {}.",
            location.line, location.column
        )),
        location,
        "parse",
    )
}
/** Unbound names and invalid VM operations never include variable or field values. */
pub(super) fn execution(location: SourceLocation, reason: &str) -> EngineError {
    located(
        EngineError::new(
            "script_execution_error",
            format!(
                "Script execution failed at line {}, column {}: {reason}.",
                location.line, location.column
            ),
            "Correct the script; no script changes were applied.",
        ),
        location,
        "execute",
    )
}
/** Budget and DOM failures retain their stable error classification. */
pub(super) fn located(
    mut error: EngineError,
    location: SourceLocation,
    phase: &str,
) -> EngineError {
    if error.location.is_none() {
        error.location = Some(Box::new(location));
        error.phase = Some(phase.into());
        error.input_kind = Some("script".into());
    }
    error
}
