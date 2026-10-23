/** Local control epochs cancel queued and in-flight writes without locks across I/O. */
use crate::protocol::{EngineError, EngineResult};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, MutexGuard},
};

/// Only native operator actions can grant or revoke control.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Owner {
    /// Native window editing is enabled.
    Human,
    /// An explicitly granted connected agent may mutate.
    Agent,
}
/// A changed epoch makes all prior admission records stale.
#[derive(Debug, Serialize)]
pub struct Authority {
    /// Current local owner.
    pub owner: Owner,
    /// Monotonic native control generation.
    pub epoch: u64,
    /// Ownership and epochs are isolated by never-reused native tab identity.
    pub tabs: HashMap<u64, (Owner, u64)>,
}
/// Requests capture authority before entering the bounded worker queue.
#[derive(Clone, Copy, Debug)]
pub struct Permit {
    /// Identity of the submitting surface.
    pub owner: Owner,
    /// Rechecked immediately before publication.
    pub epoch: u64,
    /// Page and tab UI commands remain bound to their admission tab.
    pub tab_id: Option<u64>,
}
/// No mutable document state is shared with the UI.
pub type SharedAuthority = Arc<Mutex<Authority>>;

impl Authority {
    /// Operator flags may grant initial agent ownership; page/agent operations cannot.
    pub fn shared(owner: Owner) -> SharedAuthority {
        Arc::new(Mutex::new(Self {
            owner,
            epoch: 1,
            tabs: HashMap::from([(1, (owner, 1))]),
        }))
    }
    /// Takeover and disconnect immediately invalidate queued work.
    pub fn change(&mut self, owner: Owner) {
        self.epoch = self.epoch.saturating_add(1);
        self.owner = owner;
        for state in self.tabs.values_mut() {
            state.0 = owner;
            state.1 = state.1.saturating_add(1);
        }
    }
    /// Add a new human-owned tab without granting it to an already connected agent.
    pub fn add_tab(&mut self, id: u64) {
        self.tabs.entry(id).or_insert((Owner::Human, 1));
    }
    /// Closing a tab removes its authority so stale permits cannot target a replacement.
    pub fn remove_tab(&mut self, id: u64) {
        self.tabs.remove(&id);
    }
    /// Human control actions change only the selected tab's ownership generation.
    pub fn change_tab(&mut self, id: u64, owner: Owner) -> EngineResult<()> {
        let state = self.tabs.get_mut(&id).ok_or_else(denied)?;
        state.0 = owner;
        state.1 = state.1.saturating_add(1);
        self.epoch = self.epoch.saturating_add(1);
        self.owner = owner;
        Ok(())
    }
    /// Issue a permit for one explicitly owned tab.
    pub fn permit_for(&self, owner: Owner, id: u64) -> EngineResult<Permit> {
        let (current, epoch) = self.tabs.get(&id).copied().ok_or_else(denied)?;
        let permit = Permit {
            owner,
            epoch,
            tab_id: Some(id),
        };
        if current != owner {
            return Err(denied());
        }
        Ok(permit)
    }
    /// Read-only inspection can observe a tab without claiming its mutation owner.
    pub fn observe_for(&self, owner: Owner, id: u64) -> EngineResult<Permit> {
        let (_, epoch) = self.tabs.get(&id).copied().ok_or_else(denied)?;
        Ok(Permit {
            owner,
            epoch,
            tab_id: Some(id),
        })
    }
    /// Validate both the exact tab identity and its current ownership generation.
    pub fn validate_tab(&self, permit: Permit, id: u64) -> EngineResult<()> {
        if permit.tab_id != Some(id) || self.tabs.get(&id) != Some(&(permit.owner, permit.epoch)) {
            return Err(denied());
        }
        Ok(())
    }
    /// Disconnect returns every agent-owned tab to the person and advances their epochs.
    pub fn revoke_agents(&mut self) {
        for state in self
            .tabs
            .values_mut()
            .filter(|state| state.0 == Owner::Agent)
        {
            state.0 = Owner::Human;
            state.1 = state.1.saturating_add(1);
        }
        self.owner = Owner::Human;
        self.epoch = self.epoch.saturating_add(1);
    }
    /// Read tab ownership without creating authority.
    pub fn tab_status(&self, id: u64) -> Option<(Owner, u64)> {
        self.tabs.get(&id).copied()
    }
    /// Both caller and exact generation must remain current.
    pub fn validate(&self, permit: Permit) -> EngineResult<()> {
        if self.owner != permit.owner || self.epoch != permit.epoch {
            return Err(denied());
        }
        Ok(())
    }
}
/// Poisoned authority fails closed.
pub fn lock(authority: &SharedAuthority) -> EngineResult<MutexGuard<'_, Authority>> {
    authority.lock().map_err(|_| denied())
}
/// Recovery requires a local person, never an agent self-grant.
pub fn denied() -> EngineError {
    EngineError::new(
        "control_denied",
        "Control changed or this caller does not own the browser.",
        "Ask the person to allow agent control, then inspect the current epoch and page.",
    )
}
