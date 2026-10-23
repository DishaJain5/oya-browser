/** Private listener and event records retain only document-local identities and owned callables. */
use super::{
    JsValue, VmState,
    value::{HostKind, HostRef},
};
use crate::{
    constants::{JS_EVENT_LISTENERS, JS_EVENT_RECORDS},
    document::ScriptDocument,
    protocol::{EngineError, EngineResult},
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
/** Ordered registrations use never-reused IDs so dispatch additions cannot enter an existing snapshot. */
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Listener {
    /// Monotonic identity distinguishes a re-added callback from the dispatch snapshot.
    pub id: u32,
    /// Stable element identity belongs to the exact projected document.
    pub target: usize,
    /// Only an owned callable arena record can be registered.
    pub callback: u32,
}
/** Current-target authority belongs to the active interpreter, never serialized event data. */
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EventRecord {
    /// Stable element identity belongs to the exact projected document.
    pub target: usize,
    /// Cancellation remains visible to later listeners and retained event references.
    pub default_prevented: bool,
}
/** A page retains registrations and reachable event records without retaining projected field values. */
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DomState {
    /// Navigation and tab identity bind every serialized host reference.
    pub identity: String,
    /// Registration order determines each click snapshot.
    pub listeners: Vec<Listener>,
    /// Only reachable host event references retain records at finish.
    pub events: BTreeMap<u32, EventRecord>,
    /// Removed registration identities are never recycled.
    pub next_listener: u32,
    /// Collected event identities are never recycled.
    pub next_event: u32,
}
impl DomState {
    /** Fresh page identity makes reload and tab references mutually unforgeable by normal source. */
    pub fn new(identity: String) -> Self {
        Self {
            identity,
            listeners: Vec::new(),
            events: BTreeMap::new(),
            next_listener: 0,
            next_event: 0,
        }
    }
    /** Record counts and monotonic identities are checked before any dispatch or reclamation. */
    pub fn validate(&self) -> EngineResult<()> {
        if self.listeners.len() > JS_EVENT_LISTENERS || self.events.len() > JS_EVENT_RECORDS {
            return Err(EngineError::limit(
                "V9 listeners or events",
                JS_EVENT_LISTENERS,
            ));
        }
        let mut pairs = BTreeSet::new();
        let mut previous = None;
        for listener in &self.listeners {
            if listener.id >= self.next_listener
                || previous.is_some_and(|id| id >= listener.id)
                || !pairs.insert((listener.target, listener.callback))
            {
                return Err(invalid());
            }
            previous = Some(listener.id);
        }
        if self.events.keys().any(|id| *id >= self.next_event) {
            return Err(invalid());
        }
        Ok(())
    }
    /** Host values preserve exact identity and never infer ownership from numeric ID ranges. */
    pub fn validate_host(&self, host: &HostRef, document: &ScriptDocument) -> EngineResult<()> {
        if host.document != self.identity {
            return Err(invalid());
        }
        match host.kind {
            HostKind::Document => Ok(()),
            HostKind::Element { local_id }
                if document.nodes.iter().any(|node| node.local_id == local_id) =>
            {
                Ok(())
            }
            HostKind::Event { id } if self.events.contains_key(&id) => Ok(()),
            _ => Err(invalid()),
        }
    }
}
impl VmState {
    /** Coordinator validation binds every retained arena to the exact current document. */
    pub fn validate_document(&self, document: &ScriptDocument) -> EngineResult<()> {
        document.validate()?;
        self.validate()?;
        let dom = self.dom.as_ref().ok_or_else(invalid)?;
        dom.validate()?;
        if dom.identity != document.identity {
            return Err(invalid());
        }
        for listener in &dom.listeners {
            if !document
                .nodes
                .iter()
                .any(|node| node.local_id == listener.target && node.click_listener)
                || !self.graph.functions.contains_key(&listener.callback)
            {
                return Err(invalid());
            }
        }
        for event in dom.events.values() {
            if !document
                .nodes
                .iter()
                .any(|node| node.local_id == event.target)
            {
                return Err(invalid());
            }
        }
        for value in self.values() {
            if let JsValue::Host(host) = value {
                dom.validate_host(&host, document)?;
            }
        }
        Ok(())
    }
    /** Inspection exposes only presence and never executes a callback. */
    pub fn has_click_listener(&self, identity: &str, local_id: usize) -> bool {
        self.dom.as_ref().is_some_and(|dom| {
            dom.identity == identity
                && dom
                    .listeners
                    .iter()
                    .any(|listener| listener.target == local_id)
        })
    }
    /** All arenas are inspected, including unreachable hostile records that collection would discard. */
    pub(super) fn values(&self) -> Vec<JsValue> {
        let mut values = self
            .globals
            .values()
            .chain(self.functions.values())
            .filter_map(|cell| cell.value.clone())
            .collect::<Vec<_>>();
        values.extend(self.heap.all_references().cloned());
        values.extend(self.graph.values());
        values
    }
    /** Source-only compatibility APIs cannot be used to smuggle document capabilities. */
    pub(super) fn validate_legacy(&self) -> EngineResult<()> {
        if self.dom.is_some()
            || self
                .values()
                .iter()
                .any(|value| matches!(value, JsValue::Host(_)))
        {
            return Err(invalid());
        }
        Ok(())
    }
}
/** Malformed incoming snapshots differ from source-created unsupported detached DOM state. */
fn invalid() -> EngineError {
    EngineError::invalid("Invalid V9 document, listener or event state.")
}
