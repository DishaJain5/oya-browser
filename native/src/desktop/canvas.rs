mod choice_button;
mod controls;
/** An OS view paints the engine's exact lines and hosts controls at its exact boxes. */
mod editing;
mod push_button;
use crate::{document::DisplayItem, platform, runtime::BrowserSnapshot};
use controls::Widget;
use objc2::{
    AnyThread, DefinedClass, MainThreadOnly, define_class, msg_send, rc::Retained,
    runtime::ProtocolObject, sel,
};
use objc2_app_kit::{
    NSAccessibility, NSAttributedStringNSStringDrawing, NSBezelStyle, NSBezierPath, NSButton,
    NSColor, NSControlTextEditingDelegate, NSImage, NSPopUpButton, NSTextDelegate, NSTextField,
    NSTextFieldDelegate, NSTextViewDelegate, NSView,
};
use objc2_foundation::{
    MainThreadMarker, NSData, NSNotification, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString,
};
use std::{
    cell::RefCell,
    collections::{BTreeMap, VecDeque},
};

/// Document tags are offset from negative toolbar actions.
pub(super) const NODE_TAG: isize = 100;
/// UI event storage is bounded independently of engine admission.
const ACTION_LIMIT: usize = 32;

/// Ordered intentions retain the visible page identity until worker admission.
#[derive(Clone)]
pub(super) enum Action {
    Activate(isize),
    Edit(usize, String, String),
    /** Page generation is captured at the actual native click. */
    Click(usize, String),
    /** Local text bounds reject input before allocating a queued engine request. */
    Rejected,
    Select(usize, usize, String),
    Submit(usize, String),
}

/** Main-thread-only mutable view state contains no engine or RPC transport. */
#[derive(Default)]
pub(super) struct CanvasIvars {
    snapshot: RefCell<Option<BrowserSnapshot>>,
    widgets: RefCell<BTreeMap<usize, Widget>>,
    actions: RefCell<VecDeque<Action>>,
    drafts: RefCell<BTreeMap<usize, String>>,
    reconciling: std::cell::Cell<bool>,
}

define_class!(
    #[unsafe(super=NSView)]
    #[thread_kind=MainThreadOnly]
    #[ivars=CanvasIvars]
    pub(super) struct Canvas;
    unsafe impl NSObjectProtocol for Canvas {}
    impl Canvas {
        /** Native keyboard navigation can enter the owned document viewport. */
        #[unsafe(method(acceptsFirstResponder))]
        fn accepts_focus(&self)->bool {true}
        /** Engine geometry uses a top-left document origin. */
        #[unsafe(method(isFlipped))]
        fn flipped(&self)->bool {true}
        /** Native drawing consumes the immutable layout without asking AppKit to lay out HTML. */
        #[unsafe(method(drawRect:))]
        fn draw(&self,dirty:NSRect) {self.paint(dirty);}
        /** Only fixed native selectors turn OS actions into bounded UI intentions. */
        #[unsafe(method(activate:))]
        fn activate(&self,sender:&objc2::runtime::AnyObject) {let tag:isize=unsafe {msg_send![sender,tag]};if tag>=NODE_TAG {if let Some(page)=self.page_id() {self.push(Action::Click((tag-NODE_TAG) as usize,page));}} else {self.push(Action::Activate(tag));}}
        /** Return submits only the focused field's own form. */
        #[unsafe(method(submit:))]
        fn submit_field(&self,sender:&NSTextField) {if let Some(page)=self.page_id(){self.push(Action::Submit((sender.tag()-NODE_TAG) as usize,page));}}
        /** Popups report an explicit option identity. */
        #[unsafe(method(selectOption:))]
        fn select_option(&self,sender:&NSPopUpButton) {self.select_action(sender);}
    }
    unsafe impl NSControlTextEditingDelegate for Canvas {
        /** Native field editing includes normal selection, keyboard and IME behavior. */
        #[unsafe(method(controlTextDidChange:))]
        fn changed(&self,notification:&NSNotification) {
            if let Some(field)=notification.object().and_then(|object|object.downcast::<NSTextField>().ok())
                && field.tag()>=NODE_TAG&& let Some(page)=self.page_id() {self.edit(&field,page);}
        }
    }
    unsafe impl NSTextFieldDelegate for Canvas {}
    unsafe impl NSTextDelegate for Canvas {
        /** Plain text editor changes enter the same literal-value mutation path. */
        #[unsafe(method(textDidChange:))]
        fn multiline(&self,notification:&NSNotification) {self.multiline_changed(notification);}
    }
    unsafe impl NSTextViewDelegate for Canvas {
        /** Plain textarea Return inserts a newline; Tab and Shift-Tab traverse native controls. */
        #[unsafe(method(textView:doCommandBySelector:))]
        unsafe fn multiline_command(&self,view:&objc2_app_kit::NSTextView,command:objc2::runtime::Sel)->bool {
            if command==sel!(insertTab:) || command==sel!(insertBacktab:) {if let Some(window)=self.window(){if command==sel!(insertTab:) {window.selectNextKeyView(Some(view));}else{window.selectPreviousKeyView(Some(view));}}true}else{false}
        }
    }
);

impl Canvas {
    /** Explicit allocation keeps the custom view and all callbacks on AppKit's thread. */
    pub fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(CanvasIvars::default());
        unsafe { msg_send![super(this),initWithFrame:rect(0.0,0.0,1100.0,680.0)] }
    }
    /** Native callbacks stamp the current visible generation before entering a queue. */
    fn page_id(&self) -> Option<String> {
        self.ivars()
            .snapshot
            .borrow()
            .as_ref()
            .and_then(|snapshot| snapshot.inspection["page_id"].as_str().map(str::to_owned))
    }
    /** Adjacent input events are coalesced before entering the bounded worker queue. */
    fn push(&self, action: Action) {
        let mut actions = self.ivars().actions.borrow_mut();
        if let Action::Edit(id, _, _) = &action
            && matches!(actions.back(),Some(Action::Edit(previous,_,_)) if previous==id)
        {
            actions.pop_back();
        }
        if actions.len() < ACTION_LIMIT {
            actions.push_back(action);
        }
    }
    /** Native shell consumes intentions once, in their actual UI order. */
    pub fn pop(&self) -> Option<Action> {
        self.ivars().actions.borrow_mut().pop_front()
    }
    /** Closing discards local intentions already queued for a later engine tick. */
    pub fn clear_actions(&self) {
        self.ivars().actions.borrow_mut().clear();
    }
    /** Replacing the visible page removes old controls and prevents handle aliasing. */
    pub fn update(&self, snapshot: BrowserSnapshot, human: bool) {
        let changed = self
            .ivars()
            .snapshot
            .borrow()
            .as_ref()
            .map(|old| &old.inspection["page_id"])
            != Some(&snapshot.inspection["page_id"]);
        if changed {
            self.ivars().drafts.borrow_mut().clear();
            for widget in self.ivars().widgets.borrow().values() {
                widget.view().removeFromSuperview();
            }
            self.ivars().widgets.borrow_mut().clear();
        }
        self.ivars().widgets.borrow_mut().retain(|id, widget| {
            let keep = snapshot.document.as_ref().is_some_and(|document| {
                document
                    .nodes
                    .iter()
                    .any(|node| node.local_id == *id && !node.hidden())
            });
            if !keep {
                widget.view().removeFromSuperview();
            }
            keep
        });
        *self.ivars().snapshot.borrow_mut() = Some(snapshot.clone());
        if let (Some(document), Some(display)) = (&snapshot.document, &snapshot.display) {
            for item in &display.items {
                for line in &item.validation {
                    platform::colored(
                        &line.text,
                        item.font_size,
                        false,
                        &NSColor::systemRedColor(),
                    )
                    .drawAtPoint(NSPoint::new(line.x, line.y));
                }
                if let Some(node) = document
                    .nodes
                    .iter()
                    .find(|node| node.local_id == item.local_id)
                {
                    self.sync(item, node, human && !snapshot.failed);
                }
            }
        }

        self.setNeedsDisplay(true);
    }
    /** Dirty-region filtering bounds work while native scrolling reveals owned geometry. */
    fn paint(&self, dirty: NSRect) {
        NSColor::textBackgroundColor().setFill();
        NSBezierPath::fillRect(dirty);
        let snapshot = self.ivars().snapshot.borrow();
        let Some(snapshot) = snapshot.as_ref() else {
            return;
        };
        if let Some(display) = &snapshot.display {
            for item in &display.items {
                let bounds = rect(item.x, item.y, item.width, item.height);
                if let Some(color) = item.background.as_deref().and_then(literal_color) {
                    color.setFill();
                    NSBezierPath::fillRect(bounds);
                }
                if item.border > 0.0 {
                    literal_color(item.border_color.as_deref().unwrap_or("gray"))
                        .unwrap_or_else(NSColor::separatorColor)
                        .setStroke();
                    let path = NSBezierPath::bezierPathWithRect(bounds);
                    path.setLineWidth(item.border);
                    path.stroke();
                }
                if item.tag == "img"
                    && let Some(asset) = snapshot.images.get(&item.local_id)
                {
                    let data = unsafe {
                        NSData::dataWithBytes_length(
                            asset.bytes.as_ptr().cast(),
                            asset.bytes.len() as _,
                        )
                    };
                    if let Some(image) = NSImage::initWithData(NSImage::alloc(), &data) {
                        image.drawInRect(bounds);
                        continue;
                    }
                }
                if ["input", "button", "select", "textarea"].contains(&item.tag.as_str()) {
                    continue;
                }
                for line in &item.lines {
                    if line.y + item.font_size * 1.5 >= dirty.origin.y
                        && line.y <= dirty.origin.y + dirty.size.height
                    {
                        let color =
                            if let Some(color) = item.color.as_deref().and_then(literal_color) {
                                color
                            } else if item.tag == "a" {
                                NSColor::linkColor()
                            } else {
                                NSColor::textColor()
                            };
                        platform::colored(&line.text, item.font_size, item.bold, &color)
                            .drawAtPoint(NSPoint::new(line.x, line.y));
                        if item.tag == "a" {
                            color.setStroke();
                            let underline = NSBezierPath::bezierPath();
                            underline
                                .moveToPoint(NSPoint::new(line.x, line.y + item.font_size + 3.0));
                            underline.lineToPoint(NSPoint::new(
                                line.x + line.width,
                                line.y + item.font_size + 3.0,
                            ));
                            underline.stroke();
                        }
                    }
                }
            }
        }
    }
}
/** A fixed palette and hex parser cannot execute CSS functions or load resources. */
fn literal_color(value: &str) -> Option<Retained<NSColor>> {
    let hex = match value {
        "black" => "000000",
        "white" => "ffffff",
        "red" => "ff0000",
        "green" => "008000",
        "blue" => "0000ff",
        "gray" | "grey" => "808080",
        "navy" => "000080",
        "teal" => "008080",
        "transparent" => return Some(NSColor::clearColor()),
        other => other.strip_prefix('#')?,
    };
    let hex = if hex.len() == 3 {
        hex.chars().flat_map(|c| [c, c]).collect::<String>()
    } else {
        hex.into()
    };
    let number = u32::from_str_radix(&hex, 16).ok()?;
    Some(NSColor::colorWithSRGBRed_green_blue_alpha(
        ((number >> 16) & 255) as f64 / 255.0,
        ((number >> 8) & 255) as f64 / 255.0,
        (number & 255) as f64 / 255.0,
        1.0,
    ))
}

/** Every button uses one audited selector and a browser-chosen numeric action. */
pub(super) fn button(title: &str, tag: isize, target: &Canvas) -> Retained<NSButton> {
    let button = unsafe {
        NSButton::buttonWithTitle_target_action(
            &NSString::from_str(title),
            Some(target),
            Some(sel!(activate:)),
            target.mtm(),
        )
    };
    button.setTag(tag);
    button.setBordered(false);
    button.setBezelStyle(NSBezelStyle::Push);
    button.setFont(Some(&platform::font(13.0, false)));
    button.setContentTintColor(Some(&NSColor::labelColor()));
    button
}
/** One coordinate constructor keeps all shell and document rectangles explicit. */
pub(super) fn rect(x: f64, y: f64, width: f64, height: f64) -> NSRect {
    NSRect::new(NSPoint::new(x, y), NSSize::new(width, height))
}
