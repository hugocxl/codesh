//! Crash-safe replacement of private files.

use std::fs::{self, File, OpenOptions};
use std::io;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

/// Replaces `path` with what `write` produces and makes the replacement
/// durable, so a crash leaves either the old file or the new one. The new
/// file is readable only by its owner.
pub fn write_atomic(
    path: &Path,
    write: impl FnOnce(&mut File) -> io::Result<()>,
) -> io::Result<()> {
    replace(path, write)?;
    sync_parent(path)
}

/// Writes a private temporary file beside `path`, syncs it, and renames it
/// over `path`. Once this succeeds the new contents are in place, but the
/// rename is durable only after [`sync_parent`].
pub fn replace(path: &Path, write: impl FnOnce(&mut File) -> io::Result<()>) -> io::Result<()> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::other("path has no file name"))?;
    let temporary = path.with_file_name(format!(
        ".{}-{}-{}.tmp",
        name.to_string_lossy(),
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)?;
        write(&mut file)?;
        file.sync_all()?;
        fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

/// Syncs the directory holding `path`, making a rename into it durable.
pub fn sync_parent(path: &Path) -> io::Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let directory = File::open(parent)?;
    if !directory.metadata()?.is_dir() {
        return Err(io::Error::other("parent is not a directory"));
    }
    directory.sync_all()
}
