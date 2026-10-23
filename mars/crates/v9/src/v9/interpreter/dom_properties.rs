/** The host property surface exposes bounded page data and canonical methods without native reflection. */
use super::*;
use crate::v9::value::{HostKind, HostRef};
impl Interpreter {
    /** Restricted inline reads resolve current projected membership and first-match identity after earlier arguments run. */
    pub(super) fn input_value(&self, id: &str, location: SourceLocation) -> EngineResult<JsValue> {
        let Some(document) = &self.document else {
            return self
                .inputs
                .get(id)
                .cloned()
                .ok_or_else(|| errors::execution(location, "input is unavailable or protected"));
        };
        let local_id = document
            .get_by_id(id)
            .ok_or_else(|| errors::execution(location, "Cannot read a property of null"))?;
        let host = HostRef {
            document: document.identity.clone(),
            kind: HostKind::Element { local_id },
        };
        self.host_property(&host, "value", location)
    }
    /** Document aliases share exact identity rather than creating wrapper objects. */
    pub(super) fn document_value(&self) -> EngineResult<JsValue> {
        let document = self
            .document
            .as_ref()
            .ok_or_else(super::super::registry::invalid)?;
        Ok(self.host_value(HostKind::Document, &document.identity))
    }
    /** Creating a wrapper never allocates native state or grants authority. */
    pub(super) fn host_value(&self, kind: HostKind, document: &str) -> JsValue {
        JsValue::Host(HostRef {
            document: document.into(),
            kind,
        })
    }
    /** Brand checks apply equally to direct and detached intrinsic calls. */
    pub(super) fn host_reference(
        &self,
        value: JsValue,
        location: SourceLocation,
    ) -> EngineResult<HostRef> {
        let JsValue::Host(host) = value else {
            return Err(errors::execution(
                location,
                "Incompatible host method receiver",
            ));
        };
        let document = self
            .document
            .as_ref()
            .ok_or_else(super::super::registry::invalid)?;
        self.dom
            .as_ref()
            .ok_or_else(super::super::registry::invalid)?
            .validate_host(&host, document)?;
        Ok(host)
    }
    /** Element values come from the current projection, never stale serialized input text. */
    pub(super) fn host_property(
        &self,
        host: &HostRef,
        key: &str,
        location: SourceLocation,
    ) -> EngineResult<JsValue> {
        self.host_reference(JsValue::Host(host.clone()), location)?;
        let intrinsic = match (&host.kind, key) {
            (HostKind::Document, "getElementById") => Some(Intrinsic::GetElementById),
            (HostKind::Element { .. }, "addEventListener") => Some(Intrinsic::AddEventListener),
            (HostKind::Element { .. }, "removeEventListener") => {
                Some(Intrinsic::RemoveEventListener)
            }
            (HostKind::Event { .. }, "preventDefault") => Some(Intrinsic::PreventDefault),
            _ => None,
        };
        if let Some(intrinsic) = intrinsic {
            return Ok(JsValue::Function(intrinsic.id()));
        }
        match (&host.kind, key) {
            (HostKind::Element { local_id }, "value") => {
                let node = self
                    .document
                    .as_ref()
                    .expect("projection")
                    .nodes
                    .iter()
                    .find(|node| node.local_id == *local_id)
                    .ok_or_else(super::super::registry::invalid)?;
                node.value
                    .as_deref()
                    .map(JsValue::string)
                    .unwrap_or_else(|| {
                        Err(errors::unsupported(
                            location,
                            "unavailable or protected element values",
                        ))
                    })
            }
            (HostKind::Event { id }, _) => self.event_property(*id, key, location),
            _ => Err(errors::unsupported(location, "this host property")),
        }
    }
    /** Retained events lose currentTarget immediately after their synchronous listener returns. */
    fn event_property(
        &self,
        id: u32,
        key: &str,
        location: SourceLocation,
    ) -> EngineResult<JsValue> {
        let dom = self.dom.as_ref().expect("event DOM");
        let event = dom
            .events
            .get(&id)
            .ok_or_else(super::super::registry::invalid)?;
        match key {
            "type" => JsValue::string("click"),
            "target" => Ok(self.host_value(
                HostKind::Element {
                    local_id: event.target,
                },
                &dom.identity,
            )),
            "currentTarget" if self.active_event == Some(id) => Ok(self.host_value(
                HostKind::Element {
                    local_id: event.target,
                },
                &dom.identity,
            )),
            "currentTarget" => Ok(JsValue::Null),
            "defaultPrevented" => Ok(JsValue::Boolean(event.default_prevented)),
            _ => Err(errors::unsupported(location, "this event property")),
        }
    }
    /** Only textContent writes are supported; document independently validates the resulting proposal. */
    pub(super) fn set_host_property(
        &mut self,
        host: &HostRef,
        key: String,
        value: JsValue,
        location: SourceLocation,
    ) -> EngineResult<()> {
        self.host_reference(JsValue::Host(host.clone()), location)?;
        let HostKind::Element { local_id } = host.kind else {
            return Err(errors::unsupported(
                location,
                "writes to readonly host properties",
            ));
        };
        if key != "textContent" {
            return Err(errors::unsupported(
                location,
                "writes to readonly or unsupported element properties",
            ));
        }
        let text = value
            .to_dom()
            .map_err(|error| errors::located(error, location, "dom"))?;
        if self.changes.len() >= STATEMENTS {
            return Err(EngineError::limit("V9 changes", STATEMENTS));
        }
        let change = self
            .document
            .as_mut()
            .expect("projection")
            .set_text(local_id, text)
            .map_err(|error| errors::located(error, location, "dom"))?;
        self.changes.push(change);
        Ok(())
    }
}
