use rehome_desktop_lib::core::{
    local_discovery::{discover_local_candidates, LocalScanRequest},
    restic::{
        backup_local, file_fingerprint, list_local_snapshots, restore_local, BackupIssueKind,
        BackupManifest, LocalBackupRequest,
    },
};
use std::{
    fs,
    path::Path,
    sync::{atomic::AtomicBool, Arc},
};
use tempfile::tempdir;
use uuid::Uuid;

fn write(root: &Path, path: &str, contents: &[u8]) {
    let target = root.join(path);
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(target, contents).unwrap();
}

const PROJECT_FILES: &[&str] = &[
    "README.md",
    ".env",
    ".env.local",
    "private.key",
    "id_ed25519",
    "token.json",
    "auth.json",
    ".codex/token.json",
    ".git/config",
    ".git/objects/ab/data",
    "node_modules/pkg/index.js",
    ".venv/lib/module.py",
    "target/debug/app",
    ".next/cache/data",
    ".hidden",
    "app.pid",
];

#[test]
fn configured_collection_lists_only_direct_children_including_markerless_folders() {
    let root = tempdir().unwrap();
    write(root.path(), "plain notes/readme.txt", b"synthetic");
    write(root.path(), "app/package.json", b"{}");
    write(root.path(), "app/.next/fixtures/package.json", b"{}");
    fs::create_dir(root.path().join("empty")).unwrap();
    write(root.path(), "repo/copy/package.json", b"{}");
    let result = discover_local_candidates(
        LocalScanRequest {
            roots: vec![root.path().to_path_buf()],
            excluded_roots: vec![root.path().join("repo")],
        },
        Arc::new(AtomicBool::new(false)),
    );
    let names: Vec<_> = result.candidates.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["app", "empty", "plain notes"]);
    assert_eq!(
        serde_json::to_value(&result).unwrap()["scan_limit_reached"],
        false
    );
}

#[test]
fn candidate_counts_every_regular_file_without_returning_contents() {
    let root = tempdir().unwrap();
    let project = root.path().join("project");
    write(&project, "package.json", b"{}");
    for path in PROJECT_FILES {
        write(&project, path, b"synthetic-secret-must-not-be-output");
    }
    let result = discover_local_candidates(
        LocalScanRequest {
            roots: vec![root.path().to_path_buf()],
            excluded_roots: vec![],
        },
        Arc::new(AtomicBool::new(false)),
    );
    let value = serde_json::to_value(&result.candidates[0]).unwrap();
    assert_eq!(value["file_count"], 17);
    assert_eq!(value["file_count_complete"], true);
    assert_eq!(value["skipped_entries"], 0);
    assert!(!value
        .to_string()
        .contains("synthetic-secret-must-not-be-output"));
}

#[test]
fn manual_count_command_yields_and_reports_invalid_paths_honestly() {
    use std::{
        future::{poll_fn, Future},
        task::Poll,
    };
    let root = tempdir().unwrap();
    // Enough metadata to distinguish an inline traversal from a worker handoff.
    for index in 0..512 {
        write(root.path(), &format!("{index}/file"), b"synthetic");
    }
    let paths = vec![
        root.path().to_path_buf(),
        root.path().join("missing"),
        root.path().join("0/file"),
        "relative-path".into(),
    ];
    let mut operation = Box::pin(rehome_desktop_lib::workflow::count_project_files(paths));
    let mut yielded = false;
    let result =
        tauri::async_runtime::block_on(poll_fn(|context| match operation.as_mut().poll(context) {
            Poll::Pending => {
                yielded = true;
                Poll::Pending
            }
            Poll::Ready(result) => Poll::Ready(result.unwrap()),
        }));
    assert!(yielded, "count traversal blocked the calling executor");
    assert_eq!(result[0].file_count, 512);
    assert!(result[0].file_count_complete);
    for item in &result[1..] {
        assert!(!item.file_count_complete);
        assert!(item.skipped_entries > 0);
    }
}

#[test]
fn project_fingerprint_includes_secrets_dependencies_build_outputs_and_git() {
    let root = tempdir().unwrap();
    for path in PROJECT_FILES {
        write(root.path(), path, b"synthetic-before");
        let before = file_fingerprint(root.path()).unwrap();
        write(root.path(), path, b"synthetic-only");
        assert_ne!(
            before,
            file_fingerprint(root.path()).unwrap(),
            "ignored {path}"
        );
    }
}

#[test]
fn project_selection_cannot_reclassify_codex_credentials_as_project_data() {
    let root = tempdir().unwrap();
    let codex = root.path().join("codex");
    write(&codex, "auth.json", b"synthetic-only");
    let error = backup_local(
        LocalBackupRequest {
            project_paths: vec![codex.clone()],
            codex_home: codex,
            repository: root.path().join("repo"),
            password: "synthetic".into(),
            source_device_id: Uuid::new_v4(),
        },
        &root.path().join("no-engine.exe"),
    )
    .unwrap_err();
    assert_eq!(
        error.code,
        rehome_desktop_lib::core::error::ErrorCode::UnsafePath
    );
}

#[test]
fn git_pointer_cannot_expand_backup_into_codex_home() {
    let root = tempdir().unwrap();
    let codex = root.path().join("codex");
    let project = root.path().join("project");
    write(&codex, "auth.json", b"synthetic-only");
    write(
        &project,
        ".git",
        format!("gitdir: {}", codex.display()).as_bytes(),
    );
    let error = backup_local(
        LocalBackupRequest {
            project_paths: vec![project],
            codex_home: codex,
            repository: root.path().join("repo"),
            password: "synthetic".into(),
            source_device_id: Uuid::new_v4(),
        },
        &root.path().join("no-engine.exe"),
    )
    .unwrap_err();
    assert_eq!(
        error.code,
        rehome_desktop_lib::core::error::ErrorCode::UnsafePath
    );
}

#[cfg(windows)]
fn junction(link: &Path, target: &Path) {
    let output = std::process::Command::new("cmd.exe")
        .args(["/d", "/c", "mklink", "/J"])
        .arg(link)
        .arg(target)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "could not create synthetic junction"
    );
}

#[cfg(windows)]
#[test]
fn counts_do_not_follow_junctions_and_missing_entries_are_partial() {
    use rehome_desktop_lib::core::local_discovery::count_project_files;
    let root = tempdir().unwrap();
    let project = root.path().join("project");
    let outside = root.path().join("outside");
    write(&project, "regular", b"synthetic");
    write(&outside, "must-not-count", b"synthetic");
    junction(&project.join("escape"), &outside);
    let counts = count_project_files(vec![
        project.clone(),
        project.join("escape"),
        project.join("escape/subdir"),
    ]);
    assert_eq!(counts[0].file_count, 1);
    for item in counts {
        assert!(!item.file_count_complete);
        assert!(item.skipped_entries > 0);
    }
    assert!(
        file_fingerprint(&project).is_err(),
        "cannot reuse a complete snapshot after a junction appears"
    );
    fs::remove_dir(project.join("escape")).unwrap();
}

#[cfg(windows)]
#[test]
fn locked_directory_does_not_look_like_an_empty_complete_project() {
    use rehome_desktop_lib::core::local_discovery::count_project_files;
    use std::os::windows::fs::OpenOptionsExt;
    let root = tempdir().unwrap();
    let project = root.path().join("project");
    write(&project, "nested/file", b"synthetic");
    let lock = fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .custom_flags(0x02000000)
        .open(project.join("nested"))
        .unwrap();
    let candidate = count_project_files(vec![project]).remove(0);
    assert!(!candidate.file_count_complete);
    assert!(candidate.skipped_entries > 0);
    drop(lock);
}

#[test]
fn missing_repository_suffix_cannot_hide_source_overlap() {
    let root = tempdir().unwrap();
    let project = root.path().join("project");
    write(&project, "file", b"synthetic");
    fs::create_dir(root.path().join("codex")).unwrap();
    let request = LocalBackupRequest {
        project_paths: vec![project.clone()],
        codex_home: root.path().join("codex"),
        repository: root.path().join("uncreated/../project/repo"),
        password: "synthetic".into(),
        source_device_id: Uuid::new_v4(),
    };
    let error = backup_local(request, &root.path().join("no-engine.exe")).unwrap_err();
    assert_eq!(
        error.code,
        rehome_desktop_lib::core::error::ErrorCode::UnsafePath
    );
}

#[test]
#[ignore = "large synthetic metadata fixture proving counts exceed discovery quotas"]
fn full_counts_exceed_whole_drive_discovery_quota_and_do_not_starve_later_folders() {
    let root = tempdir().unwrap();
    let first = root.path().join("a-large");
    fs::create_dir(&first).unwrap();
    for index in 0..50_010 {
        fs::write(first.join(index.to_string()), b"").unwrap();
    }
    write(root.path(), "z-later/file", b"synthetic");
    let result = discover_local_candidates(
        LocalScanRequest {
            roots: vec![root.path().to_path_buf()],
            excluded_roots: vec![],
        },
        Arc::new(AtomicBool::new(false)),
    );
    assert_eq!(result.candidates.len(), 2);
    assert_eq!(result.candidates[0].file_count, 50_010);
    assert_eq!(result.candidates[1].file_count, 1);
    assert!(result
        .candidates
        .iter()
        .all(|candidate| candidate.file_count_complete));
}

#[test]
#[ignore = "requires bundled restic; synthetic Git worktree backup and restore"]
fn bundled_restic_tracks_external_git_metadata_and_repairs_worktree() {
    let root = tempdir().unwrap();
    let project = root.path().join("project");
    let common = root.path().join("main/.git");
    let gitdir = common.join("worktrees/project");
    write(&common, "HEAD", b"ref: refs/heads/main\n");
    write(&common, "objects/ab/object", b"synthetic-git-object");
    write(&gitdir, "HEAD", b"ref: refs/heads/main\n");
    write(&gitdir, "commondir", b"../..\n");
    write(
        &gitdir,
        "gitdir",
        project.join(".git").to_string_lossy().as_bytes(),
    );
    write(
        &project,
        ".git",
        format!("gitdir: {}\n", gitdir.display()).as_bytes(),
    );
    fs::create_dir(root.path().join("codex")).unwrap();
    let request = LocalBackupRequest {
        project_paths: vec![project],
        codex_home: root.path().join("codex"),
        repository: root.path().join("repo"),
        password: "synthetic-local-password".into(),
        source_device_id: Uuid::new_v4(),
    };
    let executable = Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/restic.exe");
    let snapshot = backup_local(request.clone(), &executable).unwrap();
    assert!(snapshot.complete);
    write(&common, "objects/ab/object", b"synthetic-changed-object");
    let changed = backup_local(request.clone(), &executable).unwrap();
    assert_ne!(snapshot.restic_snapshot_id, changed.restic_snapshot_id);
    let restored = restore_local(
        &changed.restic_snapshot_id,
        &request.repository,
        &request.password,
        &root.path().join("restore"),
        &executable,
    )
    .unwrap();
    let restored_project = fs::read_dir(restored.restored_root.join("projects"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let marker = fs::read_to_string(restored_project.join(".git")).unwrap();
    let worktree = restored_project.join(marker.trim().strip_prefix("gitdir: ").unwrap());
    let common = worktree.join(
        fs::read_to_string(worktree.join("commondir"))
            .unwrap()
            .trim(),
    );
    assert_eq!(
        fs::read(common.join("objects/ab/object")).unwrap(),
        b"synthetic-changed-object"
    );
}

#[cfg(windows)]
#[test]
#[ignore = "requires bundled restic; synthetic locked file and junction"]
fn bundled_restic_partial_sources_never_reuse_a_complete_snapshot() {
    use std::os::windows::fs::OpenOptionsExt;
    let root = tempdir().unwrap();
    let project = root.path().join("project");
    write(&project, "accessible", b"synthetic");
    write(&project, "locked", b"synthetic-locked");
    write(root.path(), "outside/never-follow", b"synthetic-outside");
    fs::create_dir_all(project.join("node_modules")).unwrap();
    fs::create_dir_all(project.join(".local-audit").join("fixtures")).unwrap();
    fs::create_dir(root.path().join("codex")).unwrap();
    let dependency_redirect = project.join("node_modules").join("dependency-link");
    let audit_redirect = project
        .join(".local-audit")
        .join("fixtures")
        .join("reparse");
    let request = LocalBackupRequest {
        project_paths: vec![project.clone()],
        codex_home: root.path().join("codex"),
        repository: root.path().join("repo"),
        password: "synthetic-local-password".into(),
        source_device_id: Uuid::new_v4(),
    };
    let executable = Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/restic.exe");
    let complete = backup_local(request.clone(), &executable).unwrap();
    assert!(complete.complete);
    junction(&project.join("escape"), &root.path().join("outside"));
    junction(&dependency_redirect, &root.path().join("outside"));
    junction(&audit_redirect, &root.path().join("outside"));
    let lock = fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(project.join("locked"))
        .unwrap();
    let partial = backup_local(request.clone(), &executable).unwrap();
    assert!(!partial.complete);
    assert_ne!(partial.restic_snapshot_id, complete.restic_snapshot_id);
    assert!(partial
        .manifest
        .missing
        .iter()
        .any(|issue| issue.path.ends_with("locked")));
    assert!(partial
        .manifest
        .missing
        .iter()
        .any(|issue| issue.path.ends_with("escape")));
    assert!(partial.manifest.notices.iter().any(|issue| {
        issue.path.ends_with("dependency-link")
            && issue.kind == BackupIssueKind::RebuildableDependency
    }));
    assert!(partial.manifest.notices.iter().any(|issue| {
        issue.path.ends_with("reparse") && issue.kind == BackupIssueKind::TestArtifact
    }));
    assert!(!partial.manifest.missing.iter().any(|issue| {
        issue.path.ends_with("dependency-link") || issue.path.ends_with("reparse")
    }));
    let restored = restore_local(
        &partial.restic_snapshot_id,
        &request.repository,
        &request.password,
        &root.path().join("restore"),
        &executable,
    )
    .unwrap();
    assert!(!restored.complete);
    let restored_project = fs::read_dir(restored.restored_root.join("projects"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert!(restored_project.join("accessible").is_file());
    assert!(!restored_project.join("escape").exists());
    assert!(!restored_project
        .join("node_modules")
        .join("dependency-link")
        .exists());
    assert!(!restored_project
        .join(".local-audit")
        .join("fixtures")
        .join("reparse")
        .exists());
    assert!(!restored_project.join("locked").exists());
    drop(lock);
    fs::remove_dir(project.join("escape")).unwrap();
    fs::remove_dir(dependency_redirect).unwrap();
    fs::remove_dir(audit_redirect).unwrap();
}

#[test]
#[ignore = "requires bundled restic; synthetic encrypted local backup only"]
fn bundled_restic_restores_full_projects_but_excludes_codex_credentials() {
    let root = tempdir().unwrap();
    let project = root.path().join("project");
    let codex = root.path().join("codex");
    for path in PROJECT_FILES {
        write(&project, path, b"synthetic-only");
    }
    write(&project, ".codex/auth.json", b"synthetic-codex-credential");
    write(&project, ".codex/config.toml", b"synthetic-config");
    write(&project, "arbitrary.jsonl", b"not-json\n{partial");
    let database = project.join("project.db");
    let connection = rusqlite::Connection::open(&database).unwrap();
    connection
        .execute_batch(
            "CREATE TABLE synthetic (value TEXT); INSERT INTO synthetic VALUES ('fixture');",
        )
        .unwrap();
    drop(connection);
    let database_bytes = fs::read(&database).unwrap();
    write(&codex, "auth.json", b"synthetic-codex-credential");
    write(&codex, "token.json", b"synthetic-codex-token");
    write(&codex, "history.jsonl", b"{\"synthetic\":true}\n");
    let request = LocalBackupRequest {
        project_paths: vec![project.clone()],
        codex_home: codex.clone(),
        repository: root.path().join("repo"),
        password: "synthetic-local-password".into(),
        source_device_id: Uuid::new_v4(),
    };
    let executable = Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/restic.exe");
    let snapshot = backup_local(request.clone(), &executable).unwrap();
    assert!(snapshot.complete);
    let restored = restore_local(
        &snapshot.restic_snapshot_id,
        &request.repository,
        &request.password,
        &root.path().join("restore"),
        &executable,
    )
    .unwrap();
    assert!(restored.complete);
    let restored_project = fs::read_dir(restored.restored_root.join("projects"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    for path in PROJECT_FILES {
        assert_eq!(
            fs::read(restored_project.join(path)).unwrap_or_else(|_| panic!("missing {path}")),
            b"synthetic-only"
        );
    }
    assert_eq!(
        fs::read(restored_project.join(".codex/auth.json")).unwrap(),
        b"synthetic-codex-credential"
    );
    assert!(restored_project.join(".codex/config.toml").is_file());
    assert_eq!(
        fs::read(restored_project.join("arbitrary.jsonl")).unwrap(),
        b"not-json\n{partial"
    );
    assert_eq!(
        fs::read(restored_project.join("project.db")).unwrap(),
        database_bytes
    );
    assert!(!restored.restored_root.join("codex/auth.json").exists());
    assert!(!restored.restored_root.join("codex/token.json").exists());
    assert!(restored.restored_root.join("codex/history.jsonl").is_file());
    assert_eq!(snapshot.manifest.file_count, 21);
    // A secret-only change must not reuse an earlier complete snapshot.
    write(&project, ".env", b"synthetic-changed");
    let changed = backup_local(request.clone(), &executable).unwrap();
    assert_ne!(snapshot.restic_snapshot_id, changed.restic_snapshot_id);
    write(&codex, "auth.json", b"synthetic-changed-credential");
    let unchanged = backup_local(request, &executable).unwrap();
    assert_eq!(changed.restic_snapshot_id, unchanged.restic_snapshot_id);
}

#[test]
#[ignore = "requires bundled restic; synthetic restored manifest and counts"]
fn bundled_restic_restored_manifest_preserves_sources_missing_and_ordinary_counts() {
    let root = tempdir().unwrap();
    let project = root.path().join("project with spaces");
    let codex = root.path().join("codex");
    let missing_project = root.path().join("missing-project");
    let project_manifest = b"{\"project_owned\":true}";
    write(&project, "manifest.json", project_manifest);
    write(&project, "README.md", b"synthetic project");
    write(&codex, "history.jsonl", b"{\"synthetic\":true}\n");
    write(&codex, "auth.json", b"synthetic-excluded-credential");
    let mut request = LocalBackupRequest {
        project_paths: vec![project],
        codex_home: codex,
        repository: root.path().join("repo"),
        password: "synthetic-local-password".into(),
        source_device_id: Uuid::new_v4(),
    };
    let executable = Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/restic.exe");
    for partial in [false, true] {
        if partial {
            request.project_paths.push(missing_project.clone());
        }
        let snapshot = backup_local(request.clone(), &executable).unwrap();
        assert_eq!(snapshot.complete, !partial);
        assert_eq!(snapshot.manifest.file_count, 3);
        let restore_target = root
            .path()
            .join(if partial { "partial" } else { "complete" });
        let restored = restore_local(
            &snapshot.restic_snapshot_id,
            &request.repository,
            &request.password,
            &restore_target,
            &executable,
        )
        .unwrap();
        let manifest_bytes = fs::read(restored.restored_root.join("manifest.json"))
            .expect("restored backup must retain its internal manifest");
        let manifest: BackupManifest = serde_json::from_slice(&manifest_bytes).unwrap();
        assert_eq!(manifest, snapshot.manifest);
        assert_eq!(
            manifest.source_codex_home,
            request.codex_home.display().to_string()
        );
        assert_eq!(
            manifest.project_paths,
            request
                .project_paths
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
        );
        assert_eq!(manifest.file_count, 3);
        assert_eq!(restored.restored_files, 3);
        assert_eq!(restored.restored_bytes, manifest.byte_count);
        assert_eq!(restored.missing, manifest.missing);
        assert_eq!(restored.complete, !partial);
        assert_eq!(
            manifest.integrity_status,
            if partial { "partial" } else { "complete" }
        );
        if partial {
            assert_eq!(manifest.missing.len(), 1);
            assert_eq!(
                manifest.missing[0].path,
                missing_project.display().to_string()
            );
            assert!(!manifest.missing[0].reason.is_empty());
        } else {
            assert!(manifest.missing.is_empty());
        }
        assert!(manifest
            .exclusions
            .iter()
            .any(|issue| issue.path == "auth.json"));
        let restored_project = fs::read_dir(restored.restored_root.join("projects"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        assert_eq!(
            fs::read(restored_project.join("manifest.json")).unwrap(),
            project_manifest
        );
        let listed =
            list_local_snapshots(&request.repository, &request.password, &executable).unwrap();
        let listed = listed
            .iter()
            .find(|item| item.restic_snapshot_id == snapshot.restic_snapshot_id)
            .unwrap();
        assert_eq!(listed.file_count, 3);
        assert_eq!(listed.complete, !partial);
        let conflict = restore_local(
            &snapshot.restic_snapshot_id,
            &request.repository,
            &request.password,
            &restore_target,
            &executable,
        )
        .unwrap_err();
        assert_eq!(
            conflict.code,
            rehome_desktop_lib::core::error::ErrorCode::ProjectConflict
        );
        assert_eq!(
            fs::read(restored.restored_root.join("manifest.json")).unwrap(),
            manifest_bytes
        );
        assert_eq!(
            fs::read(restored_project.join("manifest.json")).unwrap(),
            project_manifest
        );
    }
}
