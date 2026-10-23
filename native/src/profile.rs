/** One versioned desktop cookie profile uses only the operating system Keychain. */
use crate::{
    constants::{COOKIE_PROFILE_BYTES, SESSION_PROFILE_BYTES, WEB_STORAGE_PROFILE_BYTES},
    protocol::EngineError,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Profile storage errors deliberately omit payloads and operating-system details.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileError {
    /// The saved profile cannot be read or replaced securely.
    Unavailable,
    /// The stored record is malformed, oversized, or from a future format.
    Corrupt,
    /// A timed-out replacement may have completed and cannot be safely retried.
    CommitUncertain,
    /// The recovery marker could not be persisted, so a restart may restore old cookies.
    RecoveryUnpersisted,
}

impl ProfileError {
    /// Recovery copy distinguishes retryable access failures from unsafe records.
    pub fn engine_error(self) -> EngineError {
        match self {
            Self::Unavailable => EngineError::new(
                "profile_unavailable",
                "Oya Native can’t access secure storage. No saved cookies were loaded.",
                "Allow secure storage access, then retry or clear saved cookies.",
            ),
            Self::Corrupt => EngineError::new(
                "profile_corrupt",
                "Saved cookies couldn’t be opened.",
                "Clear saved cookies to start with an empty profile.",
            ),
            Self::CommitUncertain => EngineError::new(
                "profile_commit_uncertain",
                "Cookie changes could not be confirmed in secure storage.",
                "Clear saved cookies before using this profile again.",
            ),
            Self::RecoveryUnpersisted => EngineError::new(
                "profile_recovery_unpersisted",
                "Secure storage could not record the recovery lock. Saved cookies may remain.",
                "Keep this browser open and retry Clear Saved Cookies when secure storage is available. Do not restart until clearing succeeds.",
            ),
        }
    }
}

/// One owner persists a complete candidate before publishing it to navigation.
pub trait ProfileStore: Send + Sync {
    /// Read the one default profile; absence means a new empty profile.
    fn load(&self) -> Result<Option<Vec<u8>>, ProfileError>;
    /// Atomically replace the Keychain item's payload.
    fn replace(&self, bytes: &[u8]) -> Result<(), ProfileError>;
    /// Remove the stored profile; an absent item is already clear.
    fn clear(&self) -> Result<(), ProfileError>;
}

/** Web Storage uses a separate secure item so clearing cookies preserves site state. */
pub trait WebStorageStore: Send + Sync {
    /// Read the versioned origin map; absence means a new empty profile.
    fn load(&self) -> Result<Option<Vec<u8>>, ProfileError>;
    /// Replace one complete bounded origin map atomically.
    fn replace(&self, bytes: &[u8]) -> Result<(), ProfileError>;
    /// Remove an absent or existing record when rolling back a first write.
    fn clear(&self) -> Result<(), ProfileError>;
    /// Read a separate fail-closed marker before any origin data is restored.
    fn recovery_required(&self) -> Result<bool, ProfileError>;
    /// Persist a marker that contains no origin, key, or value data.
    fn mark_recovery(&self) -> Result<(), ProfileError>;
    /// Remove the marker only after the origin map has been deleted successfully.
    fn clear_recovery(&self) -> Result<(), ProfileError>;
}

/** Session URLs use an independent secure record and never contain request bodies. */
pub trait SessionStore: Send + Sync {
    /// Read the last committed tab list; absence means a fresh browser session.
    fn load(&self) -> Result<Option<Vec<u8>>, ProfileError>;
    /// Replace the complete bounded HTTP(S) tab list.
    fn replace(&self, bytes: &[u8]) -> Result<(), ProfileError>;
}

/** Ordered committed GET destinations are restored only after browser profiles are ready. */
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SessionProfile {
    /// Version rejects formats with incompatible restore semantics.
    pub version: u32,
    /// Persisted eligible URLs preserve order and never contain POST bodies.
    pub urls: Vec<String>,
    /// Selected URL index; an empty profile uses zero.
    pub active_index: usize,
}

impl SessionProfile {
    /// Validate URLs, selection, tab count and encoded size before persistence.
    pub fn encode(&self) -> Result<Vec<u8>, ProfileError> {
        let valid = self.version == crate::constants::SESSION_PROFILE_VERSION
            && self.urls.len() <= crate::constants::TABS
            && (self.urls.is_empty() && self.active_index == 0
                || self.active_index < self.urls.len())
            && self.urls.iter().all(|value| session_url_valid(value));
        if !valid {
            return Err(ProfileError::Corrupt);
        }
        let bytes = serde_json::to_vec(self).map_err(|_| ProfileError::Corrupt)?;
        if bytes.len() > SESSION_PROFILE_BYTES {
            return Err(ProfileError::Corrupt);
        }
        Ok(bytes)
    }

    /// Decode strict session state; malformed data is ignored by startup restore.
    pub fn decode(bytes: &[u8]) -> Result<Self, ProfileError> {
        if bytes.len() > SESSION_PROFILE_BYTES {
            return Err(ProfileError::Corrupt);
        }
        let profile: Self = serde_json::from_slice(bytes).map_err(|_| ProfileError::Corrupt)?;
        profile.encode()?;
        Ok(profile)
    }
}

/// Session URLs must be credential-free absolute HTTP(S) destinations within protocol bounds.
pub(crate) fn session_url_valid(value: &str) -> bool {
    value.len() <= crate::constants::URL_BYTES
        && url::Url::parse(value).is_ok_and(|parsed| {
            ["http", "https"].contains(&parsed.scheme())
                && parsed.host_str().is_some()
                && parsed.username().is_empty()
                && parsed.password().is_none()
        })
}

/** Complete localStorage state has one independent Keychain transaction boundary. */
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WebStorageProfile {
    /// Version rejects records whose semantics this runtime does not understand.
    pub version: u32,
    /// Canonical serialized origin to bounded string key/value map.
    pub origins: BTreeMap<String, BTreeMap<String, String>>,
}

impl WebStorageProfile {
    /// Serialize only after entry, value, aggregate and encoded bounds pass.
    pub fn encode(&self) -> Result<Vec<u8>, ProfileError> {
        if self.version != crate::constants::WEB_STORAGE_PROFILE_VERSION
            || self.origins.values().any(|area| {
                area.len() > crate::constants::STORAGE_ENTRIES
                    || area.iter().any(|(key, value)| {
                        key.len() > crate::constants::STORAGE_VALUE_BYTES
                            || value.len() > crate::constants::STORAGE_VALUE_BYTES
                            || key.encode_utf16().count() > crate::constants::JS_STRING_UNITS
                            || value.encode_utf16().count() > crate::constants::JS_STRING_UNITS
                    })
                    || area
                        .iter()
                        .map(|(key, value)| key.len().saturating_add(value.len()))
                        .sum::<usize>()
                        > crate::constants::STORAGE_AREA_BYTES
            })
        {
            return Err(ProfileError::Corrupt);
        }
        let bytes = serde_json::to_vec(self).map_err(|_| ProfileError::Corrupt)?;
        if bytes.len() > WEB_STORAGE_PROFILE_BYTES {
            return Err(ProfileError::Corrupt);
        }
        Ok(bytes)
    }

    /// Decode a strict bounded record without ever including stored values in errors.
    pub fn decode(bytes: &[u8]) -> Result<Self, ProfileError> {
        if bytes.len() > WEB_STORAGE_PROFILE_BYTES {
            return Err(ProfileError::Corrupt);
        }
        let profile: Self = serde_json::from_slice(bytes).map_err(|_| ProfileError::Corrupt)?;
        profile.encode()?;
        Ok(profile)
    }
}

/// A test-only store can assert commit order without exposing profile bytes in logs.
#[cfg(any(test, feature = "test-support"))]
pub mod test_support {
    use super::{ProfileError, ProfileStore, SessionStore, WebStorageStore};
    use std::sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    };

    /// Isolated profile bytes allow restart and failure tests without the user Keychain.
    #[derive(Default)]
    pub struct MemoryStore {
        value: Mutex<Option<Vec<u8>>>,
        fail_replace: AtomicBool,
        fail_clear: AtomicBool,
        fail_load: AtomicBool,
    }

    impl MemoryStore {
        /// Seed an isolated test store without using the user's Keychain.
        pub fn seed(&self, bytes: Vec<u8>) {
            if let Ok(mut value) = self.value.lock() {
                *value = Some(bytes);
            }
        }
        /// Fail exactly the next replacement before changing the stored record.
        pub fn fail_next_replace(&self) {
            self.fail_replace.store(true, Ordering::SeqCst);
        }
        /// Fail exactly the next clear while retaining the stored record.
        pub fn fail_next_clear(&self) {
            self.fail_clear.store(true, Ordering::SeqCst);
        }
        /// Fail exactly the next load to exercise inaccessible secure storage.
        pub fn fail_next_load(&self) {
            self.fail_load.store(true, Ordering::SeqCst);
        }
        /// Read test state for assertions without formatting the secret payload.
        pub fn has_record(&self) -> bool {
            self.value.lock().is_ok_and(|value| value.is_some())
        }
    }

    impl ProfileStore for MemoryStore {
        fn load(&self) -> Result<Option<Vec<u8>>, ProfileError> {
            if self.fail_load.swap(false, Ordering::SeqCst) {
                return Err(ProfileError::Unavailable);
            }
            self.value
                .lock()
                .map(|value| value.clone())
                .map_err(|_| ProfileError::Unavailable)
        }
        fn replace(&self, bytes: &[u8]) -> Result<(), ProfileError> {
            if self.fail_replace.swap(false, Ordering::SeqCst) {
                return Err(ProfileError::Unavailable);
            }
            self.value
                .lock()
                .map(|mut value| *value = Some(bytes.to_vec()))
                .map_err(|_| ProfileError::Unavailable)
        }
        fn clear(&self) -> Result<(), ProfileError> {
            if self.fail_clear.swap(false, Ordering::SeqCst) {
                return Err(ProfileError::Unavailable);
            }
            self.value
                .lock()
                .map(|mut value| *value = None)
                .map_err(|_| ProfileError::Unavailable)
        }
    }

    /** Isolated secure-record stand-in proves localStorage restart and rollback semantics. */
    #[derive(Default)]
    pub struct MemoryWebStorageStore {
        value: Mutex<Option<Vec<u8>>>,
        recovery: Mutex<bool>,
        fail_replace: AtomicBool,
        fail_load: AtomicBool,
        fail_clear: AtomicBool,
        fail_mark: AtomicBool,
        fail_recovery_read: AtomicBool,
        fail_recovery_clear: AtomicBool,
    }

    /** Hermetic session repository supports restart and invalid-record tests. */
    #[derive(Default)]
    pub struct MemorySessionStore {
        value: Mutex<Option<Vec<u8>>>,
        fail_load: AtomicBool,
        fail_replace: AtomicBool,
    }

    impl MemorySessionStore {
        /// Seed an encoded record without exposing URL contents in diagnostics.
        pub fn seed(&self, bytes: Vec<u8>) {
            if let Ok(mut value) = self.value.lock() {
                *value = Some(bytes);
            }
        }
        /// Fail one load to verify startup remains usable without restoring.
        pub fn fail_next_load(&self) {
            self.fail_load.store(true, Ordering::SeqCst);
        }
        /// Fail one save; runtime browsing must remain available.
        pub fn fail_next_replace(&self) {
            self.fail_replace.store(true, Ordering::SeqCst);
        }
        /// Return only presence for privacy-preserving assertions.
        pub fn has_record(&self) -> bool {
            self.value.lock().is_ok_and(|value| value.is_some())
        }
        /// Read the test record for structured assertions without formatting bytes.
        pub fn record(&self) -> Option<Vec<u8>> {
            self.value.lock().ok().and_then(|value| value.clone())
        }
    }

    impl SessionStore for MemorySessionStore {
        fn load(&self) -> Result<Option<Vec<u8>>, ProfileError> {
            if self.fail_load.swap(false, Ordering::SeqCst) {
                return Err(ProfileError::Unavailable);
            }
            self.value
                .lock()
                .map(|value| value.clone())
                .map_err(|_| ProfileError::Unavailable)
        }
        fn replace(&self, bytes: &[u8]) -> Result<(), ProfileError> {
            if self.fail_replace.swap(false, Ordering::SeqCst) {
                return Err(ProfileError::Unavailable);
            }
            self.value
                .lock()
                .map(|mut value| *value = Some(bytes.to_vec()))
                .map_err(|_| ProfileError::Unavailable)
        }
    }

    impl MemoryWebStorageStore {
        /// Read only record presence for tests that must not print site values.
        pub fn has_record(&self) -> bool {
            self.value.lock().is_ok_and(|value| value.is_some())
        }
        /// Seed a strict malformed record without exposing it in assertion output.
        pub fn seed(&self, bytes: Vec<u8>) {
            if let Ok(mut value) = self.value.lock() {
                *value = Some(bytes);
            }
        }
        /// Fail one write without modifying the prior record.
        pub fn fail_next_replace(&self) {
            self.fail_replace.store(true, Ordering::SeqCst);
        }
        /// Fail one load to exercise secure profile recovery.
        pub fn fail_next_load(&self) {
            self.fail_load.store(true, Ordering::SeqCst);
        }
        /// Fail origin deletion while retaining both data and recovery state.
        pub fn fail_next_clear(&self) {
            self.fail_clear.store(true, Ordering::SeqCst);
        }
        /// Fail marker persistence to verify restart safety is reported honestly.
        pub fn fail_next_mark(&self) {
            self.fail_mark.store(true, Ordering::SeqCst);
        }
        /// Fail marker reads so startup must reject the origin map.
        pub fn fail_next_recovery_read(&self) {
            self.fail_recovery_read.store(true, Ordering::SeqCst);
        }
        /// Fail marker deletion after the origin map has already been removed.
        pub fn fail_next_recovery_clear(&self) {
            self.fail_recovery_clear.store(true, Ordering::SeqCst);
        }
        /// Seed recovery state without exposing any site value in test output.
        pub fn seed_recovery(&self) {
            if let Ok(mut value) = self.recovery.lock() {
                *value = true;
            }
        }
        /// Read marker presence without formatting stored site data.
        pub fn has_recovery(&self) -> bool {
            self.recovery.lock().is_ok_and(|value| *value)
        }
    }

    impl WebStorageStore for MemoryWebStorageStore {
        fn load(&self) -> Result<Option<Vec<u8>>, ProfileError> {
            if self.fail_load.swap(false, Ordering::SeqCst) {
                return Err(ProfileError::Unavailable);
            }
            self.value
                .lock()
                .map(|value| value.clone())
                .map_err(|_| ProfileError::Unavailable)
        }
        fn replace(&self, bytes: &[u8]) -> Result<(), ProfileError> {
            if self.fail_replace.swap(false, Ordering::SeqCst) {
                return Err(ProfileError::Unavailable);
            }
            self.value
                .lock()
                .map(|mut value| *value = Some(bytes.to_vec()))
                .map_err(|_| ProfileError::Unavailable)
        }
        fn clear(&self) -> Result<(), ProfileError> {
            if self.fail_clear.swap(false, Ordering::SeqCst) {
                return Err(ProfileError::Unavailable);
            }
            self.value
                .lock()
                .map(|mut value| *value = None)
                .map_err(|_| ProfileError::Unavailable)
        }
        fn recovery_required(&self) -> Result<bool, ProfileError> {
            if self.fail_recovery_read.swap(false, Ordering::SeqCst) {
                return Err(ProfileError::Unavailable);
            }
            self.recovery
                .lock()
                .map(|value| *value)
                .map_err(|_| ProfileError::Unavailable)
        }
        fn mark_recovery(&self) -> Result<(), ProfileError> {
            if self.fail_mark.swap(false, Ordering::SeqCst) {
                return Err(ProfileError::Unavailable);
            }
            self.recovery
                .lock()
                .map(|mut value| *value = true)
                .map_err(|_| ProfileError::Unavailable)
        }
        fn clear_recovery(&self) -> Result<(), ProfileError> {
            if self.fail_recovery_clear.swap(false, Ordering::SeqCst) {
                return Err(ProfileError::Unavailable);
            }
            self.recovery
                .lock()
                .map(|mut value| *value = false)
                .map_err(|_| ProfileError::Unavailable)
        }
    }
}

/// Convert storage failures to safe UI and protocol text without secret details.
pub fn unavailable() -> EngineError {
    ProfileError::Unavailable.engine_error()
}

/// A record exceeding its independent bound is rejected before entering Keychain.
pub fn validate_size(bytes: &[u8]) -> Result<(), ProfileError> {
    if bytes.len() > COOKIE_PROFILE_BYTES {
        return Err(ProfileError::Corrupt);
    }
    Ok(())
}

/// Build the production store on macOS without providing a plaintext fallback.
#[cfg(target_os = "macos")]
pub fn system_store() -> std::sync::Arc<dyn ProfileStore> {
    std::sync::Arc::new(keychain::KeychainStore)
}

/** Production localStorage remains in a distinct Keychain account from cookie data. */
#[cfg(target_os = "macos")]
pub fn system_web_storage_store() -> std::sync::Arc<dyn WebStorageStore> {
    std::sync::Arc::new(keychain::KeychainWebStorageStore)
}

/** The desktop session record is encrypted independently from cookies and Web Storage. */
#[cfg(target_os = "macos")]
pub fn system_session_store() -> std::sync::Arc<dyn SessionStore> {
    std::sync::Arc::new(keychain::KeychainSessionStore)
}

/// Other desktop platforms cannot silently substitute weaker persistence.
#[cfg(not(target_os = "macos"))]
pub fn system_store() -> std::sync::Arc<dyn ProfileStore> {
    std::sync::Arc::new(UnsupportedStore)
}

/** Unsupported platforms fail closed rather than persist browser state as plaintext. */
#[cfg(not(target_os = "macos"))]
pub fn system_web_storage_store() -> std::sync::Arc<dyn WebStorageStore> {
    std::sync::Arc::new(UnsupportedStore)
}

/** Unsupported platforms skip tab persistence rather than writing URLs as plaintext. */
#[cfg(not(target_os = "macos"))]
pub fn system_session_store() -> std::sync::Arc<dyn SessionStore> {
    std::sync::Arc::new(UnsupportedStore)
}

/** Platforms without the current native desktop never persist cookies insecurely. */
#[cfg(not(target_os = "macos"))]
struct UnsupportedStore;
#[cfg(not(target_os = "macos"))]
impl ProfileStore for UnsupportedStore {
    fn load(&self) -> Result<Option<Vec<u8>>, ProfileError> {
        Err(ProfileError::Unavailable)
    }
    fn replace(&self, _bytes: &[u8]) -> Result<(), ProfileError> {
        Err(ProfileError::Unavailable)
    }
    fn clear(&self) -> Result<(), ProfileError> {
        Err(ProfileError::Unavailable)
    }
}
#[cfg(not(target_os = "macos"))]
impl WebStorageStore for UnsupportedStore {
    fn load(&self) -> Result<Option<Vec<u8>>, ProfileError> {
        Err(ProfileError::Unavailable)
    }
    fn replace(&self, _bytes: &[u8]) -> Result<(), ProfileError> {
        Err(ProfileError::Unavailable)
    }
    fn clear(&self) -> Result<(), ProfileError> {
        Err(ProfileError::Unavailable)
    }
    fn recovery_required(&self) -> Result<bool, ProfileError> {
        Err(ProfileError::Unavailable)
    }
    fn mark_recovery(&self) -> Result<(), ProfileError> {
        Err(ProfileError::Unavailable)
    }
    fn clear_recovery(&self) -> Result<(), ProfileError> {
        Err(ProfileError::Unavailable)
    }
}
#[cfg(not(target_os = "macos"))]
impl SessionStore for UnsupportedStore {
    fn load(&self) -> Result<Option<Vec<u8>>, ProfileError> {
        Err(ProfileError::Unavailable)
    }
    fn replace(&self, _bytes: &[u8]) -> Result<(), ProfileError> {
        Err(ProfileError::Unavailable)
    }
}

#[cfg(target_os = "macos")]
mod keychain {
    use super::{
        ProfileError, ProfileStore, SESSION_PROFILE_BYTES, SessionStore, WEB_STORAGE_PROFILE_BYTES,
        WebStorageStore, validate_size,
    };
    use crate::constants::PROFILE_STORE_TIMEOUT;
    use security_framework::passwords::{
        delete_generic_password, get_generic_password, set_generic_password,
    };
    use security_framework_sys::base::errSecItemNotFound;
    use std::sync::atomic::{AtomicBool, Ordering};

    /** One in-flight Keychain call bounds retries if the OS stalls. */
    static STORE_BUSY: AtomicBool = AtomicBool::new(false);
    /** Recovery writes proceed independently when an origin operation is stuck late. */
    static RECOVERY_BUSY: AtomicBool = AtomicBool::new(false);

    /// Stable identifiers isolate Oya Native's default profile from other apps.
    const SERVICE: &str = "com.oyabrowser.native.profile";
    /// One complete bounded jar gives Keychain replacement a single commit boundary.
    const ACCOUNT: &str = "default-cookie-jar-v1";
    /// Web Storage is intentionally independent from the cookie-clear operation.
    const STORAGE_ACCOUNT: &str = "default-web-storage-v1";
    /// Recovery state is separate so origin replacement cannot erase the fail-closed marker.
    const STORAGE_RECOVERY_ACCOUNT: &str = "default-web-storage-recovery-v1";
    /// Tab URLs use an independent record and survive cookie or site-data clearing.
    const SESSION_ACCOUNT: &str = "default-tab-session-v1";

    /** Keychain owns encryption and access policy; no cookie data reaches disk files. */
    pub(super) struct KeychainStore;
    /** Independent secure record owns only first-party localStorage values. */
    pub(super) struct KeychainWebStorageStore;
    /** The current ordered GET session is encrypted independently from site data. */
    pub(super) struct KeychainSessionStore;

    impl WebStorageStore for KeychainWebStorageStore {
        fn load(&self) -> Result<Option<Vec<u8>>, ProfileError> {
            bounded_storage(load_storage_now)
        }
        fn replace(&self, bytes: &[u8]) -> Result<(), ProfileError> {
            if bytes.len() > WEB_STORAGE_PROFILE_BYTES {
                return Err(ProfileError::Corrupt);
            }
            let bytes = bytes.to_vec();
            bounded_storage(move || replace_storage_now(&bytes))
        }
        fn clear(&self) -> Result<(), ProfileError> {
            bounded_storage(clear_storage_now)
        }
        fn recovery_required(&self) -> Result<bool, ProfileError> {
            bounded_recovery(load_recovery_now)
        }
        fn mark_recovery(&self) -> Result<(), ProfileError> {
            bounded_recovery(mark_recovery_now)
        }
        fn clear_recovery(&self) -> Result<(), ProfileError> {
            bounded_recovery(clear_recovery_now)
        }
    }

    impl SessionStore for KeychainSessionStore {
        fn load(&self) -> Result<Option<Vec<u8>>, ProfileError> {
            bounded_storage(load_session_now)
        }
        fn replace(&self, bytes: &[u8]) -> Result<(), ProfileError> {
            if bytes.len() > SESSION_PROFILE_BYTES {
                return Err(ProfileError::Corrupt);
            }
            let bytes = bytes.to_vec();
            bounded_storage(move || replace_session_now(&bytes))
        }
    }

    impl ProfileStore for KeychainStore {
        fn load(&self) -> Result<Option<Vec<u8>>, ProfileError> {
            bounded(load_now)
        }

        fn replace(&self, bytes: &[u8]) -> Result<(), ProfileError> {
            validate_size(bytes)?;
            let bytes = bytes.to_vec();
            bounded(move || replace_now(&bytes))
        }

        fn clear(&self) -> Result<(), ProfileError> {
            bounded(clear_now)
        }
    }

    /// Keychain reads return one bounded data item or a new-profile result.
    fn load_now() -> Result<Option<Vec<u8>>, ProfileError> {
        match get_generic_password(SERVICE, ACCOUNT) {
            Ok(bytes) => {
                validate_size(&bytes)?;
                Ok(Some(bytes))
            }
            Err(error) if error.code() == errSecItemNotFound => Ok(None),
            Err(_) => Err(ProfileError::Unavailable),
        }
    }

    /// Load the independent Web Storage item with its own encoded bound.
    fn load_storage_now() -> Result<Option<Vec<u8>>, ProfileError> {
        match get_generic_password(SERVICE, STORAGE_ACCOUNT) {
            Ok(bytes) if bytes.len() <= WEB_STORAGE_PROFILE_BYTES => Ok(Some(bytes)),
            Ok(_) => Err(ProfileError::Corrupt),
            Err(error) if error.code() == errSecItemNotFound => Ok(None),
            Err(_) => Err(ProfileError::Unavailable),
        }
    }

    /// Load the separate bounded browser-session record.
    fn load_session_now() -> Result<Option<Vec<u8>>, ProfileError> {
        match get_generic_password(SERVICE, SESSION_ACCOUNT) {
            Ok(bytes) if bytes.len() <= SESSION_PROFILE_BYTES => Ok(Some(bytes)),
            Ok(_) => Err(ProfileError::Corrupt),
            Err(error) if error.code() == errSecItemNotFound => Ok(None),
            Err(_) => Err(ProfileError::Unavailable),
        }
    }

    /// Replace only the ordered HTTP(S) tab session, never cookies or site storage.
    fn replace_session_now(bytes: &[u8]) -> Result<(), ProfileError> {
        if bytes.len() > SESSION_PROFILE_BYTES {
            return Err(ProfileError::Corrupt);
        }
        set_generic_password(SERVICE, SESSION_ACCOUNT, bytes).map_err(|_| ProfileError::Unavailable)
    }

    /// Replace the origin map without touching the cookie account.
    fn replace_storage_now(bytes: &[u8]) -> Result<(), ProfileError> {
        if bytes.len() > WEB_STORAGE_PROFILE_BYTES {
            return Err(ProfileError::Corrupt);
        }
        set_generic_password(SERVICE, STORAGE_ACCOUNT, bytes).map_err(|_| ProfileError::Unavailable)
    }

    /// Clearing storage never deletes or modifies the cookie item.
    fn clear_storage_now() -> Result<(), ProfileError> {
        match delete_generic_password(SERVICE, STORAGE_ACCOUNT) {
            Ok(()) => Ok(()),
            Err(error) if error.code() == errSecItemNotFound => Ok(()),
            Err(_) => Err(ProfileError::Unavailable),
        }
    }

    /// A malformed marker is treated as present so startup always fails closed.
    fn load_recovery_now() -> Result<bool, ProfileError> {
        match get_generic_password(SERVICE, STORAGE_RECOVERY_ACCOUNT) {
            Ok(bytes) if bytes.as_slice() == b"oya-web-storage-recovery-v1" => Ok(true),
            Ok(_) => Ok(true),
            Err(error) if error.code() == errSecItemNotFound => Ok(false),
            Err(_) => Err(ProfileError::Unavailable),
        }
    }

    /// Store a fixed payload containing no page-controlled or site data.
    fn mark_recovery_now() -> Result<(), ProfileError> {
        set_generic_password(
            SERVICE,
            STORAGE_RECOVERY_ACCOUNT,
            b"oya-web-storage-recovery-v1",
        )
        .map_err(|_| ProfileError::Unavailable)
    }

    /// The caller deletes the origin map first, making marker deletion safe after late writes.
    fn clear_recovery_now() -> Result<(), ProfileError> {
        match delete_generic_password(SERVICE, STORAGE_RECOVERY_ACCOUNT) {
            Ok(()) => Ok(()),
            Err(error) if error.code() == errSecItemNotFound => Ok(()),
            Err(_) => Err(ProfileError::Unavailable),
        }
    }

    /// Web Storage timeout is uncertain because Keychain may finish after caller timeout.
    fn bounded_storage<T: Send + 'static>(
        operation: impl FnOnce() -> Result<T, ProfileError> + Send + 'static,
    ) -> Result<T, ProfileError> {
        if STORE_BUSY.swap(true, Ordering::AcqRel) {
            return Err(ProfileError::Unavailable);
        }
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        let worker = std::thread::Builder::new()
            .name("oya-storage-keychain".into())
            .spawn(move || {
                let result = operation();
                STORE_BUSY.store(false, Ordering::Release);
                let _ = sender.send(result);
            });
        if worker.is_err() {
            STORE_BUSY.store(false, Ordering::Release);
            return Err(ProfileError::Unavailable);
        }
        receiver
            .recv_timeout(PROFILE_STORE_TIMEOUT)
            .unwrap_or(Err(ProfileError::CommitUncertain))
    }

    /// Recovery marker I/O has its own deadline lane and never waits for a late origin write.
    fn bounded_recovery<T: Send + 'static>(
        operation: impl FnOnce() -> Result<T, ProfileError> + Send + 'static,
    ) -> Result<T, ProfileError> {
        if RECOVERY_BUSY.swap(true, Ordering::AcqRel) {
            return Err(ProfileError::Unavailable);
        }
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        let worker = std::thread::Builder::new()
            .name("oya-storage-recovery".into())
            .spawn(move || {
                let result = operation();
                RECOVERY_BUSY.store(false, Ordering::Release);
                let _ = sender.send(result);
            });
        if worker.is_err() {
            RECOVERY_BUSY.store(false, Ordering::Release);
            return Err(ProfileError::Unavailable);
        }
        receiver
            .recv_timeout(PROFILE_STORE_TIMEOUT)
            .unwrap_or(Err(ProfileError::CommitUncertain))
    }

    /// One Keychain item replacement commits the complete profile record.
    fn replace_now(bytes: &[u8]) -> Result<(), ProfileError> {
        validate_size(bytes)?;
        set_generic_password(SERVICE, ACCOUNT, bytes).map_err(|_| ProfileError::Unavailable)
    }

    /// Clearing a missing Keychain item is already a successful reset.
    fn clear_now() -> Result<(), ProfileError> {
        match delete_generic_password(SERVICE, ACCOUNT) {
            Ok(()) => Ok(()),
            Err(error) if error.code() == errSecItemNotFound => Ok(()),
            Err(_) => Err(ProfileError::Unavailable),
        }
    }

    /// OS calls run off the browser worker with a deadline and one-flight guard.
    fn bounded<T: Send + 'static>(
        operation: impl FnOnce() -> Result<T, ProfileError> + Send + 'static,
    ) -> Result<T, ProfileError> {
        if STORE_BUSY.swap(true, Ordering::AcqRel) {
            return Err(ProfileError::Unavailable);
        }
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        let worker = std::thread::Builder::new()
            .name("oya-profile-keychain".into())
            .spawn(move || {
                let result = operation();
                STORE_BUSY.store(false, Ordering::Release);
                let _ = sender.send(result);
            });
        if worker.is_err() {
            STORE_BUSY.store(false, Ordering::Release);
            return Err(ProfileError::Unavailable);
        }
        receiver
            .recv_timeout(PROFILE_STORE_TIMEOUT)
            .unwrap_or(Err(ProfileError::Unavailable))
    }
}
