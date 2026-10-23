/** Host references are document-local data identities, never native pointers or authority grants. */
use serde::{Deserialize, Serialize};
/// A host identity is meaningful only against the exact candidate document projection.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HostRef {
    /// Private page identity prevents aliases across navigation or tabs.
    pub document: String,
    /// Only supported document, element and event brands cross the VM boundary.
    pub kind: HostKind,
}
/// Flat host brands avoid recursive DOM or event serialization.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum HostKind {
    Document,
    Element { local_id: usize },
    Event { id: u32 },
}
