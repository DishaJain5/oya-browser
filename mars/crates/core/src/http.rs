/** Shared HTTP method data does not require a networking implementation. */
use serde::{Deserialize, Serialize};

/// Only navigation and URL-encoded form methods are admitted.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub enum Method {
    /// Safe navigation is the default for existing callers.
    #[default]
    Get,
    /// An authorized form transaction may have external effects.
    Post,
}
