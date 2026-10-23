/** Operating-system text primitives supply shaping, never document or navigation semantics. */
#[cfg(target_os = "macos")]
mod text;
#[cfg(target_os = "macos")]
pub use text::{attributed, colored, font, measure};

/// Headless non-macOS builds keep deterministic geometry without claiming GUI parity.
#[cfg(not(target_os = "macos"))]
pub fn measure(text: &str, size: f64, _bold: bool) -> f64 {
    text.chars().count() as f64 * size * 0.6
}
