/** Prepared references retain one target identity while callbacks execute before candidate stores. */
use super::{environment::Cell, *};
/** References live only during synchronous evaluation and never cross the private state boundary. */
pub(super) enum Reference {
    Identifier {
        name: String,
        target: EngineResult<Option<Cell>>,
        location: SourceLocation,
    },
    Property {
        receiver: JsValue,
        key: String,
        location: SourceLocation,
    },
}
impl Interpreter {
    /** Preparing a target charges its node without reading a plain-assignment identifier's old value. */
    pub(super) fn prepare_reference(&mut self, target: Expression) -> EngineResult<Reference> {
        self.step(target.location)?;
        match target.kind {
            ExpressionKind::Variable(name) => Ok(self.identifier_reference(name, target.location)),
            ExpressionKind::Member(base, key) => {
                let (receiver, key) = self.property_reference(*base, *key, target.location)?;
                Ok(Reference::Property {
                    receiver,
                    key,
                    location: target.location,
                })
            }
            _ => Err(errors::execution(
                target.location,
                "Invalid assignment target",
            )),
        }
    }
    /** Resolution failures remain deferred until the operation actually reads or stores the reference. */
    pub(super) fn identifier_reference(&self, name: String, location: SourceLocation) -> Reference {
        let target = self.resolve(&name, location);
        Reference::Identifier {
            name,
            target,
            location,
        }
    }
    /** Compound and update operations obtain the old value before any right-hand side executes. */
    pub(super) fn read_reference(&self, reference: &Reference) -> EngineResult<JsValue> {
        match reference {
            Reference::Identifier {
                name,
                target,
                location,
            } => match target.clone()? {
                Some(cell) => self.read_cell(cell, name, *location),
                None => self.unresolved_identifier(name, *location),
            },
            Reference::Property {
                receiver,
                key,
                location,
            } => self.get_property(receiver, key, *location),
        }
    }
    /** All assignment forms retain the existing global, private-name, readonly and quota protections. */
    pub(super) fn write_reference(
        &mut self,
        reference: Reference,
        value: JsValue,
    ) -> EngineResult<()> {
        match reference {
            Reference::Identifier {
                name,
                target,
                location,
            } => self.store_identifier(name, target?, value, location),
            Reference::Property {
                receiver,
                key,
                location,
            } => self.set_property(&receiver, key, value, location),
        }
    }
    /** Stable identifier cells are checked for initialization and mutability only at the store boundary. */
    fn store_identifier(
        &mut self,
        name: String,
        target: Option<Cell>,
        value: JsValue,
        location: SourceLocation,
    ) -> EngineResult<()> {
        if matches!(target, Some(Cell::GlobalProperty | Cell::BuiltinFunction))
            || target.is_none() && !self.strict
        {
            return self.set_property(&JsValue::Object(self.global_object), name, value, location);
        }
        if target.is_none() && properties::protected_global(&name) {
            return Err(errors::unsupported(
                location,
                "writes to protected global properties",
            ));
        }
        let target = target.ok_or_else(|| errors::execution(location, "Binding is not defined"))?;
        if let Cell::Environment(id) = target
            && self.graph.get(id)?.scope_id.kind == ScopeKind::Named
        {
            return Err(errors::unsupported(
                location,
                "assignment to a named function expression's private name",
            ));
        }
        let cell = self.cell_mut(target, &name);
        if cell.value.is_none() {
            return Err(errors::execution(
                location,
                "Cannot access a lexical binding before initialization",
            ));
        }
        if cell.kind == BindingKind::Const {
            return Err(errors::execution(
                location,
                "Cannot assign to a const binding",
            ));
        }
        cell.value = Some(value);
        self.environment()
            .map_err(|error| errors::located(error, location, "execute"))
    }
}
