/** Native window composition owns OS views and a cancellable engine worker. */
mod actions;
#[cfg(feature = "test-support")]
mod check;
mod commands;
mod events;
mod menus;
use super::{
    Options,
    broker::{Broker, Command, Event},
    canvas::{Action, Canvas, rect},
    capture::Capture,
    chrome::{self, Chrome},
    site::Site,
};
use crate::{
    protocol::{EngineError, EngineResult, Request, Response},
    runtime::{
        BrowserSnapshot,
        authority::{Owner, Permit},
    },
};
use menus::menus;
use objc2::{
    DefinedClass, MainThreadOnly, define_class,
    rc::{Retained, autoreleasepool},
};

use objc2::runtime::{NSObject, ProtocolObject};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSBackingStoreType, NSEventMask, NSWindow,
    NSWindowDelegate, NSWindowStyleMask,
};
use objc2_foundation::{
    MainThreadMarker, NSDate, NSDefaultRunLoopMode, NSObjectProtocol, NSPoint, NSSize, NSString,
};
use std::cell::Cell;

use serde_json::{Value, json};
use std::{collections::VecDeque, sync::atomic::Ordering};

/** Main-thread state contains only native widgets, intentions and immutable engine views. */
struct Window {
    window: Retained<NSWindow>,
    close_delegate: Retained<CloseDelegate>,
    canvas: Retained<Canvas>,
    chrome: Chrome,
    broker: Broker,
    snapshot: Option<BrowserSnapshot>,
    pending: VecDeque<Action>,
    busy: bool,
    ready: bool,
    error: Option<String>,
    failed_url: Option<String>,
    demo: String,
    capture: Capture,
    last_size: (f64, f64),
    last_scroll: f64,
    closed: bool,
    last_owner: Option<Owner>,
    agent_busy: bool,
    worker_dead: bool,
    activity: Option<String>,
    status_notice: Option<String>,
    retry_allowed: bool,
}

/** The title-bar close callback is deferred into the owning window state machine. */
#[derive(Default)]
struct CloseDelegateState {
    requested: Cell<bool>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = CloseDelegateState]
    struct CloseDelegate;
    unsafe impl NSObjectProtocol for CloseDelegate {}
    unsafe impl NSWindowDelegate for CloseDelegate {
        #[unsafe(method(windowShouldClose:))]
        fn should_close(&self, _window: &NSWindow) -> bool {
            self.ivars().requested.set(true);
            false
        }
    }
);

impl CloseDelegate {
    /** Keep close interception on AppKit's main thread without closing before save. */
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(CloseDelegateState::default());
        unsafe { objc2::msg_send![super(this), init] }
    }
    /** Consume one title-bar close request in the normal close workflow. */
    fn take_request(&self) -> bool {
        self.ivars().requested.replace(false)
    }
}

/// Launches the actual independent browser and joins every owned engine on close.
pub(super) fn run(options: Options) -> EngineResult<()> {
    let mtm = MainThreadMarker::new()
        .ok_or_else(|| EngineError::invalid("Desktop must start on the main thread."))?;
    let profiles = verification_profiles(&options);
    let site = Site::start()?;
    let capture = Capture::new(options.capture_dir)?;
    let executable = std::env::current_exe()
        .map_err(|_| EngineError::invalid("Cannot resolve owned engine executable."))?;
    let ordinary_launch = options.url.is_none();
    let initial = options.url.map(|url| {
        if url == "demo" || url == "forms" {
            format!(
                "{}/{}",
                site.origin,
                if url == "forms" { "forms" } else { "" }
            )
        } else {
            url
        }
    });
    let owner = if ordinary_launch {
        Owner::Human
    } else if options.agent_control {
        Owner::Agent
    } else {
        Owner::Human
    };
    let broker = if let Some(profiles) = profiles {
        Broker::start_with_profiles(
            executable,
            vec![site.origin.clone()],
            options.agent,
            owner,
            initial,
            profiles,
        )?
    } else {
        Broker::start(
            executable,
            vec![site.origin.clone()],
            options.agent,
            owner,
            initial,
        )?
    };
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
    let mut window = Window::new(mtm, broker, site.origin.clone(), capture);
    #[cfg(feature = "test-support")]
    let mut check = options.verify_ui.then(check::Check::new);
    menus(&app, &window.canvas);
    app.finishLaunching();
    window.window.makeKeyAndOrderFront(None);
    #[allow(deprecated)]
    app.activateIgnoringOtherApps(true);
    window
        .window
        .makeFirstResponder(Some(&window.chrome.address));
    while window.window.isVisible() && !window.closed {
        let result: EngineResult<()> = autoreleasepool(|_| {
            window.tick();
            #[cfg(feature = "test-support")]
            if let Some(check) = &mut check
                && check.advance(&mut window)?
            {
                window.closed = true;
            }
            let deadline = NSDate::dateWithTimeIntervalSinceNow(0.016);
            if let Some(event) = unsafe {
                app.nextEventMatchingMask_untilDate_inMode_dequeue(
                    NSEventMask::Any,
                    Some(&deadline),
                    NSDefaultRunLoopMode,
                    true,
                )
            } {
                app.sendEvent(&event);
            }
            if window.close_delegate.take_request() {
                window.request_close();
            }
            app.updateWindows();
            Ok(())
        });
        result?;
    }
    window.window.orderOut(None);
    drop(window);
    drop(site);
    Ok(())
}

/** Local UI verification owns fresh stores so it cannot read or change the user's secure profiles. */
fn verification_profiles(_options: &Options) -> Option<super::broker::ProfileStores> {
    #[cfg(feature = "test-support")]
    if _options.verify_ui || _options.test_profile {
        use crate::profile::test_support::{
            MemorySessionStore, MemoryStore, MemoryWebStorageStore,
        };
        use std::sync::Arc;
        return Some(super::broker::ProfileStores {
            cookies: Arc::new(MemoryStore::default()),
            storage: Arc::new(MemoryWebStorageStore::default()),
            sessions: Arc::new(MemorySessionStore::default()),
        });
    }
    None
}

impl Window {
    /// Native controls scope ownership to the currently selected page context.
    pub(super) fn selected_tab_id(&self) -> String {
        self.snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.inspection["tab_id"].as_str())
            .unwrap_or("tab-1")
            .into()
    }
    /** The native window holds no WebView, Electron or remote debugging connection. */
    fn new(mtm: MainThreadMarker, broker: Broker, demo: String, capture: Capture) -> Self {
        let canvas = Canvas::new(mtm);
        let chrome = Chrome::new(&canvas);
        let window = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                NSWindow::alloc(mtm),
                rect(0.0, 0.0, 1100.0, 760.0),
                NSWindowStyleMask::Titled
                    | NSWindowStyleMask::Closable
                    | NSWindowStyleMask::Miniaturizable
                    | NSWindowStyleMask::Resizable,
                NSBackingStoreType::Buffered,
                false,
            )
        };
        unsafe {
            window.setReleasedWhenClosed(false);
            window.setContentMinSize(NSSize::new(640.0, 480.0));
        }
        window.setTitle(&NSString::from_str("Oya Native"));
        let close_delegate = CloseDelegate::new(mtm);
        window.setDelegate(Some(ProtocolObject::from_ref(&*close_delegate)));
        window.setContentView(Some(&chrome.root));
        window.center();
        Self {
            window,
            close_delegate,
            canvas,
            chrome,
            broker,
            snapshot: None,
            pending: VecDeque::new(),
            busy: false,
            ready: false,
            error: None,
            failed_url: None,
            demo,
            capture,
            last_size: (0.0, 0.0),
            last_scroll: 0.0,
            closed: false,
            last_owner: None,
            agent_busy: false,
            worker_dead: false,
            activity: None,
            status_notice: None,
            retry_allowed: true,
        }
    }
    /** Native input, immutable updates and presentation changes are processed without blocking RPC. */
    fn tick(&mut self) {
        loop {
            match self.broker.events.try_recv() {
                Ok(event) => self.event(event),
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.disconnected();
                    break;
                }
            }
        }
        while let Some(action) = self.canvas.pop() {
            if matches!(
                action,
                Action::Rejected
                    | Action::Activate(
                        chrome::CONTROL
                            | chrome::CLOSE
                            | chrome::LOCATION
                            | chrome::DISMISS
                            | chrome::STOP
                            | chrome::PROFILE_CLEAR
                            | chrome::PROFILE_RETRY
                            | chrome::STORAGE_CLEAR
                    )
            ) {
                self.action(action);
            } else {
                if let Action::Edit(id, _, page) = &action
                    && matches!(self.pending.back(),Some(Action::Edit(previous,_,observed)) if previous==id && observed==page)
                {
                    self.pending.pop_back();
                }
                if self.pending.len() < 32 {
                    self.pending.push_back(action);
                }
            }
        }
        self.ownership();
        self.layout();
        if self.ready && !self.busy {
            if let Some(action) = self.pending.pop_front() {
                self.action(action);
            } else {
                self.presentation();
            }
        }
    }
    /** Ownership stays readable even while an engine is busy. */
    fn ownership(&mut self) {
        let tab_id = self.selected_tab_id();
        let human = self.broker.permit(Owner::Human, &tab_id).is_ok();
        let connected = self.broker.connected.load(Ordering::Relaxed);
        let owner = if human { Owner::Human } else { Owner::Agent };
        if self.last_owner != Some(owner) {
            if let Some(snapshot) = self.snapshot.clone() {
                self.window.makeFirstResponder(Some(&self.chrome.address));
                self.canvas.update(snapshot, human);
            }
            self.last_owner = Some(owner);
        }
        if let Some(snapshot) = &self.snapshot {
            self.chrome
                .back
                .setEnabled(human && snapshot.can_back && !snapshot.failed);
            self.chrome
                .forward
                .setEnabled(human && snapshot.can_forward && !snapshot.failed);
            self.chrome
                .reload
                .setEnabled(human && snapshot.document.is_some() && !snapshot.failed);
        }
        self.chrome.control.setTitle(&NSString::from_str(if human {
            "Allow agent"
        } else {
            "Take control"
        }));
        self.chrome.control.setEnabled(
            !human
                || (connected
                    && self.ready
                    && !self.worker_dead
                    && !self.busy
                    && self.pending.is_empty()),
        );
        self.chrome.address.setEnabled(human && self.ready);
        self.chrome
            .demo
            .setEnabled(human && self.ready && !self.busy);
        let state = if self
            .snapshot
            .as_ref()
            .is_some_and(|snapshot| snapshot.failed)
        {
            "Engine stopped · close and relaunch"
        } else if self.activity.is_some() {
            self.activity.as_deref().unwrap_or_default()
        } else if self.status_notice.is_some() {
            self.status_notice.as_deref().unwrap_or_default()
        } else if human {
            "Your control"
        } else {
            "Agent control"
        };
        self.chrome
            .status
            .setStringValue(&NSString::from_str(&format!(
                "{}{state}",
                if self.activity.is_some() || self.status_notice.is_some() {
                    if human {
                        "Your control · "
                    } else {
                        "Agent control · "
                    }
                } else {
                    ""
                }
            )));
        self.chrome.stop.setHidden(self.activity.is_none());
        self.chrome
            .stop
            .setEnabled(human && self.activity.is_some());
    }
    /** Engine-owned geometry is set from the actual scroll viewport, including error chrome. */
    fn layout(&self) {
        let size = self.chrome.root.bounds().size;
        self.chrome
            .layout(size.width, size.height, self.error.is_some());
        self.chrome
            .retry
            .setHidden(self.error.is_none() || !self.retry_allowed);
        self.chrome.error.setStringValue(&NSString::from_str(
            self.error.as_deref().unwrap_or_default(),
        ));
        let viewport = self.chrome.scroll.contentSize();
        let height = self
            .snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.display.as_ref())
            .map(|display| display.height)
            .unwrap_or(viewport.height)
            .max(viewport.height);
        self.canvas.setFrame(rect(0.0, 0.0, viewport.width, height));
    }
    /** Only one native presentation update enters the engine queue at a time. */
    fn presentation(&mut self) {
        let size = self.chrome.scroll.contentSize();
        let desired = (size.width, size.height);
        if desired != self.last_size {
            if self
                .broker
                .commands
                .try_send(Command::Viewport(desired.0, desired.1))
                .is_ok()
            {
                self.last_size = desired;
                self.busy = true;
            }
            return;
        }
        let y = self.chrome.scroll.contentView().bounds().origin.y;
        if (y - self.last_scroll).abs() > 0.5
            && self.broker.commands.try_send(Command::Scroll(y)).is_ok()
        {
            self.last_scroll = y;
            self.busy = true;
        }
    }
}
