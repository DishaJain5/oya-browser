/** Native shell composes one visible window with the same authoritative agent runtime. */
mod broker;
#[cfg(target_os = "macos")]
mod canvas;
#[cfg(target_os = "macos")]
mod capture;
#[cfg(target_os = "macos")]
mod chrome;
mod io;
mod site;
#[cfg(target_os = "macos")]
mod window;
use crate::protocol::{EngineError, EngineResult};
use std::path::PathBuf;

/// Startup-only grants cannot be acquired through page or agent messages.
pub struct Options {
    /// Connect the native versioned protocol to this visible window.
    pub agent: bool,
    /// Explicit operator authorization for unattended control.
    pub agent_control: bool,
    /// Optional repository for generated PNG names, never an agent-controlled path.
    pub capture_dir: Option<PathBuf>,
    /// Optional initial destination; demo uses an owned ephemeral origin.
    pub url: Option<String>,
    /// Explicit local test startup exercises OS callbacks without exposing an agent bypass.
    #[cfg(feature = "test-support")]
    pub verify_ui: bool,
    /// Disposable in-memory profiles isolate opt-in native tests from the user's Keychain.
    #[cfg(feature = "test-support")]
    pub test_profile: bool,
}

impl Options {
    /// Unknown flags fail before creating a window or granting control.
    pub fn parse(arguments: &[String]) -> EngineResult<Self> {
        let mut result = Self {
            agent: false,
            agent_control: false,
            capture_dir: None,
            url: None,
            #[cfg(feature = "test-support")]
            verify_ui: false,
            #[cfg(feature = "test-support")]
            test_profile: false,
        };
        let mut args = arguments.iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--agent" => result.agent = true,
                "--agent-control" => result.agent_control = true,
                "--capture-dir" => result.capture_dir = Some(args.next().ok_or_else(usage)?.into()),
                "--url" => result.url = Some(args.next().ok_or_else(usage)?.clone()),
                #[cfg(feature = "test-support")]
                "--verify-ui" => {
                    result.verify_ui = true;
                    result.test_profile = true;
                }
                #[cfg(feature = "test-support")]
                "--test-profile" => result.test_profile = true,
                _ => return Err(usage()),
            }
        }
        if result.agent_control && !result.agent {
            return Err(usage());
        }
        #[cfg(feature = "test-support")]
        if result.verify_ui && result.agent {
            return Err(usage());
        }
        Ok(result)
    }
}

/// macOS uses OS widgets and drawing only; unsupported desktops never substitute a web engine.
pub fn run(options: Options) -> EngineResult<()> {
    #[cfg(target_os = "macos")]
    {
        window::run(options)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = options;
        Err(EngineError::unsupported(
            "The native desktop currently requires macOS.",
        ))
    }
}

/** Invalid startup flags have one concrete recovery command. */
fn usage() -> EngineError {
    EngineError::invalid(
        "Expected desktop [--agent [--agent-control]] [--capture-dir DIR] [--url URL|demo].",
    )
}
