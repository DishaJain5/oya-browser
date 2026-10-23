/** Native toolbar, recovery panel and status expose ownership without page scripting. */
use super::canvas::{Canvas, button, rect};
use objc2::{MainThreadOnly, define_class, msg_send, rc::Retained, sel};
use objc2_app_kit::{
    NSAccessibility, NSBezierPath, NSButton, NSColor, NSScrollView, NSTextField, NSView,
};
use objc2_foundation::{NSObjectProtocol, NSRect, NSString};

define_class!(
    #[unsafe(super=NSView)]
    #[thread_kind=MainThreadOnly]
    struct Surface;
    unsafe impl NSObjectProtocol for Surface {}
    impl Surface {
        /** An opaque native shell also produces faithful content-view captures. */
        #[unsafe(method(drawRect:))]
        fn paint(&self,dirty:NSRect) {NSColor::windowBackgroundColor().setFill();NSBezierPath::fillRect(dirty);}
    }
);

define_class!(
    #[unsafe(super=NSView)]
    #[thread_kind=MainThreadOnly]
    struct OyaMark;
    unsafe impl NSObjectProtocol for OyaMark {}
    impl OyaMark {
        /** Draw Oya's intersecting rings using native vector strokes. */
        #[unsafe(method(drawRect:))]
        fn paint(&self, _dirty: NSRect) {
            let first = NSBezierPath::bezierPathWithOvalInRect(rect(12.0, 12.0, 36.0, 36.0));
            first.setLineWidth(8.0);
            NSColor::systemBlueColor().setStroke();
            first.stroke();
            let second = NSBezierPath::bezierPathWithOvalInRect(rect(4.0, 4.0, 36.0, 36.0));
            second.setLineWidth(8.0);
            NSColor::labelColor().setStroke();
            second.stroke();
        }
    }
);

/// Fixed browser-owned tags never overlap document-local IDs.
pub(super) const BACK: isize = -1;
pub(super) const FORWARD: isize = -2;
pub(super) const RELOAD: isize = -3;
pub(super) const ADDRESS: isize = -4;
pub(super) const CONTROL: isize = -5;
pub(super) const DEMO: isize = -6;
pub(super) const RETRY: isize = -7;
pub(super) const DISMISS: isize = -8;
pub(super) const LOCATION: isize = -9;
pub(super) const CLOSE: isize = -10;
/// Stop revokes the current operation permit without ending the browser session.
pub(super) const STOP: isize = -11;
/// Cookie-profile recovery is a browser-owned action, never a page node.
pub(super) const PROFILE_CLEAR: isize = -12;
/// Secure-storage retry does not reload or navigate the current page.
pub(super) const PROFILE_RETRY: isize = -13;
/// Site-storage recovery stays separate from cookie-profile controls.
pub(super) const STORAGE_CLEAR: isize = -14;
/// The native tab row uses negative actions that cannot collide with document node tags.
pub(super) const TAB_NEW: isize = -20;
pub(super) const TAB_CLOSE: isize = -21;
pub(super) const TAB_SELECT_BASE: isize = -1000;
/// The document viewport begins below stable native chrome.
pub(super) const TOOLBAR: f64 = 52.0;
/// The one-line tab strip sits immediately above the address toolbar.
const TAB_ROW: f64 = 34.0;
/// Each tab remains usable while overflow scrolls horizontally.
const TAB_WIDTH: f64 = 144.0;
/// Right-side actions retain space for close and create controls.
const TAB_ACTIONS: f64 = 76.0;
pub(super) const STATUS: f64 = 28.0;
const ERROR_HEIGHT: f64 = 96.0;

/// Retained native controls keep focus and selection independent of engine snapshots.
pub(super) struct Chrome {
    pub root: Retained<NSView>,
    pub address: Retained<NSTextField>,
    pub back: Retained<NSButton>,
    pub forward: Retained<NSButton>,
    pub reload: Retained<NSButton>,
    pub control: Retained<NSButton>,
    pub demo: Retained<NSButton>,
    pub status: Retained<NSTextField>,
    pub error: Retained<NSTextField>,
    pub retry: Retained<NSButton>,
    pub dismiss: Retained<NSButton>,
    pub scroll: Retained<NSScrollView>,
    pub welcome: Retained<NSTextField>,
    /** Native vector rendering keeps Oya identity crisp at every display scale. */
    pub mark: Retained<NSView>,
    pub stop: Retained<NSButton>,
    /** Horizontal overflow keeps every bounded tab reachable in one native row. */
    pub tab_scroll: Retained<NSScrollView>,
    pub tab_content: Retained<NSView>,
    pub tab_buttons: Vec<Retained<NSButton>>,
    pub tab_new: Retained<NSButton>,
    pub tab_close: Retained<NSButton>,
}

impl Chrome {
    /** Shell widgets never interpret HTML or ask the OS to render a webpage. */
    pub fn new(target: &Canvas) -> Self {
        let mtm = target.mtm();
        let root: Retained<NSView> = unsafe {
            let surface: Retained<Surface> =
                msg_send![Surface::alloc(mtm),initWithFrame:rect(0.0,0.0,1100.0,760.0)];
            surface.into_super()
        };
        let address = NSTextField::textFieldWithString(&NSString::from_str(""), mtm);
        address.setPlaceholderString(Some(&NSString::from_str("Enter an HTTP or HTTPS URL")));
        address.setTag(ADDRESS);
        if let Some(cell) = address.cell() {
            cell.setSendsActionOnEndEditing(false);
        }
        unsafe {
            address.setTarget(Some(target));
            address.setAction(Some(sel!(activate:)));
        }
        let back = button("←", BACK, target);
        let forward = button("→", FORWARD, target);
        let reload = button("↻", RELOAD, target);
        let control = button("Allow agent", CONTROL, target);
        let demo = button("Open demo", DEMO, target);
        let status = label("Your control · Native browser", target);
        let error = NSTextField::wrappingLabelWithString(&NSString::from_str(""), mtm);
        error.setMaximumNumberOfLines(3);
        let retry = button("Retry", RETRY, target);
        let dismiss = button("Dismiss", DISMISS, target);
        let stop = button("Stop", STOP, target);
        let tab_scroll =
            NSScrollView::initWithFrame(NSScrollView::alloc(mtm), rect(0.0, 0.0, 900.0, TAB_ROW));
        tab_scroll.setHasHorizontalScroller(true);
        tab_scroll.setAutohidesScrollers(true);
        let tab_content: Retained<NSView> =
            unsafe { msg_send![NSView::alloc(mtm), initWithFrame:rect(0.0,0.0,TAB_WIDTH,TAB_ROW)] };
        tab_scroll.setDocumentView(Some(&tab_content));
        let tab_new = button("+", TAB_NEW, target);
        tab_new.setAccessibilityLabel(Some(&NSString::from_str("New tab")));
        let tab_close = button("×", TAB_CLOSE, target);
        tab_close.setAccessibilityLabel(Some(&NSString::from_str("Close selected tab")));
        let welcome = label(
            "Oya Native\n\nYour browser, from the ground up. Open the demo to try forms, navigation and native agent tools.\n\nOya Native is the Rust browser being built to replace Oya's Electron browser for people and agents. The complete browser is the goal; this build is in development.",
            target,
        );
        let mark: Retained<OyaMark> =
            unsafe { msg_send![OyaMark::alloc(mtm), initWithFrame:rect(0.0,0.0,52.0,52.0)] };
        let scroll =
            NSScrollView::initWithFrame(NSScrollView::alloc(mtm), rect(0.0, STATUS, 1100.0, 680.0));
        scroll.setHasVerticalScroller(true);
        scroll.setAutohidesScrollers(true);
        scroll.setDocumentView(Some(target));
        for view in [
            &*address as &NSView,
            &*back,
            &*forward,
            &*reload,
            &*control,
            &*demo,
            &*status,
            &*error,
            &*retry,
            &*dismiss,
            &*scroll,
            &*welcome,
            &*mark,
            &*stop,
            &*tab_scroll,
            &*tab_new,
            &*tab_close,
        ] {
            root.addSubview(view);
        }
        Self {
            root,
            address,
            back,
            forward,
            reload,
            control,
            demo,
            status,
            error,
            retry,
            dismiss,
            scroll,
            welcome,
            mark: mark.into_super(),
            stop,
            tab_scroll,
            tab_content,
            tab_buttons: Vec::new(),
            tab_new,
            tab_close,
        }
    }
    /// Rebuild the bounded strip from immutable coordinator metadata.
    pub fn set_tabs(&mut self, tabs: &serde_json::Value, target: &Canvas) {
        for button in self.tab_buttons.drain(..) {
            button.removeFromSuperview();
        }
        let values = tabs["tabs"].as_array().cloned().unwrap_or_default();
        for (index, value) in values.iter().enumerate() {
            let selected = value["selected"].as_bool().unwrap_or(false);
            let title = value["title"].as_str().unwrap_or("New Tab");
            let owner = value["owner"].as_str().unwrap_or("human");
            let title = if owner == "agent" {
                format!("Agent · {title}")
            } else {
                title.to_owned()
            };
            let label = if selected {
                format!("› {title}")
            } else {
                title.clone()
            };
            let tag = TAB_SELECT_BASE - index as isize;
            let tab = button(&label, tag, target);
            tab.setAccessibilityLabel(Some(&NSString::from_str(&title)));
            tab.setFrame(rect(index as f64 * TAB_WIDTH, 1.0, TAB_WIDTH - 4.0, 30.0));
            self.tab_content.addSubview(&tab);
            self.tab_buttons.push(tab);
        }
        let width = (values.len() as f64 * TAB_WIDTH).max(TAB_WIDTH);
        self.tab_content.setFrame(rect(0.0, 0.0, width, TAB_ROW));
        self.tab_new
            .setEnabled(values.len() < crate::constants::TABS);
    }
    /** Narrow windows retain the URL field and ownership action at usable sizes. */
    pub fn layout(&self, width: f64, height: f64, error: bool) -> (f64, f64) {
        let top = height - TOOLBAR;
        self.tab_scroll
            .setFrame(rect(0.0, top - TAB_ROW, width - TAB_ACTIONS, TAB_ROW));
        self.tab_new
            .setFrame(rect(width - 68.0, top - TAB_ROW + 2.0, 30.0, 28.0));
        self.tab_close
            .setFrame(rect(width - 36.0, top - TAB_ROW + 2.0, 30.0, 28.0));
        self.back.setFrame(rect(12.0, top + 10.0, 32.0, 32.0));
        self.forward.setFrame(rect(48.0, top + 10.0, 32.0, 32.0));
        self.reload.setFrame(rect(84.0, top + 10.0, 32.0, 32.0));
        self.address
            .setFrame(rect(124.0, top + 11.0, (width - 360.0).max(220.0), 30.0));
        self.demo
            .setFrame(rect(width - 228.0, top + 10.0, 100.0, 32.0));
        self.control
            .setFrame(rect(width - 124.0, top + 10.0, 112.0, 32.0));
        let error_height = if error { ERROR_HEIGHT } else { 0.0 };
        self.scroll.setFrame(rect(
            0.0,
            STATUS,
            width,
            height - TOOLBAR - TAB_ROW - STATUS - error_height,
        ));
        self.status.setFrame(rect(14.0, 5.0, width - 100.0, 18.0));
        self.stop.setFrame(rect(width - 76.0, 1.0, 64.0, 26.0));
        self.error
            .setFrame(rect(16.0, top - 62.0, width - 32.0, 56.0));
        self.retry
            .setFrame(rect(width - 180.0, top - 94.0, 72.0, 28.0));
        self.dismiss
            .setFrame(rect(width - 104.0, top - 94.0, 88.0, 28.0));
        for view in [&*self.error as &NSView, &*self.retry, &*self.dismiss] {
            view.setHidden(!error);
        }
        self.welcome
            .setFrame(rect(40.0, height - 360.0, width - 80.0, 240.0));
        self.mark.setFrame(rect(40.0, height - 190.0, 52.0, 52.0));
        let size = self.scroll.contentSize();
        (size.width, size.height)
    }
}
/** Native labels remain accessible to macOS without exposing HTML to AppKit. */
fn label(text: &str, target: &Canvas) -> Retained<NSTextField> {
    NSTextField::labelWithString(&NSString::from_str(text), target.mtm())
}
