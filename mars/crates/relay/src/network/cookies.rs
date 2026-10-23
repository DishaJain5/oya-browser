/** Pure, bounded host-only session cookies; no filesystem or public inspection surface. */
use crate::{
    constants::{COOKIE_BYTES, COOKIE_COUNT, COOKIE_JAR_BYTES, HOST_COOKIES},
    profile::{ProfileError, validate_size},
};
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};
use url::Url;

/// Session state belongs to one coordinator and disappears with that owner.
#[derive(Clone, Default)]
pub struct CookieJar {
    records: Vec<Cookie>,
}
/** Stored records cannot be serialized into agent responses. */
#[derive(Clone)]
struct Cookie {
    name: String,
    value: String,
    host: String,
    path: String,
    secure: bool,
    same_site: SameSite,
    expires: Option<SystemTime>,
}
/** Exact-host same-site matching is deliberately stricter than registrable-domain matching. */
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum SameSite {
    Strict,
    Lax,
    None,
}
impl CookieJar {
    /// Persisted recovery records make an uncertain profile fail closed after restart.
    pub fn recovery_record() -> Vec<u8> {
        br#"{"version":255,"cookies":[]}"#.to_vec()
    }
    /// Empty profiles can be cleared without claiming a session was removed.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
    /// Assemble only applicable cookies, sorted by longest path and stable creation order.
    pub fn header(
        &mut self,
        url: &Url,
        initiator: Option<&Url>,
        safe: bool,
        now: SystemTime,
    ) -> String {
        self.expire(now);
        let same = initiator
            .is_some_and(|site| site.scheme() == url.scheme() && site.host_str() == url.host_str());
        let mut records = self
            .records
            .iter()
            .filter(|c| {
                c.host == url.host_str().unwrap_or_default()
                    && path_matches(url.path(), &c.path)
                    && (!c.secure || url.scheme() == "https")
                    && (same
                        || c.same_site == SameSite::None
                        || (safe && c.same_site == SameSite::Lax))
            })
            .collect::<Vec<_>>();
        records.sort_by_key(|c| std::cmp::Reverse(c.path.len()));
        records
            .iter()
            .map(|c| format!("{}={}", c.name, c.value))
            .collect::<Vec<_>>()
            .join("; ")
    }
    /// Return value-free diagnostics; malformed and Domain cookies never gain authority.
    pub fn receive(&mut self, url: &Url, fields: &[String], now: SystemTime) -> Vec<String> {
        self.expire(now);
        let mut diagnostics = Vec::new();
        for field in fields {
            if let Err(reason) = self.insert(url, field, now)
                && diagnostics.len() < crate::constants::DIAGNOSTICS
                && !diagnostics.iter().any(|s| s == reason)
            {
                diagnostics.push(reason.into());
            }
        }
        diagnostics
    }
    /// Encode a complete bounded jar for atomic Keychain replacement.
    pub fn profile_record(&self) -> Result<Vec<u8>, ProfileError> {
        let record = StoredJar {
            version: PROFILE_VERSION,
            cookies: self.records.iter().map(StoredCookie::from).collect(),
        };
        let bytes = serde_json::to_vec(&record).map_err(|_| ProfileError::Corrupt)?;
        validate_size(&bytes)?;
        Ok(bytes)
    }
    /// Restore the whole record or reject it, never accepting a partially valid jar.
    pub fn from_profile(bytes: &[u8], now: SystemTime) -> Result<Self, ProfileError> {
        validate_size(bytes)?;
        let stored: StoredJar = serde_json::from_slice(bytes).map_err(|_| ProfileError::Corrupt)?;
        if stored.version != PROFILE_VERSION || stored.cookies.len() > COOKIE_COUNT {
            return Err(ProfileError::Corrupt);
        }
        let mut jar = Self::default();
        let mut identities = std::collections::HashSet::new();
        for item in stored.cookies {
            let cookie = item.restore(now)?;
            if !identities.insert((
                cookie.host.clone(),
                cookie.path.clone(),
                cookie.name.clone(),
            )) {
                return Err(ProfileError::Corrupt);
            }
            if cookie.expires.is_none_or(|expiry| expiry > now) {
                jar.records.push(cookie);
            }
        }
        jar.validate_capacity()?;
        Ok(jar)
    }
    /** Expiry happens before both matching and capacity decisions. */
    fn expire(&mut self, now: SystemTime) {
        self.records
            .retain(|c| c.expires.is_none_or(|expiry| expiry > now));
    }
    /// Persisted records retain the same per-host and aggregate cookie budgets.
    fn validate_capacity(&self) -> Result<(), ProfileError> {
        let mut hosts = std::collections::HashMap::<&str, usize>::new();
        for cookie in &self.records {
            *hosts.entry(&cookie.host).or_default() += 1;
        }
        let bytes = self
            .records
            .iter()
            .map(|c| c.name.len() + c.value.len() + c.host.len() + c.path.len())
            .sum::<usize>();
        if bytes > COOKIE_JAR_BYTES || hosts.values().any(|count| *count > HOST_COOKIES) {
            return Err(ProfileError::Corrupt);
        }
        Ok(())
    }
    /** Replacement keeps creation order and cannot exceed any resource budget. */
    fn insert(&mut self, url: &Url, field: &str, now: SystemTime) -> Result<(), &'static str> {
        let cookie = parse(url, field, now)?;
        if url.scheme() != "https"
            && self.records.iter().any(|c| {
                c.secure
                    && c.host == cookie.host
                    && c.name == cookie.name
                    && (path_matches(&cookie.path, &c.path) || path_matches(&c.path, &cookie.path))
            })
        {
            return Err("An insecure response cannot replace a Secure cookie.");
        }
        let index = self
            .records
            .iter()
            .position(|c| c.host == cookie.host && c.path == cookie.path && c.name == cookie.name);
        if cookie.expires.is_some_and(|expiry| expiry <= now) {
            if let Some(index) = index {
                self.records.remove(index);
            }
            return Ok(());
        }
        let mut candidate = self.records.clone();
        if let Some(index) = index {
            candidate[index] = cookie.clone();
        } else {
            candidate.push(cookie.clone());
        }
        if candidate.len() > COOKIE_COUNT
            || candidate.iter().filter(|c| c.host == cookie.host).count() > HOST_COOKIES
            || candidate
                .iter()
                .map(|c| c.name.len() + c.value.len() + c.host.len() + c.path.len())
                .sum::<usize>()
                > COOKIE_JAR_BYTES
        {
            return Err("Session cookie capacity was reached.");
        }
        self.records = candidate;
        Ok(())
    }
}

/// Profile versioning rejects future formats rather than guessing cookie policy.
const PROFILE_VERSION: u8 = 1;

/// The complete JSON record is private to the Keychain adapter.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredJar {
    version: u8,
    cookies: Vec<StoredCookie>,
}

/// Expiry uses Unix seconds because HTTP cookie dates have second precision.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredCookie {
    name: String,
    value: String,
    host: String,
    path: String,
    secure: bool,
    same_site: SameSite,
    expires: Option<u64>,
}

impl From<&Cookie> for StoredCookie {
    fn from(cookie: &Cookie) -> Self {
        Self {
            name: cookie.name.clone(),
            value: cookie.value.clone(),
            host: cookie.host.clone(),
            path: cookie.path.clone(),
            secure: cookie.secure,
            same_site: cookie.same_site.clone(),
            expires: cookie
                .expires
                .and_then(|time| time.duration_since(SystemTime::UNIX_EPOCH).ok())
                .map(|duration| duration.as_secs()),
        }
    }
}

impl StoredCookie {
    /// Reparse through response-cookie policy before trusting persisted fields.
    fn restore(self, now: SystemTime) -> Result<Cookie, ProfileError> {
        let scheme = if self.secure { "https" } else { "http" };
        let url =
            Url::parse(&format!("{scheme}://{}/", self.host)).map_err(|_| ProfileError::Corrupt)?;
        if url.host_str() != Some(self.host.as_str()) {
            return Err(ProfileError::Corrupt);
        }
        let same_site = match self.same_site {
            SameSite::Strict => "Strict",
            SameSite::Lax => "Lax",
            SameSite::None => "None",
        };
        let secure = if self.secure { "; Secure" } else { "" };
        let field = format!(
            "{}={}; Path={}{}; SameSite={}",
            self.name, self.value, self.path, secure, same_site
        );
        let mut cookie = parse(&url, &field, now).map_err(|_| ProfileError::Corrupt)?;
        if cookie.name != self.name
            || cookie.value != self.value
            || cookie.host != self.host
            || cookie.path != self.path
            || cookie.secure != self.secure
            || cookie.same_site != self.same_site
        {
            return Err(ProfileError::Corrupt);
        }
        cookie.expires = self
            .expires
            .and_then(|seconds| SystemTime::UNIX_EPOCH.checked_add(Duration::from_secs(seconds)));
        if self.expires.is_some() && cookie.expires.is_none() {
            return Err(ProfileError::Corrupt);
        }
        Ok(cookie)
    }
}
/** Invalid records are rejected whole; secrets are never included in the reason. */
fn parse(url: &Url, field: &str, now: SystemTime) -> Result<Cookie, &'static str> {
    if field.len() > COOKIE_BYTES || !field.is_ascii() || field.chars().any(char::is_control) {
        return Err("A response cookie is invalid or too large.");
    }
    let mut parts = field.split(';');
    let (name, value) = parts
        .next()
        .and_then(|p| p.trim().split_once('='))
        .ok_or("A response cookie is invalid.")?;
    if name.is_empty()
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b))
        || !value.bytes().all(|b| {
            b == 0x21
                || (0x23..=0x2b).contains(&b)
                || (0x2d..=0x3a).contains(&b)
                || (0x3c..=0x5b).contains(&b)
                || (0x5d..=0x7e).contains(&b)
        })
    {
        return Err("A response cookie is invalid.");
    }
    let mut cookie = Cookie {
        name: name.into(),
        value: value.into(),
        host: url.host_str().unwrap_or_default().into(),
        path: default_path(url.path()),
        secure: false,
        same_site: SameSite::Lax,
        expires: None,
    };
    let mut age = None;
    let mut explicit_path = false;
    for part in parts {
        let (key, value) = part.trim().split_once('=').unwrap_or((part.trim(), ""));
        match key.trim().to_ascii_lowercase().as_str() {
            "domain" => {
                return Err("Domain cookies are not supported; only host-only cookies are stored.");
            }
            "secure" => cookie.secure = true,
            "path" if value.starts_with('/') => {
                cookie.path = value.into();
                explicit_path = true;
            }
            "max-age" => {
                age = value.parse::<i64>().ok();
            }
            "expires" => {
                cookie.expires = httpdate::parse_http_date(value).ok();
            }
            "samesite" => {
                cookie.same_site = if value.eq_ignore_ascii_case("strict") {
                    SameSite::Strict
                } else if value.eq_ignore_ascii_case("none") {
                    SameSite::None
                } else {
                    SameSite::Lax
                };
            }
            _ => {}
        }
    }
    if let Some(age) = age {
        cookie.expires = Some(if age <= 0 {
            SystemTime::UNIX_EPOCH
        } else {
            now.checked_add(Duration::from_secs(age as u64))
                .unwrap_or(now + Duration::from_secs(u32::MAX as u64))
        });
    }
    let needs_secure = cookie.same_site == SameSite::None || name.starts_with("__Secure-");
    if (cookie.secure && url.scheme() != "https")
        || (needs_secure && !cookie.secure)
        || (name.starts_with("__Host-") && (!cookie.secure || !explicit_path || cookie.path != "/"))
    {
        return Err("A response cookie violates its security requirements.");
    }
    Ok(cookie)
}
/** Slash boundaries prevent /account from matching /accounting. */
fn path_matches(request: &str, cookie: &str) -> bool {
    request == cookie
        || (request.starts_with(cookie)
            && (cookie.ends_with('/') || request.as_bytes().get(cookie.len()) == Some(&b'/')))
}
/** RFC default paths exclude the final path segment. */
fn default_path(path: &str) -> String {
    path.rfind('/')
        .filter(|i| *i > 0)
        .map(|i| path[..i].into())
        .unwrap_or_else(|| "/".into())
}
