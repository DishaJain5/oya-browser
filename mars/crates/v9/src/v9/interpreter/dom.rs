/** Canonical DOM intrinsics use the ordinary callable dispatcher and one candidate work budget. */
use super::*;
use crate::{
    constants::JS_EVENT_LISTENERS,
    v9::{dom::Listener, value::HostKind},
};
impl Interpreter {
    /** Detached methods must retain the right host brand even through call, apply and bind. */
    pub(super) fn host_intrinsic(
        &mut self,
        intrinsic: Intrinsic,
        target: JsValue,
        arguments: Vec<JsValue>,
        location: SourceLocation,
    ) -> EngineResult<JsValue> {
        let host = self.host_reference(target, location)?;
        match (intrinsic, host.kind) {
            (Intrinsic::GetElementById, HostKind::Document) => {
                let id = arguments
                    .first()
                    .unwrap_or(&JsValue::Undefined)
                    .to_dom()
                    .map_err(|error| errors::located(error, location, "dom"))?;
                let document = self.document.as_ref().expect("projection");
                Ok(document
                    .get_by_id(&id)
                    .map(|local_id| {
                        self.host_value(HostKind::Element { local_id }, &document.identity)
                    })
                    .unwrap_or(JsValue::Null))
            }
            (Intrinsic::AddEventListener, HostKind::Element { local_id }) => {
                self.listener_change(local_id, arguments, true, location)
            }
            (Intrinsic::RemoveEventListener, HostKind::Element { local_id }) => {
                self.listener_change(local_id, arguments, false, location)
            }
            (Intrinsic::PreventDefault, HostKind::Event { id }) => {
                self.dom
                    .as_mut()
                    .expect("event DOM")
                    .events
                    .get_mut(&id)
                    .ok_or_else(super::super::registry::invalid)?
                    .default_prevented = true;
                Ok(JsValue::Undefined)
            }
            _ => Err(errors::execution(
                location,
                "Incompatible host method receiver",
            )),
        }
    }
    /** Registrations preserve callback identity and source order; additions never reuse removed IDs. */
    fn listener_change(
        &mut self,
        target: usize,
        arguments: Vec<JsValue>,
        add: bool,
        location: SourceLocation,
    ) -> EngineResult<JsValue> {
        self.step(location)?;
        let event_type = arguments
            .first()
            .unwrap_or(&JsValue::Undefined)
            .to_dom()
            .map_err(|error| errors::located(error, location, "dom"))?;
        if event_type != "click" {
            return Err(errors::unsupported(
                location,
                "event types other than click",
            ));
        }
        if arguments
            .get(2)
            .is_some_and(|value| !matches!(value, JsValue::Undefined))
        {
            return Err(errors::unsupported(location, "event listener options"));
        }
        let callback = arguments.get(1).unwrap_or(&JsValue::Undefined);
        if matches!(callback, JsValue::Null | JsValue::Undefined) {
            return Ok(JsValue::Undefined);
        }
        let callback = match callback {
            JsValue::Function(id) if self.graph.functions.contains_key(id) => *id,
            _ => {
                return Err(errors::unsupported(
                    location,
                    "non-function event listeners",
                ));
            }
        };
        if !self
            .document
            .as_ref()
            .expect("projection")
            .nodes
            .iter()
            .any(|node| node.local_id == target && node.click_listener)
        {
            return Err(errors::unsupported(
                location,
                "programmatic listeners on this button",
            ));
        }
        let dom = self.dom.as_mut().expect("projection DOM");
        let existing = dom
            .listeners
            .iter()
            .position(|listener| listener.target == target && listener.callback == callback);
        if !add {
            if let Some(index) = existing {
                dom.listeners.remove(index);
            }
            return Ok(JsValue::Undefined);
        }
        if existing.is_some() {
            return Ok(JsValue::Undefined);
        }
        if dom.listeners.len() >= JS_EVENT_LISTENERS {
            return Err(EngineError::limit("V9 event listeners", JS_EVENT_LISTENERS));
        }
        let id = dom.next_listener;
        dom.next_listener = id
            .checked_add(1)
            .ok_or_else(|| EngineError::limit("V9 listener identities", JS_EVENT_LISTENERS))?;
        dom.listeners.push(Listener {
            id,
            target,
            callback,
        });
        self.environment()?;
        Ok(JsValue::Undefined)
    }
}
