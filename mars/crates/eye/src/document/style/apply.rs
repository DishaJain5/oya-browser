/** Supported declaration values update computed fields without introducing native authority. */
use super::{
    ColorScheme, ComputedStyle, unsupported,
    values::{contextual_color, contextual_length, number},
};
use crate::{
    constants::{CSS_MAX_BORDER_WIDTH, CSS_MAX_FONT_SIZE, CSS_MIN_FONT_SIZE, DIAGNOSTICS},
    protocol::EngineResult,
};
/** Unsafe unsupported presentation prevents candidate publication with value-free copy. */
pub(super) fn apply(
    style: &mut ComputedStyle,
    key: &str,
    value: &str,
    context: crate::document::RenderContext,
    diagnostics: &mut Vec<String>,
) -> EngineResult<()> {
    if key == "min-width" && value != "auto" {
        style.min_width = Some(contextual_length(value, style.font_size, context)?);
        return Ok(());
    }
    if super::flex::apply(style, key, value)? {
        return Ok(());
    }
    if key.starts_with("margin-") {
        let side = ["margin-top", "margin-right", "margin-bottom", "margin-left"]
            .iter()
            .position(|side| *side == key)
            .ok_or_else(unsupported)?;
        style.margin_auto[side] = value == "auto";
        style.margin[side] = if value == "auto" {
            0.0
        } else {
            contextual_length(value, style.font_size, context)?
        };
        return Ok(());
    }
    if key.starts_with("padding-") && value == "auto" {
        return Err(unsupported());
    }
    match key {
        "color-scheme" => {
            style.color_scheme = match value
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .as_str()
            {
                "normal" => ColorScheme::Normal,
                "light" => ColorScheme::Light,
                "dark" => ColorScheme::Dark,
                "light dark" => ColorScheme::LightDark,
                "dark light" => ColorScheme::DarkLight,
                _ => return Err(unsupported()),
            }
        }
        "line-height" => style.line_height = super::font::line_height(value, style.font_size)?,
        "font-family" if super::font::family_supported(value).is_ok() => {}
        "font-style" if value == "normal" => {}
        "display" => match value {
            "none" => style.hidden = true,
            "block" => style.inline = false,
            "inline" => style.inline = true,
            "flex" => {
                style.display_flex = true;
                style.inline = false;
            }
            _ => return Err(unsupported()),
        },
        "visibility" => match value {
            "hidden" | "collapse" => style.hidden = true,
            "visible" => {}
            _ => return Err(unsupported()),
        },
        "pointer-events" => match value {
            "none" => style.interactive = false,
            "auto" => {}
            _ => return Err(unsupported()),
        },
        "font-size" => {
            style.font_size = contextual_length(value, style.font_size, context)?
                .clamp(CSS_MIN_FONT_SIZE, CSS_MAX_FONT_SIZE)
        }
        "font-weight" => {
            style.bold = match value {
                "bold" | "bolder" | "600" | "700" | "800" | "900" => true,
                "normal" | "400" | "500" => false,
                _ => return Err(unsupported()),
            }
        }
        "color" => style.color = Some(contextual_color(value, style.used_scheme)?),
        "background" | "background-color" => {
            style.background = Some(contextual_color(value, style.used_scheme)?)
        }
        "width" => {
            style.percent_width = value.ends_with('%');
            style.width = if value == "auto" {
                None
            } else {
                Some(if style.percent_width {
                    number(value.trim_end_matches('%'))?
                } else {
                    contextual_length(value, style.font_size, context)?
                })
            };
        }
        "max-width" => {
            style.max_width = if value == "none" {
                None
            } else {
                Some(contextual_length(value, style.font_size, context)?)
            }
        }
        "margin-top" => style.margin[0] = contextual_length(value, style.font_size, context)?,
        "margin-right" => style.margin[1] = contextual_length(value, style.font_size, context)?,
        "margin-bottom" => style.margin[2] = contextual_length(value, style.font_size, context)?,
        "margin-left" => style.margin[3] = contextual_length(value, style.font_size, context)?,
        "padding-top" => style.padding[0] = contextual_length(value, style.font_size, context)?,
        "padding-right" => style.padding[1] = contextual_length(value, style.font_size, context)?,
        "padding-bottom" => style.padding[2] = contextual_length(value, style.font_size, context)?,
        "padding-left" => style.padding[3] = contextual_length(value, style.font_size, context)?,
        "border" => {
            if value == "none" || value == "0" {
                style.border = 0.0;
            } else {
                let parts = value.split_whitespace().collect::<Vec<_>>();
                if parts.len() != 3 || parts[1] != "solid" {
                    return Err(unsupported());
                }
                style.border = contextual_length(parts[0], style.font_size, context)?
                    .min(CSS_MAX_BORDER_WIDTH);
                style.border_color = Some(contextual_color(parts[2], style.used_scheme)?);
            }
        }
        "border-width" => {
            style.border =
                contextual_length(value, style.font_size, context)?.min(CSS_MAX_BORDER_WIDTH)
        }
        "border-color" => style.border_color = Some(contextual_color(value, style.used_scheme)?),
        "border-style" if ["solid", "none"].contains(&value) => {
            if value == "none" {
                style.border = 0.0;
            }
        }
        "text-align" if ["left", "center", "right", "start"].contains(&value) => {
            style.align = value.into()
        }
        "font-family" | "font-style" | "border-radius" | "box-shadow" | "text-decoration"
        | "cursor" | "outline" | "outline-color" | "outline-width" | "outline-style" => {
            let message = format!("Style decoration {key} is not supported.");
            if diagnostics.len() < DIAGNOSTICS && !diagnostics.contains(&message) {
                diagnostics.push(message);
            }
        }
        _ => return Err(unsupported()),
    }
    Ok(())
}
