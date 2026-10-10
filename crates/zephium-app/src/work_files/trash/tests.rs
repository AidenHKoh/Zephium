use super::*;
use std::fs;

struct Fixture {
    _scratch: tempfile::TempDir,
    storage: PathBuf,
    files: PathBuf,
    grant: WorkFileGrant,
}
impl Fixture {
    fn new() -> Self {
        let scratch = tempfile::Builder::new()
            .prefix("work-recovery-fixture-")
            .tempdir_in("/private/tmp")
            .unwrap();
        let root = scratch.path().canonicalize().unwrap();
        let storage = root.join("application");
        let files = root.join("files");
        fs::create_dir(&storage).unwrap();
        fs::create_dir(&files).unwrap();
        let grant = WorkFileGrant {
            roots: vec![files.clone()],
            written: vec![files.clone()],
        };
        Self {
            _scratch: scratch,
            storage,
            files,
            grant,
        }
    }
    fn receipt(&self, text: &str) -> journal::Receipt {
        journal::Receipt {
            version: 1,
            id: recovery_id(),
            path: self.files.join("note.txt"),
            digest: digest(text.as_bytes()),
            bytes: text.len(),
            step: "inert-step".into(),
        }
    }
}
#[test]
fn discovery_does_not_create_storage_and_filters_profile_and_grants() {
    let fixture = Fixture::new();
    assert!(recoveries_at(&fixture.storage, &fixture.grant, "profile-a")
        .unwrap()
        .is_empty());
    assert!(!fixture.storage.join(".zephium-work-recovery").exists());
    let journal = journal::Journal::open(&fixture.storage, "profile-a", true)
        .unwrap()
        .unwrap();
    let record = journal.prepare(fixture.receipt("saved"), b"saved").unwrap();
    let id = record.receipt.id.clone();
    drop(record);
    drop(journal);
    assert!(recoveries_at(&fixture.storage, &fixture.grant, "profile-b")
        .unwrap()
        .is_empty());
    assert!(
        recoveries_at(&fixture.storage, &WorkFileGrant::default(), "profile-a")
            .unwrap()
            .is_empty()
    );
    let rows = recoveries_at(&fixture.storage, &fixture.grant, "profile-a").unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, id);
    assert!(!rows[0].confirmed);
}
#[test]
fn durable_prepared_and_acknowledged_receipts_survive_reopen_without_replay() {
    let fixture = Fixture::new();
    fs::write(fixture.files.join("note.txt"), "source remains").unwrap();
    let journal = journal::Journal::open(&fixture.storage, "profile", true)
        .unwrap()
        .unwrap();
    let record = journal
        .prepare(fixture.receipt("saved copy"), b"saved copy")
        .unwrap();
    let id = record.receipt.id.clone();
    drop(record);
    drop(journal);
    let journal = journal::Journal::open(&fixture.storage, "profile", false)
        .unwrap()
        .unwrap();
    let record = journal.records().unwrap().pop().unwrap();
    assert_eq!(record.receipt.id, id);
    assert_eq!(record.saved().unwrap(), "saved copy");
    assert!(!record.marked("trashed").unwrap());
    assert_eq!(
        fs::read_to_string(fixture.files.join("note.txt")).unwrap(),
        "source remains"
    );
    record.mark("trashed").unwrap();
    drop(record);
    drop(journal);
    let rows = recoveries_at(&fixture.storage, &fixture.grant, "profile").unwrap();
    assert_eq!(rows.len(), 1);
    assert!(rows[0].confirmed);
    let journal = journal::Journal::open(&fixture.storage, "profile", false)
        .unwrap()
        .unwrap();
    journal
        .records()
        .unwrap()
        .pop()
        .unwrap()
        .mark("restored")
        .unwrap();
    drop(journal);
    assert!(recoveries_at(&fixture.storage, &fixture.grant, "profile")
        .unwrap()
        .is_empty());
}
#[test]
fn journal_capacity_is_bounded_and_never_discards_pending_recovery() {
    let fixture = Fixture::new();
    let journal = journal::Journal::open(&fixture.storage, "profile", true)
        .unwrap()
        .unwrap();
    for _ in 0..journal::MAX_RECORDS {
        journal.prepare(fixture.receipt("saved"), b"saved").unwrap();
    }
    assert!(matches!(
        journal.prepare(fixture.receipt("saved"), b"saved"),
        Err(Failure::Capacity)
    ));
    assert_eq!(journal.records().unwrap().len(), journal::MAX_RECORDS);
}
#[test]
fn recovery_storage_and_aliases_are_protected_from_computer_reads() {
    let fixture = Fixture::new();
    let journal = journal::Journal::open(&fixture.storage, "profile", true)
        .unwrap()
        .unwrap();
    let record = journal
        .prepare(fixture.receipt("private saved copy"), b"private saved copy")
        .unwrap();
    let storage = fixture.storage.join(".zephium-work-recovery");
    let grant = WorkFileGrant {
        roots: vec![fixture.storage.clone()],
        written: vec![fixture.storage.clone()],
    };
    assert_eq!(
        grant.resolve(storage.to_str().unwrap(), false),
        Err(WorkFileError::Denied)
    );
    for component in [
        ".ZEPHIUM-WORK-RECOVERY",
        ".zephium-work-recovery",
        ".zephium-work-recovery.",
    ] {
        assert!(super::super::protected_path(
            &fixture.storage.join(component)
        ));
    }
    assert_eq!(record.saved().unwrap(), "private saved copy");
}
#[cfg(target_os = "macos")]
#[test]
fn stale_source_identity_and_parent_swap_never_reach_trash() {
    let fixture = Fixture::new();
    let source = fixture.files.join("note.txt");
    fs::write(&source, "approved").unwrap();
    let prepared = PreparedTrash::prepare_checked(
        &fixture.grant,
        source.to_str().unwrap(),
        &digest(b"approved"),
    )
    .unwrap();
    fs::rename(&source, fixture.files.join("original.txt")).unwrap();
    fs::write(&source, "approved").unwrap();
    assert_eq!(
        prepared
            .apply_at(&fixture.storage, "profile", "step")
            .unwrap_err(),
        Failure::Changed
    );
    assert_eq!(fs::read_to_string(&source).unwrap(), "approved");
    let prepared = PreparedTrash::prepare_checked(
        &fixture.grant,
        source.to_str().unwrap(),
        &digest(b"approved"),
    )
    .unwrap();
    let moved = fixture.files.with_file_name("moved");
    fs::rename(&fixture.files, &moved).unwrap();
    fs::create_dir(&fixture.files).unwrap();
    fs::write(&source, "replacement").unwrap();
    assert_eq!(
        prepared
            .apply_at(&fixture.storage, "profile", "step")
            .unwrap_err(),
        Failure::Changed
    );
    assert_eq!(fs::read_to_string(&source).unwrap(), "replacement");
    assert_eq!(
        fs::read_to_string(moved.join("note.txt")).unwrap(),
        "approved"
    );
}
#[cfg(target_os = "macos")]
#[test]
fn rejects_binary_large_hardlinked_and_symlink_sources() {
    let fixture = Fixture::new();
    let path = fixture.files.join("note.txt");
    fs::write(&path, b"binary\0").unwrap();
    assert!(matches!(
        PreparedTrash::prepare_checked(
            &fixture.grant,
            path.to_str().unwrap(),
            &digest(b"binary\0")
        ),
        Err(Failure::Input)
    ));
    fs::write(&path, vec![b'x'; MAX_BYTES + 1]).unwrap();
    assert!(matches!(
        PreparedTrash::prepare_checked(&fixture.grant, path.to_str().unwrap(), "unused"),
        Err(Failure::Input)
    ));
    fs::write(&path, b"inert").unwrap();
    fs::hard_link(&path, fixture.files.join("link.txt")).unwrap();
    assert!(PreparedTrash::prepare_checked(
        &fixture.grant,
        path.to_str().unwrap(),
        &digest(b"inert")
    )
    .is_err());
    fs::remove_file(fixture.files.join("link.txt")).unwrap();
    fs::remove_file(&path).unwrap();
    let outside = fixture.storage.join("outside.txt");
    fs::write(&outside, b"inert").unwrap();
    std::os::unix::fs::symlink(outside, &path).unwrap();
    assert!(PreparedTrash::prepare_checked(
        &fixture.grant,
        path.to_str().unwrap(),
        &digest(b"inert")
    )
    .is_err());
}
#[cfg(target_os = "macos")]
#[test]
#[ignore = "Native Trash exact-ack qualification; moves only one inert scratch file, then cleans its exact returned item"]
fn native_trash_acknowledges_exact_file_and_saved_copy_restores_exclusively() {
    use std::os::unix::fs::MetadataExt;
    let fixture = Fixture::new();
    let source = fixture
        .files
        .join(format!("zephium-inert-{}.txt", WorkArtifactId::generate()));
    fs::write(&source, "inert recovery fixture\n").unwrap();
    let before = fs::metadata(&source).unwrap();
    let prepared = PreparedTrash::prepare_checked(
        &fixture.grant,
        source.to_str().unwrap(),
        &digest(b"inert recovery fixture\n"),
    )
    .unwrap();
    let id = prepared.id.clone();
    let returned = prepared
        .apply_at(&fixture.storage, "profile", "native-step")
        .unwrap();
    let acknowledged = fs::symlink_metadata(&returned).unwrap();
    assert_eq!(
        (before.dev(), before.ino()),
        (acknowledged.dev(), acknowledged.ino())
    );
    assert!(!source.exists());
    let journal = journal::Journal::open(&fixture.storage, "profile", false)
        .unwrap()
        .unwrap();
    let record = journal.records().unwrap().pop().unwrap();
    assert_eq!(record.receipt.id, id);
    assert!(record.marked("trashed").unwrap());
    let saved = record.saved().unwrap();
    let kind = WorkStepKindV1::WriteFile {
        path: source.to_string_lossy().into_owned(),
        content: saved,
        decision: Some(true),
    };
    let change = fixture.grant.prepare(&kind, None).unwrap();
    assert!(change.fact().before_digest.is_none());
    fixture.grant.apply(change).unwrap();
    assert_eq!(
        fs::read_to_string(&source).unwrap(),
        "inert recovery fixture\n"
    );
    // A replay must not overwrite a newer file.
    let change = fixture.grant.prepare(&kind, None).unwrap();
    assert!(change.fact().before_digest.is_some());
    record.mark("restored").unwrap();
    // Cleanup is restricted to the exact acknowledged inode from this fixture.
    let current = fs::symlink_metadata(&returned).unwrap();
    assert_eq!((current.dev(), current.ino()), (before.dev(), before.ino()));
    fs::remove_file(returned).unwrap();
}

#[test]
fn restore_uses_current_grant_and_refuses_existing_or_raced_files() {
    let fixture = Fixture::new();
    let journal = journal::Journal::open(&fixture.storage, "profile", true)
        .unwrap()
        .unwrap();
    let record = journal.prepare(fixture.receipt("saved"), b"saved").unwrap();
    let id = record.receipt.id.clone();
    drop(record);
    drop(journal);
    assert!(
        restore_proposal_at(&fixture.storage, &WorkFileGrant::default(), "profile", &id).is_err()
    );
    let path = fixture.files.join("note.txt");
    fs::write(&path, "newer content").unwrap();
    assert!(matches!(
        restore_proposal_at(&fixture.storage, &fixture.grant, "profile", &id),
        Err(Failure::File(WorkFileError::Exists))
    ));
    fs::remove_file(&path).unwrap();
    let (_, change, _, _) =
        restore_proposal_at(&fixture.storage, &fixture.grant, "profile", &id).unwrap();
    fs::write(&path, "racing content").unwrap();
    assert!(fixture.grant.apply(change).is_err());
    assert_eq!(fs::read_to_string(path).unwrap(), "racing content");
}

#[cfg(target_os = "macos")]
#[test]
fn cancellation_before_worker_admission_does_not_create_recovery_or_move_source() {
    let fixture = Fixture::new();
    let source = fixture.files.join("note.txt");
    fs::write(&source, "approved").unwrap();
    let prepared = PreparedTrash::prepare_checked(
        &fixture.grant,
        source.to_str().unwrap(),
        &digest(b"approved"),
    )
    .unwrap();
    let cancel = std::sync::atomic::AtomicBool::new(true);
    assert_eq!(
        prepared
            .apply_with_cancel(&fixture.storage, "profile", "step", &cancel)
            .unwrap_err(),
        Failure::Stopped
    );
    assert_eq!(fs::read_to_string(source).unwrap(), "approved");
    assert!(!fixture.storage.join(".zephium-work-recovery").exists());
}
#[test]
fn every_admitted_saved_copy_fits_the_durable_restore_step() {
    let fixture = Fixture::new();
    let journal = journal::Journal::open(&fixture.storage, "profile", true)
        .unwrap()
        .unwrap();
    let saved = "x".repeat(MAX_BYTES);
    let record = journal
        .prepare(fixture.receipt(&saved), saved.as_bytes())
        .unwrap();
    let id = record.receipt.id.clone();
    drop(record);
    drop(journal);
    let (kind, _, _, _) =
        restore_proposal_at(&fixture.storage, &fixture.grant, "profile", &id).unwrap();
    zephium_core::work::runtime::WorkStepFact {
        id: zephium_core::work::WorkStepId::generate(),
        turn: 1,
        kind,
        status: zephium_core::work::runtime::WorkStepStatus::Running,
        usage: None,
        artifacts: vec![],
        evidence: None,
        note: None,
        measurements: None,
        local: None,
        account: None,
        part: None,
        model_fault: None,
    }
    .validate()
    .unwrap();
}

#[cfg(target_os = "macos")]
#[test]
fn refuses_in_grant_symlink_spelling_instead_of_trashing_its_target() {
    let fixture = Fixture::new();
    let target = fixture.files.join("target.txt");
    fs::write(&target, "inert").unwrap();
    let alias = fixture.files.join("alias.txt");
    std::os::unix::fs::symlink(&target, &alias).unwrap();
    assert!(PreparedTrash::prepare_checked(
        &fixture.grant,
        alias.to_str().unwrap(),
        &digest(b"inert")
    )
    .is_err());
    assert_eq!(fs::read_to_string(target).unwrap(), "inert");
}
