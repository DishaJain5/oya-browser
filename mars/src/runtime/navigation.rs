/** Navigation publishes one candidate while history explicitly prevents POST replay. */
use super::{
    page::{Empty, Page},
    resources::{Acquired, uncertain},
    supervisor::Runtime,
};
use crate::{
    constants::{HISTORY_ENTRIES, STYLE_BYTES, STYLE_SHEETS, WEB_TIMEOUT},
    document::{Document, StyleSource},
    network::{Method, WebRequest, WebResponse},
    protocol::{EngineError, EngineResult, parameters},
    v9::Execution,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::time::Instant;

/** History retains the internal destination without exposing protected queries. */
#[derive(Clone)]
pub(super) struct HistoryEntry {
    url: String,
    sensitive: bool,
    replayable: bool,
}
/** Navigation never accepts request headers or policy overrides. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Navigate {
    url: String,
}
/** History is changed only when its corresponding page commits. */
enum History {
    New,
    At(usize),
}

impl Runtime {
    /** Restored destinations use GET and keep query data out of inspection. */
    pub(super) fn navigate_restored_request(&mut self, url: &str) -> EngineResult<()> {
        self.load_request(WebRequest::get(url), true, History::New)
            .map(|_| ())
    }

    /** Address and tool navigation use the same candidate pipeline. */
    pub(super) fn navigate(&mut self, params: Value) -> EngineResult<Value> {
        let input: Navigate = parameters(params)?;
        self.load_request(WebRequest::get(&input.url), false, History::New)
    }
    /** Revisit never changes the cursor on a failed load or replay refusal. */
    fn revisit(&mut self, index: usize) -> EngineResult<Value> {
        let entry = &self.history[index];
        if !entry.replayable {
            return Err(replay());
        }
        self.load_request(
            WebRequest::get(&entry.url),
            entry.sensitive,
            History::At(index),
        )
    }
    /** Back cannot silently turn a POST response into a GET. */
    pub(super) fn back(&mut self, params: Value) -> EngineResult<Value> {
        let _: Empty = parameters(params)?;
        let index = self
            .history_cursor
            .and_then(|i| i.checked_sub(1))
            .ok_or_else(no_history)?;
        self.revisit(index)
    }
    /** Forward follows the same explicit replay policy. */
    pub(super) fn forward(&mut self, params: Value) -> EngineResult<Value> {
        let _: Empty = parameters(params)?;
        let index = self
            .history_cursor
            .map(|i| i + 1)
            .filter(|i| *i < self.history.len())
            .ok_or_else(no_history)?;
        self.revisit(index)
    }
    /** Reload never automatically repeats a server mutation. */
    pub(super) fn reload(&mut self, params: Value) -> EngineResult<Value> {
        let _: Empty = parameters(params)?;
        let index = self.history_cursor.ok_or_else(no_history)?;
        self.revisit(index)
    }
    /** Relative links retain the sensitive-chain projection of their base document. */
    pub(super) fn follow(&mut self, href: &str) -> EngineResult<Value> {
        let page = self.current()?;
        let sensitive = page.sensitive;
        let url = url::Url::parse(&page.url)
            .and_then(|url| url.join(href))
            .map_err(|_| EngineError::invalid("Invalid relative link URL."))?;
        self.load_request(WebRequest::get(url.as_str()), sensitive, History::New)
    }
    /** Form proposals acquire external-effect authority separately from page publication. */
    pub(super) fn navigate_submission(
        &mut self,
        submission: crate::document::Submission,
    ) -> EngineResult<Value> {
        let page = self.current()?;
        let sensitive =
            page.sensitive || (submission.protected && submission.method == Method::Get);
        let mut url = url::Url::parse(&page.url)
            .and_then(|base| base.join(&submission.action))
            .map_err(|_| EngineError::invalid("Invalid form action URL."))?;
        let mut request = WebRequest::get("");
        request.method = submission.method;
        if request.method == Method::Get {
            url.set_query(Some(&submission.encoded));
        } else {
            request.body = submission.encoded;
        }
        url.set_fragment(None);
        request.url = url.into();
        self.load_request(request, sensitive, History::New)
    }
    /** Expensive parsing, VM and layout happen outside the brief final authority lock. */
    fn load_request(
        &mut self,
        request: WebRequest,
        sensitive: bool,
        history: History,
    ) -> EngineResult<Value> {
        self.ensure_storage_ready()?;
        let is_post = request.method == Method::Post;
        let result = self.candidate(request, sensitive).and_then(|page| {
            let rollback = if page.storage_dirty {
                self.write_storage(&page.origin, &page.storage)?
            } else {
                None
            };
            let origin = page.origin.clone();
            let storage = page.storage.clone();
            let published = self.publish(move |runtime| {
                let mut page = page;
                if page.storage_dirty {
                    runtime.commit_storage(&origin, &storage);
                }
                page.storage_dirty = false;
                runtime.generation = page.generation;
                let entry = HistoryEntry {
                    url: page.url.clone(),
                    sensitive: page.sensitive,
                    replayable: page.replayable,
                };
                match history {
                    History::New => {
                        let keep = runtime.history_cursor.map(|i| i + 1).unwrap_or_default();
                        runtime.history.truncate(keep);
                        runtime.history.push(entry);
                        if runtime.history.len() > HISTORY_ENTRIES {
                            runtime.history.remove(0);
                        }
                        runtime.history_cursor = Some(runtime.history.len() - 1);
                    }
                    History::At(index) => {
                        runtime.history[index] = entry;
                        runtime.history_cursor = Some(index);
                    }
                }
                runtime.scroll_y = 0.0;
                runtime.page = Some(page);
                runtime.inspection()
            });
            if published.is_err()
                && let Some(previous) = rollback
            {
                self.rollback_storage(previous);
            }
            published
        });
        result.map_err(|mut error| {
            if is_post
                && !["submission_uncertain", "validation_error"].contains(&error.code.as_str())
            {
                if error.code == "permission_denied" {
                    error.message =
                        "The form was not sent. The destination or current ownership refused it."
                            .into();
                } else if error.code == "unsupported_feature" {
                    error = uncertain();
                }
            }
            if !is_post
                && error.service.is_none()
                && !["permission_denied", "control_denied", "invalid_request"]
                    .contains(&error.code.as_str())
            {
                error.retryable = true;
            }
            error.previous_page_retained = self.page.is_some();
            error
        })
    }
    /** Required CSS is acquired before hidden content can gain native geometry. */
    fn candidate(&mut self, request: WebRequest, sensitive: bool) -> EngineResult<Page> {
        let deadline = Instant::now() + WEB_TIMEOUT;
        let initiator = self
            .page
            .as_ref()
            .and_then(|p| url::Url::parse(&p.url).ok());
        let acquired = if self.web_origins.is_some() {
            self.acquire(request, deadline, initiator, true, true)?
        } else {
            if request.method != Method::Get {
                return Err(EngineError::unsupported(
                    "Headless forms require desktop networking.",
                ));
            }
            let value = self.call("network", "network.fetch", json!({"url":request.url}))?;
            Acquired {
                response: WebResponse {
                    source: value["source"]
                        .as_str()
                        .ok_or_else(|| EngineError::service("network", false))?
                        .into(),
                    image_data: false,
                    url: request.url,
                    status: 200,
                    mime: "text/html".into(),
                    location: None,
                    set_cookies: Vec::new(),
                },
                method: Method::Get,
                diagnostics: Vec::new(),
            }
        };
        let post = acquired.method == Method::Post;
        self.build_page(acquired, deadline, sensitive)
            .map_err(|error| if post { uncertain() } else { error })
    }
    /** Cookie network-state commits precede document parsing, under current authority. */
    fn build_page(
        &mut self,
        acquired: Acquired,
        deadline: Instant,
        sensitive: bool,
    ) -> EngineResult<Page> {
        if acquired.response.mime != "text/html" {
            return Err(EngineError::unsupported(
                "The document requires UTF-8 text/html.",
            ));
        }
        let parsed = self.call(
            "document",
            "document.parse",
            json!({"source":acquired.response.source}),
        )?;
        let mut document: Document = self.returned("document", parsed)?;
        document.diagnostics.extend(acquired.diagnostics);
        let script_resources = std::mem::take(&mut document.scripts);
        let images = self.resolve_images(&mut document, &acquired.response.url, deadline)?;
        let sheets = self.stylesheets(&document, &acquired.response.url, deadline)?;
        let styled = self.call(
            "document",
            "document.styles",
            json!({"document":document,"sheets":sheets,"context":self.render_context}),
        )?;
        document = self.returned("document", styled)?;
        let scripts = self.resolve_scripts(script_resources, &acquired.response.url, deadline)?;
        let origin = canonical_origin(&acquired.response.url)?;
        let base_storage = self.storage_for(&origin)?;
        let generation = self
            .generation
            .checked_add(1)
            .ok_or_else(|| EngineError::limit("page generations", usize::MAX))?;
        let script_identity = format!("{}:document-{generation}", self.active_tab_id);
        let projection = crate::document::script_document(&document, &script_identity)?;
        let value = self.call(
            "v9",
            "v9.document_load",
            json!({"sources":scripts,"document":projection,"storage":base_storage}),
        )?;
        let execution: Execution = self.returned("v9", value)?;
        document = self.apply(&document, &execution.changes)?;
        execution
            .state
            .validate_document(&crate::document::script_document(
                &document,
                &script_identity,
            )?)?;
        let storage = execution.storage;
        let state = execution.state;
        let mut page = Page {
            generation,
            script_identity,
            revision: 1,
            document,
            scripts,
            resolved_stylesheets: sheets,
            url: acquired.response.url,
            sensitive,
            replayable: acquired.method == Method::Get,
            status: acquired.response.status,
            state,
            images,
            display: None,
            layout_generation: 1,
            origin,
            storage_dirty: storage != base_storage,
            storage,
        };
        self.prepare(&mut page)?;
        Ok(page)
    }
    /** Optional static images share governed networking but ordinary failures use alt text. */
    fn resolve_images(
        &mut self,
        document: &mut Document,
        base: &str,
        deadline: Instant,
    ) -> EngineResult<std::collections::BTreeMap<usize, super::images::ImageAsset>> {
        use crate::constants::{IMAGE_BYTES, IMAGE_COUNT};
        let base =
            url::Url::parse(base).map_err(|_| EngineError::invalid("Invalid image base URL."))?;
        let ids = document
            .nodes
            .iter()
            .filter(|node| node.tag == "img")
            .map(|node| node.local_id)
            .collect::<Vec<_>>();
        if ids.len() > IMAGE_COUNT {
            return Err(EngineError::limit("page images", IMAGE_COUNT));
        }
        let mut images = std::collections::BTreeMap::new();
        let mut total = 0usize;
        let mut decoded = 0u64;
        for id in ids {
            let src = document.nodes[id].image_src.clone();
            let loaded = if let Some(src) = src {
                match self.read_image(&base, &src, deadline) {
                    Ok(image) => Some(image),
                    Err(error)
                        if error.service.is_some()
                            || ["permission_denied", "control_denied", "timeout"]
                                .contains(&error.code.as_str()) =>
                    {
                        return Err(error);
                    }
                    Err(_) => None,
                }
            } else {
                None
            };
            match loaded {
                Some((asset, size)) => {
                    total = total.saturating_add(asset.bytes.len());
                    if total > IMAGE_BYTES {
                        return Err(EngineError::limit("page image bytes", IMAGE_BYTES));
                    }
                    decoded = decoded.saturating_add(u64::from(size.0) * u64::from(size.1) * 4);
                    if decoded > crate::constants::IMAGE_DECODED_BYTES as u64 {
                        return Err(EngineError::limit(
                            "decoded page image bytes",
                            crate::constants::IMAGE_DECODED_BYTES,
                        ));
                    }
                    document.nodes[id].image_size = Some(size);
                    images.insert(id, asset);
                }
                None => document.nodes[id].text = document.nodes[id].image_alt.clone(),
            }
            if Instant::now() >= deadline {
                return Err(EngineError::navigation_timeout());
            }
        }
        Ok(images)
    }
    /** A single image uses the existing exact-origin policy, deadline and service authority. */
    fn read_image(
        &mut self,
        base: &url::Url,
        source: &str,
        deadline: Instant,
    ) -> EngineResult<(super::images::ImageAsset, (u32, u32))> {
        let url = base
            .join(source)
            .map_err(|_| EngineError::invalid("Invalid image URL."))?;
        let mut request = WebRequest::get(url.as_str());
        request.image = true;
        let acquired = self.acquire(request, deadline, Some(base.clone()), false, true)?;
        if acquired.response.status != 200 || !acquired.response.image_data {
            return Err(EngineError::unsupported("The image could not be loaded."));
        }
        let bytes = super::images::decode_base64(&acquired.response.source)?;
        let size = super::images::dimensions(&acquired.response.mime, &bytes)?;
        Ok((
            super::images::ImageAsset {
                mime: acquired.response.mime,
                bytes: std::sync::Arc::new(bytes),
            },
            (size.width, size.height),
        ))
    }
    /** Resource destinations inherit no fixture grant beyond the configured exact origins. */
    fn stylesheets(
        &mut self,
        document: &Document,
        base: &str,
        deadline: Instant,
    ) -> EngineResult<Vec<String>> {
        let base = url::Url::parse(base)
            .map_err(|_| EngineError::invalid("Invalid document base URL."))?;
        let mut sheets = Vec::new();
        let mut linked = 0;
        for source in &document.styles {
            let css = match source {
                StyleSource::Inline(css) => css.clone(),
                StyleSource::Linked(href) => {
                    linked += 1;
                    if linked > STYLE_SHEETS {
                        return Err(EngineError::limit("linked stylesheets", STYLE_SHEETS));
                    }
                    let url = base
                        .join(href)
                        .map_err(|_| EngineError::invalid("Invalid stylesheet URL."))?;
                    let acquired = self.acquire(
                        WebRequest::get(url.as_str()),
                        deadline,
                        Some(base.clone()),
                        false,
                        true,
                    )?;
                    if acquired.response.status != 200 || acquired.response.mime != "text/css" {
                        return Err(EngineError::unsupported(
                            "A required stylesheet did not return UTF-8 text/css.",
                        ));
                    }
                    acquired.response.source
                }
            };
            if css.len() > STYLE_BYTES
                || sheets.iter().map(String::len).sum::<usize>() + css.len()
                    > STYLE_BYTES * STYLE_SHEETS
            {
                return Err(EngineError::limit(
                    "required stylesheet bytes",
                    STYLE_BYTES * STYLE_SHEETS,
                ));
            }
            sheets.push(css);
        }
        Ok(sheets)
    }
    /** External scripts use the shared deadline and reject redirects before following them. */
    fn resolve_scripts(
        &mut self,
        mut resources: Vec<crate::document::ScriptResource>,
        base: &str,
        deadline: Instant,
    ) -> EngineResult<Vec<String>> {
        use crate::constants::{SCRIPT_BYTES, SCRIPT_TOTAL_BYTES};
        let base = url::Url::parse(base)
            .map_err(|_| EngineError::invalid("Invalid document base URL."))?;
        let mut total = 0usize;
        for (index, script) in resources.iter_mut().enumerate() {
            let candidate = if let Some(src) = script.src.take() {
                let url = base.join(&src).map_err(|_| {
                    script_error(
                        EngineError::unsupported("A page script has an invalid URL."),
                        index,
                    )
                })?;
                let acquired = self
                    .acquire(
                        WebRequest::get(url.as_str()),
                        deadline,
                        Some(base.clone()),
                        false,
                        false,
                    )
                    .map_err(|error| {
                        let safe = if error.service.is_some()
                            || ["timeout", "permission_denied", "control_denied"]
                                .contains(&error.code.as_str())
                        {
                            error
                        } else {
                            EngineError::unsupported(
                                "Page script could not be loaded; the page was not changed.",
                            )
                        };
                        script_error(safe, index)
                    })?;
                if acquired.response.status != 200 {
                    return Err(script_error(
                        EngineError::unsupported(
                            "Page script could not be loaded; the page was not changed.",
                        ),
                        index,
                    ));
                }
                if !javascript_mime(&acquired.response.mime) {
                    return Err(script_error(
                        EngineError::unsupported(
                            "A page script returned an unsupported content type.",
                        ),
                        index,
                    ));
                }
                acquired.response.source
            } else {
                std::mem::take(&mut script.source)
            };
            if candidate.len() > SCRIPT_BYTES {
                return Err(script_error(
                    EngineError::limit("script bytes", SCRIPT_BYTES),
                    index,
                ));
            }
            total = total.saturating_add(candidate.len());
            if total > SCRIPT_TOTAL_BYTES {
                return Err(EngineError::limit("page script bytes", SCRIPT_TOTAL_BYTES));
            }
            script.source = candidate;
        }
        Ok(resources
            .iter()
            .map(|script| script.source.clone())
            .collect())
    }
}

/** Normalize browser storage ownership to the standards-defined serialized origin. */
fn canonical_origin(value: &str) -> EngineResult<String> {
    let url =
        url::Url::parse(value).map_err(|_| EngineError::invalid("Invalid storage origin."))?;
    Ok(url.origin().ascii_serialization())
}
/** External resource errors identify their source order without exposing its URL. */
fn script_error(mut error: EngineError, index: usize) -> EngineError {
    if let Some(location) = error.location.as_deref_mut() {
        location.script_index = Some(index + 1);
    } else {
        error.location = Some(Box::new(crate::protocol::SourceLocation {
            byte_offset: 0,
            line: 1,
            column: 1,
            script_index: Some(index + 1),
        }));
    }
    error
}
/** Only common UTF-8 classic JavaScript media types are executable in this slice. */
fn javascript_mime(mime: &str) -> bool {
    [
        "text/javascript",
        "application/javascript",
        "text/ecmascript",
        "application/ecmascript",
    ]
    .contains(&mime)
}
/** A prior POST requires an explicit new form action, never a generic Retry. */
fn replay() -> EngineError {
    EngineError::new(
        "resubmission_required",
        "This page came from a form submission and cannot be reloaded automatically.",
        "Return to the form and submit it deliberately; no request was replayed.",
    )
}
/** Missing history remains a recoverable operation error. */
fn no_history() -> EngineError {
    EngineError::new(
        "invalid_target",
        "No destination exists in that history direction.",
        "Navigate to another page first.",
    )
}
