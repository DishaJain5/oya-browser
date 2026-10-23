/** Snapshot publication and responses stay ordered on the native UI thread. */
use super::*;
impl Window {
    /** A retired worker cannot leave a live-looking last snapshot or indefinite Loading state. */
    pub(super) fn disconnected(&mut self) {
        if self.broker.transport == Transport::Mcp {
            self.request_close();
            return;
        }
        if self.worker_dead {
            return;
        }
        self.worker_dead = true;
        self.broker.connected.store(false, Ordering::Relaxed);
        self.ready = false;
        self.retry_allowed = false;
        self.busy = false;
        self.agent_busy = false;
        self.pending.clear();
        self.broker.revoke_agents();
        let error = EngineError::new(
            "service_unavailable",
            "Browser engine stopped.",
            "Close and restart Mars to continue; no operation was replayed.",
        );
        self.error = Some(format!("{}\n{}", error.message, error.hint));
        if let Some(snapshot) = &mut self.snapshot {
            snapshot.failed = true;
            self.canvas.update(snapshot.clone(), false);
        }
        self.reply(Response::new(None, Err(error)));
    }
    /** Replies cannot precede the corresponding visible native snapshot. */
    pub(super) fn event(&mut self, event: Event) {
        match event {
            Event::Loading(op) => {
                self.agent_busy = true;
                self.activity = self.activity_for(&op, &json!({}));
            }
            Event::Failed(error) => {
                if self.broker.transport == Transport::Mcp {
                    self.request_close();
                    return;
                }
                self.ready = false;
                self.retry_allowed = false;
                self.error = Some(error.message);
            }
            Event::Window(request, permit) => {
                let id = request.id.clone();
                if request.op == "window.appearance" {
                    if let Err(error) = self.begin_appearance(request, permit) {
                        self.reply(Response::new(Some(id), Err(error)));
                    }
                } else {
                    let result = self.window_request(request, permit);
                    self.reply(Response::new(Some(id), result));
                }
            }
            Event::Profile { result, snapshot } => {
                self.busy = false;
                self.activity = None;
                let snapshot = *snapshot;
                let tab_id = snapshot.inspection["tab_id"].as_str().unwrap_or("tab-1");
                let human = self.broker.permit(Owner::Human, tab_id).is_ok();
                let changed = self.snapshot.as_ref().map(|old| &old.inspection["page_id"])
                    != Some(&snapshot.inspection["page_id"])
                    || self.snapshot.as_ref().is_some_and(|old| {
                        old.inspection["tab_id"] != snapshot.inspection["tab_id"]
                    });
                if changed {
                    self.chrome.address.setStringValue(&NSString::from_str(
                        snapshot.inspection["url"].as_str().unwrap_or_default(),
                    ));
                }
                self.chrome.set_tabs(&snapshot.tabs, &self.canvas);
                self.canvas.update(snapshot.clone(), human);
                self.chrome.welcome.setHidden(snapshot.document.is_some());
                self.chrome.mark.setHidden(snapshot.document.is_some());
                self.chrome.back.setEnabled(snapshot.can_back && human);
                self.chrome
                    .forward
                    .setEnabled(snapshot.can_forward && human);
                self.chrome
                    .reload
                    .setEnabled(snapshot.document.is_some() && human && !snapshot.failed);
                self.ready = !snapshot.failed;
                self.snapshot = Some(snapshot);
                let recovery = result
                    .as_ref()
                    .err()
                    .filter(|error| recovery_error(&error.code))
                    .map(|error| error.code.clone());
                match result {
                    Ok(message) => {
                        self.error = None;
                        self.status_notice = Some(message.into());
                    }
                    Err(error) => {
                        self.status_notice = None;
                        self.error = Some(error.message);
                    }
                }
                if let Some(code) = recovery
                    && self.broker.transport != Transport::Mcp
                {
                    self.profile_recovery_prompt(&code);
                }
            }
            Event::Update {
                snapshot,
                result,
                agent_id,
                appearance_update,
                clear_error,
                destination,
                rejected_edit,
            } => {
                let profile_recovery = result
                    .as_ref()
                    .err()
                    .and_then(|error| recovery_error(&error.code).then(|| error.code.clone()));
                let snapshot = *snapshot;
                if snapshot.failed && self.broker.transport == Transport::Mcp {
                    self.request_close();
                    return;
                }
                self.activity = None;
                if agent_id.is_some() {
                    self.agent_busy = false;
                }
                self.ready = !snapshot.failed;
                if agent_id.is_none() {
                    self.busy = false;
                }
                let tab_id = snapshot.inspection["tab_id"].as_str().unwrap_or("tab-1");
                let human = self.broker.permit(Owner::Human, tab_id).is_ok();
                let tab_changed = self
                    .snapshot
                    .as_ref()
                    .is_some_and(|old| old.inspection["tab_id"] != snapshot.inspection["tab_id"]);
                let navigated = self.snapshot.as_ref().map(|old| &old.inspection["page_id"])
                    != Some(&snapshot.inspection["page_id"]);
                if navigated || tab_changed {
                    self.chrome.address.setStringValue(&NSString::from_str(
                        snapshot.inspection["url"].as_str().unwrap_or_default(),
                    ));
                }
                self.chrome.set_tabs(&snapshot.tabs, &self.canvas);
                self.canvas.update(snapshot.clone(), human);
                self.chrome.welcome.setHidden(snapshot.document.is_some());
                self.chrome.mark.setHidden(snapshot.document.is_some());
                self.chrome.back.setEnabled(snapshot.can_back && human);
                self.chrome
                    .forward
                    .setEnabled(snapshot.can_forward && human);
                self.chrome
                    .reload
                    .setEnabled(snapshot.document.is_some() && human && !snapshot.failed);
                if self.snapshot.as_ref().map(|old| old.scroll_y) != Some(snapshot.scroll_y) {
                    self.canvas
                        .scrollPoint(NSPoint::new(0.0, snapshot.scroll_y));
                    self.last_scroll = snapshot.scroll_y;
                }
                self.snapshot = Some(snapshot);
                if let Some((page, id, text)) = rejected_edit {
                    self.canvas.reject_edit(&page, id, &text);
                }
                let mut result = result;
                if appearance_update {
                    self.finish_appearance(agent_id.as_deref(), &mut result);
                }
                if let Err(error) = &result {
                    let script_event = error.phase.is_some() && destination.is_none();
                    self.retry_allowed =
                        error.retryable && error.service.is_none() && !script_event;
                    if let Some(id) = error.invalid_node.as_deref() {
                        self.canvas.focus_invalid(*id, &error.message);
                    }
                    if let Some(destination) = destination {
                        self.failed_url = Some(destination);
                        if let Some(url) = self
                            .snapshot
                            .as_ref()
                            .and_then(|snapshot| snapshot.inspection["url"].as_str())
                        {
                            self.canvas.restore(&self.chrome.address, url);
                        }
                    }
                    let script_index = error
                        .location
                        .as_deref()
                        .and_then(|location| location.script_index);
                    let title = if let (Some(service), Some(index)) =
                        (error.service.as_deref(), script_index)
                    {
                        format!(
                            "Browser {service} service stopped while loading page script {index}."
                        )
                    } else if let (Some(index), true) = (script_index, error.code == "timeout") {
                        format!("Page script {index} timed out.")
                    } else if let Some(index) = script_index {
                        format!("Page script {index}: {}", error.message)
                    } else if error.code == "permission_denied" {
                        "Navigation blocked.".to_owned()
                    } else if error.code == "tls_error" {
                        "The server's certificate could not be verified.".to_owned()
                    } else if let Some(service) = error.service.as_deref() {
                        format!("Browser {service} service stopped.")
                    } else {
                        error.message.clone()
                    };
                    let retention = if error.previous_page_retained {
                        "Your previous page was kept."
                    } else {
                        "No page was changed."
                    };
                    self.error = Some(format!("{}\n{}\n{}", title, retention, error.hint));
                } else if clear_error {
                    self.error = None;
                    self.retry_allowed = true;
                }
                self.layout();
                if let Some(value) = result
                    .as_mut()
                    .ok()
                    .filter(|value| value.get("protocol_version").is_some())
                {
                    value["demo_url"] = json!(format!("{}/", self.demo));
                    value["operations"]["page.screenshot"] = json!({"tab_id":"string","page_id":"string","expected_revision":"u64","layout_generation":"u64"});
                    value["operations"]["window.inspect"] = json!({});
                    value["operations"]["window.resize"] = json!({"tab_id":"string","width":"logical_points","height":"logical_points","control_epoch":"u64"});
                    value["operations"]["window.appearance"] = json!({"tab_id":"string","appearance":"light|dark|system","control_epoch":"u64"});
                }
                if let Some(id) = agent_id {
                    self.chrome.root.layoutSubtreeIfNeeded();
                    self.chrome.root.displayIfNeeded();
                    self.reply(Response::new(Some(id), result));
                }
                if let Some(code) = profile_recovery
                    && self.broker.transport != Transport::Mcp
                {
                    self.profile_recovery_prompt(&code);
                }
            }
        }
    }
    /** A stalled output consumer cannot stall the main thread or keep ownership. */
    pub(super) fn reply(&self, response: Response) {
        if self.broker.replies.try_send(response).is_err() {
            if self.broker.transport == Transport::Mcp {
                self.broker.close_mcp();
                return;
            }
            self.broker.revoke_agents();
            self.broker.connected.store(false, Ordering::Relaxed);
        }
    }
}

/** Only secure-profile failures open recovery prompts; page errors stay in page context. */
fn recovery_error(code: &str) -> bool {
    code.starts_with("profile_") || code.starts_with("web_storage_")
}
