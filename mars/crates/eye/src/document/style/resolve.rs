/** Inherited computed values preserve factors and authored scheme negotiation. */
use super::{ColorScheme, ComputedStyle};
use crate::document::Appearance;
/** Noninherited box fields always start fresh for each formatting subject. */
pub(super) fn inherited(parent: &ComputedStyle) -> ComputedStyle {
    ComputedStyle {
        font_size: parent.font_size,
        bold: parent.bold,
        color: parent.color.clone(),
        align: parent.align.clone(),
        hidden: parent.hidden,
        interactive: parent.interactive,
        line_height: parent.line_height,
        color_scheme: parent.color_scheme,
        used_scheme: parent.used_scheme,
        ..ComputedStyle::default()
    }
}
/** Single offered schemes win; either ordered pair admits the effective native preference. */
pub(super) fn used_scheme(scheme: ColorScheme, preference: Appearance) -> Appearance {
    match scheme {
        ColorScheme::Normal | ColorScheme::Light => Appearance::Light,
        ColorScheme::Dark => Appearance::Dark,
        ColorScheme::LightDark | ColorScheme::DarkLight => preference,
    }
}

/** UA link color outranks inherited canvas text; an author's own link declaration still wins. */
pub(super) fn link_foreground(style: &ComputedStyle) -> Option<String> {
    (style.color_scheme != ColorScheme::Normal || style.color.is_some()).then(|| {
        match style.used_scheme {
            Appearance::Light => "#0000ee".into(),
            Appearance::Dark => "#9e9eff".into(),
        }
    })
}
