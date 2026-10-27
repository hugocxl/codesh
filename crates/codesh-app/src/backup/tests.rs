#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;
use std::io::Write;

use codesh_app_core::{
    AppState, ServerId,
    settings::{Settings, TerminalSettings},
};

use super::*;

fn profile() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    Settings::load(&directory.path().join("settings.toml")).unwrap();
    fs::write(directory.path().join("terminal.toml"), "font_size = 17\n").unwrap();
    directory
}

#[test]
fn backups_with_extension_panes_restore_native_documents() {
    let target = profile();
    let backup = target.path().join("old.codesh");
    let files: Files = BTreeMap::from([
        (
            "settings.toml".into(),
            codesh_app_core::backup::settings_source(&Settings::default())
                .unwrap()
                .into_bytes(),
        ),
        (
            "desktop-state.json".into(),
            include_bytes!(
                "../../../codesh-app-core/tests/fixtures/desktop-state-extension-panes.json"
            )
            .to_vec(),
        ),
    ]);
    let mut zip = zip::ZipWriter::new(fs::File::create(&backup).unwrap());
    let options = zip::write::SimpleFileOptions::default();
    zip.start_file("manifest.json", options).unwrap();
    zip.write_all(&serde_json::to_vec(&serde_json::json!({"schemaVersion":2, "format":"codesh.configuration", "complete":true, "files":files.keys().collect::<Vec<_>>()})).unwrap()).unwrap();
    for (name, bytes) in files {
        zip.start_file(name, options).unwrap();
        zip.write_all(&bytes).unwrap();
    }
    zip.finish().unwrap();
    let import = prepare(target.path(), &backup).unwrap();
    stage(target.path(), &import).unwrap();
    assert!(apply_pending(target.path()).unwrap().is_none());
    assert!(!target.path().join("extension-enabled.json").exists());
    assert!(!target.path().join("extension-settings").exists());
    let state = codesh_app_core::store::load(target.path().join("desktop-state.json")).unwrap();
    let panes: Vec<_> = state
        .home()
        .tabs
        .iter()
        .flat_map(|tab| &tab.panes)
        .map(|pane| &pane.content)
        .collect();
    assert!(panes.iter().any(
        |pane| matches!(pane, codesh_app_core::PaneContent::Editor { path } if path == "src/main.rs")
    ));
    assert!(panes.iter().any(|pane| matches!(pane, codesh_app_core::PaneContent::Diff { path, staged: false } if path == "README.md")));
    assert_eq!(panes.len(), 3);
    let saved = fs::read_to_string(target.path().join("desktop-state.json")).unwrap();
    assert!(!saved.contains("webview"));
}

#[test]
fn backup_round_trip_is_portable_and_restore_waits_for_restart() {
    let source = profile();
    let target = profile();
    let project = tempfile::tempdir().unwrap();
    let mut state = AppState::bootstrap().unwrap();
    let id = state
        .add_project(ServerId::local(), project.path().into())
        .unwrap();
    state.open_terminal_tab(id).unwrap();
    fs::create_dir(source.path().join("themes")).unwrap();
    fs::write(source.path().join("themes/custom"), "background = 111111").unwrap();
    fs::write(source.path().join("identity.pem"), "private identity").unwrap();
    fs::write(
        source.path().join("server.toml"),
        "history_budget_bytes = 1048576\nshell_integration = false\n",
    )
    .unwrap();
    let archive = source.path().join("test.codesh");
    export(source.path(), &archive, &state).unwrap();
    let (files, restore) = archive::read(&archive).unwrap();
    assert_eq!(restore, Restore::Complete);
    assert!(!files.contains_key("identity.pem"));
    let saved: AppState = serde_json::from_slice(&files["desktop-state.json"]).unwrap();
    assert!(saved.project_intents(ServerId::local()).is_empty());
    fs::write(target.path().join("terminal.toml"), "font_size = 12\n").unwrap();
    let import = prepare(target.path(), &archive).unwrap();
    stage(target.path(), &import).unwrap();
    assert_eq!(
        fs::read_to_string(target.path().join("terminal.toml")).unwrap(),
        "font_size = 12\n"
    );
    apply_pending(target.path()).unwrap();
    assert_eq!(
        TerminalSettings::load_native(&target.path().join("terminal.toml"))
            .unwrap()
            .font_size
            .to_bits(),
        17.0_f32.to_bits()
    );
    assert!(target.path().join("themes/custom").exists());
    let restored = codesh_app_core::store::load(target.path().join("desktop-state.json")).unwrap();
    assert!(restored.project(id).is_some());
    let creates = restored
        .project_intents(ServerId::local())
        .iter()
        .filter(|intent| matches!(intent.mutation, codesh_protocol::ProjectMutation::Create(_)))
        .count();
    assert_eq!(creates, 1);
    let recovery = fs::read_dir(target.path().join("Backups"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert_eq!(
        fs::read_to_string(recovery.join("terminal.toml")).unwrap(),
        "font_size = 12\n"
    );
    assert!(!target.path().join(PENDING).exists());
    apply_pending(target.path()).unwrap();
}

#[test]
fn invalid_archive_and_cancel_leave_current_configuration_untouched() {
    let directory = profile();
    let archive = directory.path().join("test.codesh");
    export(directory.path(), &archive, &AppState::bootstrap().unwrap()).unwrap();
    stage(
        directory.path(),
        &prepare(directory.path(), &archive).unwrap(),
    )
    .unwrap();
    cancel_pending(directory.path()).unwrap();
    apply_pending(directory.path()).unwrap();
    assert!(!directory.path().join("Backups").exists());
    let (mut files, _) = archive::read(&archive).unwrap();
    files.insert(
        "settings.toml".into(),
        b"[window]\ndefault_size = [0, 0]".to_vec(),
    );
    archive::write(&archive, &files, Restore::Complete).unwrap();
    let before = fs::read(directory.path().join("settings.toml")).unwrap();
    assert!(prepare(directory.path(), &archive).is_err());
    assert_eq!(
        fs::read(directory.path().join("settings.toml")).unwrap(),
        before
    );
}

#[test]
fn traversal_symlinks_missing_manifest_and_unknown_versions_are_rejected() {
    let directory = profile();
    let archive = directory.path().join("unsafe.codesh");
    for name in [
        "../escape",
        "/absolute",
        "themes/../../escape",
        "themes\\escape",
    ] {
        unsafe_archive(&archive, name);
        assert!(prepare(directory.path(), &archive).is_err(), "{name}");
    }
    let mut zip = zip::ZipWriter::new(fs::File::create(&archive).unwrap());
    zip.start_file("manifest.json", zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(
        br#"{"schemaVersion":2,"format":"codesh.configuration","files":["settings.toml"]}"#,
    )
    .unwrap();
    zip.add_symlink(
        "settings.toml",
        "/etc/passwd",
        zip::write::SimpleFileOptions::default(),
    )
    .unwrap();
    zip.finish().unwrap();
    assert!(prepare(directory.path(), &archive).is_err());
    let mut zip = zip::ZipWriter::new(fs::File::create(&archive).unwrap());
    zip.start_file("manifest.json", zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(br#"{"schemaVersion":99,"files":[]}"#)
        .unwrap();
    zip.finish().unwrap();
    assert!(prepare(directory.path(), &archive).is_err());
    fs::write(&archive, b"not a ZIP").unwrap();
    assert!(prepare(directory.path(), &archive).is_err());
    std::os::unix::fs::symlink("/etc/passwd", directory.path().join("themes")).unwrap();
    assert!(export(directory.path(), &archive, &AppState::bootstrap().unwrap()).is_err());
}

#[test]
fn interrupted_restore_recovers_original_files_before_loading_the_profile() {
    let directory = profile();
    let recovery = directory.path().join("Backups/pre-import-test");
    fs::create_dir_all(&recovery).unwrap();
    fs::write(recovery.join("terminal.toml"), "font_size = 17\n").unwrap();
    fs::write(recovery.join("roots.json"), r#"["terminal.toml","themes"]"#).unwrap();
    fs::write(
        directory.path().join("restore-in-progress.json"),
        serde_json::to_vec(&recovery).unwrap(),
    )
    .unwrap();
    fs::write(directory.path().join("terminal.toml"), "font_size = 23\n").unwrap();
    fs::create_dir(directory.path().join("themes")).unwrap();
    fs::write(directory.path().join("themes/partial"), "incomplete").unwrap();
    fs::write(directory.path().join(PENDING), "invalid backup").unwrap();
    let error = apply_pending(directory.path()).unwrap().unwrap();
    assert!(error.contains("Could not restore backup"));
    assert_eq!(
        fs::read_to_string(directory.path().join("terminal.toml")).unwrap(),
        "font_size = 17\n"
    );
    assert!(!directory.path().join("themes").exists());
    assert!(!directory.path().join("restore-in-progress.json").exists());
    assert!(recovery.join("terminal.toml").exists());
    cancel_pending(directory.path()).unwrap();
    assert!(apply_pending(directory.path()).unwrap().is_none());
}

#[test]
fn failed_rollback_still_prevents_loading_a_partially_restored_profile() {
    let directory = profile();
    let recovery = directory.path().join("Backups/pre-import-test");
    fs::create_dir_all(&recovery).unwrap();
    fs::write(recovery.join("roots.json"), "invalid journal").unwrap();
    let marker = directory.path().join("restore-in-progress.json");
    fs::write(&marker, serde_json::to_vec(&recovery).unwrap()).unwrap();
    let before = fs::read(directory.path().join("terminal.toml")).unwrap();
    assert!(apply_pending(directory.path()).is_err());
    assert!(marker.exists());
    assert_eq!(
        fs::read(directory.path().join("terminal.toml")).unwrap(),
        before
    );
}

#[test]
fn full_restore_resets_omitted_configuration_and_partial_import_keeps_it() {
    let source = profile();
    let target = profile();
    fs::write(
        target.path().join("server.toml"),
        "shell_integration = false\n",
    )
    .unwrap();
    fs::create_dir(target.path().join("themes")).unwrap();
    fs::write(target.path().join("themes/custom"), "background = 111111").unwrap();
    let partial = source.path().join("partial.codesh");
    archive::write(
        &partial,
        &Files::from([(
            "settings.toml".into(),
            b"[appearance]\ndark_theme = \"Dracula\"\n".to_vec(),
        )]),
        Restore::Partial,
    )
    .unwrap();
    stage(target.path(), &prepare(target.path(), &partial).unwrap()).unwrap();
    apply_pending(target.path()).unwrap();
    assert!(target.path().join("server.toml").exists());
    assert!(target.path().join("themes/custom").exists());
    let archive = source.path().join("test.codesh");
    export(source.path(), &archive, &AppState::bootstrap().unwrap()).unwrap();
    stage(target.path(), &prepare(target.path(), &archive).unwrap()).unwrap();
    apply_pending(target.path()).unwrap();
    for name in ["server.toml", "themes"] {
        assert!(!target.path().join(name).exists(), "{name}");
    }
    assert!(
        fs::read_dir(target.path().join("Backups"))
            .unwrap()
            .any(|entry| entry.unwrap().path().join("themes/custom").exists())
    );
}

#[test]
fn mobile_preferences_exclude_credentials_and_apply_through_the_server() {
    let source = profile();
    let target = profile();
    let raw = br#"{"enabled":true,"port":7420,"certificate":"secret-certificate","private_key":"secret-key","devices":["secret-device"]}"#;
    fs::write(source.path().join("remote.json"), raw).unwrap();
    fs::write(target.path().join("remote.json"), raw).unwrap();
    let archive = source.path().join("test.codesh");
    export(source.path(), &archive, &AppState::bootstrap().unwrap()).unwrap();
    let import = prepare(target.path(), &archive).unwrap();
    assert!(!import.files.contains_key("remote.json"));
    let mobile = &import.files[mobile::FILE];
    assert!(!String::from_utf8_lossy(mobile).contains("secret"));
    stage(target.path(), &import).unwrap();
    apply_pending(target.path()).unwrap();
    assert!(apply_mobile_settings(target.path(), |_| Err("Server unavailable".into())).is_err());
    assert!(target.path().join(mobile::FILE).exists());
    let mut applied = None;
    apply_mobile_settings(target.path(), |settings| {
        applied = Some(settings);
        Ok(())
    })
    .unwrap();
    assert_eq!(
        applied,
        Some(codesh_protocol::RemoteAccessSettings {
            enabled: true,
            port: 7420
        })
    );
    assert!(!target.path().join(mobile::FILE).exists());
    assert_eq!(fs::read(target.path().join("remote.json")).unwrap(), raw);
}

fn unsafe_archive(path: &Path, name: &str) {
    let mut zip = zip::ZipWriter::new(fs::File::create(path).unwrap());
    let options = zip::write::SimpleFileOptions::default();
    zip.start_file("manifest.json", options).unwrap();
    zip.write_all(
        &serde_json::to_vec(&serde_json::json!({
            "schemaVersion": 2, "format": "codesh.configuration", "files": [name]
        }))
        .unwrap(),
    )
    .unwrap();
    zip.start_file(name, options).unwrap();
    zip.write_all(b"unsafe").unwrap();
    zip.finish().unwrap();
}
