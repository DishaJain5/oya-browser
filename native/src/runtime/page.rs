/** Sole committed page snapshots retain generation, VM and native display identity. */
use super::supervisor::Runtime;
use crate::constants::{FRAME_BYTES, ID_BYTES};
use crate::document::{DisplayList, Document, Node, TextChange};
use crate::protocol::{EngineError, EngineResult, Response, encode, parameters};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::collections::BTreeMap;

/** Fixed metadata reserve covers viewport/history fields added to public inspection. */
const INSPECTION_RESERVE: usize = 8 * 1024;

/** A candidate never appears here before all engine stages and wire budgets pass. */
#[derive(Clone)]
pub(super) struct Page {
    /** Changes only on successful navigation. */
    pub generation: u64,
    /** Advances once per committed document mutation. */
    pub revision: u64,
    /** Fully validated owned document. */
    pub document: Document,
    /** Resolved page scripts stay coordinator-owned and never enter document rendering. */
    pub scripts: Vec<String>,
    /** Final URL after policy-checked redirects. */
    pub url: String,
    /** Sensitive GET navigation keeps its actual query inside the runtime. */
    pub sensitive: bool,
    /** Direct POST responses must never be silently replayed by history controls. */
    pub replayable: bool,
    /** HTTP validation and error documents preserve their server status. */
    pub status: u16,
    /** Committed lexical cells and heap share the page generation and revision. */
    pub state: crate::v9::VmState,
    /** Encoded image bytes remain page-owned and never cross the document service. */
    pub images: BTreeMap<usize, super::images::ImageAsset>,
    /** Exact geometry consumed by the visible native window. */
    pub display: Option<DisplayList>,
    /** Presentation generation is independent of document revision. */
    pub layout_generation: u64,
    /** Canonical origin is coordinator-derived and never included in public snapshots. */
    pub origin: String,
    /** Last committed candidate maps stay private to the runtime transaction. */
    pub storage: crate::v9::StorageState,
    /** Dirty candidates publish storage together with page and VM changes. */
    pub storage_dirty: bool,
}

impl Page {
    /** Source HTML IDs never serve as native authority. */
    pub fn id(&self) -> String {
        format!("page-{}", self.generation)
    }
    /** A stale handle cannot alias a later page's local node. */
    pub fn node_id(&self, local: usize) -> String {
        format!("{}:node-{local}", self.id())
    }
    /** Inspection observes one revision without executing page code. */
    pub fn inspect(&self) -> Value {
        let nodes = self
            .document
            .nodes
            .iter()
            .map(|node| self.inspect_node(node))
            .collect::<Vec<_>>();
        json!({"page_id":self.id(),"revision":self.revision,"url":safe_url(&self.url,self.sensitive),"title":self.document.title,"status":self.status,"diagnostics":self.document.diagnostics,"layout_generation":self.layout_generation,"nodes":nodes})
    }
    /** Agent geometry and semantic controls refer to the exact painted display item. */
    fn inspect_node(&self, node: &Node) -> Value {
        let editable = ["input", "textarea"].contains(&node.tag.as_str())
            && !node.disabled
            && !node.hidden()
            && !node.readonly();
        let mut value = json!({"node_id":self.node_id(node.local_id),"html_id":node.html_id,"tag":node.tag,"name":crate::document::label(&self.document,node),"text":node.text,"placeholder":node.placeholder,"href":node.href.as_deref().map(|url|safe_url(url,self.sensitive)),"role":crate::document::role(node),"type":node.input_type(),"protected":node.input_type()=="password","readonly":node.readonly(),"checked":node.checked,"selected":node.selected,"enabled":!node.disabled && !node.hidden() && node.style.interactive,"editable":editable});
        if node.tag == "img" {
            value["alt"] = json!(node.image_alt);
        }
        if node.tag == "th"
            && let Some(scope) = node.attributes.get("scope")
        {
            value["header_scope"] = json!(scope);
        }
        if let Some(cell) = &node.table_cell {
            let headers = cell
                .headers
                .iter()
                .copied()
                .filter(|id| !self.document.nodes[*id].hidden())
                .collect::<Vec<_>>();
            value["table_node_id"] = json!(self.node_id(cell.table));
            value["row_node_id"] = json!(self.node_id(cell.row));
            value["row_index"] = json!(cell.row_index);
            value["column_index"] = json!(cell.column_index);
            value["header_node_ids"] = json!(
                headers
                    .iter()
                    .map(|id| self.node_id(*id))
                    .collect::<Vec<_>>()
            );
            value["header_labels"] = json!(
                headers
                    .iter()
                    .map(|id| crate::document::label(&self.document, &self.document.nodes[*id]))
                    .collect::<Vec<_>>()
            );
        }
        if !node.secret() {
            value["value"] = json!(node.value);
        }
        if let Some(item) = self.display.as_ref().and_then(|display| {
            display
                .items
                .iter()
                .find(|item| item.local_id == node.local_id)
        }) {
            value["bounds"] = json!({"x":item.x,"y":item.y,"width":item.width,"height":item.height,"unit":"logical_points"});
        }
        value
    }
    /** A maximum-length maximally escaped valid ID must still fit after publication. */
    pub(super) fn publishable(&self) -> EngineResult<()> {
        let bytes = encode(&Response::new(
            Some("\\".repeat(ID_BYTES)),
            Ok(self.inspect()),
        ))?;
        if bytes.len() + INSPECTION_RESERVE > FRAME_BYTES {
            return Err(EngineError::limit(
                "inspection wire bytes",
                FRAME_BYTES - INSPECTION_RESERVE,
            ));
        }
        if self.display.is_some()
            && encode(&Response::new(
                Some("\\".repeat(ID_BYTES)),
                Ok(self.point_layout()),
            ))?
            .len()
                + INSPECTION_RESERVE
                > FRAME_BYTES
        {
            return Err(EngineError::limit(
                "layout wire bytes",
                FRAME_BYTES - INSPECTION_RESERVE,
            ));
        }
        Ok(())
    }
    /** Native IDs replace local indices in exactly the display list used by the window. */
    pub(super) fn point_layout(&self) -> Value {
        let Some(display) = self.display.as_ref() else {
            return json!({});
        };
        let items = display
            .items
            .iter()
            .map(|item| {
                let mut value = serde_json::to_value(item).unwrap_or_default();
                value
                    .as_object_mut()
                    .map(|fields| fields.remove("local_id"));
                value["node_id"] = json!(self.node_id(item.local_id));
                value
            })
            .collect::<Vec<_>>();
        json!({"page_id":self.id(),"revision":self.revision,"layout_generation":self.layout_generation,"unit":display.unit,"width":display.width,"height":display.height,"items":items})
    }
}

/** UI consumes immutable snapshots; it never receives a second runtime. */
#[derive(Clone)]
pub struct BrowserSnapshot {
    /// Structured native inspection from the committed page.
    pub inspection: Value,
    /// Native controls read literal values from this same document.
    pub document: Option<Document>,
    /// Encoded images are decoded by AppKit from bounded, validated static resources.
    pub images: BTreeMap<usize, super::images::ImageAsset>,
    /// All painting, placement and hit testing use this display list.
    pub display: Option<DisplayList>,
    /// Service failure disables mutations while retaining visible content.
    pub failed: bool,
    /// Native history controls reflect committed state only.
    pub can_back: bool,
    /// Forward history is unavailable after a fresh navigation branch.
    pub can_forward: bool,
    /// Shared point viewport.
    pub viewport: (f64, f64),
    /// Native document scroll offset.
    pub scroll_y: f64,
    /// Bounded tab labels and selection state drive native tab chrome.
    pub tabs: Value,
}

/** Parameter objects reject unrecognized fields at the edge. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Empty {}

impl Runtime {
    /// The window can continue displaying retained content after an engine failure.
    pub fn snapshot(&self) -> BrowserSnapshot {
        BrowserSnapshot {
            inspection: self.inspection(),
            document: self.page.as_ref().map(|page| page.document.clone()),
            images: self
                .page
                .as_ref()
                .map(|page| page.images.clone())
                .unwrap_or_default(),
            display: self.page.as_ref().and_then(|page| page.display.clone()),
            failed: self.failed,
            can_back: self.history_cursor.is_some_and(|index| index > 0),
            can_forward: self
                .history_cursor
                .is_some_and(|index| index + 1 < self.history.len()),
            viewport: self.viewport,
            scroll_y: self.scroll_y,
            tabs: self.tabs_value().unwrap_or_else(|_| json!({"tabs":[]})),
        }
    }
    /** One read view supplies both public inspection and UI snapshots. */
    pub(super) fn inspection(&self) -> Value {
        let Some(page) = self.page.as_ref() else {
            return json!({"tab_id":self.active_tab_id});
        };
        let mut value = page.inspect();
        value["tab_id"] = json!(self.active_tab_id);
        if page.display.is_some() {
            value["viewport"] = json!({"width":self.viewport.0,"height":self.viewport.1,"scroll_y":self.scroll_y,"unit":"logical_points"});
            if let Some(nodes) = value["nodes"].as_array_mut() {
                for node in nodes {
                    let y = node["bounds"]["y"].as_f64().unwrap_or_default();
                    let height = node["bounds"]["height"].as_f64().unwrap_or_default();
                    node["visible"] = json!(
                        node.get("bounds").is_some()
                            && y + height > self.scroll_y
                            && y < self.scroll_y + self.viewport.1
                    );
                }
            }
        }
        value
    }
    /** Point layout is prepared by the engine before the publication authority check. */
    pub(super) fn prepare(&mut self, page: &mut Page) -> EngineResult<()> {
        if self.web_origins.is_some() {
            let value = self.call(
                "document",
                "document.render",
                json!({"document":page.document,"width":self.viewport.0}),
            )?;
            page.display = Some(self.returned("document", value)?);
        }
        page.publishable()
    }
    /** All source and native text writes share the same document validation. */
    pub(super) fn apply(
        &mut self,
        document: &Document,
        changes: &[TextChange],
    ) -> EngineResult<Document> {
        let value = self.call(
            "document",
            "document.apply",
            json!({"document":document,"changes":changes}),
        )?;
        self.returned("document", value)
    }
    /** Missing pages do not silently create an empty browser context. */
    pub(super) fn current(&self) -> EngineResult<&Page> {
        self.page.as_ref().ok_or_else(|| {
            EngineError::new(
                "no_page",
                "No page is loaded.",
                "Open demo or navigate to a supported URL.",
            )
        })
    }
    /** Structural child-result errors retire the whole session. */
    pub(super) fn returned<T: DeserializeOwned>(
        &mut self,
        name: &str,
        value: Value,
    ) -> EngineResult<T> {
        match serde_json::from_value(value) {
            Ok(value) => Ok(value),
            Err(_) => {
                self.stop();
                Err(EngineError::service(name, false))
            }
        }
    }
    /** Inspection reads native state directly, without injected source. */
    pub(super) fn inspect(&mut self, params: Value) -> EngineResult<Value> {
        let _: Empty = parameters(params)?;
        self.current()?;
        Ok(self.inspection())
    }
}
/** Browser-generated surfaces omit queries/fragments for protected navigation chains. */
pub(super) fn safe_url(value: &str, sensitive: bool) -> String {
    if !sensitive {
        return value.into();
    }
    if let Ok(mut url) = url::Url::parse(value) {
        url.set_query(None);
        url.set_fragment(None);
        url.into()
    } else {
        value.split(['?', '#']).next().unwrap_or_default().into()
    }
}
