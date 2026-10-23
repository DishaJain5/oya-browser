/** Human and agent controls share native targeting, validation and V9 event execution. */
use super::{navigation::input_values, page::Page, supervisor::Runtime};
use crate::document::{Node, TextChange};
use crate::protocol::{EngineError, EngineResult, parameters};
use crate::v9::Execution;
use serde::Deserialize;
use serde_json::{Value, json};

/** Text and value edits share exact page and revision preconditions. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Edit {
    /** Observed page generation. */
    page_id: String,
    /** Engine-generated native node handle. */
    node_id: String,
    /** Prevents overwriting an unobserved change. */
    expected_revision: u64,
    /** Literal replacement data. */
    text: String,
}
/** Click does not accept a selector, script or arbitrary native callback. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Click {
    /** Observed page generation. */
    page_id: String,
    /** Exact native control identity. */
    node_id: String,
    /** Observed document state. */
    expected_revision: u64,
}
/** Checked state is explicit, preventing blind replay toggles. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Checked {
    page_id: String,
    node_id: String,
    expected_revision: u64,
    checked: bool,
}
/** Selection uses an exact option identity, including duplicate author values. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Selection {
    page_id: String,
    node_id: String,
    expected_revision: u64,
    option_id: String,
}

impl Runtime {
    /** Programmatic text replacement remains distinct from typing into a field. */
    pub(super) fn set_text(&mut self, params: Value) -> EngineResult<Value> {
        let input: Edit = parameters(params)?;
        let mut page = self.current()?.clone();
        let node = target(
            &page,
            &input.page_id,
            &input.node_id,
            input.expected_revision,
        )?;
        if node.secret() {
            return Err(invalid_target());
        }
        page.document = self.apply(
            &page.document,
            &[TextChange {
                html_id: node.html_id,
                text: input.text,
            }],
        )?;
        let result = self.finish_mutation(page)?;
        Ok(edit_result(result, &input.node_id))
    }
    /** Native text entry replaces only an enabled input's literal value. */
    pub(super) fn type_text(&mut self, params: Value) -> EngineResult<Value> {
        let input: Edit = parameters(params)?;
        let mut page = self.current()?.clone();
        let node = target(
            &page,
            &input.page_id,
            &input.node_id,
            input.expected_revision,
        )?;
        let value = self.call(
            "document",
            "document.value",
            json!({"document":page.document,"local_id":node.local_id,"value":input.text}),
        )?;
        page.document = self.returned("document", value)?;
        let result = self.finish_mutation(page)?;
        Ok(edit_result(result, &input.node_id))
    }
    /** Links and buttons enter the same owned action path for mouse, keyboard and tools. */
    pub(super) fn click(&mut self, params: Value) -> EngineResult<Value> {
        let input: Click = parameters(params)?;
        let page = self.current()?.clone();
        let node = target(
            &page,
            &input.page_id,
            &input.node_id,
            input.expected_revision,
        )?;
        if node.disabled || !node.style.interactive {
            return Err(invalid_target());
        }
        if node.tag == "a" {
            return self.follow(node.href.as_deref().ok_or_else(invalid_target)?);
        }
        if node.tag == "label" {
            let control = page
                .document
                .nodes
                .iter()
                .find(|n| {
                    node.attributes.get("for") == Some(&n.html_id)
                        || n.parent == Some(node.local_id)
                            && ["input", "select", "textarea"].contains(&n.tag.as_str())
                })
                .cloned()
                .ok_or_else(invalid_target)?;
            if control.disabled || control.hidden() {
                return Err(invalid_target());
            }
            return self.activate_control(page, control);
        }
        if node.tag == "input" {
            return self.activate_control(page, node);
        }
        if node.tag != "button" {
            return Err(invalid_target());
        }
        self.button(page, node)
    }
    /** V9 gets only committed globals and current input values, never an agent permit. */
    fn button(&mut self, mut page: Page, node: Node) -> EngineResult<Value> {
        if node.handler.is_none()
            && node.input_type() == "submit"
            && crate::document::form_owner(&page.document, &node).is_some()
        {
            return self.submit_node(&page, node.local_id);
        }
        let Some(handler) = node.handler else {
            return Ok(page.inspect());
        };
        let sources = page.scripts.clone();
        let storage = self.storage_for(&page.origin)?;
        let value=self.call("v9","v9.event",json!({"sources":sources,"handler":handler,"state":page.state,"inputs":input_values(&page.document),"storage":storage})).map_err(|mut error|{if error.phase.is_some(){error.hint="Correct the script. No script changes were applied.".into();}error})?;
        let execution: Execution = self.returned("v9", value)?;
        page.document = self.apply(&page.document, &execution.changes)?;
        page.state = execution.state;
        page.storage_dirty = execution.storage != storage;
        page.storage = execution.storage;
        self.finish_mutation(page)
    }
    /** Native input clicks share form/radio behavior with agent activation. */
    fn activate_control(&mut self, mut page: Page, node: Node) -> EngineResult<Value> {
        if ["checkbox", "radio"].contains(&node.input_type()) {
            page.document = crate::document::set_checked(
                &page.document,
                node.local_id,
                if node.input_type() == "radio" {
                    true
                } else {
                    !node.checked
                },
            )?;
            return self.finish_mutation(page);
        }
        if node.input_type() == "submit" {
            return self.submit_node(&page, node.local_id);
        }
        Err(invalid_target())
    }
    /** Explicit state setters are idempotent and retain ordinary revision checks. */
    pub(super) fn check(&mut self, params: Value) -> EngineResult<Value> {
        let input: Checked = parameters(params)?;
        let mut page = self.current()?.clone();
        let node = target(
            &page,
            &input.page_id,
            &input.node_id,
            input.expected_revision,
        )?;
        page.document = crate::document::set_checked(&page.document, node.local_id, input.checked)?;
        self.finish_mutation(page)
    }
    /** The selected option must belong to the exact current select. */
    pub(super) fn select_option(&mut self, params: Value) -> EngineResult<Value> {
        let input: Selection = parameters(params)?;
        let mut page = self.current()?.clone();
        let node = target(
            &page,
            &input.page_id,
            &input.node_id,
            input.expected_revision,
        )?;
        let option = page
            .document
            .nodes
            .iter()
            .find(|n| page.node_id(n.local_id) == input.option_id)
            .ok_or_else(invalid_target)?
            .local_id;
        page.document = crate::document::select(&page.document, node.local_id, option)?;
        self.finish_mutation(page)
    }
    /** Enter and explicit tool submission share the exact same successful-control algorithm. */
    pub(super) fn submit(&mut self, params: Value) -> EngineResult<Value> {
        let input: Click = parameters(params)?;
        let page = self.current()?.clone();
        let node = target(
            &page,
            &input.page_id,
            &input.node_id,
            input.expected_revision,
        )?;
        self.submit_node(&page, node.local_id)
    }
    /** Implicit submission chooses the first enabled submitter in document order. */
    fn submit_node(&mut self, page: &Page, id: usize) -> EngineResult<Value> {
        let node = page
            .document
            .nodes
            .iter()
            .find(|n| n.local_id == id)
            .ok_or_else(invalid_target)?;
        let form = if node.tag == "form" {
            id
        } else {
            crate::document::form_owner(&page.document, node).ok_or_else(invalid_target)?
        };
        let submitter =
            if node.input_type() == "submit" && ["input", "button"].contains(&node.tag.as_str()) {
                Some(id)
            } else {
                page.document
                    .nodes
                    .iter()
                    .find(|n| {
                        ["button", "input"].contains(&n.tag.as_str())
                            && n.input_type() == "submit"
                            && crate::document::form_owner(&page.document, n) == Some(form)
                    })
                    .map(|n| n.local_id)
            };
        let proposal = match crate::document::submission(&page.document, form, submitter) {
            Ok(proposal) => proposal,
            Err(error) => {
                if let Some(id) = error.invalid_node.as_deref() {
                    let mut candidate = page.clone();
                    if let Some(node) = candidate
                        .document
                        .nodes
                        .iter_mut()
                        .find(|n| n.local_id == *id)
                    {
                        node.validation = Some("This field is required.".into());
                    }
                    self.finish_mutation(candidate)?;
                }
                return Err(error);
            }
        };
        self.navigate_submission(proposal)
    }
    /** Expensive execution/layout occurs before the exact authority/commit boundary. */
    fn finish_mutation(&mut self, mut page: Page) -> EngineResult<Value> {
        page.revision = page
            .revision
            .checked_add(1)
            .ok_or_else(|| EngineError::limit("document revisions", usize::MAX))?;
        page.layout_generation = page.layout_generation.saturating_add(1);
        self.prepare(&mut page)?;
        let rollback = if page.storage_dirty {
            self.write_storage(&page.origin, &page.storage)?
        } else {
            None
        };
        let origin = page.origin.clone();
        let storage = page.storage.clone();
        let published = self.publish(move |runtime| {
            let mut page = page;
            if page.storage_dirty {
                runtime.commit_storage(&origin, &storage);
            }
            page.storage_dirty = false;
            runtime.page = Some(page);
            runtime.clamp_scroll();
            runtime.inspection()
        });
        if published.is_err()
            && let Some(previous) = rollback
        {
            self.rollback_storage(previous);
        }
        published
    }
}

/** Target identity and revision are validated before any service or navigation action. */
fn target(page: &Page, page_id: &str, node_id: &str, revision: u64) -> EngineResult<Node> {
    if page.id() != page_id {
        return Err(invalid_target());
    }
    let node = page
        .document
        .nodes
        .iter()
        .find(|node| page.node_id(node.local_id) == node_id)
        .cloned()
        .ok_or_else(invalid_target)?;
    if node.hidden() || node.disabled || !node.style.interactive {
        return Err(invalid_target());
    }
    if page.revision != revision {
        return Err(EngineError::new(
            "revision_conflict",
            "The document revision has changed.",
            "Inspect the current page before changing it.",
        ));
    }
    Ok(node)
}
/** Compact mutation acknowledgments retain the original API's top-level text/value fields. */
fn edit_result(inspection: Value, node_id: &str) -> Value {
    let node = inspection["nodes"]
        .as_array()
        .and_then(|nodes| nodes.iter().find(|node| node["node_id"] == node_id))
        .cloned()
        .unwrap_or_default();
    json!({"page_id":inspection["page_id"],"revision":inspection["revision"],"node_id":node_id,"text":node["text"],"value":node["value"]})
}
/** Disabled, missing and noninteractive nodes never fall through to another input mechanism. */
fn invalid_target() -> EngineError {
    EngineError::new(
        "invalid_target",
        "The target is not an enabled interactive node in this document.",
        "Inspect the current page and choose a supported link, button or input.",
    )
}
