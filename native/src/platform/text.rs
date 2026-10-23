/** AppKit attributed strings use identical OS font shaping for measurement and display. */
use objc2::{
    AnyThread,
    rc::{Retained, autoreleasepool},
    runtime::AnyObject,
};
use objc2_app_kit::{
    NSAttributedStringNSStringDrawing, NSColor, NSFont, NSFontAttributeName,
    NSForegroundColorAttributeName,
};
use objc2_foundation::{NSAttributedString, NSDictionary, NSString};

/// The document engine selects size/weight; the OS supplies glyph metrics.
pub fn font(size: f64, bold: bool) -> Retained<NSFont> {
    if bold {
        NSFont::boldSystemFontOfSize(size)
    } else {
        NSFont::systemFontOfSize(size)
    }
}

/// Attribute types are fixed here so page-controlled markup cannot inject native objects.
pub fn attributed(text: &str, size: f64, bold: bool) -> Retained<NSAttributedString> {
    colored(text, size, bold, &NSColor::textColor())
}

/// Link color changes presentation only; the exact font metrics remain shared.
pub fn colored(text: &str, size: f64, bold: bool, color: &NSColor) -> Retained<NSAttributedString> {
    let font = font(size, bold);
    let object: &AnyObject = font.as_ref();
    // SAFETY: the only attribute is Apple's font key with an actual retained NSFont value.
    unsafe {
        let color_object: &AnyObject = color.as_ref();
        let attributes = NSDictionary::from_slices(
            &[NSFontAttributeName, NSForegroundColorAttributeName],
            &[object, color_object],
        );
        NSAttributedString::initWithString_attributes(
            NSAttributedString::alloc(),
            &NSString::from_str(text),
            Some(&attributes),
        )
    }
}

/// Exact complete-run measurement is reused by wrapping and native painting.
pub fn measure(text: &str, size: f64, bold: bool) -> f64 {
    autoreleasepool(|_| attributed(text, size, bold).size().width)
}
