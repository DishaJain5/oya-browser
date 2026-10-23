/** Callable records retain page-owned data without accepting executable code in private messages. */
use super::*;
use crate::{
    constants::*,
    v9::{JsString, registry::Registry},
};

/** Intrinsic selectors have exactly one canonical record each; wire records cannot add host operations. */
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Intrinsic {
    FunctionConstructor,
    FunctionPrototype,
    Call,
    Apply,
    Bind,
}

/** Source closures, bound argument lists and fixed intrinsic identities share one function arena. */
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Callable {
    Closure {
        code_id: usize,
        environment: Option<Id>,
        lexical_this: Option<JsValue>,
    },
    Bound {
        target: Id,
        receiver: JsValue,
        arguments: Vec<JsValue>,
        #[serde(with = "name_wire")]
        name: JsString,
        length: usize,
    },
    Intrinsic {
        intrinsic: Intrinsic,
    },
}
/** Bound names reuse the existing UTF-16-aware value codec and its per-string validation. */
mod name_wire {
    use super::*;
    pub fn serialize<S: serde::Serializer>(
        name: &JsString,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        JsValue::String(name.clone()).serialize(serializer)
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<JsString, D::Error> {
        match JsValue::deserialize(deserializer)? {
            JsValue::String(name) => Ok(name),
            _ => Err(serde::de::Error::custom(
                "Bound names must be JavaScript strings.",
            )),
        }
    }
}

impl Graph {
    /** Binding retains one target edge rather than flattening identity or silently changing receivers. */
    pub fn bound(
        &mut self,
        target: Id,
        receiver: JsValue,
        arguments: Vec<JsValue>,
        registry: &Registry,
    ) -> EngineResult<JsValue> {
        self.bound_chain(target, arguments.len(), 1)?;
        let length = self
            .length(target, registry)?
            .saturating_sub(arguments.len());
        let name =
            JsString::from_utf8(&format!("bound {}", self.name(target, registry)?.to_dom()?))?;
        if self.functions.len() >= VM_FUNCTIONS {
            return Err(EngineError::limit("V9 functions", VM_FUNCTIONS));
        }
        let id = self.next_function;
        self.next_function = id.checked_add(1).ok_or_else(invalid)?;
        self.functions.insert(
            id,
            Function {
                kind: Callable::Bound {
                    target,
                    receiver,
                    arguments,
                    name,
                    length,
                },
                properties: BTreeMap::new(),
            },
        );
        self.bounds()?;
        Ok(JsValue::Function(id))
    }
    /** Every chain is finite and cannot hide excessive cumulative argument storage. */
    pub(super) fn bound_chain(
        &self,
        mut target: Id,
        mut count: usize,
        mut hops: usize,
    ) -> EngineResult<()> {
        loop {
            if hops > VM_BOUND_HOPS || count > VM_CALL_ARGUMENTS {
                return Err(EngineError::limit(
                    "V9 bound chain or arguments",
                    VM_BOUND_HOPS,
                ));
            }
            let function = self.functions.get(&target).ok_or_else(invalid)?;
            let Callable::Bound {
                target: next,
                arguments,
                ..
            } = &function.kind
            else {
                return Ok(());
            };
            if *next >= target {
                return Err(invalid());
            }
            target = *next;
            count += arguments.len();
            hops += 1;
        }
    }
    /** Source and intrinsic metadata is rebuilt rather than trusted from saved state. */
    pub fn name(&self, id: Id, registry: &Registry) -> EngineResult<JsString> {
        match &self.functions.get(&id).ok_or_else(invalid)?.kind {
            Callable::Closure { code_id, .. } => JsString::from_utf8(
                &registry
                    .codes
                    .get(code_id)
                    .ok_or_else(invalid)?
                    .display_name,
            ),
            Callable::Bound { name, .. } => Ok(name.clone()),
            Callable::Intrinsic { intrinsic } => JsString::from_utf8(intrinsic.metadata().0),
        }
    }
    /** Simple parameters and previously verified bound prefixes determine immutable arity. */
    pub fn length(&self, id: Id, registry: &Registry) -> EngineResult<usize> {
        match &self.functions.get(&id).ok_or_else(invalid)?.kind {
            Callable::Closure { code_id, .. } => Ok(registry
                .codes
                .get(code_id)
                .ok_or_else(invalid)?
                .parameters
                .len()),
            Callable::Bound { length, .. } => Ok(*length),
            Callable::Intrinsic { intrinsic } => Ok(intrinsic.metadata().1),
        }
    }
    /** Function expandos share the same aggregate property allowance as objects and arrays. */
    pub fn property_count(&self) -> usize {
        self.functions
            .values()
            .map(|function| function.properties.len())
            .sum()
    }
}
