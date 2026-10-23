/** Private computed styles must satisfy the same bounds before render as styles parsed from CSS. */
use super::{ColorScheme, ComputedStyle, FlexBasis, FlexDirection, FlexWrap};
use crate::document::Appearance;
use crate::{
    constants::{CSS_MAX_BORDER_WIDTH, CSS_MAX_FONT_SIZE, CSS_MAX_SCALAR, CSS_MIN_FONT_SIZE},
    protocol::{EngineError, EngineResult},
};
impl ComputedStyle {
    /// Reject forged geometry and unsupported resolved combinations before native measurement or publication.
    pub fn validate(&self) -> EngineResult<()> {
        let mut lengths = self
            .margin
            .iter()
            .chain(self.padding.iter())
            .copied()
            .chain(self.width)
            .chain(self.max_width)
            .chain(self.min_width)
            .chain([
                self.flex_grow,
                self.flex_shrink,
                self.row_gap,
                self.column_gap,
            ]);
        if matches!(
            (self.color_scheme, self.used_scheme),
            (ColorScheme::Normal | ColorScheme::Light, Appearance::Dark)
                | (ColorScheme::Dark, Appearance::Light)
        ) {
            return Err(invalid());
        }
        let advance = self.line_height.used(self.font_size);
        if !advance.is_finite() || advance <= 0.0 || advance > CSS_MAX_SCALAR {
            return Err(invalid());
        }
        if lengths.any(|value| !scalar(value))
            || !self.font_size.is_finite()
            || !(CSS_MIN_FONT_SIZE..=CSS_MAX_FONT_SIZE).contains(&self.font_size)
            || !self.border.is_finite()
            || !(0.0..=CSS_MAX_BORDER_WIDTH).contains(&self.border)
            || self.percent_width && self.width.is_none()
            || self.display_flex && self.inline
            || !["left", "center", "right", "start"].contains(&self.align.as_str())
        {
            return Err(invalid());
        }
        if let FlexBasis::Pixels(value) | FlexBasis::Percent(value) = self.flex_basis
            && !scalar(value)
        {
            return Err(invalid());
        }
        for value in [&self.color, &self.background, &self.border_color]
            .into_iter()
            .flatten()
        {
            super::values::color(value).map_err(|_| invalid())?;
        }
        if self.display_flex
            && self.flex_direction == FlexDirection::Column
            && self.flex_wrap == FlexWrap::Wrap
        {
            return Err(EngineError::unsupported(
                "Wrapped flex columns require a definite height, which is not supported yet.",
            ));
        }
        Ok(())
    }
}
/** CSS scalars share one finite nonnegative budget independently of their source spelling. */
fn scalar(value: f64) -> bool {
    value.is_finite() && (0.0..=CSS_MAX_SCALAR).contains(&value)
}
/** Value-free failures do not disclose private author declarations. */
fn invalid() -> EngineError {
    EngineError::invalid("Computed style values are outside supported bounds.")
}
