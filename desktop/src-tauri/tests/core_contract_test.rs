use rehome_desktop_lib::core::{
    error::{ErrorCode, RehomeError},
    models::{
        ChangeKind, CodexAccessVerification, CodexInventory, ContentCounts,
        ContinuationProbeOptions, ConversationEntry, CreatePackageReport, CreatePackageRequest,
        ExclusionSummary, FileConflictResolution, MigrationJobSnapshot, MigrationJobStage,
        MigrationJobStatus, PackageManifest, PackageMode, PackagePreview, PendingRecovery,
        PlannedOperation, PlannedSession, ProjectEntry, RecoveryStatus, ReferenceRewrite,
        ReferenceRewriteKind, RestoreOptions, RestorePlan, RestoreReport, RollbackReport,
        SessionAction, SourceOs, TargetInventory, VerificationReport,
    },
};
use serde::{de::DeserializeOwned, Serialize};
use std::{fmt::Debug, path::PathBuf};
use uuid::Uuid;

#[test]
fn manifest_round_trip() {
    let manifest = PackageManifest {
        format: "codex-rehome".into(),
        schema_version: 1,
        package_id: Uuid::nil(),
        created_at: "2026-07-22T00:00:00Z".into(),
        source_os: SourceOs::Windows,
        source_arch: "x86_64".into(),
        source_device_id: Uuid::nil(),
        mode: PackageMode::Full,
        parent_checkpoint: None,
        counts: ContentCounts::default(),
        projects: vec![],
        conversations: vec![],
        exclusions: ExclusionSummary::default(),
    };

    assert_eq!(
        serde_json::from_str::<PackageManifest>(&serde_json::to_string(&manifest).unwrap())
            .unwrap(),
        manifest
    );
}

#[test]
fn populated_manifest_preserves_source_syntax_and_portable_archive_paths() {
    let project_id = Uuid::parse_str("22222222-2222-4222-8222-222222222222").unwrap();
    let task_id = Uuid::parse_str("11111111-1111-4111-8111-111111111111").unwrap();
    let manifest = PackageManifest {
        format: "codex-rehome".into(),
        schema_version: 1,
        package_id: Uuid::nil(),
        created_at: "2026-07-22T00:00:00Z".into(),
        source_os: SourceOs::Windows,
        source_arch: "x86_64".into(),
        source_device_id: Uuid::nil(),
        mode: PackageMode::Full,
        parent_checkpoint: None,
        counts: ContentCounts {
            projects: 1,
            project_files: 1,
            conversations: 1,
            ..ContentCounts::default()
        },
        projects: vec![ProjectEntry {
            project_id,
            name: "visual".into(),
            source_path: r"C:\Users\OldUser\Documents\visual".into(),
            source_available: true,
            archive_path: "projects/22222222-2222-4222-8222-222222222222/files".into(),
            file_count: 1,
            content_bytes: 18,
            git_remote: None,
            git_branch: None,
            git_head: None,
        }],
        conversations: vec![ConversationEntry {
            task_id,
            project_id: Some(project_id),
            title: "Synthetic migration thread".into(),
            updated_at: "2026-07-22T00:00:00Z".into(),
            content_hash: "fixed-content-hash".into(),
            archive_path: "codex/sessions/2026/07/22/thread.jsonl".into(),
            classification: None,
        }],
        exclusions: ExclusionSummary::default(),
    };

    let source_path: &String = &manifest.projects[0].source_path;
    assert_eq!(source_path, r"C:\Users\OldUser\Documents\visual");

    let json = serde_json::to_value(manifest).unwrap();
    assert_eq!(json["source_os"], "windows");
    assert_eq!(json["mode"], "full");
    assert_eq!(
        json["projects"][0]["source_path"],
        r"C:\Users\OldUser\Documents\visual"
    );
    assert_eq!(
        json["projects"][0]["archive_path"],
        "projects/22222222-2222-4222-8222-222222222222/files"
    );
    assert_eq!(
        json["conversations"][0]["archive_path"],
        "codex/sessions/2026/07/22/thread.jsonl"
    );
}

#[test]
fn older_project_entries_default_to_an_available_source() {
    let project = serde_json::from_value::<ProjectEntry>(serde_json::json!({
        "project_id": "22222222-2222-4222-8222-222222222222",
        "name": "visual",
        "source_path": "C:\\Users\\OldUser\\Documents\\visual",
        "archive_path": "projects/22222222-2222-4222-8222-222222222222/files",
        "file_count": 1,
        "content_bytes": 18,
        "git_remote": null,
        "git_branch": null,
        "git_head": null
    }))
    .unwrap();

    assert!(project.source_available);
}

#[test]
fn public_models_support_the_core_contract_traits() {
    fn assert_contract<T>()
    where
        T: Debug + Clone + Serialize + DeserializeOwned + PartialEq,
    {
    }

    assert_contract::<PackageManifest>();
    assert_contract::<SourceOs>();
    assert_contract::<PackageMode>();
    assert_contract::<ContentCounts>();
    assert_contract::<ProjectEntry>();
    assert_contract::<ConversationEntry>();
    assert_contract::<ExclusionSummary>();
    assert_contract::<CodexInventory>();
    assert_contract::<TargetInventory>();
    assert_contract::<CreatePackageRequest>();
    assert_contract::<CreatePackageReport>();
    assert_contract::<PackagePreview>();
    assert_contract::<ChangeKind>();
    assert_contract::<FileConflictResolution>();
    assert_contract::<SessionAction>();
    assert_contract::<ReferenceRewriteKind>();
    assert_contract::<ReferenceRewrite>();
    assert_contract::<PlannedSession>();
    assert_contract::<PlannedOperation>();
    assert_contract::<RestorePlan>();
    assert_contract::<RestoreOptions>();
    assert_contract::<RestoreReport>();
    assert_contract::<RollbackReport>();
    assert_contract::<PendingRecovery>();
    assert_contract::<RecoveryStatus>();
    assert_contract::<VerificationReport>();
    assert_contract::<ContinuationProbeOptions>();
    assert_contract::<CodexAccessVerification>();
    assert_contract::<MigrationJobStage>();
    assert_contract::<MigrationJobStatus>();
    assert_contract::<MigrationJobSnapshot>();
    assert_contract::<ErrorCode>();
    assert_contract::<RehomeError>();
}

#[test]
fn planning_and_recovery_enums_serialize_as_stable_snake_case_values() {
    let change_kinds = [
        (ChangeKind::Add, "add"),
        (ChangeKind::Update, "update"),
        (ChangeKind::Unchanged, "unchanged"),
        (ChangeKind::Preserve, "preserve"),
        (ChangeKind::Conflict, "conflict"),
    ];
    let session_actions = [
        (SessionAction::Skip, "skip"),
        (SessionAction::Import, "import"),
        (SessionAction::ImportAsBranch, "import_as_branch"),
    ];
    let conflict_resolutions = [
        (FileConflictResolution::KeepExisting, "keep_existing"),
        (FileConflictResolution::UsePackage, "use_package"),
    ];
    let rewrite_kinds = [
        (ReferenceRewriteKind::ConversationId, "conversation_id"),
        (
            ReferenceRewriteKind::ConversationTitle,
            "conversation_title",
        ),
        (ReferenceRewriteKind::ProjectPath, "project_path"),
        (ReferenceRewriteKind::SessionPath, "session_path"),
    ];
    let recovery_statuses = [
        (RecoveryStatus::Prepared, "prepared"),
        (RecoveryStatus::Applying, "applying"),
        (RecoveryStatus::Verifying, "verifying"),
        (RecoveryStatus::Committed, "committed"),
        (RecoveryStatus::RollingBack, "rolling_back"),
        (RecoveryStatus::RolledBack, "rolled_back"),
        (RecoveryStatus::RollbackFailed, "rollback_failed"),
    ];

    for (value, expected) in change_kinds {
        assert_eq!(serde_json::to_value(value).unwrap(), expected);
    }
    for (value, expected) in session_actions {
        assert_eq!(serde_json::to_value(value).unwrap(), expected);
    }
    for (value, expected) in conflict_resolutions {
        assert_eq!(serde_json::to_value(value).unwrap(), expected);
    }
    for (value, expected) in rewrite_kinds {
        assert_eq!(serde_json::to_value(value).unwrap(), expected);
    }
    for (value, expected) in recovery_statuses {
        assert_eq!(serde_json::to_value(value).unwrap(), expected);
    }
}

#[test]
fn restore_and_recovery_contracts_use_typed_state() {
    let operation = PlannedOperation {
        package_source: "projects/project-id/files/README.md".into(),
        target: PathBuf::from(r"C:\Users\NewUser\Documents\visual\README.md"),
        expected_previous_hash: None,
        action: ChangeKind::Add,
        rollback_required: true,
    };
    let plan = RestorePlan {
        plan_id: Uuid::nil(),
        package_path: PathBuf::from("handoff.rehome"),
        package_id: Uuid::nil(),
        archive_hash: "archive-sha256".into(),
        target_codex_home: PathBuf::from(r"C:\Users\NewUser\.codex"),
        projects_root: PathBuf::from(r"C:\Users\NewUser\Documents"),
        operations: vec![operation.clone()],
        sessions: vec![],
        reference_rewrites: vec![],
        bridge_verification: Default::default(),
        conflict_count: 0,
        required_bytes: 18,
    };
    let recovery = PendingRecovery {
        transaction_id: Uuid::nil(),
        package_id: Uuid::nil(),
        created_at: "2026-07-22T00:00:00Z".into(),
        status: RecoveryStatus::Prepared,
        backup_root: PathBuf::from(r"C:\Users\NewUser\AppData\Local\ReHome\backups"),
    };

    assert_eq!(plan.operations, vec![operation]);
    assert_eq!(recovery.status, RecoveryStatus::Prepared);
}

#[test]
fn error_codes_serialize_as_stable_snake_case_values() {
    let cases = [
        (ErrorCode::CodexNotFound, "codex_not_found"),
        (ErrorCode::PackageInvalid, "package_invalid"),
        (ErrorCode::ChecksumMismatch, "checksum_mismatch"),
        (ErrorCode::UnsupportedSchema, "unsupported_schema"),
        (ErrorCode::CodexRunning, "codex_running"),
        (
            ErrorCode::CodexAppServerUnavailable,
            "codex_app_server_unavailable",
        ),
        (
            ErrorCode::CodexAuthenticationRequired,
            "codex_authentication_required",
        ),
        (
            ErrorCode::CodexVerificationFailed,
            "codex_verification_failed",
        ),
        (ErrorCode::MigrationJobNotFound, "migration_job_not_found"),
        (ErrorCode::DiskSpaceInsufficient, "disk_space_insufficient"),
        (ErrorCode::ProjectConflict, "project_conflict"),
        (ErrorCode::RestoreFailed, "restore_failed"),
        (ErrorCode::RollbackFailed, "rollback_failed"),
        (ErrorCode::RegistrationIncomplete, "registration_incomplete"),
    ];

    for (code, expected) in cases {
        assert_eq!(
            serde_json::to_string(&code).unwrap(),
            format!(r#""{expected}""#)
        );
        assert_eq!(
            serde_json::from_value::<ErrorCode>(expected.into()).unwrap(),
            code
        );
    }
}

#[test]
fn rehome_error_is_human_readable_and_has_a_stable_payload() {
    let error = RehomeError::new(ErrorCode::PackageInvalid, "manifest.json is missing");

    assert_eq!(error.to_string(), "manifest.json is missing");
    assert_eq!(
        serde_json::to_value(&error).unwrap(),
        serde_json::json!({
            "code": "package_invalid",
            "message": "manifest.json is missing"
        })
    );
}

fn legacy_verification_json() -> serde_json::Value {
    serde_json::json!({
        "package_checksum_valid": true,
        "files_valid": true,
        "sessions_valid": true,
        "session_index_valid": true,
        "sqlite_threads_valid": true,
        "path_mapping_valid": true,
        "forbidden_files_absent": true,
        "project_files_valid": true,
        "app_registration_valid": false,
        "app_visible_ready": false
    })
}

#[test]
fn continuation_probe_defaults_to_disabled() {
    let mut value = serde_json::json!({
        "codex_closed_confirmed": true,
        "backup_root": "C:/Synthetic/backups",
        "register_projects": false
    });
    let options: RestoreOptions = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(options.continuation_probe, None);
    assert_eq!(options.backup_root, PathBuf::from("C:/Synthetic/backups"));
    assert!(options.codex_closed_confirmed);
    assert!(!options.register_projects);

    value["continuation_probe"] = serde_json::Value::Null;
    assert_eq!(
        serde_json::from_value::<RestoreOptions>(value.clone()).unwrap(),
        options
    );
    for required in ["codex_closed_confirmed", "backup_root", "register_projects"] {
        let mut missing = value.clone();
        missing.as_object_mut().unwrap().remove(required);
        assert!(serde_json::from_value::<RestoreOptions>(missing).is_err());
    }
}

#[test]
fn codex_access_verification_defaults_to_not_requested() {
    let report: VerificationReport = serde_json::from_value(legacy_verification_json()).unwrap();
    assert_eq!(report.codex_access, CodexAccessVerification::default());
    assert_eq!(
        serde_json::to_value(&report).unwrap()["codex_access"],
        serde_json::json!({
            "required_threads": 0,
            "recognized_threads": 0,
            "probe_thread_id": null,
            "threads_recognized": false,
            "continuation_probe_valid": false,
            "ephemeral_fork": false
        })
    );
    assert!(report.files_valid);
    assert!(!report.app_visible_ready);
}

#[test]
fn core_contract_legacy_restore_report_defaults_nested_codex_access() {
    let report: RestoreReport = serde_json::from_value(serde_json::json!({
        "transaction_id": "11111111-1111-4111-8111-111111111111",
        "package_id": "22222222-2222-4222-8222-222222222222",
        "completed_at": "2026-09-20T00:00:00Z",
        "restored_files": 2,
        "restored_bytes": 128,
        "registrations": [],
        "verification": legacy_verification_json()
    }))
    .unwrap();
    assert_eq!(report.restored_files, 2);
    assert_eq!(
        report.verification.codex_access,
        CodexAccessVerification::default()
    );
    assert!(!report.verification.app_visible_ready);
}

#[test]
fn core_contract_probe_preserves_explicit_consent_and_validates_its_shape() {
    let mut value = serde_json::json!({
        "codex_closed_confirmed": true,
        "backup_root": "C:/Synthetic/backups",
        "register_projects": false,
        "continuation_probe": {
            "probe_thread_id": "11111111-1111-4111-8111-111111111111",
            "online_usage_confirmed": false
        }
    });
    for confirmed in [false, true] {
        value["continuation_probe"]["online_usage_confirmed"] = confirmed.into();
        let options: RestoreOptions = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(
            options.continuation_probe,
            Some(ContinuationProbeOptions {
                probe_thread_id: Uuid::parse_str("11111111-1111-4111-8111-111111111111").unwrap(),
                online_usage_confirmed: confirmed,
            })
        );
        assert_eq!(serde_json::to_value(options).unwrap(), value);
    }
    let mut missing_consent = value.clone();
    missing_consent["continuation_probe"]
        .as_object_mut()
        .unwrap()
        .remove("online_usage_confirmed");
    assert!(serde_json::from_value::<RestoreOptions>(missing_consent).is_err());
    value["continuation_probe"]["probe_thread_id"] = "invalid-uuid".into();
    assert!(serde_json::from_value::<RestoreOptions>(value).is_err());
}

#[test]
fn core_contract_codex_access_evidence_survives_a_report_round_trip() {
    let mut value = legacy_verification_json();
    value["codex_access"] = serde_json::json!({
        "required_threads": 3,
        "recognized_threads": 3,
        "probe_thread_id": "11111111-1111-4111-8111-111111111111",
        "threads_recognized": true,
        "continuation_probe_valid": true,
        "ephemeral_fork": true
    });
    let report: VerificationReport = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(report.codex_access.required_threads, 3);
    assert_eq!(report.codex_access.recognized_threads, 3);
    assert!(!report.app_visible_ready);
    assert_eq!(serde_json::to_value(report).unwrap(), value);
}

#[test]
fn core_contract_migration_stages_have_stable_wire_names() {
    for (stage, name) in [
        (MigrationJobStage::Preflight, "preflight"),
        (MigrationJobStage::RestoringFiles, "restoring_files"),
        (MigrationJobStage::FilesVerified, "files_verified"),
        (MigrationJobStage::RecognizingThreads, "recognizing_threads"),
        (
            MigrationJobStage::ProbingContinuation,
            "probing_continuation",
        ),
        (MigrationJobStage::Committing, "committing"),
        (MigrationJobStage::RollingBack, "rolling_back"),
        (MigrationJobStage::Finished, "finished"),
    ] {
        assert_eq!(serde_json::to_value(stage).unwrap(), name);
        assert_eq!(
            serde_json::from_value::<MigrationJobStage>(name.into()).unwrap(),
            stage
        );
    }
    assert!(serde_json::from_value::<MigrationJobStage>("unknown".into()).is_err());
}

#[test]
fn core_contract_migration_snapshots_preserve_all_statuses_and_optional_results() {
    let verification: VerificationReport =
        serde_json::from_value(legacy_verification_json()).unwrap();
    let report = RestoreReport {
        transaction_id: Uuid::from_u128(1),
        package_id: Uuid::from_u128(2),
        completed_at: "2026-09-20T00:00:00Z".into(),
        restored_files: 2,
        restored_bytes: 128,
        registrations: vec![],
        verification,
    };
    for (status, name, transaction_id, result, error) in [
        (MigrationJobStatus::Running, "running", None, None, None),
        (
            MigrationJobStatus::Succeeded,
            "succeeded",
            Some(report.transaction_id),
            Some(report.clone()),
            None,
        ),
        (
            MigrationJobStatus::FailedBeforeWrite,
            "failed_before_write",
            None,
            None,
            Some(RehomeError::new(
                ErrorCode::CodexAppServerUnavailable,
                "App Server unavailable",
            )),
        ),
        (
            MigrationJobStatus::RolledBack,
            "rolled_back",
            Some(report.transaction_id),
            None,
            Some(RehomeError::new(
                ErrorCode::CodexAuthenticationRequired,
                "Authentication required",
            )),
        ),
        (
            MigrationJobStatus::RollbackFailed,
            "rollback_failed",
            Some(report.transaction_id),
            None,
            Some(RehomeError::new(
                ErrorCode::RollbackFailed,
                "Manual recovery required",
            )),
        ),
    ] {
        let snapshot = MigrationJobSnapshot {
            job_id: Uuid::from_u128(3),
            plan_id: Uuid::from_u128(4),
            transaction_id,
            stage: if status == MigrationJobStatus::Running {
                MigrationJobStage::Preflight
            } else {
                MigrationJobStage::Finished
            },
            status,
            report: result,
            error,
            updated_at: "2026-09-20T00:00:00Z".into(),
        };
        let value = serde_json::to_value(&snapshot).unwrap();
        assert_eq!(value["status"], name);
        assert_eq!(
            value["stage"],
            if name == "running" {
                "preflight"
            } else {
                "finished"
            }
        );
        assert_eq!(value["transaction_id"].is_null(), transaction_id.is_none());
        assert_eq!(value["report"].is_null(), name != "succeeded");
        assert_eq!(
            value["error"].is_null(),
            matches!(name, "running" | "succeeded")
        );
        assert_eq!(
            serde_json::from_value::<MigrationJobSnapshot>(value).unwrap(),
            snapshot
        );
    }
    assert!(serde_json::from_value::<MigrationJobStatus>("unknown".into()).is_err());
}
