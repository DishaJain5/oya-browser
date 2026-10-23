/** Owned form algorithms produce literal requests shared by humans and native tools. */
use super::{Document, Node};
use crate::{
    constants::FORM_BYTES,
    protocol::{EngineError, EngineResult},
};
use mars_core::http::Method;

/// A request proposal contains data, never permission to send it.
#[derive(Clone)]
pub struct Submission {
    /// Relative or absolute author action is resolved by the coordinator.
    pub action: String,
    /// Only GET and URL-encoded POST are supported.
    pub method: Method,
    /// Ordered successful controls preserve repeated names.
    pub encoded: String,
    /// GET URLs containing passwords require a protected public projection.
    pub protected: bool,
}
/** Parsed control state is normalized once before scripts or native display. */
pub(super) fn normalize(document: &mut Document) -> EngineResult<()> {
    if document.nodes.iter().filter(|n| n.tag == "form").count() > 32
        || document.nodes.iter().filter(|n| control(n)).count() > 256
    {
        return Err(EngineError::limit("form controls", 256));
    }
    for node in &document.nodes {
        if node.tag == "form" && ancestor(document, node, "form").is_some() {
            return Err(EngineError::unsupported("Nested forms are not supported."));
        }
        if node.tag == "input" && node.attributes.contains_key("pattern") {
            return Err(EngineError::unsupported(
                "Pattern validation is not supported yet.",
            ));
        }
    }
    let disabled = document
        .nodes
        .iter()
        .filter(|n| ancestor(document, n, "fieldset").is_some_and(|id| document.nodes[id].disabled))
        .map(|n| n.local_id)
        .collect::<Vec<_>>();
    for id in disabled {
        document.nodes[id].disabled = true;
    }
    let radios = document
        .nodes
        .iter()
        .filter(|n| n.tag == "input" && n.input_type() == "radio" && n.checked)
        .map(|n| n.local_id)
        .collect::<Vec<_>>();
    for id in radios {
        let copy = document.clone();
        *document = set_checked(&copy, id, true)?;
    }
    let selects = document
        .nodes
        .iter()
        .filter(|n| n.tag == "select")
        .map(|n| n.local_id)
        .collect::<Vec<_>>();
    for id in selects {
        let options = options(document, id);
        let selected = options
            .iter()
            .rev()
            .find(|n| n.selected && !n.disabled)
            .or_else(|| options.iter().find(|n| !n.disabled))
            .map(|n| n.local_id);
        for node in &mut document.nodes {
            if node.tag == "option" && node.parent == Some(id) {
                node.selected = Some(node.local_id) == selected;
            }
        }
    }
    Ok(())
}
/// Explicit form ownership takes precedence over ancestor ownership.
pub fn form_owner(document: &Document, node: &Node) -> Option<usize> {
    if let Some(id) = node.attributes.get("form") {
        return document
            .nodes
            .iter()
            .find(|n| n.tag == "form" && n.html_id == *id)
            .map(|n| n.local_id);
    }
    ancestor(document, node, "form")
}
/** Ancestor walks use stable IDs and the already-validated bounded tree. */
fn ancestor(document: &Document, node: &Node, tag: &str) -> Option<usize> {
    let mut parent = node.parent;
    while let Some(id) = parent {
        let item = document.nodes.iter().find(|n| n.local_id == id)?;
        if item.tag == tag {
            return Some(id);
        }
        parent = item.parent;
    }
    None
}
/// Accessible labels resolve by association rather than fixture-specific author IDs.
pub fn label(document: &Document, node: &Node) -> String {
    if let Some(name) = node.attributes.get("aria-label") {
        return name.clone();
    }
    let labels = document
        .nodes
        .iter()
        .filter(|label| {
            label.tag == "label"
                && (label.attributes.get("for") == Some(&node.html_id)
                    || ancestor(document, node, "label") == Some(label.local_id))
        })
        .map(|label| text(document, label))
        .collect::<Vec<_>>()
        .join(" ");
    if !labels.is_empty() {
        labels
    } else if !node.text.is_empty() {
        text(document, node)
    } else {
        node.placeholder.clone()
    }
}
/** Text extraction cannot include values from password/hidden/native fields. */
fn text(document: &Document, node: &Node) -> String {
    let mut result = node.text.clone();
    for child in document
        .nodes
        .iter()
        .filter(|n| n.parent == Some(node.local_id) && !control(n))
    {
        result.push(' ');
        result.push_str(&text(document, child));
    }
    result.split_whitespace().collect::<Vec<_>>().join(" ")
}
/** Only real form controls enter successful-control enumeration. */
fn control(node: &Node) -> bool {
    ["input", "textarea", "select", "button"].contains(&node.tag.as_str())
}
/** Options remain scoped to the supplied select, never another form's popup. */
fn options(document: &Document, id: usize) -> Vec<&Node> {
    document
        .nodes
        .iter()
        .filter(|n| n.tag == "option" && n.parent == Some(id))
        .collect()
}
/// Idempotent checked state prevents agents from blindly toggling a checkbox.
pub fn set_checked(document: &Document, id: usize, checked: bool) -> EngineResult<Document> {
    let node = document
        .nodes
        .iter()
        .find(|n| {
            n.local_id == id && n.tag == "input" && ["checkbox", "radio"].contains(&n.input_type())
        })
        .ok_or_else(invalid)?;
    if node.disabled || node.hidden() {
        return Err(invalid());
    }
    let mut candidate = document.clone();
    let owner = form_owner(document, node);
    let name = node.attributes.get("name");
    for item in &mut candidate.nodes {
        if item.local_id == id {
            item.checked = checked;
        } else if checked
            && node.input_type() == "radio"
            && name.is_some()
            && item.tag == "input"
            && item.input_type() == "radio"
            && item.attributes.get("name") == name
            && form_owner(document, item) == owner
        {
            item.checked = false;
        }
    }
    Ok(candidate)
}
/// Select one enabled option by native option identity, including duplicate author values.
pub fn select(document: &Document, id: usize, option: usize) -> EngineResult<Document> {
    let node = document
        .nodes
        .iter()
        .find(|n| n.local_id == id && n.tag == "select" && !n.disabled && !n.hidden())
        .ok_or_else(invalid)?;
    if !options(document, node.local_id)
        .iter()
        .any(|n| n.local_id == option && !n.disabled)
    {
        return Err(invalid());
    }
    let mut candidate = document.clone();
    for node in &mut candidate.nodes {
        if node.tag == "option" && node.parent == Some(id) {
            node.selected = node.local_id == option;
        }
    }
    Ok(candidate)
}
/// Build a bounded request without performing network I/O or changing the document.
pub fn submission(
    document: &Document,
    form: usize,
    submitter: Option<usize>,
) -> EngineResult<Submission> {
    let form_node = document
        .nodes
        .iter()
        .find(|n| n.local_id == form && n.tag == "form" && !n.hidden())
        .ok_or_else(invalid)?;
    let submit = submitter
        .map(|id| {
            document
                .nodes
                .iter()
                .find(|n| {
                    n.local_id == id
                        && !n.disabled
                        && !n.hidden()
                        && form_owner(document, n) == Some(form)
                        && n.input_type() == "submit"
                })
                .ok_or_else(invalid)
        })
        .transpose()?;
    if submit.is_some_and(|n| {
        [
            "formaction",
            "formmethod",
            "formenctype",
            "formtarget",
            "formnovalidate",
        ]
        .iter()
        .any(|key| n.attributes.contains_key(*key))
    }) || form_node
        .attributes
        .get("target")
        .is_some_and(|s| s != "_self" && !s.is_empty())
    {
        return Err(EngineError::unsupported(
            "Form targets and submitter overrides are not supported.",
        ));
    }
    let method = match form_node
        .attributes
        .get("method")
        .map(|s| s.to_ascii_lowercase())
        .as_deref()
        .unwrap_or("get")
    {
        "get" => Method::Get,
        "post" => Method::Post,
        _ => {
            return Err(EngineError::unsupported(
                "Only GET and POST forms are supported.",
            ));
        }
    };
    if form_node
        .attributes
        .get("enctype")
        .is_some_and(|s| !s.eq_ignore_ascii_case("application/x-www-form-urlencoded"))
    {
        return Err(EngineError::unsupported(
            "Only URL-encoded form submission is supported.",
        ));
    }
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    let mut protected = false;
    for node in document
        .nodes
        .iter()
        .filter(|n| control(n) && !n.disabled && form_owner(document, n) == Some(form))
    {
        if !form_node.attributes.contains_key("novalidate")
            && node.attributes.contains_key("required")
            && !valid_required(document, node)
        {
            let mut error = EngineError::new(
                "validation_error",
                format!("Complete required field: {}.", label(document, node)),
                "Focus the required field and provide its value.",
            );
            error.invalid_node = Some(Box::new(node.local_id));
            return Err(error);
        }
        let Some(name) = node.attributes.get("name").filter(|s| !s.is_empty()) else {
            continue;
        };
        let values = successful(document, node, submitter);
        if node.input_type() == "password" && !values.is_empty() {
            protected = true;
        }
        for value in values {
            serializer.append_pair(name, &normalize_lines(&value));
        }
    }
    let encoded = serializer.finish();
    if encoded.len() > FORM_BYTES {
        return Err(EngineError::limit("encoded form bytes", FORM_BYTES));
    }
    Ok(Submission {
        action: form_node
            .attributes
            .get("action")
            .cloned()
            .unwrap_or_default(),
        method,
        encoded,
        protected,
    })
}
/** Hidden/read-only fields submit; only the activated submitter is successful. */
fn successful(document: &Document, node: &Node, submitter: Option<usize>) -> Vec<String> {
    if node.tag == "select" {
        return options(document, node.local_id)
            .iter()
            .filter(|n| n.selected && !n.disabled)
            .map(|n| {
                n.attributes
                    .get("value")
                    .cloned()
                    .unwrap_or_else(|| n.text.clone())
            })
            .collect();
    }
    if ["button", "submit"].contains(&node.input_type()) || node.tag == "button" {
        return if submitter == Some(node.local_id) && node.input_type() == "submit" {
            vec![node.value.clone()]
        } else {
            vec![]
        };
    }
    if ["checkbox", "radio"].contains(&node.input_type()) {
        return if node.checked {
            vec![
                node.attributes
                    .get("value")
                    .cloned()
                    .unwrap_or_else(|| "on".into()),
            ]
        } else {
            vec![]
        };
    }
    vec![node.value.clone()]
}
/** Only implemented required constraints can block dispatch. */
fn valid_required(document: &Document, node: &Node) -> bool {
    if node.readonly()
        || (node.tag == "input" && ["hidden", "submit", "button"].contains(&node.input_type()))
    {
        return true;
    }
    if node.input_type() == "checkbox" {
        return node.checked;
    }
    if node.input_type() == "radio" {
        return document.nodes.iter().any(|n| {
            n.tag == "input"
                && n.input_type() == "radio"
                && n.checked
                && n.attributes.get("name") == node.attributes.get("name")
                && form_owner(document, n) == form_owner(document, node)
        });
    }
    if node.tag == "select" {
        return options(document, node.local_id).iter().any(|n| {
            n.selected && !n.disabled && !n.attributes.get("value").unwrap_or(&n.text).is_empty()
        });
    }
    !node.value.is_empty()
}
/** Form encoding normalizes every submitted line ending to CRLF. */
fn normalize_lines(value: &str) -> String {
    value
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\n', "\r\n")
}
/** Failure copy contains no current or initial field values. */
fn invalid() -> EngineError {
    EngineError::new(
        "invalid_target",
        "Choose an enabled visible form control.",
        "Inspect the current document and use its native node identity.",
    )
}
