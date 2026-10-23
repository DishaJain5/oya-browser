/** One canonical point display and viewport feed both native painting and agent results. */
use super::{page::Empty, supervisor::Runtime};
use crate::constants::LAYOUT_UNIT;
use crate::document::Row;
use crate::protocol::{EngineError, EngineResult, parameters};
use serde::Deserialize;
use serde_json::{Value, json};

/** Legacy abstract layout remains a stable headless API. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Geometry {
    /** Declared units. */
    unit: String,
    /** Source-order rows. */
    rows: Vec<Row>,
}
/** Native scrolling cannot accidentally target a replacement page. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Scroll {
    /** Current page. */
    page_id: String,
    /** Observed revision. */
    expected_revision: u64,
    /** Desired top-left document offset. */
    y: f64,
}

impl Runtime {
    /** Native mode returns the same point boxes and lines consumed by the window. */
    pub(super) fn layout(&mut self, params: Value) -> EngineResult<Value> {
        let _: Empty = parameters(params)?;
        let page = self.current()?.clone();
        if page.display.is_some() {
            let mut result = page.point_layout();
            result["viewport"] =
                json!({"width":self.viewport.0,"height":self.viewport.1,"scroll_y":self.scroll_y});
            return Ok(result);
        }
        let value = self.call(
            "document",
            "document.layout",
            json!({"document":page.document}),
        )?;
        let geometry: Geometry = self.returned("document", value)?;
        if geometry.unit != LAYOUT_UNIT || geometry.rows.len() != page.document.nodes.len() {
            self.stop();
            return Err(EngineError::service("document", false));
        }
        let rows=geometry.rows.into_iter().map(|row|json!({"node_id":page.node_id(row.local_id),"x":row.x,"y":row.y,"width":row.width,"height":row.height})).collect::<Vec<_>>();
        Ok(json!({"page_id":page.id(),"revision":page.revision,"unit":LAYOUT_UNIT,"rows":rows}))
    }
    /// Native window resize updates presentation without changing document revision.
    pub fn set_viewport(&mut self, width: f64, height: f64) -> EngineResult<()> {
        if !width.is_finite()
            || !height.is_finite()
            || !(280.0..=4096.0).contains(&width)
            || !(120.0..=4096.0).contains(&height)
        {
            return Err(EngineError::invalid("Invalid native viewport bounds."));
        }
        if self.viewport == (width, height) {
            return Ok(());
        }
        if let Some(mut page) = self.page.clone() {
            let value = self.call(
                "document",
                "document.render",
                json!({"document":page.document,"width":width}),
            )?;
            page.display = Some(self.returned("document", value)?);
            page.layout_generation = page.layout_generation.saturating_add(1);
            page.publishable()?;
            self.page = Some(page);
        }
        self.viewport = (width, height);
        self.clamp_scroll();
        Ok(())
    }
    /** Agent scrolling is owner-checked and revision-scoped like other native writes. */
    pub(super) fn scroll(&mut self, params: Value) -> EngineResult<Value> {
        let input: Scroll = parameters(params)?;
        let page = self.current()?;
        if !input.y.is_finite()
            || page.id() != input.page_id
            || page.revision != input.expected_revision
        {
            return Err(EngineError::new(
                "invalid_target",
                "Scroll targets a stale page/revision or invalid offset.",
                "Inspect the current page and viewport.",
            ));
        }
        self.publish(move |runtime| {
            runtime.scroll_y = input.y;
            runtime.clamp_scroll();
            runtime.inspection()
        })
    }
    /// Native observation scrolling remains available while an agent owns document mutation.
    pub fn observe_scroll(&mut self, y: f64) {
        if y.is_finite() {
            self.scroll_y = y;
            self.clamp_scroll();
        }
    }
    /** Offsets always remain within the exact current display height. */
    pub(super) fn clamp_scroll(&mut self) {
        let height = self
            .page
            .as_ref()
            .and_then(|page| page.display.as_ref())
            .map(|display| display.height)
            .unwrap_or_default();
        self.scroll_y = self
            .scroll_y
            .clamp(0.0, (height - self.viewport.1).max(0.0));
    }
}
