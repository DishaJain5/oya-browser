/** Governed one-hop resources keep effect admission, cookies and deadlines in the coordinator. */
use super::supervisor::Runtime;
use crate::{
    constants::{REDIRECTS, WEB_TIMEOUT},
    network::{Method, WebRequest, WebResponse},
    protocol::{EngineError, EngineResult},
};
use serde_json::json;
use std::time::{Instant, SystemTime};
use url::Url;

/** Response metadata keeps final method for history replay policy. */
pub(super) struct Acquired {
    pub response: WebResponse,
    pub method: Method,
    pub diagnostics: Vec<String>,
}
impl Runtime {
    /** One navigation deadline covers all redirects and required stylesheets. */
    pub(super) fn acquire(
        &mut self,
        mut request: WebRequest,
        deadline: Instant,
        initiator: Option<Url>,
        top_level: bool,
        allow_redirects: bool,
    ) -> EngineResult<Acquired> {
        let grants = self
            .web_origins
            .clone()
            .ok_or_else(|| EngineError::unsupported("Forms require desktop networking."))?;
        let mut admitted_post = false;
        let mut diagnostics = Vec::new();
        let result = (|| {
            for redirects in 0..=REDIRECTS {
                let url = crate::network::policy::web_url(&request.url)?;
                request.cookie = self.cookies.header(
                    &url,
                    initiator.as_ref(),
                    top_level && request.method == Method::Get,
                    SystemTime::now(),
                );
                let remaining = deadline
                    .saturating_duration_since(Instant::now())
                    .min(WEB_TIMEOUT);
                if remaining.is_zero() {
                    return Err(EngineError::navigation_timeout());
                }
                let token=self.call_deadline("network","network.prepare",json!({"request":request,"fixture_origins":grants,"remaining_ms":remaining.as_millis() as u64}), deadline)?;
                self.publish(|runtime| {
                    if request.method == Method::Post
                        && let Some(page) = &mut runtime.page
                    {
                        page.revision = page.revision.saturating_add(1);
                    }
                })?;
                admitted_post |= request.method == Method::Post;
                let value = self.call_deadline(
                    "network",
                    "network.transmit",
                    json!({"token":token["token"]}),
                    deadline,
                )?;
                let response: WebResponse = self.returned("network", value)?;
                let reports = self.commit_cookies(&url, &response.set_cookies)?;
                for report in reports {
                    if diagnostics.len() < crate::constants::DIAGNOSTICS
                        && !diagnostics.contains(&report)
                    {
                        diagnostics.push(report);
                    }
                }
                if ![301, 302, 303, 307, 308].contains(&response.status) {
                    return Ok(Acquired {
                        response,
                        method: request.method,
                        diagnostics,
                    });
                }
                if !allow_redirects {
                    return Err(EngineError::unsupported(
                        "A required script resource redirected.",
                    ));
                }
                if redirects == REDIRECTS {
                    return Err(EngineError::unsupported("Redirect limit exceeded."));
                }
                let next = crate::network::redirect_url(
                    &url,
                    response
                        .location
                        .as_deref()
                        .ok_or_else(|| EngineError::unsupported("Redirect has no Location."))?,
                    &grants,
                )?;
                if request.method == Method::Post {
                    if [301, 302, 303].contains(&response.status) {
                        request.method = Method::Get;
                        request.body.clear();
                    } else if next.origin() != url.origin() {
                        return Err(EngineError::unsupported(
                            "Cross-origin POST-preserving redirects are not supported.",
                        ));
                    }
                }
                request.url = next.into();
                request.cookie.clear();
            }
            Err(EngineError::unsupported("Redirect limit exceeded."))
        })();
        result.map_err(|error| if admitted_post { uncertain() } else { error })
    }
}

impl Runtime {
    /// Durable replacement precedes publication; stale authority rolls storage back.
    fn commit_cookies(&mut self, url: &Url, fields: &[String]) -> EngineResult<Vec<String>> {
        if fields.is_empty() {
            return Ok(Vec::new());
        }
        let previous = self
            .cookies
            .profile_record()
            .map_err(|_| crate::profile::unavailable())?;
        let mut candidate = self.cookies.clone();
        let reports = candidate.receive(url, fields, SystemTime::now());
        let next = candidate
            .profile_record()
            .map_err(|_| crate::profile::unavailable())?;
        if previous == next {
            return Ok(reports);
        }
        if let Some(store) = self.profile_store.as_ref()
            && store.replace(&next).is_err()
        {
            let error = if self.persist_recovery_marker() {
                crate::profile::ProfileError::CommitUncertain
            } else {
                crate::profile::ProfileError::RecoveryUnpersisted
            };
            self.profile_error = Some(error);
            return Err(error.engine_error());
        }
        match self.publish(|runtime| runtime.cookies = candidate) {
            Ok(()) => Ok(reports),
            Err(error) => {
                if self.rollback_cookies(&previous) {
                    return Err(error);
                }
                let error = if self.persist_recovery_marker() {
                    crate::profile::ProfileError::CommitUncertain
                } else {
                    crate::profile::ProfileError::RecoveryUnpersisted
                };
                self.profile_error = Some(error);
                Err(error.engine_error())
            }
        }
    }

    /// A failed rollback replaces stale-authority errors with actionable profile recovery.
    fn rollback_cookies(&self, previous: &[u8]) -> bool {
        self.profile_store
            .as_ref()
            .is_none_or(|store| store.replace(previous).is_ok())
    }

    /// Best-effort marker preserves fail-closed recovery across a later process restart.
    fn persist_recovery_marker(&self) -> bool {
        self.profile_store.as_ref().is_some_and(|store| {
            store
                .replace(&crate::network::CookieJar::recovery_record())
                .is_ok()
        })
    }
}
/** A dispatched POST is never offered a generic retry, including later rendering failure. */
pub(super) fn uncertain() -> EngineError {
    EngineError::new(
        "submission_uncertain",
        "We couldn’t confirm the submission result. The server may have received it. Check the result before submitting again.",
        "Dismiss this message; no submission was replayed.",
    )
}
