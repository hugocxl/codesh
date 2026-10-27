use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::{AppError, AppState, PaneContent};

pub fn default_path() -> Result<PathBuf, AppError> {
    codesh_core::dirs::codesh_dir()
        .map(|directory| directory.join("desktop-state.json"))
        .map_err(|source| AppError::Io {
            path: PathBuf::from("Codesh state directory"),
            source,
        })
}

pub fn load(path: impl AsRef<Path>) -> Result<AppState, AppError> {
    let path = path.as_ref();
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return AppState::bootstrap();
        }
        Err(source) => {
            return Err(AppError::Io {
                path: path.into(),
                source,
            });
        }
    };
    decode(&bytes).map_err(|source| AppError::Json {
        path: path.into(),
        source,
    })
}

/// Decodes saved desktop state, including retired pane formats in backups.
pub fn decode(bytes: &[u8]) -> serde_json::Result<AppState> {
    let mut value: Value = serde_json::from_slice(bytes)?;
    migrate_extension_panes(&mut value);
    serde_json::from_value(value)
}

/// Rewrites panes of the retired bundled `files` and `git` extensions as
/// native editor and diff panes. Every other extension webview becomes a
/// legacy settings pane, which loading closes and its split re-equalizes.
/// Works on raw JSON so it outlives the webview pane type.
fn migrate_extension_panes(state: &mut Value) {
    let Some(projects) = state.get_mut("projects").and_then(Value::as_array_mut) else {
        return;
    };
    let panes = projects
        .iter_mut()
        .filter_map(|project| project.get_mut("tabs").and_then(Value::as_array_mut))
        .flatten()
        .filter_map(|tab| tab.get_mut("panes").and_then(Value::as_array_mut))
        .flatten();
    for pane in panes {
        let Some(content) = pane.get("content") else {
            continue;
        };
        if content.get("type").and_then(Value::as_str) != Some("webview") {
            continue;
        }
        let native = native_content(content);
        let title = native.as_ref().and_then(PaneContent::native_title);
        let content = native
            .and_then(|native| serde_json::to_value(native).ok())
            .unwrap_or_else(|| serde_json::json!({ "type": "settings" }));
        pane["content"] = content;
        if let Some(title) = title {
            pane["title"] = Value::String(title);
        }
    }
}

fn native_content(webview: &Value) -> Option<PaneContent> {
    let text = |key: &str| webview["data"][key].as_str();
    match (webview["owner"].as_str()?, webview["kind"].as_str()?) {
        ("files", "code-editor") => Some(PaneContent::Editor {
            path: crate::pane::relative_path(text("filePath")?).ok()?,
        }),
        ("git", "diff-viewer") => Some(PaneContent::Diff {
            path: crate::pane::relative_path(
                text("focusPath")
                    .or_else(|| text("filePath"))
                    .or_else(|| text("path"))?,
            )
            .ok()?,
            staged: webview["data"]["staged"].as_bool().unwrap_or(false),
        }),
        _ => None,
    }
}

pub fn save(path: impl AsRef<Path>, state: &AppState) -> Result<(), AppError> {
    let path = path.as_ref();
    let mut bytes = serde_json::to_vec_pretty(state).map_err(|source| AppError::Json {
        path: path.into(),
        source,
    })?;
    bytes.push(b'\n');
    let mut temporary = path.as_os_str().to_owned();
    temporary.push(".tmp");
    let temporary = PathBuf::from(temporary);
    let mut file = open_temporary(&temporary).map_err(|source| AppError::Io {
        path: temporary.clone(),
        source,
    })?;
    let result = file
        .set_len(0)
        .and_then(|()| file.write_all(&bytes))
        .and_then(|()| file.sync_all())
        .and_then(|()| fs::rename(&temporary, path))
        .and_then(|()| File::open(path.parent().unwrap_or_else(|| Path::new(".")))?.sync_all());
    if let Err(source) = result {
        let _ = fs::remove_file(&temporary);
        return Err(AppError::Io {
            path: path.into(),
            source,
        });
    }
    Ok(())
}

fn open_temporary(path: &Path) -> io::Result<File> {
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(path)?;
    file.try_lock()?;
    let opened = file.metadata()?;
    let current = fs::metadata(path)?;
    if (opened.dev(), opened.ino()) != (current.dev(), current.ino()) {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "temporary state file was replaced by another writer",
        ));
    }
    Ok(file)
}
