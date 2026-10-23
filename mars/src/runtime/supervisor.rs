use super::authority::{self, Owner, Permit, SharedAuthority};
/** Supervised private pipes isolate engine processes without public listeners. */
use super::page::Page;
use crate::constants::{RPC_POLL, RPC_TIMEOUT, WEB_RPC_TIMEOUT};
use crate::protocol::{
    EngineError, EngineResult, Request, Response, decode, encode, read_frame, write_message,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// Explicit executable injection permits fault testing without production crash tools.
#[derive(Clone)]
pub struct EnginePrograms {
    /// Network service executable, resolved by the composition root.
    pub network: PathBuf,
    /// Document service executable.
    pub document: PathBuf,
    /// V9 service executable.
    pub v9: PathBuf,
}

impl EnginePrograms {
    /// Production always launches its own executable, never a page-controlled path.
    pub fn same(executable: &Path) -> Self {
        Self {
            network: executable.into(),
            document: executable.into(),
            v9: executable.into(),
        }
    }
}

/// One local owner and one committed page; services never retain hidden page state.
pub struct Runtime {
    /** Exactly three children after successful startup. */
    services: Vec<Service>,
    /** Child/protocol failures poison the session without automatic restart. */
    pub(super) failed: bool,
    /** A candidate is never visible here until every navigation step succeeds. */
    pub(super) page: Option<Page>,
    /** Allocated only for successful navigation publication. */
    pub(super) generation: u64,
    /** Startup fixture grants never come from page/agent parameters. */
    pub(super) web_origins: Option<Vec<String>>,
    /** Headless mode retains its explicitly local single-owner contract. */
    pub(super) authority: Option<SharedAuthority>,
    /** Publication rechecks the active operation's exact authority. */
    pub(super) permit: Option<Permit>,
    /** Closing the window interrupts child waits and rejects late presentation publication. */
    pub(super) cancel: Arc<AtomicBool>,
    /** History records committed URLs only. */
    pub(super) history: Vec<super::navigation::HistoryEntry>,
    /** Current history position is absent before successful navigation. */
    pub(super) history_cursor: Option<usize>,
    /** Canonical point viewport is shared by display and inspection. */
    pub(super) render_context: crate::document::RenderContext,
    /** Scrolling is presentation state, not a separate page. */
    pub(super) scroll_y: f64,
    /** Session secrets are never part of public page or desktop snapshots. */
    pub(super) cookies: crate::network::CookieJar,
    /** Desktop cookies publish only after the secure profile accepts their candidate. */
    pub(super) profile_store: Option<std::sync::Arc<dyn crate::profile::ProfileStore>>,
    /** A corrupt or inaccessible profile blocks all navigation until recovery. */
    pub(super) profile_error: Option<crate::profile::ProfileError>,
    /** Web Storage has an independent secure record and recovery state. */
    pub(super) storage_store: Option<std::sync::Arc<dyn crate::profile::WebStorageStore>>,
    pub(super) storage_error: Option<crate::profile::ProfileError>,
    /** Eligible URLs are saved only after ordinary-launch restoration finishes. */
    session_store: Option<Arc<dyn crate::profile::SessionStore>>,
    session_writes_enabled: bool,
    session_restore_pending: bool,
    session_record: Option<Vec<u8>>,
    pub(super) session_dirty: bool,
    /** Origin-scoped candidate values stay coordinator-owned and never enter snapshots. */
    pub(super) local_storage: BTreeMap<String, BTreeMap<String, String>>,
    /** Session values are keyed by exact tab identity and canonical origin. */
    pub(super) session_storage: std::collections::HashMap<String, BTreeMap<String, String>>,
    /** Inactive tabs retain only committed page and presentation state. */
    tabs: Vec<TabState>,
    /** UI selection is stable while an agent addresses another tab. */
    selected_tab_id: String,
    /** Temporary coordinator context can differ from the visible tab. */
    pub(super) active_tab_id: String,
    /** Tab IDs are monotonic and never reused after close. */
    next_tab_id: u64,
    /** Recent selection order chooses the destination when a tab closes. */
    recent_tabs: Vec<String>,
}

/** One inactive tab owns its document and navigation presentation state. */
struct TabState {
    id: String,
    page: Option<Page>,
    history: Vec<super::navigation::HistoryEntry>,
    history_cursor: Option<usize>,
    scroll_y: f64,
}

impl TabState {
    /// New tabs have no document or history until navigation commits.
    fn empty(id: String) -> Self {
        Self {
            id,
            page: None,
            history: Vec::new(),
            history_cursor: None,
            scroll_y: 0.0,
        }
    }
}

impl Runtime {
    /// Return the selected identity for desktop commands issued by native controls.
    pub fn selected_tab_id(&self) -> &str {
        &self.selected_tab_id
    }

    /// Run a page operation in one tab and restore the human-selected state afterward.
    pub(super) fn run_in_tab<T>(
        &mut self,
        id: &str,
        operation: impl FnOnce(&mut Self) -> EngineResult<T>,
    ) -> EngineResult<T> {
        let selected = self.selected_tab_id.clone();
        if id == selected {
            return operation(self);
        }
        self.activate_tab(id)?;
        let result = operation(self);
        self.store_active_tab();
        self.activate_tab(&selected)?;
        result
    }

    /// A new human tab starts empty and becomes the visible selection.
    pub(super) fn open_tab(&mut self) -> EngineResult<Value> {
        if self.tabs.len() >= crate::constants::TABS {
            return Err(EngineError::limit("open tabs", crate::constants::TABS));
        }
        let id = format!("tab-{}", self.next_tab_id);
        self.next_tab_id = self.next_tab_id.saturating_add(1);
        if let Some(authority) = &self.authority {
            authority::lock(authority)?.add_tab(tab_key(&id)?);
        }
        self.tabs.push(TabState::empty(id.clone()));
        self.select_tab(&id)?;
        self.tabs_value()
    }

    /// Tab selection changes the native visible page without an agent navigation.
    pub(super) fn select_tab_by_id(&mut self, id: &str) -> EngineResult<Value> {
        self.select_tab(id)?;
        self.tabs_value()
    }

    /// Closing the final tab leaves one fresh native page context.
    pub(super) fn close_tab(&mut self, id: &str) -> EngineResult<Value> {
        if !self.tabs.iter().any(|tab| tab.id == id) {
            return Err(EngineError::invalid("Unknown tab_id."));
        }
        if self.active_tab_id == id {
            self.store_active_tab();
        }
        self.tabs.retain(|tab| tab.id != id);
        let prefix = format!("{id}|");
        self.session_storage
            .retain(|key, _| !key.starts_with(&prefix));
        self.session_storage
            .retain(|key, _| !key.starts_with(&format!("{id}|")));
        if let Some(authority) = &self.authority {
            authority::lock(authority)?.remove_tab(tab_key(id)?);
        }
        self.recent_tabs.retain(|recent| recent != id);
        if self.tabs.is_empty() {
            let replacement = format!("tab-{}", self.next_tab_id);
            self.next_tab_id = self.next_tab_id.saturating_add(1);
            if let Some(authority) = &self.authority {
                authority::lock(authority)?.add_tab(tab_key(&replacement)?);
            }
            self.tabs.push(TabState::empty(replacement.clone()));
            self.recent_tabs.insert(0, replacement);
        }
        let next = if self.selected_tab_id == id {
            self.recent_tabs[0].clone()
        } else {
            self.selected_tab_id.clone()
        };
        self.selected_tab_id.clear();
        self.select_tab(&next)?;
        self.tabs_value()
    }

    /// Agent discovery is bounded and exposes titles, not page or cookie internals.
    pub(super) fn tabs_value(&self) -> EngineResult<Value> {
        let tabs = self
            .tabs
            .iter()
            .map(|tab| {
                let page = if tab.id == self.active_tab_id {
                    self.page.as_ref()
                } else {
                    tab.page.as_ref()
                };
                let control = self.authority.as_ref().and_then(|authority|authority::lock(authority).ok()).and_then(|authority|tab_key(&tab.id).ok().and_then(|id|authority.tab_status(id)));
                json!({"tab_id":tab.id,"title":page.map(|page|if page.document.title.is_empty(){url::Url::parse(&page.url).ok().and_then(|url|url.host_str().map(str::to_owned)).unwrap_or_else(||"New Tab".into())}else{page.document.title.clone()}).unwrap_or_else(||"New Tab".into()),"selected":tab.id==self.selected_tab_id,"owner":control.map(|state|state.0).unwrap_or(crate::runtime::authority::Owner::Human),"epoch":control.map(|state|state.1).unwrap_or_default()})
            })
            .collect::<Vec<_>>();
        Ok(
            json!({"selected_tab_id":self.selected_tab_id,"tabs":tabs,"limit":crate::constants::TABS}),
        )
    }

    /** Reject navigation before it can contact a site while storage recovery is required. */
    pub(super) fn ensure_storage_ready(&self) -> EngineResult<()> {
        self.storage_error
            .map_or(Ok(()), |error| Err(storage_profile_error(error)))
    }

    /** Build isolated maps for one canonical origin and current tab. */
    pub(super) fn storage_for(&self, origin: &str) -> EngineResult<crate::v9::StorageState> {
        if self.storage_error.is_some() {
            return Err(storage_profile_error(
                self.storage_error
                    .unwrap_or(crate::profile::ProfileError::Unavailable),
            ));
        }
        let key = storage_key(&self.active_tab_id, origin);
        Ok(crate::v9::StorageState {
            local: self.local_storage.get(origin).cloned().unwrap_or_default(),
            session: self.session_storage.get(&key).cloned().unwrap_or_default(),
        })
    }

    /** Commit validated V9 proposals during the page publication transaction. */
    pub(super) fn commit_storage(&mut self, origin: &str, storage: &crate::v9::StorageState) {
        self.local_storage
            .insert(origin.into(), storage.local.clone());
        self.session_storage.insert(
            storage_key(&self.active_tab_id, origin),
            storage.session.clone(),
        );
    }

    /** Persist one localStorage candidate before publication and return rollback bytes. */
    pub(super) fn write_storage(
        &mut self,
        origin: &str,
        storage: &crate::v9::StorageState,
    ) -> EngineResult<Option<Option<Vec<u8>>>> {
        let Some(store) = self.storage_store.clone() else {
            return Ok(None);
        };
        let previous = self.local_storage.get(origin).cloned().unwrap_or_default();
        if previous == storage.local {
            return Ok(None);
        }
        let old = self.storage_profile_bytes()?;
        let mut next = self.local_storage.clone();
        next.insert(origin.into(), storage.local.clone());
        let bytes = crate::profile::WebStorageProfile {
            version: crate::constants::WEB_STORAGE_PROFILE_VERSION,
            origins: next,
        }
        .encode()
        .map_err(storage_profile_error)?;
        if let Err(error) = store.replace(&bytes) {
            self.block_storage(error, store.as_ref());
            return Err(storage_profile_error(self.storage_error.unwrap_or(error)));
        }
        Ok(Some(old))
    }

    /** Restore the prior item after a stale authority result or failed commit. */
    pub(super) fn rollback_storage(&mut self, previous: Option<Vec<u8>>) {
        let Some(store) = self.storage_store.clone() else {
            return;
        };
        let result = previous.map_or_else(|| store.clear(), |bytes| store.replace(&bytes));
        if let Err(error) = result {
            self.block_storage(error, store.as_ref());
        }
    }

    /// Make uncertain candidate state restart-safe before returning to the caller.
    fn block_storage(
        &mut self,
        error: crate::profile::ProfileError,
        store: &dyn crate::profile::WebStorageStore,
    ) {
        self.storage_error = Some(if store.mark_recovery().is_err() {
            crate::profile::ProfileError::RecoveryUnpersisted
        } else {
            error
        });
    }

    /// Explicitly discard storage in safe order before lifting a restart recovery lock.
    pub fn clear_web_storage(&mut self) -> EngineResult<()> {
        let store = self
            .storage_store
            .as_ref()
            .ok_or_else(storage_unavailable)?;
        if let Err(error) = store.clear() {
            let store = Arc::clone(store);
            self.block_storage(error, store.as_ref());
            if self.storage_error == Some(crate::profile::ProfileError::RecoveryUnpersisted) {
                return Err(storage_profile_error(
                    crate::profile::ProfileError::RecoveryUnpersisted,
                ));
            }
            return Err(EngineError::new(
                "web_storage_clear_failed",
                "Saved site storage could not be cleared.",
                "Navigation remains blocked. The stored data may be partially cleared. Keep Mars open and retry Clear Saved Site Storage when secure storage is available.",
            ));
        }
        if let Err(error) = store.clear_recovery() {
            self.storage_error = Some(error);
            return Err(EngineError::new(
                "web_storage_clear_failed",
                "Saved site storage could not be cleared.",
                "Navigation remains blocked. The stored data may be partially cleared. Keep Mars open and retry Clear Saved Site Storage when secure storage is available.",
            ));
        }
        self.local_storage.clear();
        self.session_storage.clear();
        self.storage_error = None;
        self.restore_pending_session()?;
        Ok(())
    }

    /** Encode the current localStorage map without exposing its contents in errors. */
    fn storage_profile_bytes(&self) -> EngineResult<Option<Vec<u8>>> {
        if self.local_storage.is_empty() {
            return Ok(None);
        }
        crate::profile::WebStorageProfile {
            version: crate::constants::WEB_STORAGE_PROFILE_VERSION,
            origins: self.local_storage.clone(),
        }
        .encode()
        .map(Some)
        .map_err(storage_profile_error)
    }

    /// Save the current coordinator fields before loading another tab's state.
    fn store_active_tab(&mut self) {
        if let Some(tab) = self
            .tabs
            .iter_mut()
            .find(|tab| tab.id == self.active_tab_id)
        {
            tab.page = self.page.take();
            tab.history = std::mem::take(&mut self.history);
            tab.history_cursor = self.history_cursor.take();
            tab.scroll_y = std::mem::replace(&mut self.scroll_y, 0.0);
        }
    }

    /// Load an existing tab without changing which page the person selected.
    fn activate_tab(&mut self, id: &str) -> EngineResult<()> {
        if !self.tabs.iter().any(|tab| tab.id == id) {
            return Err(EngineError::invalid("Unknown tab_id."));
        }
        if self.active_tab_id == id {
            return Ok(());
        }
        let Some(tab) = self.tabs.iter().find(|tab| tab.id == id) else {
            return Err(EngineError::invalid("Unknown tab_id."));
        };
        let mut candidate = (
            tab.page.clone(),
            tab.history.clone(),
            tab.history_cursor,
            tab.scroll_y,
        );
        if let Some(page) = candidate.0.as_mut() {
            self.prepare_presentation(page, self.render_context)?;
            page.layout_generation = page.layout_generation.saturating_add(1);
            page.publishable()?;
        }
        self.store_active_tab();
        self.page = candidate.0;
        self.history = candidate.1;
        self.history_cursor = candidate.2;
        self.scroll_y = candidate.3;
        self.active_tab_id = id.into();
        self.clamp_scroll();
        Ok(())
    }

    /// Human selection becomes visible only after saving the former tab.
    fn select_tab(&mut self, id: &str) -> EngineResult<()> {
        self.activate_tab(id)?;
        self.selected_tab_id = id.into();
        self.recent_tabs.retain(|recent| recent != id);
        self.recent_tabs.insert(0, id.into());
        Ok(())
    }

    /// Partial startup is cleaned by owned Service drops.
    pub fn start(programs: EnginePrograms) -> EngineResult<Self> {
        Self::start_cancel(programs, Arc::new(AtomicBool::new(false)))
    }

    /** Cancellation owns startup as well as post-readiness RPC work. */
    fn start_cancel(programs: EnginePrograms, cancel: Arc<AtomicBool>) -> EngineResult<Self> {
        let mut runtime = Self {
            services: Vec::new(),
            failed: false,
            page: None,
            generation: 0,
            web_origins: None,
            authority: None,
            permit: None,
            cancel,
            history: Vec::new(),
            history_cursor: None,
            render_context: crate::document::RenderContext::default(),
            scroll_y: 0.0,
            cookies: Default::default(),
            profile_store: None,
            profile_error: None,
            storage_store: None,
            storage_error: None,
            session_store: None,
            session_writes_enabled: false,
            session_restore_pending: false,
            session_record: None,
            session_dirty: false,
            local_storage: Default::default(),
            session_storage: Default::default(),
            tabs: vec![TabState::empty("tab-1".into())],
            selected_tab_id: "tab-1".into(),
            active_tab_id: "tab-1".into(),
            next_tab_id: 2,
            recent_tabs: vec!["tab-1".into()],
        };
        for (name, executable) in [
            ("network", programs.network),
            ("document", programs.document),
            ("v9", programs.v9),
        ] {
            runtime.services.push(Service::spawn(
                name,
                &executable,
                Arc::clone(&runtime.cancel),
            )?);
            let ready = runtime.call(name, "service.ready", json!({}))?;
            if ready != json!({"service":name, "ready":true}) {
                return Err(EngineError::service(name, false));
            }
        }
        Ok(runtime)
    }

    /// The visible window and its agent connection share one runtime.
    pub fn desktop(
        programs: EnginePrograms,
        origins: Vec<String>,
        authority: SharedAuthority,
        cancel: Arc<AtomicBool>,
    ) -> EngineResult<Self> {
        let mut runtime = Self::start_cancel(programs, Arc::clone(&cancel))?;
        runtime.web_origins = Some(origins);
        runtime.authority = Some(authority);
        runtime.cancel = Arc::clone(&cancel);
        for service in &mut runtime.services {
            service.cancel = Arc::clone(&cancel);
        }
        Ok(runtime)
    }

    /// Desktop startup restores a complete secure profile before any initial navigation.
    pub fn desktop_with_profile(
        programs: EnginePrograms,
        origins: Vec<String>,
        authority: SharedAuthority,
        cancel: Arc<AtomicBool>,
        store: Arc<dyn crate::profile::ProfileStore>,
    ) -> EngineResult<Self> {
        let mut runtime = Self::desktop(programs, origins, authority, cancel)?;
        runtime.profile_store = Some(Arc::clone(&store));
        match store.load() {
            Ok(None) => {}
            Ok(Some(bytes)) => {
                match crate::network::CookieJar::from_profile(&bytes, std::time::SystemTime::now())
                {
                    Ok(jar) => runtime.cookies = jar,
                    Err(error) => runtime.profile_error = Some(error),
                }
            }
            Err(error) => runtime.profile_error = Some(error),
        }
        Ok(runtime)
    }

    /** Production desktop restores cookies and localStorage from distinct secure records. */
    pub fn desktop_with_profiles(
        programs: EnginePrograms,
        origins: Vec<String>,
        authority: SharedAuthority,
        cancel: Arc<AtomicBool>,
        cookies: Arc<dyn crate::profile::ProfileStore>,
        storage: Arc<dyn crate::profile::WebStorageStore>,
    ) -> EngineResult<Self> {
        let mut runtime =
            Self::desktop_with_profile(programs, origins, authority, cancel, cookies)?;
        runtime.storage_store = Some(Arc::clone(&storage));
        match storage.recovery_required() {
            Ok(true) => runtime.storage_error = Some(crate::profile::ProfileError::CommitUncertain),
            Err(error) => runtime.storage_error = Some(error),
            Ok(false) => match storage.load() {
                Ok(None) => {}
                Ok(Some(bytes)) => match crate::profile::WebStorageProfile::decode(&bytes) {
                    Ok(profile) => runtime.local_storage = profile.origins,
                    Err(error) => runtime.storage_error = Some(error),
                },
                Err(error) => runtime.storage_error = Some(error),
            },
        }
        Ok(runtime)
    }

    /** Attach independent secure tab persistence without enabling writes before restore. */
    pub fn desktop_with_session_profiles(
        programs: EnginePrograms,
        origins: Vec<String>,
        authority: SharedAuthority,
        cancel: Arc<AtomicBool>,
        cookies: Arc<dyn crate::profile::ProfileStore>,
        storage: Arc<dyn crate::profile::WebStorageStore>,
        sessions: Arc<dyn crate::profile::SessionStore>,
    ) -> EngineResult<Self> {
        let mut runtime =
            Self::desktop_with_profiles(programs, origins, authority, cancel, cookies, storage)?;
        runtime.session_store = Some(sessions);
        Ok(runtime)
    }

    /** Restore eligible GET tabs only after cookie and Web Storage recovery is clear. */
    pub fn restore_session(&mut self) -> EngineResult<usize> {
        self.session_restore_pending = true;
        self.restore_pending_session()
    }

    /** Resume only an explicitly requested restore after secure-profile recovery succeeds. */
    fn restore_pending_session(&mut self) -> EngineResult<usize> {
        if !self.session_restore_pending {
            return Ok(0);
        }
        if let Some(error) = self.profile_error {
            return Err(error.engine_error());
        }
        if let Some(error) = self.storage_error {
            return Err(storage_profile_error(error));
        }
        let Some(store) = self.session_store.clone() else {
            self.session_restore_pending = false;
            return Ok(0);
        };
        let loaded = match store.load() {
            Ok(Some(bytes)) => {
                self.session_record = Some(bytes.clone());
                crate::profile::SessionProfile::decode(&bytes).ok()
            }
            Ok(None) => None,
            Err(_) => {
                self.session_restore_pending = false;
                return Ok(0);
            }
        };
        let count = self.restore_session_urls(loaded.as_ref());
        self.session_restore_pending = false;
        self.session_writes_enabled = true;
        self.persist_session();
        Ok(count)
    }

    /// Restore each saved destination independently; failed URLs never block later tabs.
    fn restore_session_urls(&mut self, profile: Option<&crate::profile::SessionProfile>) -> usize {
        let Some(profile) = profile else {
            return 0;
        };
        let mut restored = Vec::new();
        for (index, url) in profile.urls.iter().enumerate() {
            let created = !restored.is_empty();
            if created && self.open_tab().is_err() {
                break;
            }
            let tab_id = self.active_tab_id.clone();
            let result = self.navigate_restored(&tab_id, url);
            if result.is_ok() {
                restored.push((index, tab_id));
            } else if created {
                let _ = self.close_tab(&tab_id);
            }
        }
        let selected = restored
            .iter()
            .find(|(index, _)| *index == profile.active_index)
            .or_else(|| restored.first())
            .map(|(_, id)| id.clone());
        if let Some(selected) = selected {
            let _ = self.select_tab(&selected);
        }
        restored.len()
    }

    /// Admit restoration as a human GET and hide query strings from the agent surface.
    fn navigate_restored(&mut self, tab_id: &str, url: &str) -> EngineResult<()> {
        self.active_tab_id = tab_id.into();
        self.permit = self.human_permit(tab_id).ok();
        let result = self.navigate_restored_request(url);
        self.permit = None;
        result
    }

    /// Build the current ordered committed URL list without exposing it to diagnostics.
    pub(super) fn persist_session(&mut self) {
        if !self.session_writes_enabled {
            return;
        }
        let Some(store) = self.session_store.clone() else {
            return;
        };
        let profile = self.session_profile();
        let Ok(bytes) = profile.encode() else {
            return;
        };
        if self.session_record.as_ref() == Some(&bytes) {
            self.session_dirty = false;
            return;
        }
        self.session_dirty = true;
        if store.replace(&bytes).is_ok() {
            self.session_record = Some(bytes);
            self.session_dirty = false;
        }
    }

    /** Persist pending native tab state before a desktop close decision is completed. */
    pub fn save_session_now(&mut self) -> bool {
        if !self.session_writes_enabled {
            return true;
        }
        self.persist_session();
        !self.session_dirty
    }

    /** Honor an explicit close-without-saving choice and prevent stop from retrying it. */
    pub fn discard_pending_session(&mut self) {
        self.session_dirty = false;
        self.session_writes_enabled = false;
    }

    /// Select only committed, credential-free HTTP(S) pages and preserve active identity.
    fn session_profile(&self) -> crate::profile::SessionProfile {
        let entries = self
            .tabs
            .iter()
            .filter_map(|tab| {
                let page = if tab.id == self.active_tab_id {
                    self.page.as_ref()
                } else {
                    tab.page.as_ref()
                };
                let url = page.map(|page| page.url.as_str())?;
                (page.is_some_and(|page| page.replayable) && crate::profile::session_url_valid(url))
                    .then(|| (tab.id.as_str(), url.to_owned()))
            })
            .collect::<Vec<_>>();
        let active_index = entries
            .iter()
            .position(|(id, _)| *id == self.selected_tab_id)
            .unwrap_or_default();
        crate::profile::SessionProfile {
            version: crate::constants::SESSION_PROFILE_VERSION,
            urls: entries.into_iter().map(|(_, url)| url).collect(),
            active_index,
        }
    }

    /// Obtain a current human permit without changing tab ownership.
    fn human_permit(&self, tab_id: &str) -> EngineResult<Permit> {
        let authority = self.authority.as_ref().ok_or_else(authority::denied)?;
        authority::lock(authority)?.permit_for(Owner::Human, tab_key(tab_id)?)
    }

    /// Delete a recovered Web Storage profile without touching the separate cookie jar.
    pub fn clear_saved_web_storage(&mut self) -> EngineResult<()> {
        self.clear_web_storage()
    }

    /// Retry reads storage without changing the active document or exposing cookie values.
    pub fn retry_profile(&mut self) -> EngineResult<()> {
        if let Some(
            error @ (crate::profile::ProfileError::CommitUncertain
            | crate::profile::ProfileError::RecoveryUnpersisted),
        ) = self.profile_error
        {
            return Err(error.engine_error());
        }
        let store = self
            .profile_store
            .as_ref()
            .ok_or_else(crate::profile::unavailable)?;
        let restored = match store.load() {
            Ok(None) => Ok(crate::network::CookieJar::default()),
            Ok(Some(bytes)) => {
                crate::network::CookieJar::from_profile(&bytes, std::time::SystemTime::now())
            }
            Err(error) => Err(error),
        };
        match restored {
            Ok(jar) => {
                self.cookies = jar;
                self.profile_error = None;
                self.restore_pending_session()?;
                Ok(())
            }
            Err(error) => {
                self.profile_error = Some(error);
                Err(error.engine_error())
            }
        }
    }

    /// Clearing deletes secure storage before dropping the live jar.
    pub fn clear_profile(&mut self) -> EngineResult<bool> {
        let store = self
            .profile_store
            .as_ref()
            .ok_or_else(crate::profile::unavailable)?;
        let had_cookies = match store.load() {
            Ok(Some(bytes)) => {
                crate::network::CookieJar::from_profile(&bytes, std::time::SystemTime::now())
                    .map(|jar| !jar.is_empty())
                    .unwrap_or(true)
            }
            Ok(None) => false,
            Err(_) => true,
        };
        if store.clear().is_err() {
            let error = if store
                .replace(&crate::network::CookieJar::recovery_record())
                .is_ok()
            {
                crate::profile::ProfileError::CommitUncertain
            } else {
                crate::profile::ProfileError::RecoveryUnpersisted
            };
            self.profile_error = Some(error);
            return Err(error.engine_error());
        }
        self.cookies = Default::default();
        self.profile_error = None;
        self.restore_pending_session()?;
        Ok(had_cookies)
    }

    /** No I/O occurs under this lock; guard validation and publication are atomic. */
    pub(super) fn publish<T>(&mut self, operation: impl FnOnce(&mut Self) -> T) -> EngineResult<T> {
        if self.cancel.load(Ordering::Relaxed) {
            return Err(EngineError::service("runtime", false));
        }
        let authority = self.authority.clone();
        let _guard = if let Some(authority) = authority.as_ref() {
            let guard = authority::lock(authority)?;
            let permit = self.permit.ok_or_else(authority::denied)?;
            let tab_id = tab_key(&self.active_tab_id)?;
            guard.validate_tab(permit, tab_id)?;
            Some(guard)
        } else {
            None
        };
        Ok(operation(self))
    }

    /** Page operations poll actual child state even if they need no RPC. */
    pub(super) fn healthy(&mut self) -> EngineResult<()> {
        if self.failed {
            return Err(EngineError::service("runtime", false));
        }
        let dead = self
            .services
            .iter_mut()
            .find_map(|service| match service.child.try_wait() {
                Ok(None) => None,
                _ => Some(service.name),
            });
        if let Some(name) = dead {
            self.stop();
            return Err(EngineError::service(name, false));
        }
        Ok(())
    }

    /** A transport fault retires all services before returning the failure. */
    pub(super) fn call(&mut self, name: &str, op: &str, params: Value) -> EngineResult<Value> {
        self.call_until(name, op, params, None)
    }

    /** A navigation resource RPC shares its existing absolute deadline. */
    pub(super) fn call_deadline(
        &mut self,
        name: &str,
        op: &str,
        params: Value,
        deadline: Instant,
    ) -> EngineResult<Value> {
        self.call_until(name, op, params, Some(deadline))
    }

    /** Every call keeps the service cap and optionally applies a tighter owner deadline. */
    fn call_until(
        &mut self,
        name: &str,
        op: &str,
        params: Value,
        deadline: Option<Instant>,
    ) -> EngineResult<Value> {
        self.healthy()?;
        let service = self
            .services
            .iter_mut()
            .find(|service| service.name == name)
            .ok_or_else(|| EngineError::service(name, false))?;
        let result = service.call(op, params, deadline);
        if result.as_ref().is_err_and(|error| error.service.is_some()) {
            self.stop();
        }
        result
    }

    /// Exposes owned process identities for native diagnostics, without transport handles.
    pub fn process_ids(&self) -> Vec<(&str, u32)> {
        self.services
            .iter()
            .map(|service| (service.name, service.child.id()))
            .collect()
    }

    /// Closing a local owner is idempotent and reaps every child.
    pub fn stop(&mut self) -> bool {
        if self.session_dirty {
            self.persist_session();
        }
        self.failed = true;
        for service in &mut self.services {
            let _ = service.child.kill();
        }
        self.services.clear();
        !self.session_dirty
    }
}

/// Native wire tab IDs map to the authority's bounded integer keys.
pub fn tab_key(id: &str) -> EngineResult<u64> {
    id.strip_prefix("tab-")
        .and_then(|value| value.parse().ok())
        .filter(|value| *value > 0)
        .ok_or_else(|| EngineError::invalid("Invalid tab_id."))
}

/** The supervisor retains the Child handle while a worker may block in pipe I/O. */
struct Service {
    /** Runtime owner may cancel while transport is blocked in either direction. */
    cancel: Arc<AtomicBool>,
    /** Static service identity is never supplied by page input. */
    name: &'static str,
    /** Retained outside the worker so deadline expiry can release blocked writes. */
    child: Child,
    /** One in-flight request; the coordinator has no unbounded queue. */
    requests: Option<SyncSender<Request>>,
    /** The worker returns exactly one reply for each accepted request. */
    replies: Receiver<EngineResult<Value>>,
    /** Joined only after child termination releases both pipe ends. */
    worker: Option<JoinHandle<()>>,
    /** Internal correlation prevents accepting a late or mismatched reply. */
    sequence: u64,
}

impl Service {
    /** Cleared environment avoids passing API keys and proxy configuration to engines. */
    fn spawn(name: &'static str, executable: &Path, cancel: Arc<AtomicBool>) -> EngineResult<Self> {
        let mut child = Command::new(executable)
            .args(["_service", name])
            .env_clear()
            .env(
                crate::constants::OWNER_PID_ENV,
                std::process::id().to_string(),
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| EngineError::service(name, false))?;
        let stdin = child.stdin.take().expect("piped stdin");
        let stdout = child.stdout.take().expect("piped stdout");
        let (sender, requests) = mpsc::sync_channel(1);
        let (replies, receiver) = mpsc::sync_channel(1);
        let worker = match thread::Builder::new()
            .name(format!("oya-{name}-rpc"))
            .spawn(move || transport(name, stdin, stdout, requests, replies))
        {
            Ok(worker) => worker,
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(EngineError::service(name, false));
            }
        };
        Ok(Self {
            cancel,
            name,
            child,
            requests: Some(sender),
            replies: receiver,
            worker: Some(worker),
            sequence: 0,
        })
    }

    /** Both worker writes and reads are covered by this external deadline. */
    fn call(
        &mut self,
        op: &str,
        params: Value,
        owner_deadline: Option<Instant>,
    ) -> EngineResult<Value> {
        let rpc_deadline = Instant::now() + rpc_timeout(op);
        let deadline = owner_deadline.map_or(rpc_deadline, |owner| owner.min(rpc_deadline));
        if Instant::now() >= deadline {
            return Err(crate::protocol::EngineError::navigation_timeout());
        }
        self.sequence += 1;
        let request = Request::new(&self.sequence.to_string(), op, params);
        encode(&request)?;
        self.requests
            .as_ref()
            .ok_or_else(|| EngineError::service(self.name, false))?
            .try_send(request)
            .map_err(|_| EngineError::service(self.name, false))?;
        loop {
            if self.cancel.load(Ordering::Relaxed) {
                return Err(EngineError::service(self.name, false));
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(EngineError::service(self.name, true));
            }
            match self.replies.recv_timeout(remaining.min(RPC_POLL)) {
                Ok(result) => return result,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(_) => return Err(EngineError::service(self.name, false)),
            }
        }
    }
}

impl Drop for Service {
    /** Kill first, release channels second, then join; blocked pipe writes cannot leak. */
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        self.requests.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

/** Network connection phases retain a wider cap while navigation supplies the hard ceiling. */
fn rpc_timeout(op: &str) -> Duration {
    if op.starts_with("network.") && op != "network.fetch" {
        WEB_RPC_TIMEOUT
    } else {
        RPC_TIMEOUT
    }
}

/** A worker owns all pipe I/O so the caller never blocks outside its deadline. */
fn transport(
    name: &str,
    mut stdin: ChildStdin,
    stdout: ChildStdout,
    requests: Receiver<Request>,
    replies: SyncSender<EngineResult<Value>>,
) {
    let mut stdout = BufReader::new(stdout);
    while let Ok(request) = requests.recv() {
        let result = exchange(&mut stdin, &mut stdout, &request)
            .unwrap_or_else(|_| Err(EngineError::service(name, false)));
        if replies.send(result).is_err() {
            return;
        }
    }
}

/** Envelope violations are transport faults; valid engine errors remain ordinary failures. */
fn exchange(
    stdin: &mut ChildStdin,
    stdout: &mut BufReader<ChildStdout>,
    request: &Request,
) -> EngineResult<EngineResult<Value>> {
    write_message(stdin, request)?;
    let frame = read_frame(stdout)?.ok_or_else(|| EngineError::invalid("Service EOF."))?;
    let reply: Response = decode(&frame)?;
    if reply.version != request.version || reply.id.as_deref() != Some(&request.id) {
        return Err(EngineError::invalid("Mismatched service reply."));
    }
    match (reply.result, reply.error) {
        (Some(result), None) => Ok(Ok(result)),
        (None, Some(error)) => Ok(Err(error)),
        _ => Err(EngineError::invalid("Invalid service reply shape.")),
    }
}

/** Delimiter cannot occur in canonical origin or monotonic tab identifiers. */
fn storage_key(tab_id: &str, origin: &str) -> String {
    format!("{tab_id}|{origin}")
}

/** Storage recovery errors never contain profile keys, origins or values. */
fn storage_profile_error(error: crate::profile::ProfileError) -> EngineError {
    match error {
        crate::profile::ProfileError::Corrupt => EngineError::new(
            "web_storage_corrupt",
            "Saved site storage could not be opened.",
            "Repair or remove the separate Web Storage profile before using stored site state.",
        ),
        crate::profile::ProfileError::CommitUncertain => EngineError::new(
            "web_storage_commit_uncertain",
            "Site storage changes could not be confirmed in secure storage.",
            "Clear saved site storage before using this profile again.",
        ),
        crate::profile::ProfileError::RecoveryUnpersisted => EngineError::new(
            "web_storage_recovery_unpersisted",
            "Secure storage could not record the site-storage recovery lock.",
            "Keep the browser open and clear saved site storage when secure storage is available. Do not restart until clearing succeeds.",
        ),
        crate::profile::ProfileError::Unavailable => storage_unavailable(),
    }
}

/** Secure storage failures stay explicit without claiming cookie recovery actions apply. */
fn storage_unavailable() -> EngineError {
    EngineError::new(
        "web_storage_unavailable",
        "Mars cannot access secure site storage.",
        "Allow secure storage access and restart the browser before relying on saved site state.",
    )
}
