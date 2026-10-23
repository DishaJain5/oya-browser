/** Shared edge enumeration keeps function tracing and retained string accounting consistent. */
use super::*;
impl Function {
    /** Private receiver, prefixes and expandos remain subject to the page's string budget. */
    pub(super) fn string_units(&self) -> usize {
        let values = self
            .properties
            .iter()
            .map(|(key, value)| key.encode_utf16().count() + value.string_units())
            .sum::<usize>();
        values
            + match &self.kind {
                Callable::Closure { lexical_this, .. } => {
                    lexical_this.as_ref().map_or(0, JsValue::string_units)
                }
                Callable::Bound {
                    receiver,
                    arguments,
                    name,
                    ..
                } => {
                    receiver.string_units()
                        + name.len()
                        + arguments.iter().map(JsValue::string_units).sum::<usize>()
                }
                Callable::Intrinsic { .. } => 0,
            }
    }
    /** Tracing and hostile-wire validation follow precisely the same callable-owned value edges. */
    pub(super) fn references(&self) -> Vec<JsValue> {
        let mut values = self.properties.values().cloned().collect::<Vec<_>>();
        match &self.kind {
            Callable::Closure {
                lexical_this: Some(value),
                ..
            } => values.push(value.clone()),
            Callable::Bound {
                target,
                receiver,
                arguments,
                ..
            } => {
                values.push(JsValue::Function(*target));
                values.push(receiver.clone());
                values.extend(arguments.iter().cloned());
            }
            _ => {}
        }
        values
    }
}

impl Graph {
    /** Host validation inspects all lexical and callable edges before garbage collection. */
    pub fn values(&self) -> Vec<JsValue> {
        let mut values = self
            .environments
            .values()
            .flat_map(|env| {
                env.bindings
                    .values()
                    .filter_map(|cell| cell.value.clone())
                    .chain(env.receiver.clone())
            })
            .collect::<Vec<_>>();
        for function in self.functions.values() {
            values.extend(function.references());
        }
        values
    }
}
