//! Crash-safe replacement of private files.

use std::fs::{self, File, OpenOptions};
use std::io;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
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
    let (temporary, mut file) = create_temporary(path)?;
    let result = write(&mut file)
        .and_then(|()| file.sync_all())
        .and_then(|()| fs::rename(&temporary, path));
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

/// A new owner-only file beside `path`. A temporary left by a crashed process
/// with a reused pid only costs another attempt.
fn create_temporary(path: &Path) -> io::Result<(PathBuf, File)> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::other("path has no file name"))?
        .to_string_lossy();
    let mut attempts = 0;
    loop {
        let temporary = path.with_file_name(format!(
            ".{name}-{}-{}.tmp",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)
        {
            Ok(file) => return Ok((temporary, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists && attempts < 16 => {
                attempts += 1;
            }
            Err(error) => return Err(error),
        }
    }
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
