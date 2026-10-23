/** Function values capture lexical identities and invoke source-owned code without native callbacks. */
use super::*;
impl Interpreter {
    /** A named expression owns a private self environment outside its later parameter activation. */
    pub(super) fn closure(
        &mut self,
        site: usize,
        location: SourceLocation,
    ) -> EngineResult<JsValue> {
        let code = self
            .registry
            .codes
            .get(&site)
            .cloned()
            .ok_or_else(super::super::registry::invalid)?;
        let parent = self.locals.last().copied();
        let lexical_this = (code.kind == super::super::parser::FunctionKind::Arrow)
            .then(|| self.current_receiver())
            .transpose()?;
        let environment = if code.name.is_some() {
            Some(
                self.new_scope(
                    ScopeId {
                        site,
                        kind: ScopeKind::Named,
                    },
                    parent,
                )
                .map_err(|error| errors::located(error, location, "execute"))?,
            )
        } else {
            parent
        };
        let value = self
            .graph
            .function(site, environment, lexical_this)
            .map_err(|error| errors::located(error, location, "execute"))?;
        if let Some(name) = code.name {
            self.graph
                .get_mut(environment.expect("private named scope"))?
                .bindings
                .get_mut(&name)
                .expect("private name schema")
                .value = Some(value.clone());
        }
        self.environment()
            .map_err(|error| errors::located(error, location, "execute"))?;
        Ok(value)
    }
    /** The restricted event front door resolves a named author function before argument evaluation. */
    pub(super) fn call(
        &mut self,
        name: &str,
        arguments: Vec<Expression>,
        location: SourceLocation,
    ) -> EngineResult<JsValue> {
        let value = self.variable(name, location)?;
        let arguments = self.arguments(arguments)?;
        self.call_value(value, JsValue::Undefined, arguments, location)
    }
    /** Supported expression callees are read exactly once before arguments, including returned or stored callbacks. */
    pub(super) fn invoke(
        &mut self,
        callee: Expression,
        arguments: Vec<Expression>,
        location: SourceLocation,
    ) -> EngineResult<JsValue> {
        let (value, receiver) = if let ExpressionKind::Member(base, key) = callee.kind {
            self.step(callee.location)?;
            let (receiver, key) = self.property_reference(*base, *key, callee.location)?;
            (
                self.get_property(&receiver, &key, callee.location)?,
                receiver,
            )
        } else {
            (self.expression(callee)?, JsValue::Undefined)
        };
        let arguments = self.arguments(arguments)?;
        self.call_value(value, receiver, arguments, location)
    }
    /** Source execution installs one activation after the common callable dispatcher has charged the call. */
    pub(super) fn call_closure(
        &mut self,
        code_id: usize,
        environment: Option<Id>,
        lexical_this: Option<JsValue>,
        receiver: JsValue,
        arguments: Vec<JsValue>,
        location: SourceLocation,
    ) -> EngineResult<JsValue> {
        let code = self
            .registry
            .codes
            .get(&code_id)
            .cloned()
            .ok_or_else(super::super::registry::invalid)?;
        let activation = self
            .new_scope(
                ScopeId {
                    site: code_id,
                    kind: ScopeKind::Function,
                },
                environment,
            )
            .map_err(|error| errors::located(error, location, "execute"))?;
        let receiver = self.call_receiver(lexical_this, &code, receiver, location)?;
        self.graph.get_mut(activation)?.receiver = Some(receiver);
        self.run_activation(activation, code, arguments, location)
    }
    /** Cleanup restores the caller's lexical stack and language mode on both normal and abrupt return. */
    fn run_activation(
        &mut self,
        activation: Id,
        code: super::super::registry::Code,
        arguments: Vec<JsValue>,
        location: SourceLocation,
    ) -> EngineResult<JsValue> {
        let base = self.locals.len();
        let strict = self.strict;
        self.strict = code.strict;
        self.locals.push(activation);
        let result = self
            .bind_parameters(&code.parameters, arguments, location)
            .and_then(|()| self.hoist(&code.body, false))
            .and_then(|()| self.statements(code.body))
            .map(|completion| match completion {
                Completion::Return(value) => value,
                _ => JsValue::Undefined,
            });
        self.locals.truncate(base);
        self.strict = strict;
        result
    }
    /** Missing arguments initialize to undefined before nested declarations are hoisted. */
    fn bind_parameters(
        &mut self,
        parameters: &[String],
        arguments: Vec<JsValue>,
        location: SourceLocation,
    ) -> EngineResult<()> {
        let id = *self.locals.last().expect("active function context");
        let bindings = &mut self.graph.get_mut(id)?.bindings;
        for (index, name) in parameters.iter().enumerate() {
            bindings.get_mut(name).expect("parameter schema").value =
                Some(arguments.get(index).cloned().unwrap_or(JsValue::Undefined));
        }
        self.environment()
            .map_err(|error| errors::located(error, location, "execute"))
    }
    /** Argument callbacks execute in left-to-right caller order without losing temporary identities. */
    fn arguments(&mut self, arguments: Vec<Expression>) -> EngineResult<Vec<JsValue>> {
        arguments
            .into_iter()
            .map(|argument| self.expression(argument))
            .collect()
    }
}
