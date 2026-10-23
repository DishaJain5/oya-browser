/** Request-boundary tracing reclaims cycles without requiring evaluator-temporary roots. */
use super::*;
use crate::v9::value::Heap;
use std::collections::BTreeSet;
/** Explicit edge kinds keep tracing iterative even through mutually recursive closures and objects. */
enum Edge {
    Value(JsValue),
    Environment(Id),
}
impl Graph {
    /** No collection happens while a callee, return value or partially-built literal lives only in an evaluator temporary. */
    pub fn collect(
        &mut self,
        heap: &mut Heap,
        roots: impl Iterator<Item = JsValue>,
    ) -> EngineResult<()> {
        let mut work = roots.map(Edge::Value).collect::<Vec<_>>();
        work.extend(
            (0..crate::constants::VM_FUNCTION_INTRINSICS)
                .map(|id| Edge::Value(JsValue::Function(id))),
        );
        let (mut objects, mut functions, mut environments) =
            (BTreeSet::new(), BTreeSet::new(), BTreeSet::new());
        while let Some(edge) = work.pop() {
            match edge {
                Edge::Value(JsValue::Object(id)) if objects.insert(id) => {
                    work.extend(heap.references(id)?.into_iter().map(Edge::Value))
                }
                Edge::Value(JsValue::Function(id)) if functions.insert(id) => {
                    let function = self.functions.get(&id).ok_or_else(invalid)?;
                    work.extend(function.references().into_iter().map(Edge::Value));
                    if let Callable::Closure {
                        environment: Some(env),
                        ..
                    } = &function.kind
                    {
                        work.push(Edge::Environment(*env));
                    }
                }
                Edge::Environment(id) if environments.insert(id) => {
                    let env = self.get(id)?;
                    if let Some(value) = &env.receiver {
                        work.push(Edge::Value(value.clone()));
                    }
                    if let Some(parent) = env.parent {
                        work.push(Edge::Environment(parent));
                    }
                    work.extend(
                        env.bindings
                            .values()
                            .filter_map(|cell| cell.value.clone())
                            .map(Edge::Value),
                    );
                }
                _ => {}
            }
        }
        heap.sweep(&objects);
        self.functions.retain(|id, _| functions.contains(id));
        self.environments.retain(|id, _| environments.contains(id));
        Ok(())
    }
}
