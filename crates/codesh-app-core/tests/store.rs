use std::error::Error;
use std::fs::{self, File};
use std::io::Read;
use std::path::PathBuf;

use codesh_app_core::{AppError, AppState, PaneId, ProjectId, WindowBounds, store};
use codesh_protocol::SessionId;
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Result<Self, Box<dyn Error>> {
        let directory = std::env::temp_dir().join(format!("codesh-app-core-{}", ProjectId::new()));
        fs::create_dir(&directory)?;
        Ok(Self(directory))
    }

    fn path(&self) -> PathBuf {
        self.0.join("state.json")
    }

    fn temporary(&self) -> PathBuf {
        self.0.join("state.json.tmp")
    }

    fn write(&self, value: &Value) -> TestResult {
        fs::write(self.path(), serde_json::to_vec(value)?)?;
        Ok(())
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn populated() -> Result<AppState, Box<dyn Error>> {
    let mut state = AppState::bootstrap()?;
    let home = state.home().id;
    let first = state.open_terminal_tab(home)?;
    state.open_terminal_tab(home)?;
    state.select_tab(home, first)?;
    let pane = state.home().tabs[0].panes[0].id;
    state.set_pane_title(pane, "日本語 — editor")?;
    state.set_pane_session(pane, SessionId::new(u64::MAX))?;
    state.set_window_bounds(Some(WindowBounds {
        x: -1400.0,
        y: 40.0,
        width: 1200.0,
        height: 900.0,
    }))?;
    Ok(state)
}

#[test]
fn save_replaces_file_atomically_and_reuses_abandoned_temporary_file() -> TestResult {
    let fixture = Fixture::new()?;
    let mut state = populated()?;
    store::save(fixture.path(), &state)?;
    let old_bytes = fs::read_to_string(fixture.path())?;
    let mut old_file = File::open(fixture.path())?;
    let home = state.home().id;
    state.open_terminal_tab(home)?;
    fs::write(fixture.temporary(), "interrupted write")?;
    store::save(fixture.path(), &state)?;
    let mut still_old = String::new();
    old_file.read_to_string(&mut still_old)?;
    assert_eq!(still_old, old_bytes);
    assert_ne!(fs::read_to_string(fixture.path())?, old_bytes);
    assert_eq!(store::load(fixture.path())?, state);
    assert!(!fixture.temporary().exists());
    Ok(())
}

#[test]
fn corrupt_json_is_an_error_naming_the_path_and_is_not_overwritten() -> TestResult {
    let fixture = Fixture::new()?;
    fs::write(fixture.path(), b"{ broken json")?;
    let error = store::load(fixture.path())
        .err()
        .ok_or("corruption accepted")?;
    assert!(matches!(error, AppError::Json { .. }));
    assert!(
        error
            .to_string()
            .contains(&fixture.path().display().to_string())
    );
    assert!(error.source().is_some());
    assert_eq!(fs::read(fixture.path())?, b"{ broken json");
    Ok(())
}

#[test]
fn malformed_domain_state_is_rejected_with_the_path() -> TestResult {
    let fixture = Fixture::new()?;
    let valid = serde_json::to_value(populated()?)?;
    let mut invalid_states = Vec::new();
    for version in [0, 1, 2, 4] {
        let mut value = valid.clone();
        value["version"] = json!(version);
        invalid_states.push(value);
    }
    for panes in [
        json!([]),
        json!(vec![valid["projects"][0]["tabs"][0]["panes"][0].clone(); 2]),
    ] {
        let mut value = valid.clone();
        value["projects"][0]["tabs"][0]["panes"] = panes;
        invalid_states.push(value);
    }
    for (pointer, replacement) in [
        ("/window/active_pane", json!(PaneId::new())),
        (
            "/projects/0/tabs/0/panes/0/content",
            json!({"type": "unknown"}),
        ),
        ("/projects/0/tabs/0/panes/0/content/session", json!(0)),
        ("/projects/0/id", json!("invalid")),
        ("/projects/0/color", json!("blue")),
        ("/window/bounds/width", json!(-1)),
    ] {
        let mut value = valid.clone();
        *value.pointer_mut(pointer).ok_or("missing fixture field")? = replacement;
        invalid_states.push(value);
    }
    let mut duplicate_tab = valid.clone();
    let tab = valid["projects"][0]["tabs"][0].clone();
    duplicate_tab["projects"][0]["tabs"]
        .as_array_mut()
        .ok_or("tabs")?
        .push(tab);
    invalid_states.push(duplicate_tab);
    for value in invalid_states {
        fixture.write(&value)?;
        let error = store::load(fixture.path())
            .err()
            .ok_or_else(|| format!("invalid state accepted: {value}"))?;
        assert!(
            error
                .to_string()
                .contains(&fixture.path().display().to_string())
        );
        assert_eq!(
            serde_json::from_slice::<Value>(&fs::read(fixture.path())?)?,
            value
        );
    }
    Ok(())
}

#[test]
fn overlapping_save_preserves_the_active_writer_and_previous_state() -> TestResult {
    let fixture = Fixture::new()?;
    let state = populated()?;
    store::save(fixture.path(), &state)?;
    let old_bytes = fs::read(fixture.path())?;
    let active_writer = File::create(fixture.temporary())?;
    active_writer.lock()?;
    fs::write(fixture.temporary(), "active write")?;
    let error = store::save(fixture.path(), &state)
        .err()
        .ok_or("overlapping save was accepted")?;
    assert!(
        matches!(error, AppError::Io { source, .. } if source.kind() == std::io::ErrorKind::WouldBlock)
    );
    assert_eq!(fs::read(fixture.temporary())?, b"active write");
    assert_eq!(fs::read(fixture.path())?, old_bytes);
    drop(active_writer);
    store::save(fixture.path(), &state)?;
    assert_eq!(store::load(fixture.path())?, state);
    Ok(())
}
