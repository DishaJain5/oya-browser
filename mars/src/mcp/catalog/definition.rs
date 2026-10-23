/** Only this reviewed tool table can select native operations from the MCP front door. */
use super::field::Field;
use crate::protocol::{EngineError, EngineResult};
use serde_json::{Value, json};
/** One entry owns the public name, native operation, discovery copy and complete required argument set. */
pub(super) struct Definition {
    /// Discovery names are exact lookup keys rather than transformed arbitrary native operations.
    pub name: &'static str,
    /// The selected native operation still performs its normal authority and target validation.
    pub operation: &'static str,
    /// Designer wording describes native behavior without asserting successful execution.
    description: &'static str,
    /// Every listed field is required; an empty list accepts only an empty object.
    fields: &'static [Field],
}
impl Definition {
    /** Object schemas reject extra fields and deliberately omit an output schema for this initial subset. */
    pub fn schema(&self) -> Value {
        let properties = self
            .fields
            .iter()
            .map(|field| (field.name().to_owned(), field.schema()))
            .collect::<serde_json::Map<_, _>>();
        let required = self
            .fields
            .iter()
            .map(|field| field.name())
            .collect::<Vec<_>>();
        json!({"name":self.name,"description":self.description,"inputSchema":{"type":"object","properties":properties,"required":required,"additionalProperties":false}})
    }
    /** Semantic validation enforces exact keys and UTF-8 limits even for callers that bypass JSON Schema. */
    pub fn validate(&self, arguments: &Value) -> EngineResult<()> {
        let Some(arguments) = arguments.as_object() else {
            return Err(invalid());
        };
        if arguments.len() != self.fields.len()
            || self
                .fields
                .iter()
                .any(|field| !field.valid(arguments.get(field.name())))
        {
            return Err(invalid());
        }
        Ok(())
    }
}
/** Invalid arguments reveal no supplied keys, identifiers, text, URL or field values. */
fn invalid() -> EngineError {
    EngineError::invalid("Tool arguments do not match the documented native tool schema.")
}
use Field::{Checked, Epoch, Node, Option, Page, Revision, Tab, Text, Url, Y};
/** The exhaustive MCP exposure deliberately excludes native human-only and unreviewed operations. */
pub(super) const TOOLS: &[Definition] = &[
    Definition {
        name: "runtime_capabilities",
        operation: "runtime.capabilities",
        description: "List this native runtime's supported features and limits.",
        fields: &[],
    },
    Definition {
        name: "tab_list",
        operation: "tab.list",
        description: "List native tabs and their identities.",
        fields: &[],
    },
    Definition {
        name: "control_status",
        operation: "control.status",
        description: "Read ownership and the current control epoch for one native tab.",
        fields: &[Tab],
    },
    Definition {
        name: "page_inspect",
        operation: "page.inspect",
        description: "Inspect one native tab's committed page, controls and current identities.",
        fields: &[Tab],
    },
    Definition {
        name: "page_layout",
        operation: "page.layout",
        description: "Read the native display geometry for one tab in logical points.",
        fields: &[Tab],
    },
    Definition {
        name: "page_navigate",
        operation: "page.navigate",
        description: "Load a supported URL in the specified native tab.",
        fields: &[Tab, Epoch, Url],
    },
    Definition {
        name: "page_type",
        operation: "page.type",
        description: "Replace an enabled editable field's value with literal text.",
        fields: &[Tab, Epoch, Page, Node, Revision, Text],
    },
    Definition {
        name: "page_click",
        operation: "page.click",
        description: "Activate a supported native link or control by its discovered node identity.",
        fields: &[Tab, Epoch, Page, Node, Revision],
    },
    Definition {
        name: "page_set_checked",
        operation: "page.set_checked",
        description: "Set a supported checkbox or radio control's checked state explicitly.",
        fields: &[Tab, Epoch, Page, Node, Revision, Checked],
    },
    Definition {
        name: "page_select",
        operation: "page.select",
        description: "Select an enabled option using its discovered native option identity.",
        fields: &[Tab, Epoch, Page, Node, Revision, Option],
    },
    Definition {
        name: "page_submit",
        operation: "page.submit",
        description: "Submit the targeted form using the native validation and submitter rules.",
        fields: &[Tab, Epoch, Page, Node, Revision],
    },
    Definition {
        name: "page_back",
        operation: "page.back",
        description: "Go back in the specified tab, respecting native form-resubmission safeguards.",
        fields: &[Tab, Epoch],
    },
    Definition {
        name: "page_forward",
        operation: "page.forward",
        description: "Go forward in the specified tab, respecting native form-resubmission safeguards.",
        fields: &[Tab, Epoch],
    },
    Definition {
        name: "page_reload",
        operation: "page.reload",
        description: "Reload the specified tab when its current page can be replayed safely.",
        fields: &[Tab, Epoch],
    },
    Definition {
        name: "page_scroll",
        operation: "page.scroll",
        description: "Set the document's vertical scroll offset in logical points.",
        fields: &[Tab, Epoch, Page, Revision, Y],
    },
    Definition {
        name: "control_release",
        operation: "control.release",
        description: "Return agent control of the specified native tab to the person.",
        fields: &[Tab, Epoch],
    },
];
