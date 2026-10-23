/** Tool schemas and semantic checks share exact native types, copy and UTF-8 byte bounds. */
use crate::constants::{ID_BYTES, TEXT_BYTES, URL_BYTES};
use serde_json::{Value, json};
/** Reused target fields retain one spelling and description across all native tools. */
#[derive(Clone, Copy)]
pub(super) enum Field {
    Tab,
    Epoch,
    Page,
    Node,
    Revision,
    Option,
    Text,
    Checked,
    Url,
    Y,
}
impl Field {
    /** Native parameter names are fixed identifiers, never derived from external operation strings. */
    pub fn name(self) -> &'static str {
        match self {
            Self::Tab => "tab_id",
            Self::Epoch => "control_epoch",
            Self::Page => "page_id",
            Self::Node => "node_id",
            Self::Revision => "expected_revision",
            Self::Option => "option_id",
            Self::Text => "text",
            Self::Checked => "checked",
            Self::Url => "url",
            Self::Y => "y",
        }
    }
    /** Exact designer guidance makes discovered preconditions meaningful to connecting agents. */
    fn description(self) -> &'static str {
        match self {
            Self::Tab => {
                "Exact native tab ID returned by tab_list. No tab is selected automatically."
            }
            Self::Epoch => {
                "The epoch returned by control_status for this tab after the human grant. A changed epoch is refused."
            }
            Self::Page => "Current page ID returned by page_inspect for this tab.",
            Self::Node => "Native node ID returned by page_inspect for this page.",
            Self::Revision => "Observed page revision. A newer committed change causes refusal.",
            Self::Option => {
                "Native option ID discovered for the targeted select; not its author value or label."
            }
            Self::Y => {
                "Desired document scroll offset in logical points. The native runtime clamps it to the page."
            }
            Self::Text => "Literal replacement text.",
            Self::Checked => "Boolean checked state.",
            Self::Url => "Supported URL to load in the specified native tab.",
        }
    }
    /** Discovery length limits supplement, rather than replace, the mandatory byte checks at translation. */
    pub fn schema(self) -> Value {
        let mut schema = match self {
            Self::Tab | Self::Page | Self::Node | Self::Option => {
                json!({"type":"string","minLength":1,"maxLength":ID_BYTES})
            }
            Self::Text => json!({"type":"string","maxLength":TEXT_BYTES}),
            Self::Url => json!({"type":"string","maxLength":URL_BYTES}),
            Self::Epoch | Self::Revision => {
                json!({"type":"integer","minimum":0,"maximum":u64::MAX})
            }
            Self::Checked => json!({"type":"boolean"}),
            Self::Y => json!({"type":"number"}),
        };
        schema["description"] = json!(self.description());
        schema
    }
    /** All fields are required and retain their native JSON type without coercion or truncation. */
    pub fn valid(self, value: Option<&Value>) -> bool {
        let Some(value) = value else {
            return false;
        };
        match self {
            Self::Tab | Self::Page | Self::Node | Self::Option => value
                .as_str()
                .is_some_and(|value| !value.is_empty() && value.len() <= ID_BYTES),
            Self::Text => value
                .as_str()
                .is_some_and(|value| value.len() <= TEXT_BYTES),
            Self::Url => value.as_str().is_some_and(|value| value.len() <= URL_BYTES),
            Self::Epoch | Self::Revision => value.as_u64().is_some(),
            Self::Checked => value.as_bool().is_some(),
            Self::Y => value.as_f64().is_some_and(f64::is_finite),
        }
    }
}
