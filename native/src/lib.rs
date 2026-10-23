/** Independent engine modules; no existing browser engine executes page code. */
extern crate self as oya_native;
pub mod constants;
pub mod desktop;
pub mod document;
pub mod network;
pub mod platform;
/** Secure, versioned desktop profile persistence stays outside agent protocols. */
pub mod profile;
pub mod protocol;
pub mod runtime;
pub mod v9;
