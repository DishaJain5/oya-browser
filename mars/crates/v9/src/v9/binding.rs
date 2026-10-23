/** Serializable lexical cells preserve declaration kind and initialization independently of values. */
use super::JsValue;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
/// A lexical declaration's mutability survives every stateless V9 event request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BindingKind {
    /// Declaration-scope cells initialize before execution and permit redeclaration.
    Var,
    Let,
    Const,
}
/// An uninitialized lexical cell is distinct from an initialized JavaScript undefined value.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    /// Var and let cells admit reassignment after initialization.
    pub kind: BindingKind,
    /// None represents the temporal dead zone, never a committed successful global.
    pub value: Option<JsValue>,
}
/// The coordinator owns this private, bounded per-page state; agents cannot supply it.
pub type Globals = BTreeMap<String, Binding>;
