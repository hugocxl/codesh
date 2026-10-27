use objc2::MainThreadMarker;
use objc2_app_kit::NSPasteboard;
use objc2_foundation::{NSString, NSURL};

use crate::tr;

use super::Content;
use std::path::PathBuf;
pub fn read_content(_cx: &gpui::App) -> Result<Content, String> {
    MainThreadMarker::new().ok_or("Clipboard requires the main thread")?;
    let board = NSPasteboard::generalPasteboard();
    let mut paths = Vec::new();
    if let Some(items) = board.pasteboardItems() {
        for item in &items {
            if let Some(value) = item.stringForType(&NSString::from_str("public.file-url"))
                && let Some(url) = NSURL::URLWithString(&value)
                && url.isFileURL()
                && let Some(path) = url.path()
            {
                paths.push(PathBuf::from(path.to_string()));
            }
        }
    }
    if !paths.is_empty() {
        return Ok(Content::Files(paths));
    }
    for identifier in [
        "public.png",
        "public.tiff",
        "public.jpeg",
        "com.compuserve.gif",
        "org.webmproject.webp",
    ] {
        if let Some(data) = board.dataForType(&NSString::from_str(identifier)) {
            if data.len() > 25 * 1024 * 1024 {
                return Err(tr!("Copied image is larger than 25 MiB").to_string());
            }
            return Ok(Content::Image(data.to_vec()));
        }
    }
    Ok(Content::Text)
}
