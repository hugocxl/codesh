use codesh_protocol::{
    ErrorCode, OperationId, ProjectDescriptor, ProjectId, ProjectIntent, ProjectMutation,
    ProjectPatch, ServerPath, SessionStatus, Size,
};
use codesh_server::{Registry, ServerSettings};
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, mpsc};

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
const SIZE: Size = Size { cols: 80, rows: 24 };

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Result<Self> {
        let path = std::env::temp_dir().join(format!("codesh-catalog-{}", ProjectId::new()));
        fs::create_dir(&path)?;
        Ok(Self(path))
    }
    fn registry(&self) -> Result<Registry> {
        let (events, _) = mpsc::channel();
        Ok(Registry::persistent(
            ServerSettings {
                default_shell: Some("/bin/sh".into()),
                ..ServerSettings::default()
            },
            events,
            &self.0.join("sessions"),
        )?)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn project(path: &Path) -> ProjectDescriptor {
    ProjectDescriptor {
        id: ProjectId::new(),
        home: false,
        name: "Test".into(),
        directory: ServerPath(path.as_os_str().as_bytes().into()),
        kind: None,
        parent_id: None,
    }
}
fn intent(mutation: ProjectMutation) -> ProjectIntent {
    ProjectIntent {
        operation: OperationId::new(),
        mutation,
    }
}

#[test]
fn exact_identity_duplicate_paths_private_catalog_and_idempotent_mutations() -> Result {
    let fixture = Fixture::new()?;
    let registry = fixture.registry()?;
    let before = registry.read_catalog(None, None)?;
    let first = project(&fixture.0);
    let second = project(&fixture.0);
    let create = intent(ProjectMutation::Create(first.clone()));
    registry.mutate_project(&create)?;
    registry.mutate_project(&create)?;
    registry.mutate_project(&intent(ProjectMutation::Create(second.clone())))?;
    assert_eq!(registry.read_catalog(None, None)?.projects.len(), 3);
    let rename = intent(ProjectMutation::Patch {
        project: first.id,
        patch: ProjectPatch::Name("Shared".into()),
    });
    registry.mutate_project(&rename)?;
    let revision = registry.catalog_revision();
    registry.mutate_project(&rename)?;
    assert_eq!(registry.catalog_revision(), revision);
    assert_eq!(
        registry
            .read_catalog(None, Some(before.revision))
            .err()
            .ok_or("expected stale revision")?
            .code(),
        ErrorCode::CatalogChanged
    );
    assert_eq!(
        fs::metadata(fixture.0.join("sessions/catalog.json"))?
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    drop(registry);
    let reopened = fixture.registry()?;
    let after = reopened.read_catalog(None, None)?;
    assert_eq!((after.server, after.home), (before.server, before.home));
    assert!(
        after
            .projects
            .iter()
            .any(|project| project.id == first.id && project.name == "Shared")
    );
    assert!(
        reopened
            .mutate_project(&intent(ProjectMutation::Delete(after.home)))
            .is_err()
    );
    Ok(())
}

#[test]
fn retry_never_respawns_ended_or_discarded_sessions_and_cwd_never_owns() -> Result {
    let fixture = Fixture::new()?;
    let registry = fixture.registry()?;
    let owner = project(&fixture.0);
    registry.mutate_project(&intent(ProjectMutation::Create(owner.clone())))?;
    let operation = OperationId::new();
    let first = registry.create_project_session(owner.id, operation, Path::new("/tmp"), SIZE)?;
    assert_eq!(first.project, owner.id);
    assert_eq!(
        registry.create_project_session(owner.id, operation, Path::new("/tmp"), SIZE)?,
        first
    );
    assert_eq!(registry.list().len(), 1);
    registry.end(first.id)?;
    let listed = registry.list_project_sessions(owner.id, None, None)?;
    assert_eq!(listed.sessions[0].status, SessionStatus::Ended);
    assert_eq!(
        registry.create_project_session(owner.id, operation, Path::new("/tmp"), SIZE)?,
        first
    );
    assert!(registry.list().is_empty());
    registry.discard(first.id)?;
    assert!(
        registry
            .list_project_sessions(owner.id, None, None)?
            .sessions
            .is_empty()
    );
    drop(registry);
    let registry = fixture.registry()?;
    assert_eq!(
        registry.create_project_session(owner.id, operation, Path::new("/tmp"), SIZE)?,
        first
    );
    assert!(registry.list().is_empty());
    Ok(())
}

#[test]
fn deletion_races_creation_and_retries_without_touching_files() -> Result {
    let fixture = Fixture::new()?;
    let registry = Arc::new(fixture.registry()?);
    let owner = project(&fixture.0);
    registry.mutate_project(&intent(ProjectMutation::Create(owner.clone())))?;
    fs::write(fixture.0.join("keep"), b"user files")?;
    let first = registry.create_project_session(owner.id, OperationId::new(), &fixture.0, SIZE)?;
    let deleting = Arc::clone(&registry);
    let delete = intent(ProjectMutation::Delete(owner.id));
    let repeated = delete.clone();
    let worker = std::thread::spawn(move || deleting.mutate_project(&delete));
    let created = registry.create_project_session(owner.id, OperationId::new(), &fixture.0, SIZE);
    worker.join().map_err(|_| "delete worker panicked")??;
    if let Ok(created) = created {
        assert!(registry.handle(created.id).is_none());
    }
    assert!(registry.handle(first.id).is_none());
    assert!(registry.read_saved_screen(first.id).is_err());
    registry.mutate_project(&repeated)?;
    assert_eq!(fs::read(fixture.0.join("keep"))?, b"user files");
    drop(registry);
    let reopened = fixture.registry()?;
    assert_eq!(reopened.read_catalog(None, None)?.projects.len(), 1);
    Ok(())
}

#[test]
fn cancellation_before_or_after_create_prevents_orphan_shells() -> Result {
    let fixture = Fixture::new()?;
    let registry = fixture.registry()?;
    let home = registry.home_project();
    let cancelled = OperationId::new();
    registry.cancel_creation(cancelled)?;
    assert!(
        registry
            .create_project_session(home, cancelled, &fixture.0, SIZE)
            .is_err()
    );
    let operation = OperationId::new();
    let session = registry.create_project_session(home, operation, &fixture.0, SIZE)?;
    registry.cancel_creation(operation)?;
    registry.cancel_creation(operation)?;
    assert!(registry.handle(session.id).is_none());
    assert!(registry.read_saved_screen(session.id).is_err());
    drop(registry);
    let registry = fixture.registry()?;
    assert!(
        registry
            .create_project_session(home, operation, &fixture.0, SIZE)
            .is_err()
    );
    assert!(registry.list().is_empty());
    Ok(())
}
