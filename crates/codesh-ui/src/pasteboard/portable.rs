use super::Content;

/// Copied files, then an image, then text: a file manager offers the files'
/// names as text beside them.
pub fn read_content(cx: &gpui::App) -> Result<Content, String> {
    let Some(item) = cx.read_from_clipboard() else {
        return Ok(Content::Text);
    };
    for entry in item.entries() {
        if let gpui::ClipboardEntry::ExternalPaths(files) = entry
            && !files.paths().is_empty()
        {
            return Ok(Content::Files(files.paths().to_vec()));
        }
    }
    for entry in item.entries() {
        if let gpui::ClipboardEntry::Image(image) = entry {
            if image.bytes().len() > 25 * 1024 * 1024 {
                return Err(crate::tr!("Copied image is larger than 25 MiB").to_string());
            }
            return Ok(Content::Image(image.bytes().to_vec()));
        }
    }
    Ok(Content::Text)
}
