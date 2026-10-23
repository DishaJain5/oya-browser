/** Real AppKit primitives receive only owned control roles and engine geometry. */
use super::*;
use objc2_app_kit::{NSButtonType, NSPopUpButton, NSScrollView, NSSecureTextField, NSTextView};

/** Native controls retain identity across revisions and never parse HTML themselves. */
pub(super) enum Widget {
    Input(Retained<NSTextField>),
    Button(Retained<NSButton>),
    Push(Retained<super::push_button::PushButton>),
    Select(Retained<NSPopUpButton>, Vec<usize>),
    Textarea(Retained<NSScrollView>, Retained<NSTextView>),
}
impl Widget {
    /** Common placement uses the engine's exact box. */
    pub fn view(&self) -> &NSView {
        match self {
            Self::Input(v) => v,
            Self::Button(v) => v,
            Self::Push(v) => v,
            Self::Select(v, _) => v,
            Self::Textarea(v, _) => v,
        }
    }
    /** Ownership disables all edit/activation surfaces, including multiline editing. */
    pub fn enable(&self, enabled: bool) {
        match self {
            Self::Input(v) => v.setEnabled(enabled),
            Self::Button(v) => v.setEnabled(enabled),
            Self::Push(v) => v.setEnabled(enabled),
            Self::Select(v, _) => v.setEnabled(enabled),
            Self::Textarea(_, v) => v.setEditable(enabled),
        }
    }
}
impl Canvas {
    /** Only supported interactive roles become native widgets. */
    pub(super) fn sync(&self, item: &DisplayItem, node: &crate::document::Node, human: bool) {
        if !["input", "button", "a", "label", "select", "textarea"].contains(&item.tag.as_str())
            || node.hidden()
        {
            return;
        }
        let mut widgets = self.ivars().widgets.borrow_mut();
        let widget = widgets
            .entry(item.local_id)
            .or_insert_with(|| self.widget(item, node));
        widget.view().setFrame(rect(
            item.x,
            item.y,
            item.width,
            item.control_height.unwrap_or(item.height),
        ));
        widget.enable(human && !node.disabled && node.style.interactive);
        let name = self
            .ivars()
            .snapshot
            .borrow()
            .as_ref()
            .and_then(|s| s.document.as_ref())
            .map(|d| crate::document::label(d, node))
            .unwrap_or_else(|| node.placeholder.clone());
        match widget {
            Widget::Input(field) => {
                if field.currentEditor().is_none() {
                    if field.stringValue().to_string() != node.value {
                        field.setStringValue(&NSString::from_str(&node.value));
                    }
                    self.ivars()
                        .drafts
                        .borrow_mut()
                        .insert(node.local_id, node.value.clone());
                }
                field.setPlaceholderString(Some(&NSString::from_str(&node.placeholder)));
                field.setEditable(!node.readonly());
                field.setAccessibilityLabel(Some(&NSString::from_str(&name)));
            }
            Widget::Button(button) => {
                button.setAccessibilityLabel(Some(&NSString::from_str(&name)));
                if ["checkbox", "radio"].contains(&node.input_type()) {
                    button.setState(if node.checked { 1 } else { 0 });
                }
            }
            Widget::Push(button) => {
                button.update(item);
                button.setAccessibilityLabel(Some(&NSString::from_str(&name)));
            }
            Widget::Select(button, ids) => {
                if let Some(document) = self
                    .ivars()
                    .snapshot
                    .borrow()
                    .as_ref()
                    .and_then(|s| s.document.as_ref())
                {
                    let selected = document
                        .nodes
                        .iter()
                        .find(|n| n.parent == Some(node.local_id) && n.selected)
                        .map(|n| n.local_id);
                    if let Some(index) = ids.iter().position(|id| Some(*id) == selected) {
                        button.selectItemAtIndex(index as isize);
                    }
                }
                button.setAccessibilityLabel(Some(&NSString::from_str(&name)));
            }
            Widget::Textarea(_, field) => {
                let focused = self
                    .window()
                    .and_then(|w| w.firstResponder())
                    .is_some_and(|v| {
                        std::ptr::eq(
                            &*v as *const _ as *const (),
                            &**field as *const _ as *const (),
                        )
                    });
                if !focused && field.string().to_string() != node.value {
                    field.setString(&NSString::from_str(&node.value));
                }
                field.setEditable(human && !node.disabled && !node.readonly());
                field.setAccessibilityLabel(Some(&NSString::from_str(&name)));
            }
        }
    }
    /** Targets/selectors are fixed Rust choices, never author-provided native code. */
    fn widget(&self, item: &DisplayItem, node: &crate::document::Node) -> Widget {
        if item.tag == "textarea" {
            return self.textarea(item, node);
        }
        if item.tag == "select" {
            return self.popup(item, node);
        }
        if item.tag == "input"
            && !["submit", "button", "checkbox", "radio"].contains(&node.input_type())
        {
            let field = if node.input_type() == "password" {
                NSSecureTextField::initWithFrame(
                    NSSecureTextField::alloc(self.mtm()),
                    rect(0.0, 0.0, item.width, item.height),
                )
                .into_super()
            } else {
                NSTextField::textFieldWithString(&NSString::from_str(&node.value), self.mtm())
            };
            field.setStringValue(&NSString::from_str(&node.value));
            field.setFont(Some(&platform::font(item.font_size, item.bold)));
            field.setTag(item.local_id as isize + NODE_TAG);
            unsafe {
                field.setDelegate(Some(ProtocolObject::from_ref(self)));
                field.setTarget(Some(self));
                field.setAction(Some(sel!(submit:)));
                if let Some(cell) = field.cell() {
                    cell.setSendsActionOnEndEditing(false);
                }
                self.addSubview(&field);
            }
            return Widget::Input(field);
        }
        if item.tag == "button"
            || (item.tag == "input" && ["submit", "button"].contains(&node.input_type()))
        {
            let button = super::push_button::PushButton::new(item, self);
            self.addSubview(&button);
            return Widget::Push(button);
        }
        let button = if ["checkbox", "radio"].contains(&node.input_type()) {
            super::choice_button::ChoiceButton::make(item, node.input_type() == "radio", self)
        } else {
            button("", item.local_id as isize + NODE_TAG, self)
        };
        if ["checkbox", "radio"].contains(&node.input_type()) {
            button.setButtonType(if node.input_type() == "radio" {
                NSButtonType::Radio
            } else {
                NSButtonType::Switch
            });
            button.setBordered(true);
        } else if item.tag == "button"
            || (item.tag == "input" && ["submit", "button"].contains(&node.input_type()))
        {
            button.setBordered(true);
        }
        button.setFont(Some(&platform::font(item.font_size, item.bold)));
        self.addSubview(&button);
        Widget::Button(button)
    }
    /** Popup option identities stay separate from duplicate visible labels/values. */
    fn popup(&self, item: &DisplayItem, node: &crate::document::Node) -> Widget {
        let button = NSPopUpButton::initWithFrame_pullsDown(
            NSPopUpButton::alloc(self.mtm()),
            rect(0.0, 0.0, item.width, item.height),
            false,
        );
        let mut ids = Vec::new();
        if let Some(document) = self
            .ivars()
            .snapshot
            .borrow()
            .as_ref()
            .and_then(|s| s.document.as_ref())
        {
            for option in document
                .nodes
                .iter()
                .filter(|n| n.parent == Some(node.local_id) && n.tag == "option")
            {
                let entry = unsafe {
                    objc2_app_kit::NSMenuItem::initWithTitle_action_keyEquivalent(
                        objc2_app_kit::NSMenuItem::alloc(self.mtm()),
                        &NSString::from_str(&option.text),
                        None,
                        &NSString::from_str(""),
                    )
                };
                entry.setEnabled(!option.disabled);
                if let Some(menu) = button.menu() {
                    menu.setAutoenablesItems(false);
                    menu.addItem(&entry);
                }
                ids.push(option.local_id);
            }
        }
        button.setTag(item.local_id as isize + NODE_TAG);
        unsafe {
            button.setTarget(Some(self));
            button.setAction(Some(sel!(selectOption:)));
        }
        self.addSubview(&button);
        Widget::Select(button, ids)
    }
    /** Plain multiline editing preserves newlines while its own scroll view bounds overflow. */
    fn textarea(&self, item: &DisplayItem, node: &crate::document::Node) -> Widget {
        let scroll = NSScrollView::initWithFrame(
            NSScrollView::alloc(self.mtm()),
            rect(0.0, 0.0, item.width, item.height),
        );
        scroll.setHasVerticalScroller(true);
        let field = NSTextView::initWithFrame(
            NSTextView::alloc(self.mtm()),
            rect(0.0, 0.0, item.width, item.height),
        );
        field.setRichText(false);
        field.setImportsGraphics(false);
        field.setAllowsUndo(true);
        field.setFont(Some(&platform::font(item.font_size, item.bold)));
        field.setString(&NSString::from_str(&node.value));
        field.setDelegate(Some(ProtocolObject::from_ref(self)));
        field.setVerticallyResizable(true);
        field.setHorizontallyResizable(false);
        scroll.setDocumentView(Some(&field));
        self.addSubview(&scroll);
        Widget::Textarea(scroll, field)
    }
    /** Native popup target/action emits one explicit selection intention. */
    pub(super) fn select_action(&self, sender: &NSPopUpButton) {
        let id = (sender.tag() - NODE_TAG) as usize;
        if let Some(Widget::Select(_, options)) = self.ivars().widgets.borrow().get(&id)
            && let Some(option) = options.get(sender.indexOfSelectedItem() as usize)
            && let Some(page) = self.page_id()
        {
            self.push(Action::Select(id, *option, page));
        }
    }
    /** Multiline delegate callbacks use retained native identity, never source IDs. */
    pub(super) fn multiline_changed(&self, notification: &NSNotification) {
        if self.ivars().reconciling.get() {
            return;
        }
        let Some(field) = notification
            .object()
            .and_then(|o| o.downcast::<NSTextView>().ok())
        else {
            return;
        };
        let id = self.ivars().widgets.borrow().iter().find_map(|(id, w)| {
            if let Widget::Textarea(_, v) = w {
                std::ptr::eq(&**v, &*field).then_some(*id)
            } else {
                None
            }
        });
        if let Some(id) = id
            && let Some(page) = self.page_id()
        {
            let text = field.string().to_string();
            if text.len() > crate::constants::TEXT_BYTES {
                if let Some(value) = self.committed(id) {
                    self.ivars().reconciling.set(true);
                    field.setString(&NSString::from_str(&value));
                    self.ivars().reconciling.set(false);
                }
                self.push(Action::Rejected);
            } else {
                self.push(Action::Edit(id, text, page));
            }
        }
    }
}
