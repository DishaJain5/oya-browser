/** Explicit native observation and screenshot APIs run on the OS owning thread. */
use super::*;
use crate::{protocol::parameters, runtime::authority};
use serde::Deserialize;
/** Native OS observations retain strict parameter schemas like the engine protocol. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Empty {}
/** Captures explicitly identify the page and exact painted presentation. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Shot {
    tab_id: String,
    page_id: String,
    expected_revision: u64,
    layout_generation: u64,
}
/** Window size is a startup-granted agent mutation, never a page action. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Resize {
    tab_id: String,
    width: f64,
    height: f64,
    control_epoch: u64,
}

impl Window {
    /** The owning UI thread implements only a small explicit OS command table. */
    pub(super) fn window_request(
        &mut self,
        request: Request,
        permit: Permit,
    ) -> EngineResult<Value> {
        request.validate()?;
        match request.op.as_str() {
            "window.inspect" => {
                let _: Empty = parameters(request.params)?;
                let size = self.chrome.root.bounds().size;
                let tab_id = self.selected_tab_id();
                let owner = if self.broker.permit(Owner::Human, &tab_id).is_ok() {
                    Owner::Human
                } else {
                    Owner::Agent
                };
                let observation = json!({"title":self.window.title().to_string(),"width":size.width,"height":size.height,"page":self.snapshot.as_ref().map(|snapshot|snapshot.inspection.clone()),"error":self.error,"control":owner,"tab_id":tab_id});
                #[cfg(feature = "test-support")]
                let observation = {
                    let mut observation = observation;
                    observation["native_frames"] = self.canvas.native_frames();
                    observation["native_banner"] = self.chrome.banner_observation();
                    observation["render_context"] = self
                        .snapshot
                        .as_ref()
                        .map(|snapshot| snapshot.render_context)
                        .map_or(Value::Null, |context| json!(context));
                    observation["native_validation"] =
                        self.canvas.validation_regions(&self.chrome.root);
                    observation
                };
                Ok(observation)
            }
            "page.screenshot" => {
                let shot: Shot = parameters(request.params)?;
                let inspection = self
                    .snapshot
                    .as_ref()
                    .map(|snapshot| &snapshot.inspection)
                    .ok_or_else(|| EngineError::invalid("No visible page."))?;
                if inspection["tab_id"] != shot.tab_id
                    || inspection["page_id"] != shot.page_id
                    || inspection["revision"] != shot.expected_revision
                    || inspection["layout_generation"] != shot.layout_generation
                {
                    return Err(EngineError::invalid(
                        "Screenshot targets a stale visible page or layout.",
                    ));
                }
                self.capture.save(&self.chrome.root, inspection)
            }
            "window.resize" => {
                let size: Resize = parameters(request.params)?;
                if !size.width.is_finite()
                    || !size.height.is_finite()
                    || !(640.0..=2400.0).contains(&size.width)
                    || !(480.0..=1600.0).contains(&size.height)
                {
                    return Err(EngineError::invalid(
                        "Window size is outside supported bounds.",
                    ));
                }
                let state = authority::lock(&self.broker.authority)?;
                state.validate_tab(permit, crate::runtime::tab_key(&size.tab_id)?)?;
                if size.control_epoch != permit.epoch {
                    return Err(authority::denied());
                }
                drop(state);
                self.window
                    .setContentSize(NSSize::new(size.width, size.height));
                Ok(json!({"width":size.width,"height":size.height,"layout_pending":true}))
            }
            _ => Err(EngineError::unsupported("Unknown native window operation.")),
        }
    }
}
