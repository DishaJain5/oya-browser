/** One bounded worker serializes native UI and agent operations against one Runtime. */
use crate::{
    protocol::{EngineError, EngineResult, Request, Response},
    runtime::{
        BrowserSnapshot, EnginePrograms, Runtime,
        authority::{self, Owner, Permit, SharedAuthority},
    },
};
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

/// Bounded admission prevents stalled consumers from growing a work backlog.
pub(super) const QUEUE: usize = 16;
/// Cancellation is checked even while no commands arrive.
pub(super) const POLL: Duration = Duration::from_millis(25);

/** Secure profiles remain independent while the composition root passes them together. */
pub struct ProfileStores {
    /// Persistent cookies and their recovery marker.
    pub cookies: std::sync::Arc<dyn oya_native::profile::ProfileStore>,
    /// Origin-scoped Web Storage and its recovery marker.
    pub storage: std::sync::Arc<dyn oya_native::profile::WebStorageStore>,
    /// Ordered saved tab URLs.
    pub sessions: std::sync::Arc<dyn oya_native::profile::SessionStore>,
}

/// Native UI actions are admitted with a local human epoch.
pub(super) enum Command {
    /** Locally ordered edits rebase only within the same visible page generation. */
    Human {
        op: String,
        params: Value,
        page: Option<String>,
        permit: Permit,
    },
    /** Agent revision preconditions are never rewritten. */
    Agent(Request, Permit),
    /** OS presentation events do not grant document mutation. */
    Viewport(f64, f64),
    /** Scrolling remains an observation action for the person. */
    Scroll(f64),
    /** Human-confirmed profile recovery remains serialized with navigation. */
    ProfileClear,
    /** A recovery retry reads secure storage without reloading the current page. */
    ProfileRetry,
    /** Explicit site-storage recovery never clears the independent cookie item. */
    ProfileStorageClear,
    /** A close decision saves session state on the runtime-owning worker. */
    SaveSession(mpsc::SyncSender<bool>),
    /** Explicit close-without-saving prevents the worker's final retry. */
    DiscardSession(mpsc::SyncSender<()>),
}

/// Replies are acknowledged only after the UI has consumed their immutable snapshot.
pub(super) enum Event {
    /** A long agent action leaves the committed page visible while work continues. */
    Loading(String),
    /** Successful startup and every completed engine action feed the same view. */
    Update {
        snapshot: Box<BrowserSnapshot>,
        result: EngineResult<Value>,
        agent_id: Option<String>,
        clear_error: bool,
        destination: Option<String>,
        rejected_edit: Option<(String, usize, String)>,
    },
    /** OS-only observations and captures execute on the owning main thread. */
    Window(Request, Permit),
    /** Profile recovery is reported without inventing a new page snapshot. */
    Profile {
        result: EngineResult<&'static str>,
        snapshot: Box<BrowserSnapshot>,
    },
    /** Startup cannot leave a misleading live-looking shell. */
    Failed(EngineError),
}

/// All tasks are cancellable and joined before the desktop owner exits.
pub(super) struct Broker {
    pub commands: SyncSender<Command>,
    pub events: Receiver<Event>,
    pub replies: SyncSender<Response>,
    pub authority: SharedAuthority,
    pub cancel: Arc<AtomicBool>,
    pub connected: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
    io: Vec<JoinHandle<()>>,
}

impl Broker {
    /** Only the executable composition root supplies service paths and fixture grants. */
    pub fn start(
        program: PathBuf,
        origins: Vec<String>,
        agent: bool,
        owner: Owner,
        initial: Option<String>,
    ) -> EngineResult<Self> {
        Self::start_with_profiles(
            program,
            origins,
            agent,
            owner,
            initial,
            ProfileStores {
                cookies: oya_native::profile::system_store(),
                storage: oya_native::profile::system_web_storage_store(),
                sessions: oya_native::profile::system_session_store(),
            },
        )
    }
    /** Isolated profile injection keeps broker tests hermetic and failure-testable. */
    pub fn start_with_profiles(
        program: PathBuf,
        origins: Vec<String>,
        agent: bool,
        owner: Owner,
        initial: Option<String>,
        stores: ProfileStores,
    ) -> EngineResult<Self> {
        let (commands, input) = mpsc::sync_channel(QUEUE);
        let (updates, events) = mpsc::sync_channel(QUEUE);
        let (replies, output) = mpsc::sync_channel(QUEUE);
        let authority = crate::runtime::authority::Authority::shared(owner);
        let cancel = Arc::new(AtomicBool::new(false));
        let connected = Arc::new(AtomicBool::new(agent));
        let guard = Arc::clone(&authority);
        let stop = Arc::clone(&cancel);
        let io = if agent {
            super::io::start(
                commands.clone(),
                output,
                Arc::clone(&authority),
                Arc::clone(&cancel),
                Arc::clone(&connected),
            )?
        } else {
            Vec::new()
        };
        let start = WorkerStart {
            program,
            origins,
            initial,
            profile: stores.cookies,
            storage: stores.storage,
            sessions: stores.sessions,
        };
        let worker = thread::spawn(move || worker(start, guard, stop, input, updates));
        Ok(Self {
            commands,
            events,
            replies,
            authority,
            cancel,
            connected,
            worker: Some(worker),
            io,
        })
    }
    /** Permission is captured before queue admission, never inferred when work finishes. */
    pub fn permit(&self, owner: Owner, tab_id: &str) -> EngineResult<Permit> {
        let state = authority::lock(&self.authority)?;
        state.permit_for(owner, crate::runtime::tab_key(tab_id)?)
    }
    /// UI-only tab actions do not inherit or depend on the selected tab's agent grant.
    pub fn local_tab_permit(&self) -> EngineResult<Permit> {
        let state = authority::lock(&self.authority)?;
        Ok(Permit {
            owner: Owner::Human,
            epoch: state.epoch,
            tab_id: None,
        })
    }
    /** The native toolbar grants or revokes one exact tab's agent access. */
    pub fn change(&self, owner: Owner, tab_id: &str) -> EngineResult<()> {
        authority::lock(&self.authority)?.change_tab(crate::runtime::tab_key(tab_id)?, owner)?;
        Ok(())
    }
    /// Agent disconnect and transport failure revoke every tab immediately.
    pub fn revoke_agents(&self) {
        if let Ok(mut authority) = self.authority.lock() {
            authority.revoke_agents();
        }
    }
    /** Invalidate queued human and agent permits before a close-save decision. */
    pub fn begin_close(&self) {
        if let Ok(mut authority) = self.authority.lock() {
            authority.change(Owner::Human);
        }
    }
    /** Ask the runtime owner to write the latest session and return only success state. */
    pub fn save_session(&self) -> bool {
        self.session_command(Command::SaveSession)
    }
    /** Disable the final retry after the person explicitly chose to close without saving. */
    pub fn discard_session(&self) -> bool {
        self.session_command_ack(Command::DiscardSession)
    }
    /** Bridge one bounded close decision through the runtime-owning worker. */
    fn session_command(&self, command: fn(mpsc::SyncSender<bool>) -> Command) -> bool {
        let (reply, result) = mpsc::sync_channel(1);
        self.commands.try_send(command(reply)).is_ok() && result.recv().unwrap_or(false)
    }
    /** Confirm an explicit discard was applied before allowing the window to close. */
    fn session_command_ack(&self, command: fn(mpsc::SyncSender<()>) -> Command) -> bool {
        let (reply, result) = mpsc::sync_channel(1);
        self.commands.try_send(command(reply)).is_ok() && result.recv().is_ok()
    }
}
impl Drop for Broker {
    /** Polling I/O and child waits observe cancellation before tasks are joined. */
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        for worker in self.io.drain(..) {
            let _ = worker.join();
        }
    }
}

/// Startup-only inputs are grouped so worker lifetime has one coherent owner.
struct WorkerStart {
    /// The executable shared by native network/document/V9 services.
    program: PathBuf,
    /// Fixture grants never come from page or agent input.
    origins: Vec<String>,
    /// Optional navigation occurs only after profile restoration.
    initial: Option<String>,
    /// Desktop cookies are persisted only through secure storage.
    profile: std::sync::Arc<dyn oya_native::profile::ProfileStore>,
    /// Storage values remain independent from cookies and the tab-session item.
    storage: std::sync::Arc<dyn oya_native::profile::WebStorageStore>,
    /// Session URLs remain in a third independent encrypted record.
    sessions: std::sync::Arc<dyn oya_native::profile::SessionStore>,
}

/** Failed startup or a closed UI receiver cannot strand supervised children. */
fn worker(
    start: WorkerStart,
    authority: SharedAuthority,
    cancel: Arc<AtomicBool>,
    input: Receiver<Command>,
    events: SyncSender<Event>,
) {
    let WorkerStart {
        program,
        origins,
        initial,
        profile,
        storage,
        sessions,
    } = start;
    let mut runtime = match Runtime::desktop_with_session_profiles(
        EnginePrograms::same(&program),
        origins,
        authority.clone(),
        cancel.clone(),
        profile,
        storage,
        sessions,
    ) {
        Ok(runtime) => runtime,
        Err(error) => {
            let _ = events.try_send(Event::Failed(error));
            return;
        }
    };
    let destination = initial.clone();
    let result = if let Some(url) = initial {
        let state = authority::lock(&authority).expect("new authority");
        let permit = Permit {
            owner: state.owner,
            epoch: state.epoch,
            tab_id: Some(1),
        };
        drop(state);
        let mut params = json!({"url":url,"tab_id":runtime.selected_tab_id()});
        if permit.owner == Owner::Agent {
            params["control_epoch"] = json!(permit.epoch);
        }
        runtime.handle_as(Request::new("startup", "page.navigate", params), permit)
    } else {
        runtime.restore_session().map(|_| json!({}))
    };
    let _ = events.try_send(Event::Update {
        snapshot: Box::new(runtime.snapshot()),
        result,
        agent_id: None,
        clear_error: true,
        destination,
        rejected_edit: None,
    });
    while !cancel.load(Ordering::Relaxed) {
        match input.recv_timeout(POLL) {
            Ok(command) => {
                if execute(&mut runtime, command, &events).is_err() {
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(_) => break,
        }
    }
    runtime.stop();
}

/** UI-only requests never gain a second document model or an agent self-grant path. */
fn execute(runtime: &mut Runtime, command: Command, events: &SyncSender<Event>) -> Result<(), ()> {
    if let Command::SaveSession(reply) = command {
        let _ = reply.send(runtime.save_session_now());
        return Ok(());
    }
    if let Command::DiscardSession(reply) = command {
        runtime.discard_pending_session();
        let _ = reply.send(());
        return Ok(());
    }
    if matches!(
        &command,
        Command::ProfileClear | Command::ProfileRetry | Command::ProfileStorageClear
    ) {
        let result = match command {
            Command::ProfileClear => runtime.clear_profile().map(|had_cookies| {
                if had_cookies {
                    "Saved cookies cleared."
                } else {
                    "There are no saved cookies to clear."
                }
            }),
            Command::ProfileRetry => runtime.retry_profile().map(|()| "Saved cookies restored."),
            Command::ProfileStorageClear => runtime
                .clear_saved_web_storage()
                .map(|()| "Saved site storage cleared. You can continue browsing."),
            _ => unreachable!(),
        };
        return events
            .try_send(Event::Profile {
                result,
                snapshot: Box::new(runtime.snapshot()),
            })
            .map_err(|_| ());
    }
    if let Command::Agent(request, permit) = command {
        if request.op.starts_with("window.") || request.op == "page.screenshot" {
            return events
                .try_send(Event::Window(request, permit))
                .map_err(|_| ());
        }
        let id = request.id.clone();
        let clear_error = crate::runtime::mutating(&request.op);
        let destination = request
            .params
            .get("url")
            .and_then(Value::as_str)
            .map(str::to_owned);
        if clear_error {
            events
                .try_send(Event::Loading(request.op.clone()))
                .map_err(|_| ())?;
        }
        let result = runtime.handle_as(request, permit);
        return events
            .try_send(Event::Update {
                snapshot: Box::new(runtime.snapshot()),
                result,
                agent_id: Some(id),
                clear_error,
                destination,
                rejected_edit: None,
            })
            .map_err(|_| ());
    }
    let clear_error = matches!(command, Command::Human { .. });
    let edited = if let Command::Human { op, params, .. } = &command {
        if op == "page.type"
            && !runtime.snapshot().inspection["nodes"]
                .as_array()
                .is_some_and(|nodes| {
                    nodes
                        .iter()
                        .any(|n| n["node_id"] == params["node_id"] && n["protected"] == true)
                })
        {
            params["page_id"]
                .as_str()
                .zip(params["node_id"].as_str())
                .zip(params["text"].as_str())
                .and_then(|((page, node), text)| {
                    node.rsplit_once("node-")
                        .and_then(|(_, id)| id.parse::<usize>().ok())
                        .map(|id| (page.to_owned(), id, text.to_owned()))
                })
        } else {
            None
        }
    } else {
        None
    };
    let destination = if let Command::Human { params, .. } = &command {
        params.get("url").and_then(Value::as_str).map(str::to_owned)
    } else {
        None
    };
    let result = match command {
        Command::Human {
            op,
            mut params,
            page,
            permit,
        } => {
            let current = runtime.snapshot().inspection;
            if page.is_some() && page.as_deref() != current["page_id"].as_str() {
                Err(EngineError::new(
                    "invalid_target",
                    "The visible page changed before this action.",
                    "Try again on the current page.",
                ))
            } else {
                if params.get("expected_revision").is_some() {
                    params["expected_revision"] = current["revision"].clone();
                }
                if op.starts_with("page.") {
                    params["tab_id"] = current["tab_id"].clone();
                }
                runtime.handle_as(Request::new("human", &op, params), permit)
            }
        }
        Command::Viewport(width, height) => runtime.set_viewport(width, height).map(|()| json!({})),
        Command::Scroll(y) => {
            runtime.observe_scroll(y);
            Ok(json!({}))
        }
        Command::Agent(_, _)
        | Command::ProfileClear
        | Command::ProfileRetry
        | Command::ProfileStorageClear
        | Command::SaveSession(_)
        | Command::DiscardSession(_) => unreachable!(),
    };
    let rejected_edit = if result.is_err() { edited } else { None };
    events
        .try_send(Event::Update {
            snapshot: Box::new(runtime.snapshot()),
            result,
            agent_id: None,
            clear_error,
            destination,
            rejected_edit,
        })
        .map_err(|_| ())
}
