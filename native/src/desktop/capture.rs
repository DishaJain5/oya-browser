/** Screenshots capture the actual native content view into an operator-granted directory. */
use crate::protocol::{EngineError, EngineResult};
use objc2_app_kit::{NSBitmapImageFileType, NSView};
use objc2_foundation::NSDictionary;
use serde_json::{Value, json};
use std::path::PathBuf;

/// The agent chooses no path or filename and cannot overwrite an existing capture.
pub(super) struct Capture {
    directory: Option<PathBuf>,
    sequence: u64,
}
impl Capture {
    /** Resolve the startup grant once before accepting agent messages. */
    pub fn new(directory: Option<PathBuf>) -> EngineResult<Self> {
        let directory = directory
            .map(|path| {
                std::fs::create_dir_all(&path)?;
                std::fs::canonicalize(path)
            })
            .transpose()
            .map_err(|_| error("Cannot open the startup capture directory."))?;
        Ok(Self {
            directory,
            sequence: 0,
        })
    }
    /** Display completion precedes a native bitmap read on the main thread. */
    pub fn save(&mut self, view: &NSView, inspection: &Value) -> EngineResult<Value> {
        let directory = self
            .directory
            .as_ref()
            .ok_or_else(|| error("Screenshots require --capture-dir at startup."))?;
        view.displayIfNeeded();
        let bounds = view.bounds();
        let bitmap = view
            .bitmapImageRepForCachingDisplayInRect(bounds)
            .ok_or_else(|| error("Native capture could not allocate a bitmap."))?;
        view.cacheDisplayInRect_toBitmapImageRep(bounds, &bitmap);
        let data = unsafe {
            bitmap.representationUsingType_properties(
                NSBitmapImageFileType::PNG,
                &NSDictionary::new(),
            )
        }
        .ok_or_else(|| error("Native PNG encoding failed."))?;
        self.sequence += 1;
        let path = directory.join(format!(
            "oya-native-{}-{}.png",
            std::process::id(),
            self.sequence
        ));
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|_| error("Cannot create a new native capture."))?;
        file.write_all(&data.to_vec())
            .map_err(|_| error("Cannot write the native capture."))?;
        Ok(
            json!({"path":path,"page_id":inspection["page_id"],"revision":inspection["revision"],"layout_generation":inspection["layout_generation"],"width":bounds.size.width,"height":bounds.size.height,"unit":"logical_points"}),
        )
    }
}
/** Capture errors never silently switch to a different screenshot provider. */
fn error(message: &str) -> EngineError {
    EngineError::new(
        "capture_unavailable",
        message,
        "Launch desktop with an accessible --capture-dir directory.",
    )
}
