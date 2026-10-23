/** Owned bounded CSS cascade controls geometry and fails closed on unsupported hiding rules. */
mod apply;
mod cascade;
mod flex;
mod flex_model;
mod font;
mod formatting;
mod model;
mod resolve;
mod selectors;
mod shorthand;
mod syntax;
mod validation;
mod values;
use crate::protocol::EngineError;
pub use cascade::{compute_styles, compute_styles_with_context};
pub use flex_model::{AlignItems, BoxSizing, FlexBasis, FlexDirection, FlexWrap, JustifyContent};
pub use formatting::{ColorScheme, DocumentFormatting, FormattingElement, LineHeight};
pub use model::{ComputedStyle, StyleSource};
/** Never include arbitrary declarations or selectors in an error that could contain secrets. */
fn unsupported() -> EngineError {
    EngineError::unsupported(
        "This page uses a CSS selector, condition, value or layout property Eye does not support yet.",
    )
}
