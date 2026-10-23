/** Literal HTTP requests carry no page-controlled headers or policy overrides. */
use crate::constants::FORM_BYTES;
use crate::protocol::{EngineError, EngineResult};
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
/// These fields are internal service data, never an agent header API.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WebRequest {
    /// Method is selected by the owned form algorithm.
    pub method: Method,
    /// The coordinator retains the actual URL separately from its public projection.
    pub url: String,
    /// Only the owned cookie jar assembles this field.
    pub cookie: String,
    /// Already URL-encoded successful form controls.
    pub body: String,
    /// Selects the smaller binary resource budget for explicitly requested images.
    #[serde(default)]
    pub image: bool,
}
impl WebRequest {
    /// Navigation cannot accidentally inherit a previous submission body.
    pub fn get(url: &str) -> Self {
        Self {
            url: url.into(),
            ..Self::default()
        }
    }
    /// Internal validation also bounds misuse by a faulty coordinator.
    pub fn validate(&self) -> EngineResult<()> {
        if self.body.len() > FORM_BYTES
            || self.cookie.len() > crate::constants::HEADER_BYTES
            || self.cookie.chars().any(char::is_control)
            || (self.method == Method::Get && !self.body.is_empty())
        {
            return Err(EngineError::invalid("Invalid bounded web request."));
        }
        Ok(())
    }
}
/// Raw response metadata is private to the coordinator; never export cookies to agents.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WebResponse {
    /// Final one-hop destination, already checked by networking policy.
    pub url: String,
    /// Server status is preserved for validation/error documents.
    pub status: u16,
    /// UTF-8 source, or base64 for explicitly requested image bytes.
    pub source: String,
    /// Tells the coordinator to decode source as bounded base64 image bytes.
    #[serde(default)]
    pub image_data: bool,
    /// Canonical media type without parameters.
    pub mime: String,
    /// A redirect is processed by the coordinator with fresh authority.
    pub location: Option<String>,
    /// Repeated fields remain separate, never comma-joined.
    pub set_cookies: Vec<String>,
}
