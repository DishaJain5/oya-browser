/** A child independently detects owner death even while its main thread blocks in OS DNS. */
use crate::{
    constants::{OWNER_PID_ENV, RPC_POLL},
    protocol::{EngineError, EngineResult},
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
};

/** One fixed watcher per service owns no transport or page data. */
pub(super) struct OwnerWatch {
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}
impl OwnerWatch {
    /** Expected owner identity is injected by the coordinator before the child can be orphaned. */
    pub fn start() -> EngineResult<Self> {
        let expected = std::env::var(OWNER_PID_ENV)
            .ok()
            .map(|value| value.parse::<libc::pid_t>())
            .transpose()
            .map_err(|_| error())?
            .unwrap_or_else(|| unsafe { libc::getppid() });
        if expected <= 1 || unsafe { libc::getppid() } != expected {
            return Err(error());
        }
        let stop = Arc::new(AtomicBool::new(false));
        let signal = stop.clone();
        let worker = thread::Builder::new()
            .name("oya-owner-watch".into())
            .spawn(move || watch(expected, &signal))
            .map_err(|_| error())?;
        Ok(Self {
            stop,
            worker: Some(worker),
        })
    }
}
impl Drop for OwnerWatch {
    /** Normal EOF joins the watcher; owner death instead exits the whole blocked child. */
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            worker.thread().unpark();
            let _ = worker.join();
        }
    }
}
/** Reparenting is decisive even if an unrelated future process reuses the old PID. */
fn watch(expected: libc::pid_t, stop: &AtomicBool) {
    while !stop.load(Ordering::Relaxed) {
        if unsafe { libc::getppid() } != expected {
            std::process::exit(1);
        }
        thread::park_timeout(RPC_POLL);
    }
}
/** A missing/dead owner never starts a detached engine. */
fn error() -> EngineError {
    EngineError::new(
        "service_unavailable",
        "The engine has no live coordinator owner.",
        "Start engines through the native coordinator.",
    )
}
