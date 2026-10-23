/** All callable variants share invocation limits and source-owned receiver conversion. */
use super::*;
use crate::constants::{VM_BOUND_HOPS, VM_CALL_ARGUMENTS};
impl Interpreter {
    /** Intrinsic recursion and bound adaptation consume the same depth allowance as source functions. */
    pub(super) fn call_value(
        &mut self,
        value: JsValue,
        receiver: JsValue,
        arguments: Vec<JsValue>,
        location: SourceLocation,
    ) -> EngineResult<JsValue> {
        let depth = self.function_depth;
        let result = self
            .charge_call(arguments.len(), location)
            .and_then(|()| self.dispatch_callable(value, receiver, arguments, location));
        self.function_depth = depth;
        result
    }
    /** Bound chains are followed iteratively so serialized chains cannot grow the Rust stack. */
    fn dispatch_callable(
        &mut self,
        mut value: JsValue,
        mut receiver: JsValue,
        mut arguments: Vec<JsValue>,
        location: SourceLocation,
    ) -> EngineResult<JsValue> {
        let mut hops = 0;
        loop {
            let id = self.callable_id(&value, location)?;
            match self.graph.functions[&id].kind.clone() {
                Callable::Closure {
                    code_id,
                    environment,
                    lexical_this,
                } => {
                    return self.call_closure(
                        code_id,
                        environment,
                        lexical_this,
                        receiver,
                        arguments,
                        location,
                    );
                }
                Callable::Intrinsic { intrinsic } => {
                    return self.intrinsic(intrinsic, receiver, arguments, location);
                }
                Callable::Bound {
                    target,
                    receiver: bound_this,
                    arguments: prefix,
                    ..
                } => {
                    self.step(location)?;
                    hops += 1;
                    if hops > VM_BOUND_HOPS {
                        return Err(errors::located(
                            EngineError::limit("V9 bound chain", VM_BOUND_HOPS),
                            location,
                            "execute",
                        ));
                    }
                    self.argument_count(prefix.len() + arguments.len(), location)?;
                    arguments.splice(0..0, prefix);
                    receiver = bound_this;
                    value = JsValue::Function(target);
                }
            }
        }
    }
    /** Every adapter hop charges work before dispatch and rejects oversized lists before further allocation. */
    fn charge_call(&mut self, arguments: usize, location: SourceLocation) -> EngineResult<()> {
        self.step(location)?;
        self.argument_count(arguments, location)?;
        if self.function_depth >= CALL_DEPTH {
            return Err(errors::located(
                EngineError::limit("V9 call depth", CALL_DEPTH),
                location,
                "execute",
            ));
        }
        self.function_depth += 1;
        Ok(())
    }
    /** The bounded argument list applies to direct, intrinsic and accumulated bound calls alike. */
    pub(super) fn argument_count(
        &self,
        count: usize,
        location: SourceLocation,
    ) -> EngineResult<()> {
        if count > VM_CALL_ARGUMENTS {
            return Err(errors::located(
                EngineError::limit("V9 call arguments", VM_CALL_ARGUMENTS),
                location,
                "execute",
            ));
        }
        Ok(())
    }
    /** A function tag is callable only while its owned arena record exists. */
    pub(super) fn callable_id(
        &self,
        value: &JsValue,
        location: SourceLocation,
    ) -> EngineResult<Id> {
        match value {
            JsValue::Function(id) if self.graph.functions.contains_key(id) => Ok(*id),
            _ => Err(errors::execution(location, "The value is not callable")),
        }
    }
    /** First-class methods adapt page values and never evaluate source or invoke host callbacks. */
    fn intrinsic(
        &mut self,
        intrinsic: Intrinsic,
        target: JsValue,
        arguments: Vec<JsValue>,
        location: SourceLocation,
    ) -> EngineResult<JsValue> {
        match intrinsic {
            Intrinsic::FunctionConstructor => Err(errors::unsupported(
                location,
                "dynamic Function construction",
            )),
            Intrinsic::FunctionPrototype => Ok(JsValue::Undefined),
            Intrinsic::Call => self.function_call(target, arguments, location),
            Intrinsic::Apply => self.function_apply(target, arguments, location),
            Intrinsic::Bind => self.function_bind(target, arguments, location),
        }
    }
    /** Call validates its method receiver before forwarding its first value as the target receiver. */
    fn function_call(
        &mut self,
        target: JsValue,
        arguments: Vec<JsValue>,
        location: SourceLocation,
    ) -> EngineResult<JsValue> {
        self.callable_id(&target, location)?;
        let mut arguments = arguments.into_iter();
        let receiver = arguments.next().unwrap_or(JsValue::Undefined);
        self.call_value(target, receiver, arguments.collect(), location)
    }
    /** Binding records preserve inner receivers and source metadata without flattening identity. */
    fn function_bind(
        &mut self,
        target: JsValue,
        arguments: Vec<JsValue>,
        location: SourceLocation,
    ) -> EngineResult<JsValue> {
        let target = self.callable_id(&target, location)?;
        let mut arguments = arguments.into_iter();
        let receiver = arguments.next().unwrap_or(JsValue::Undefined);
        let value = self
            .graph
            .bound(target, receiver, arguments.collect(), &self.registry)
            .map_err(|error| errors::located(error, location, "execute"))?;
        self.environment()
            .map_err(|error| errors::located(error, location, "execute"))?;
        Ok(value)
    }
}
