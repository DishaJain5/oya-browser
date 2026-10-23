/** Starts the independent coordinator or one explicitly owned engine process. */
use oya_native::protocol::{EngineError, EngineResult};
use oya_native::runtime::{EnginePrograms, Runtime, run_agent, run_demo, run_service};
use std::io::{self, BufReader};
use std::process::ExitCode;

/** Human help never starts a browser or requests an account. */
const HELP: &str = "Oya Native — Rust browser for people and agents\nReplacement target: Oya's Electron browser. Current build is in development; compatibility is not yet at parity.\nOpen the macOS window: oya-native desktop --url demo\nForms and synthetic session: oya-native desktop --url forms\nNative agents in that same window: desktop --agent [--agent-control]\nGrant native PNG capture at startup: --capture-dir DIR\nHeadless local fixture: oya-native demo\nHeadless native protocol: oya-native agent\nOwn Rust network, document/layout and V9 services; current supported features are listed in native/README.md.";

/** Both public commands run under the same owned runtime lifetime. */
type Handler = fn(&mut Runtime) -> EngineResult<()>;

/** Errors use structured stderr and a failing exit status, never protocol stdout. */
fn main() -> ExitCode {
    match launch() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{}", serde_json::to_string(&error).unwrap_or_default());
            ExitCode::FAILURE
        }
    }
}

/** Only the composition root chooses executable paths and engine modes. */
fn launch() -> EngineResult<()> {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments.is_empty() || arguments.as_slice() == ["--help"] || arguments.as_slice() == ["-h"]
    {
        println!("{HELP}");
        return Ok(());
    }
    if arguments.len() == 2 && arguments[0] == "_service" {
        return run_service(&arguments[1]);
    }
    if arguments[0] == "desktop" {
        return oya_native::desktop::run(oya_native::desktop::Options::parse(&arguments[1..])?);
    }
    if arguments.len() != 1 {
        return Err(usage());
    }
    let handlers: [(&str, Handler); 2] = [("demo", run_demo), ("agent", agent)];
    let handler = handlers
        .iter()
        .find(|(name, _)| *name == arguments[0])
        .map(|(_, handler)| handler)
        .ok_or_else(usage)?;
    let executable = std::env::current_exe().map_err(|_| usage())?;
    let mut runtime = Runtime::start(EnginePrograms::same(&executable))?;
    handler(&mut runtime)
}

/** The public agent session owns its children for exactly its stdin lifetime. */
fn agent(runtime: &mut Runtime) -> EngineResult<()> {
    run_agent(
        runtime,
        &mut BufReader::new(io::stdin().lock()),
        &mut io::stdout().lock(),
    )
}

/** Invalid CLI arguments have a concrete recovery command. */
fn usage() -> EngineError {
    EngineError::new(
        "invalid_request",
        "Expected desktop, demo, agent, or --help.",
        "Run oya-native --help.",
    )
}
