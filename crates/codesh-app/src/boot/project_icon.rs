//! Finds an icon among a project's files, such as an app's `.icns`, a
//! `favicon.ico`, or a `logo.png`.

use codesh_client::Client;
use codesh_protocol::{FileEntry, FilesAction, FilesReply, FilesRequest, ProjectId, ServerPath};

/// Directories that commonly hold a project's artwork. The search only
/// descends into these, so a large project costs a handful of listings.
const DIRECTORIES: &[&str] = &[
    ".github",
    "app",
    "art",
    "artwork",
    "assets",
    "brand",
    "branding",
    "build",
    "design",
    "docs",
    "icon",
    "icons",
    "images",
    "img",
    "logo",
    "logos",
    "mac",
    "macos",
    "media",
    "packaging",
    "public",
    "res",
    "resources",
    "src-tauri",
    "static",
    "windows",
];
const MAX_DEPTH: usize = 3;
const MAX_LISTINGS: usize = 16;
/// Candidates read before giving up, in case the best ones don't decode.
const MAX_READS: usize = 3;
const PNG: &[u8] = b"\x89PNG\r\n\x1a\n";

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Rank {
    name: u8,
    format: u8,
    shallow: std::cmp::Reverse<usize>,
}

/// The project's icon, ready to draw, or none if it has no readable one.
pub(super) fn find(client: &Client, project: ProjectId) -> Option<gpui::RenderImage> {
    let list = |path: &ServerPath| match client.files(FilesRequest {
        project,
        action: FilesAction::List(path.clone()),
    }) {
        Ok(FilesReply::Entries(entries)) => entries,
        _ => Vec::new(),
    };
    let read = |path: &ServerPath| match client.files(FilesRequest {
        project,
        action: FilesAction::ReadBytes(path.clone()),
    }) {
        Ok(FilesReply::Bytes(file)) => Some(file.bytes),
        _ => None,
    };
    candidates(list)
        .iter()
        .take(MAX_READS)
        .find_map(|path| decode(&read(path)?))
}

/// Icon files found breadth-first from the project root, best first.
fn candidates(mut list: impl FnMut(&ServerPath) -> Vec<FileEntry>) -> Vec<ServerPath> {
    let mut found = Vec::new();
    let mut queue = std::collections::VecDeque::from([(ServerPath(Vec::new()), 0)]);
    let mut listings = 0;
    while let Some((directory, depth)) = queue.pop_front() {
        if listings == MAX_LISTINGS {
            break;
        }
        listings += 1;
        for entry in list(&directory) {
            if entry.is_ignored {
                continue;
            }
            let Ok(name) = std::str::from_utf8(&entry.name.0) else {
                continue;
            };
            let name = name.to_ascii_lowercase();
            if entry.is_directory {
                if depth + 1 < MAX_DEPTH && searched(&name) {
                    queue.push_back((entry.path, depth + 1));
                }
            } else if let Some(rank) = rank(&name, depth) {
                found.push((rank, entry.path));
            }
        }
    }
    found.sort_by(|a, b| b.0.cmp(&a.0));
    found.into_iter().map(|(_, path)| path).collect()
}

fn searched(name: &str) -> bool {
    DIRECTORIES.contains(&name)
        || name.ends_with(".xcassets")
        || name.ends_with(".appiconset")
        || name.ends_with(".iconset")
}

fn rank(name: &str, depth: usize) -> Option<Rank> {
    let (stem, extension) = name.rsplit_once('.')?;
    let format = match extension {
        "icns" => 3,
        "png" => 2,
        "ico" => 1,
        _ => return None,
    };
    let stem = stem.replace(['-', '_', ' '], "");
    let named = ["appicon", "icon", "logo", "favicon"].contains(&stem.as_str());
    let mentions = ["icon", "logo"].iter().any(|word| stem.contains(word));
    // Any .icns or .ico is an icon; a .png only when it says so.
    if !named && !mentions && format == 2 {
        return None;
    }
    let variant = [
        "beta", "dev", "nightly", "canary", "debug", "preview", "dark",
    ]
    .iter()
    .any(|word| stem.contains(word));
    let name = match (named, mentions, variant) {
        (true, _, _) => 3,
        (false, true, false) => 2,
        (false, false, false) => 1,
        (false, _, true) => 0,
    };
    Some(Rank {
        name,
        format,
        shallow: std::cmp::Reverse(depth),
    })
}

fn decode(bytes: &[u8]) -> Option<gpui::RenderImage> {
    if bytes.starts_with(b"icns") {
        return crate::favicon::decode(icns_png(bytes)?);
    }
    crate::favicon::decode(bytes)
}

/// The smallest PNG in an `.icns` that is still sharp at sidebar size.
/// Modern `.icns` files store their larger sizes as PNG.
fn icns_png(bytes: &[u8]) -> Option<&[u8]> {
    let mut images = Vec::new();
    let mut offset = 8;
    while let Some(header) = bytes.get(offset..offset + 8) {
        let length = u32::from_be_bytes(header[4..8].try_into().ok()?) as usize;
        let data = bytes.get(offset + 8..offset + length.max(8))?;
        if data.starts_with(PNG)
            && let Some(width) = data.get(16..20)
        {
            images.push((u32::from_be_bytes(width.try_into().ok()?), data));
        }
        offset += length.max(8);
    }
    images
        .iter()
        .filter(|(width, _)| *width >= 64)
        .min_by_key(|(width, _)| *width)
        .or_else(|| images.iter().max_by_key(|(width, _)| *width))
        .map(|(_, data)| *data)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str, is_directory: bool, is_ignored: bool) -> FileEntry {
        FileEntry {
            name: ServerPath(path.rsplit('/').next().unwrap_or(path).as_bytes().to_vec()),
            path: ServerPath(path.as_bytes().to_vec()),
            is_directory,
            is_ignored,
        }
    }

    #[test]
    fn prefers_named_app_icons_and_skips_ignored_and_unrelated_folders() {
        let found = candidates(|directory| match directory.0.as_slice() {
            b"" => vec![
                entry("packaging", true, false),
                entry("node_modules", true, true),
                entry("crates", true, false),
                entry("screenshot.png", false, false),
            ],
            b"packaging" => vec![entry("packaging/macos", true, false)],
            b"packaging/macos" => vec![
                entry("packaging/macos/AppIconBeta.png", false, false),
                entry("packaging/macos/AppIcon.png", false, false),
            ],
            b"crates" | b"node_modules" => panic!("searched {directory:?}"),
            _ => Vec::new(),
        });
        assert_eq!(
            found,
            [
                ServerPath(b"packaging/macos/AppIcon.png".to_vec()),
                ServerPath(b"packaging/macos/AppIconBeta.png".to_vec()),
            ]
        );
    }

    #[test]
    fn icns_files_yield_their_embedded_png() {
        let mut png = std::io::Cursor::new(Vec::new());
        image::RgbaImage::from_pixel(128, 128, image::Rgba([0, 0, 255, 255]))
            .write_to(&mut png, image::ImageFormat::Png)
            .expect("png");
        let png = png.into_inner();
        let mut icns = b"icns\0\0\0\0".to_vec();
        icns.extend(b"it32\0\0\0\x0c\0\0\0\0");
        icns.extend(b"ic07");
        icns.extend(u32::try_from(png.len() + 8).expect("length").to_be_bytes());
        icns.extend(&png);
        assert_eq!(icns_png(&icns), Some(png.as_slice()));
        assert!(decode(&icns).is_some());
        assert!(decode(b"icns\0\0\0\x10junk").is_none());
    }
}
