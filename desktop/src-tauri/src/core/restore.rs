use crate::core::{
    app_server::{CodexAccessRequest, CodexAccessVerifier},
    backup::{
        ensure_applied_states, prepare_transaction, record_applied_mutation, rollback_prepared,
        update_status, PreparedTransaction,
    },
    bridge::{
        apply_bridge_plan_for_transaction, apply_file_source_for_transaction,
        register_project_with_detected_cli, validate_restore_target,
        validate_restore_target_ancestry,
    },
    error::{ErrorCode, RehomeError},
    models::{
        ChangeKind, CodexAccessVerification, MigrationJobStage, PendingRecovery, PlannedOperation,
        ProjectRegistration, RecoveryStatus, ReferenceRewriteKind, RegistrationStatus,
        RestoreOptions, RestorePlan, RestoreReport, RollbackReport, SourceOs, TransactionHistory,
        TransactionSummary, VerificationReport,
    },
    package::{inspect_package_for_planning, VerifiedPackage},
    paths::restore_target_root,
    planner::rewrite_jsonl_payload,
    stable_fs::PinnedParent,
};
use chrono::{SecondsFormat, Utc};
use rusqlite::{Connection, OpenFlags};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsStr,
    fs,
    io::{self, Write},
    path::{Component, Path, PathBuf},
};
use tempfile::NamedTempFile;
use uuid::Uuid;

const SESSION_INDEX_SOURCE: &str = "codex/session_index.jsonl";
const THREAD_METADATA_SOURCE: &str = "codex/metadata/threads.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RestoreProgressEvent {
    Stage(MigrationJobStage),
    TransactionPrepared(Uuid),
    RollbackCompleted,
    RollbackFailed,
}

pub trait RestoreProgressSink {
    fn update(&mut self, event: RestoreProgressEvent);
}

struct NoProgress;

impl RestoreProgressSink for NoProgress {
    fn update(&mut self, _event: RestoreProgressEvent) {}
}

pub fn apply_restore_with_services(
    plan: RestorePlan,
    options: RestoreOptions,
    registrar: &mut dyn FnMut(SourceOs, &Path) -> RegistrationStatus,
    verifier: Option<&mut dyn CodexAccessVerifier>,
    progress: &mut dyn RestoreProgressSink,
) -> Result<RestoreReport, RehomeError> {
    let plan = crate::core::plan_store::load_exact(&plan)?;
    apply_server_plan(
        plan,
        options,
        registrar,
        verifier,
        progress,
        &mut available_disk_space,
    )
}

pub fn apply_restore(
    plan: RestorePlan,
    options: RestoreOptions,
) -> Result<RestoreReport, RehomeError> {
    apply_restore_with_services(
        plan,
        options,
        &mut register_project_with_detected_cli,
        None,
        &mut NoProgress,
    )
}

pub fn apply_restore_by_id(
    plan_id: Uuid,
    options: RestoreOptions,
) -> Result<RestoreReport, RehomeError> {
    let plan = crate::core::plan_store::load(plan_id)?;
    apply_server_plan(
        plan,
        options,
        &mut register_project_with_detected_cli,
        None,
        &mut NoProgress,
        &mut available_disk_space,
    )
}

pub fn apply_restore_with_registrar(
    plan: RestorePlan,
    options: RestoreOptions,
    mut registrar: impl FnMut(SourceOs, &Path) -> RegistrationStatus,
) -> Result<RestoreReport, RehomeError> {
    apply_restore_with_services(plan, options, &mut registrar, None, &mut NoProgress)
}

fn apply_server_plan(
    plan: RestorePlan,
    options: RestoreOptions,
    registrar: &mut dyn FnMut(SourceOs, &Path) -> RegistrationStatus,
    mut verifier: Option<&mut dyn CodexAccessVerifier>,
    progress: &mut dyn RestoreProgressSink,
    available_space: &mut dyn FnMut(&Path) -> Result<u64, RehomeError>,
) -> Result<RestoreReport, RehomeError> {
    progress.update(RestoreProgressEvent::Stage(MigrationJobStage::Preflight));
    if !options.codex_closed_confirmed {
        return Err(RehomeError::new(
            ErrorCode::CodexRunning,
            "restore requires confirmation that current Codex work is saved",
        ));
    }
    validate_plan(&plan)?;
    if let Some(probe) = &options.continuation_probe {
        if !probe.online_usage_confirmed
            || plan.sessions.is_empty()
            || !plan
                .sessions
                .iter()
                .any(|session| session.target_task_id == probe.probe_thread_id)
        {
            return Err(RehomeError::new(
                ErrorCode::CodexVerificationFailed,
                "continuation verification requires online consent and a thread in the stored plan",
            ));
        }
        if verifier.is_none() {
            return Err(RehomeError::new(
                ErrorCode::CodexAppServerUnavailable,
                "continuation verification requires an access verifier",
            ));
        }
    }
    let verified = inspect_package_for_planning(&plan.package_path)?;
    validate_package_identity(&plan, &verified)?;
    if verified.preview.forbidden_files_total > 0 {
        return Err(RehomeError::new(
            ErrorCode::PackageInvalid,
            "restore package contains forbidden files",
        ));
    }
    validate_preserved_targets(&plan)?;
    let mut transaction_plan = plan.clone();
    if options.continuation_probe.is_some()
        && !plan.operations.iter().any(|operation| {
            operation.package_source == THREAD_METADATA_SOURCE && operation.rollback_required
        })
    {
        // Already-present sessions can omit a bridge write. The verifier still opens
        // SQLite, so back up the stored plan's database without adding a restore write.
        let target = plan
            .bridge_verification
            .sqlite_database
            .clone()
            .ok_or_else(|| {
                restore_failed(
                    "continuation verification requires a planned SQLite database to protect",
                )
            })?;
        transaction_plan.operations.push(PlannedOperation {
            package_source: THREAD_METADATA_SOURCE.into(),
            expected_previous_hash: hash_optional_file(&target)?,
            target,
            action: ChangeKind::Update,
            rollback_required: true,
        });
    }
    preflight_storage(
        &transaction_plan,
        &verified,
        &options.backup_root,
        available_space,
    )?;
    if options.continuation_probe.is_some() {
        verifier
            .as_deref_mut()
            .expect("validated verifier")
            .preflight()?;
    }
    let mut transaction = prepare_transaction(&transaction_plan, &options.backup_root)?;
    progress.update(RestoreProgressEvent::TransactionPrepared(
        transaction.journal.transaction_id,
    ));

    let result = apply_transaction(
        &plan,
        &options,
        &verified,
        &mut transaction,
        registrar,
        verifier,
        progress,
    );
    match result {
        Ok(report) => Ok(report),
        Err(error) if error.code == ErrorCode::CodexCleanupUnconfirmed => {
            progress.update(RestoreProgressEvent::RollbackFailed);
            Err(RehomeError::new(error.code, format!(
                "{}; automatic rollback not attempted while child termination is unconfirmed; journal and backups retained for manual recovery",
                error.message,
            )))
        }
        Err(error) => {
            progress.update(RestoreProgressEvent::Stage(MigrationJobStage::RollingBack));
            match rollback_prepared(&mut transaction) {
                Ok(_) => {
                    progress.update(RestoreProgressEvent::RollbackCompleted);
                    Err(error)
                }
                Err(rollback_error) => {
                    progress.update(RestoreProgressEvent::RollbackFailed);
                    Err(RehomeError::new(
                        ErrorCode::RollbackFailed,
                        format!(
                            "restore failed: {}; automatic rollback failed: {}",
                            error.message, rollback_error.message
                        ),
                    ))
                }
            }
        }
    }
}

fn preflight_storage(
    plan: &RestorePlan,
    verified: &VerifiedPackage,
    backup_root: &Path,
    available_space: &mut dyn FnMut(&Path) -> Result<u64, RehomeError>,
) -> Result<(), RehomeError> {
    let overflow = || {
        RehomeError::new(
            ErrorCode::DiskSpaceInsufficient,
            "restore storage estimate exceeds the supported range",
        )
    };
    let mut directories = BTreeSet::from([
        plan.target_codex_home.clone(),
        plan.projects_root.clone(),
        backup_root.to_path_buf(),
        std::env::temp_dir(),
    ]);
    let mut targets = BTreeSet::new();
    for operation in &plan.operations {
        let root = restore_target_root(
            &plan.target_codex_home,
            &plan.projects_root,
            &operation.package_source,
            &operation.target,
        )?;
        directories.insert(root.clone());
        if matches!(operation.action, ChangeKind::Add | ChangeKind::Update) {
            validate_restore_target(&root, &operation.target)?;
            targets.insert(operation.target.clone());
            if operation.package_source == THREAD_METADATA_SOURCE {
                for suffix in ["-wal", "-shm", "-journal"] {
                    let mut sidecar = operation.target.as_os_str().to_os_string();
                    sidecar.push(suffix);
                    let sidecar = PathBuf::from(sidecar);
                    validate_restore_target(&root, &sidecar)?;
                    targets.insert(sidecar);
                }
            }
        }
    }
    let mut old_bytes = 0_u64;
    for target in &targets {
        directories.insert(
            target
                .parent()
                .ok_or_else(|| restore_failed("restore target has no parent"))?
                .to_path_buf(),
        );
        match fs::symlink_metadata(target) {
            Ok(metadata) => {
                old_bytes = old_bytes.checked_add(metadata.len()).ok_or_else(overflow)?
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(restore_failed(format!(
                    "could not estimate restore backup: {error}"
                )))
            }
        }
    }
    let mut incoming_bytes = plan.required_bytes;
    for operation in plan.operations.iter().filter(|operation| {
        matches!(operation.action, ChangeKind::Add | ChangeKind::Update)
            && (operation.package_source == SESSION_INDEX_SOURCE
                || plan
                    .sessions
                    .iter()
                    .any(|session| session.package_source == operation.package_source))
    }) {
        let original = verified.authenticated_planning_payload(&operation.package_source)?;
        // Use the bridge's serializer: every matching row and JSON escape counts.
        // Shrinking output cannot reduce the original staging/copy allowance.
        let rewritten = rewrite_jsonl_payload(
            original,
            &plan.reference_rewrites,
            &operation.package_source,
        )?;
        incoming_bytes = incoming_bytes
            .checked_sub(original.len() as u64)
            .and_then(|bytes| bytes.checked_add(original.len().max(rewritten.len()) as u64))
            .ok_or_else(overflow)?;
    }
    // Conservative whole-job allowance at EACH ancestor: payload, old-target snapshots,
    // staging/atomic copies, SQLite growth, and per-target journal/checkpoint overhead.
    // This deliberately overcounts shared volumes; it is a preflight estimate, not a reservation.
    let required = incoming_bytes
        .checked_add(old_bytes)
        .and_then(|bytes| bytes.checked_mul(4))
        .and_then(|bytes| bytes.checked_add((targets.len() as u64).checked_mul(128 * 1024)?))
        .and_then(|bytes| bytes.checked_add(1024 * 1024))
        .ok_or_else(overflow)?;
    let ancestors = directories
        .iter()
        .map(|directory| existing_storage_directory(directory))
        .collect::<Result<BTreeSet<_>, _>>()?;
    // Complete the capacity checks before creating even a disposable writability probe.
    for directory in &ancestors {
        if available_space(directory)? < required {
            return Err(RehomeError::new(
                ErrorCode::DiskSpaceInsufficient,
                format!(
                    "restore requires at least {required} available bytes at {}",
                    directory.display()
                ),
            ));
        }
    }
    for directory in &ancestors {
        let pinned = PinnedParent::open(directory).map_err(|error| {
            restore_failed(format!("could not pin preflight directory: {error}"))
        })?;
        let name = format!(".enhe-restore-preflight-{}.tmp", Uuid::new_v4());
        let name = OsStr::new(&name);
        let mut probe = pinned.create_new_file(name).map_err(|error| {
            restore_failed(format!("restore directory is not writable: {error}"))
        })?;
        let written = probe
            .write_all(b"preflight\n")
            .and_then(|()| probe.sync_all());
        drop(probe);
        let removed = pinned.remove_file(name); // Attempt cleanup even if write/flush failed.
        written.map_err(|error| {
            restore_failed(format!("restore writability probe failed: {error}"))
        })?;
        removed.map_err(|error| {
            restore_failed(format!(
                "could not clean restore writability probe: {error}"
            ))
        })?;
    }
    for target in targets {
        if target.exists() {
            // Open without truncation; readonly metadata is not an effective-permission check.
            let parent = PinnedParent::open(target.parent().expect("validated parent")).map_err(
                |error| restore_failed(format!("could not pin restore target parent: {error}")),
            )?;
            drop(
                parent
                    .open_file_for_write(target.file_name().expect("validated file"))
                    .map_err(|error| {
                        restore_failed(format!("restore target is not writable: {error}"))
                    })?,
            );
        }
    }
    Ok(())
}

fn existing_storage_directory(directory: &Path) -> Result<PathBuf, RehomeError> {
    if !directory.is_absolute()
        || directory
            .components()
            .any(|part| part == Component::ParentDir)
    {
        return Err(restore_failed(
            "storage preflight requires an absolute safe directory",
        ));
    }
    validate_restore_target_ancestry(directory, &directory.join(".enhe-preflight"))?;
    for ancestor in directory.ancestors() {
        match fs::symlink_metadata(ancestor) {
            Ok(_) => {
                return fs::canonicalize(ancestor).map_err(|error| {
                    restore_failed(format!("could not resolve preflight directory: {error}"))
                })
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(restore_failed(format!(
                    "could not inspect preflight directory: {error}"
                )))
            }
        }
    }
    Err(restore_failed("storage directory has no existing ancestor"))
}

#[cfg(windows)]
fn available_disk_space(directory: &Path) -> Result<u64, RehomeError> {
    use std::os::windows::ffi::OsStrExt;
    let path: Vec<u16> = directory.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut available = 0_u64;
    // SAFETY: path is NUL-terminated and the sole output pointer is a live u64.
    let success = unsafe {
        windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW(
            path.as_ptr(),
            &mut available,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if success == 0 {
        return Err(restore_failed(format!(
            "could not query available restore space: {}",
            io::Error::last_os_error()
        )));
    }
    Ok(available)
}

#[cfg(unix)]
fn available_disk_space(directory: &Path) -> Result<u64, RehomeError> {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};
    let path = CString::new(directory.as_os_str().as_bytes())
        .map_err(|_| restore_failed("storage directory contains NUL"))?;
    let mut stats = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    // SAFETY: valid C path and writable statvfs; read only after success.
    if unsafe { libc::statvfs(path.as_ptr(), stats.as_mut_ptr()) } != 0 {
        return Err(restore_failed(format!(
            "could not query available restore space: {}",
            io::Error::last_os_error()
        )));
    }
    let stats = unsafe { stats.assume_init() };
    (stats.f_bavail as u64)
        .checked_mul(stats.f_frsize as u64)
        .ok_or_else(|| restore_failed("available restore space exceeds the supported range"))
}

#[cfg(not(any(windows, unix)))]
fn available_disk_space(_directory: &Path) -> Result<u64, RehomeError> {
    Err(restore_failed(
        "native available-space preflight is unsupported on this host",
    ))
}

pub fn rollback(transaction_id: Uuid) -> Result<RollbackReport, RehomeError> {
    crate::core::backup::rollback(transaction_id)
}

pub fn list_transactions() -> Result<Vec<TransactionSummary>, RehomeError> {
    crate::core::backup::list_transactions()
}

pub fn list_transaction_history() -> Result<TransactionHistory, RehomeError> {
    crate::core::backup::list_transaction_history()
}

pub fn transaction_summary(
    transaction_id: Uuid,
) -> Result<Option<TransactionSummary>, RehomeError> {
    crate::core::backup::transaction_summary(transaction_id)
}

pub fn recover_incomplete_transactions() -> Result<Vec<PendingRecovery>, RehomeError> {
    crate::core::backup::recover_incomplete_transactions()
}

fn apply_transaction(
    plan: &RestorePlan,
    options: &RestoreOptions,
    verified: &VerifiedPackage,
    transaction: &mut PreparedTransaction,
    registrar: &mut dyn FnMut(SourceOs, &Path) -> RegistrationStatus,
    verifier: Option<&mut dyn CodexAccessVerifier>,
    progress: &mut dyn RestoreProgressSink,
) -> Result<RestoreReport, RehomeError> {
    update_status(transaction, RecoveryStatus::Applying)?;
    progress.update(RestoreProgressEvent::Stage(
        MigrationJobStage::RestoringFiles,
    ));
    let transaction_id = transaction.journal.transaction_id;
    let (mut restored_files, mut restored_bytes) =
        apply_regular_files(plan, verified, transaction)?;
    let bridge = apply_bridge_plan_for_transaction(plan, transaction_id, |target| {
        record_applied_mutation(transaction, target)
    })
    .map_err(|error| {
        if plan
            .operations
            .iter()
            .any(|operation| operation.package_source == THREAD_METADATA_SOURCE)
        {
            RehomeError::new(
                error.code,
                format!("SQLite/index bridge update failed: {}", error.message),
            )
        } else {
            error
        }
    })?;
    restored_files += (bridge.sessions_written
        + bridge.index_entries_merged
        + bridge.sqlite_threads_imported) as u64;
    restored_bytes += changed_target_bytes(plan)?;

    update_status(transaction, RecoveryStatus::Verifying)?;
    let file_result = verify_restore(plan, verified);
    // SQLite reads may create WAL sidecars even when verification returns an error.
    let checkpoint = refresh_app_server_mutation_checkpoints(transaction);
    let mut verification = checkpointed_result(file_result, checkpoint)?;
    if !data_verification_passed(&verification) {
        return Err(restore_failed(format!(
            "restore verification did not pass: {verification:?}"
        )));
    }
    progress.update(RestoreProgressEvent::Stage(
        MigrationJobStage::FilesVerified,
    ));
    if let Some(probe) = &options.continuation_probe {
        let request = CodexAccessRequest {
            codex_home: plan.target_codex_home.clone(),
            required_thread_ids: plan
                .sessions
                .iter()
                .map(|session| session.target_task_id)
                .collect(),
            probe_thread_id: probe.probe_thread_id,
        };
        let mut recognized = false;
        progress.update(RestoreProgressEvent::Stage(
            MigrationJobStage::RecognizingThreads,
        ));
        let result = verifier
            .expect("validated verifier")
            .verify(&request, &mut || {
                if !recognized {
                    recognized = true;
                    progress.update(RestoreProgressEvent::Stage(
                        MigrationJobStage::ProbingContinuation,
                    ));
                }
            });
        if let Err(error) = &result {
            if error.code == ErrorCode::CodexCleanupUnconfirmed {
                return Err(error.clone());
            }
        }
        // Confirmed shutdown permits checkpoints on BOTH success and ordinary error paths.
        let checkpoint = refresh_app_server_mutation_checkpoints(transaction);
        let access = checkpointed_result(result, checkpoint)?;
        if !recognized
            || !access.threads_recognized
            || !access.continuation_probe_valid
            || !access.ephemeral_fork
            || access.required_threads != request.required_thread_ids.len() as u64
            || access.recognized_threads != request.required_thread_ids.len() as u64
            || access.probe_thread_id != Some(probe.probe_thread_id)
        {
            return Err(RehomeError::new(ErrorCode::CodexVerificationFailed,
                "Codex did not prove recognition and ephemeral continuation for the planned threads"));
        }
        verification.codex_access = access;
    }
    ensure_applied_states(transaction)?;
    progress.update(RestoreProgressEvent::Stage(MigrationJobStage::Committing));
    update_status(transaction, RecoveryStatus::Committed)?;
    let registrations = register_projects(plan, options, verified, registrar);
    verification.app_registration_valid = options.register_projects
        && !registrations.is_empty()
        && registrations
            .iter()
            .all(|result| result.status == RegistrationStatus::Registered);
    verification.app_visible_ready = verification.codex_access.threads_recognized
        && verification.codex_access.continuation_probe_valid;
    progress.update(RestoreProgressEvent::Stage(MigrationJobStage::Finished));

    Ok(RestoreReport {
        transaction_id: transaction.journal.transaction_id,
        package_id: plan.package_id,
        completed_at: timestamp(),
        restored_files,
        restored_bytes,
        registrations,
        verification,
    })
}

fn refresh_app_server_mutation_checkpoints(
    transaction: &mut PreparedTransaction,
) -> Result<(), RehomeError> {
    if let Some(target) = transaction
        .journal
        .operations
        .iter()
        .find(|operation| operation.package_source == THREAD_METADATA_SOURCE)
        .map(|operation| operation.target.clone())
    {
        record_applied_mutation(transaction, &target)?;
    }
    Ok(())
}

fn checkpointed_result<T>(
    result: Result<T, RehomeError>,
    checkpoint: Result<(), RehomeError>,
) -> Result<T, RehomeError> {
    match (result, checkpoint) {
        (result, Ok(())) => result,
        (Ok(_), Err(error)) => Err(restore_failed(format!(
            "verification checkpoint failed: {}",
            error.message
        ))),
        (Err(error), Err(checkpoint)) => Err(RehomeError::new(
            error.code,
            format!(
                "{}; verification checkpoint failed: {}",
                error.message, checkpoint.message
            ),
        )),
    }
}

fn validate_plan(plan: &RestorePlan) -> Result<(), RehomeError> {
    if !plan.package_path.is_absolute()
        || !plan.target_codex_home.is_absolute()
        || !plan.projects_root.is_absolute()
    {
        return Err(restore_failed("restore plan paths must be absolute"));
    }
    if plan.conflict_count > 0
        || plan
            .operations
            .iter()
            .any(|operation| operation.action == ChangeKind::Conflict)
    {
        return Err(RehomeError::new(
            ErrorCode::ProjectConflict,
            "restore plan contains unresolved conflicts",
        ));
    }
    if plan.target_codex_home.starts_with(&plan.projects_root)
        || plan.projects_root.starts_with(&plan.target_codex_home)
    {
        return Err(restore_failed("restore roots must not overlap"));
    }
    Ok(())
}

fn validate_package_identity(
    plan: &RestorePlan,
    verified: &VerifiedPackage,
) -> Result<(), RehomeError> {
    if verified.preview.manifest.package_id != plan.package_id {
        return Err(RehomeError::new(
            ErrorCode::PackageInvalid,
            "restore plan package ID does not match the package",
        ));
    }
    if !verified
        .preview
        .archive_hash
        .eq_ignore_ascii_case(&plan.archive_hash)
    {
        return Err(RehomeError::new(
            ErrorCode::PackageInvalid,
            "restore plan archive hash does not match the package",
        ));
    }
    for operation in &plan.operations {
        if !verified.payloads.contains_key(&operation.package_source) {
            return Err(RehomeError::new(
                ErrorCode::PackageInvalid,
                format!(
                    "restore operation references a missing package payload: {}",
                    operation.package_source
                ),
            ));
        }
    }
    Ok(())
}

fn apply_regular_files(
    plan: &RestorePlan,
    verified: &VerifiedPackage,
    transaction: &mut PreparedTransaction,
) -> Result<(u64, u64), RehomeError> {
    let mut restored_files = 0_u64;
    let mut restored_bytes = 0_u64;
    let mut payload_archive = verified.open_payload_archive()?;
    for operation in &plan.operations {
        if !matches!(operation.action, ChangeKind::Add | ChangeKind::Update)
            || is_bridge_operation(plan, &operation.package_source)
        {
            continue;
        }
        let mut staged = NamedTempFile::new().map_err(|error| {
            restore_failed(format!("could not stage restored payload: {error}"))
        })?;
        let bytes =
            payload_archive.write_payload(&operation.package_source, staged.as_file_mut())?;
        staged.as_file().sync_all().map_err(|error| {
            restore_failed(format!("could not flush restored payload: {error}"))
        })?;
        let root = restore_target_root(
            &plan.target_codex_home,
            &plan.projects_root,
            &operation.package_source,
            &operation.target,
        )?;
        apply_file_source_for_transaction(
            &root,
            operation,
            staged.path(),
            transaction.journal.transaction_id,
        )?;
        record_applied_mutation(transaction, &operation.target)?;
        restored_files += 1;
        restored_bytes = restored_bytes
            .checked_add(bytes)
            .ok_or_else(|| restore_failed("restored byte count overflowed"))?;
    }
    Ok((restored_files, restored_bytes))
}

fn is_bridge_operation(plan: &RestorePlan, source: &str) -> bool {
    source == SESSION_INDEX_SOURCE
        || source == THREAD_METADATA_SOURCE
        || plan
            .sessions
            .iter()
            .any(|session| session.package_source == source)
}

fn changed_target_bytes(plan: &RestorePlan) -> Result<u64, RehomeError> {
    plan.operations
        .iter()
        .filter(|operation| {
            matches!(operation.action, ChangeKind::Add | ChangeKind::Update)
                && is_bridge_operation(plan, &operation.package_source)
        })
        .try_fold(0_u64, |total, operation| {
            match fs::metadata(&operation.target) {
                Ok(metadata) if metadata.is_file() => total
                    .checked_add(metadata.len())
                    .ok_or_else(|| restore_failed("restored byte count overflowed")),
                Ok(_) => Err(restore_failed("restored target is not a regular file")),
                Err(error) => Err(restore_failed(format!(
                    "could not inspect restored target {}: {error}",
                    operation.target.display()
                ))),
            }
        })
}

fn verify_restore(
    plan: &RestorePlan,
    verified: &VerifiedPackage,
) -> Result<VerificationReport, RehomeError> {
    let current = inspect_package_for_planning(&plan.package_path)?;
    let package_checksum_valid = current.preview.checksum_valid
        && current.preview.manifest.package_id == plan.package_id
        && current
            .preview
            .archive_hash
            .eq_ignore_ascii_case(&plan.archive_hash);
    let files_valid = verify_plain_files(plan, verified)?;
    let sessions_valid = plan.sessions.iter().try_fold(true, |valid, session| {
        Ok::<_, RehomeError>(
            valid
                && hash_optional_file(&session.target)?.is_some_and(|hash| {
                    hash.eq_ignore_ascii_case(&session.expected_final_content_hash)
                }),
        )
    })?;
    let bridge = verify_bridge_metadata(plan)?;
    let forbidden_files_absent = current.preview.forbidden_files_total == 0;
    let project_files_valid = verify_project_files(plan, verified)?;
    Ok(VerificationReport {
        package_checksum_valid,
        files_valid,
        sessions_valid,
        session_index_valid: bridge.session_index_valid,
        sqlite_threads_valid: bridge.sqlite_threads_valid,
        path_mapping_valid: bridge.path_mapping_valid,
        forbidden_files_absent,
        project_files_valid,
        app_registration_valid: false,
        app_visible_ready: false,
        codex_access: CodexAccessVerification::default(),
    })
}

fn verify_plain_files(plan: &RestorePlan, verified: &VerifiedPackage) -> Result<bool, RehomeError> {
    for operation in &plan.operations {
        if operation.action == ChangeKind::Conflict
            || is_bridge_operation(plan, &operation.package_source)
        {
            continue;
        }
        if operation.action == ChangeKind::Preserve {
            if !preserved_target_matches(operation)? {
                return Ok(false);
            }
            continue;
        }
        let expected = &verified
            .payloads
            .get(&operation.package_source)
            .ok_or_else(|| restore_failed("verified payload metadata is missing"))?
            .content_hash;
        if !hash_optional_file(&operation.target)?
            .is_some_and(|actual| actual.eq_ignore_ascii_case(expected))
        {
            return Ok(false);
        }
    }
    Ok(true)
}

fn validate_preserved_targets(plan: &RestorePlan) -> Result<(), RehomeError> {
    for operation in plan
        .operations
        .iter()
        .filter(|operation| operation.action == ChangeKind::Preserve)
    {
        if !preserved_target_matches(operation)? {
            return Err(restore_failed(format!(
                "preserved target changed after planning: {}",
                operation.target.display()
            )));
        }
    }
    Ok(())
}

fn preserved_target_matches(
    operation: &crate::core::models::PlannedOperation,
) -> Result<bool, RehomeError> {
    let metadata = match fs::symlink_metadata(&operation.target) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(operation.expected_previous_hash.is_none())
        }
        Err(error) => {
            return Err(restore_failed(format!(
                "could not inspect preserved target {}: {error}",
                operation.target.display()
            )))
        }
    };
    if metadata_is_link_or_reparse(&metadata) || !metadata.is_file() {
        return Ok(false);
    }
    let Some(expected) = operation.expected_previous_hash.as_deref() else {
        return Ok(false);
    };
    Ok(hash_optional_file(&operation.target)?
        .is_some_and(|actual| actual.eq_ignore_ascii_case(expected)))
}

fn verify_project_files(
    plan: &RestorePlan,
    verified: &VerifiedPackage,
) -> Result<bool, RehomeError> {
    for operation in plan
        .operations
        .iter()
        .filter(|operation| operation.target.starts_with(&plan.projects_root))
        .filter(|operation| operation.action != ChangeKind::Conflict)
    {
        let expected = &verified
            .payloads
            .get(&operation.package_source)
            .ok_or_else(|| restore_failed("project payload metadata is missing"))?
            .content_hash;
        if !hash_optional_file(&operation.target)?
            .is_some_and(|actual| actual.eq_ignore_ascii_case(expected))
        {
            return Ok(false);
        }
    }
    Ok(true)
}

struct BridgeVerification {
    session_index_valid: bool,
    sqlite_threads_valid: bool,
    path_mapping_valid: bool,
}

fn verify_bridge_metadata(plan: &RestorePlan) -> Result<BridgeVerification, RehomeError> {
    let index_rows = read_index_rows(plan)?;
    let sqlite_rows = read_sqlite_rows(plan)?;
    let requires_index = plan.bridge_verification.session_index.is_some();
    let requires_sqlite = plan.bridge_verification.sqlite_database.is_some();
    let mut index_valid = !requires_index;
    let mut sqlite_valid = !requires_sqlite;
    let mut mapping_valid = true;

    if requires_index {
        index_valid = plan.sessions.iter().all(|session| {
            index_rows
                .get(&session.target_task_id.to_string())
                .is_some_and(|row| {
                    row.get("rollout_path").and_then(Value::as_str) == session.target.to_str()
                })
        });
    }
    if requires_sqlite {
        sqlite_valid = plan.sessions.iter().all(|session| {
            sqlite_rows
                .get(&session.target_task_id.to_string())
                .is_some_and(|(_, rollout)| rollout.as_deref() == session.target.to_str())
        });
    }

    for session in &plan.sessions {
        let expected_project_paths = plan
            .reference_rewrites
            .iter()
            .filter(|rewrite| {
                rewrite.source_task_id == session.source_task_id
                    && rewrite.kind == ReferenceRewriteKind::ProjectPath
                    && rewrite.package_source == session.package_source
            })
            .map(|rewrite| rewrite.to.as_str())
            .collect::<Vec<_>>();
        if expected_project_paths.is_empty() {
            continue;
        }
        let session_bytes = fs::read(&session.target).map_err(|error| {
            restore_failed(format!(
                "could not read restored session for verification: {error}"
            ))
        })?;
        let session_values = parse_jsonl_values(&session_bytes)?;
        let index_cwd = index_rows
            .get(&session.target_task_id.to_string())
            .and_then(|row| row.get("cwd"))
            .and_then(Value::as_str);
        let sqlite_cwd = sqlite_rows
            .get(&session.target_task_id.to_string())
            .and_then(|(cwd, _)| cwd.as_deref());
        let mapped = expected_project_paths.iter().any(|expected| {
            session_values
                .iter()
                .any(|value| json_contains_string(value, expected))
                // Some Codex versions use a minimal session_index row containing only
                // id/title/timestamps/rollout_path. In that schema the authoritative
                // project binding is the session metadata plus the SQLite thread row;
                // do not require a cwd field that the index does not expose.
                && (!requires_index || index_cwd.is_none_or(|cwd| cwd == *expected))
                && (!requires_sqlite || sqlite_cwd == Some(*expected))
        });
        mapping_valid &= mapped;
    }

    Ok(BridgeVerification {
        session_index_valid: index_valid,
        sqlite_threads_valid: sqlite_valid,
        path_mapping_valid: mapping_valid,
    })
}

fn parse_jsonl_values(bytes: &[u8]) -> Result<Vec<Value>, RehomeError> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| restore_failed("restored session JSONL is not UTF-8"))?;
    text.lines()
        .filter(|line| !line.is_empty())
        .map(|line| {
            serde_json::from_str(line).map_err(|error| {
                restore_failed(format!("restored session JSONL is invalid: {error}"))
            })
        })
        .collect()
}

fn json_contains_string(value: &Value, expected: &str) -> bool {
    match value {
        Value::String(value) => value == expected,
        Value::Array(values) => values
            .iter()
            .any(|value| json_contains_string(value, expected)),
        Value::Object(values) => values
            .values()
            .any(|value| json_contains_string(value, expected)),
        _ => false,
    }
}

fn read_index_rows(plan: &RestorePlan) -> Result<BTreeMap<String, Value>, RehomeError> {
    let Some(target) = plan.bridge_verification.session_index.as_deref() else {
        return Ok(BTreeMap::new());
    };
    let bytes = fs::read(target).map_err(|error| {
        restore_failed(format!("could not read restored session index: {error}"))
    })?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| restore_failed("restored session index is not UTF-8"))?;
    let mut rows = BTreeMap::new();
    for line in text.lines().filter(|line| !line.is_empty()) {
        let value: Value = serde_json::from_str(line).map_err(|error| {
            restore_failed(format!("restored session index is invalid: {error}"))
        })?;
        if let Some(id) = value.get("id").and_then(Value::as_str) {
            rows.insert(id.to_owned(), value);
        }
    }
    Ok(rows)
}

type SqliteRows = BTreeMap<String, (Option<String>, Option<String>)>;

fn read_sqlite_rows(plan: &RestorePlan) -> Result<SqliteRows, RehomeError> {
    let Some(target) = plan.bridge_verification.sqlite_database.as_deref() else {
        return Ok(BTreeMap::new());
    };
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let connection = Connection::open_with_flags(target, flags).map_err(|error| {
        restore_failed(format!("could not open restored SQLite database: {error}"))
    })?;
    let mut statement = connection
        .prepare("SELECT id, cwd, rollout_path FROM threads")
        .map_err(|error| {
            restore_failed(format!("could not inspect restored SQLite rows: {error}"))
        })?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })
        .map_err(|error| {
            restore_failed(format!("could not query restored SQLite rows: {error}"))
        })?;
    let mut result = BTreeMap::new();
    for row in rows {
        let (id, cwd, rollout) = row.map_err(|error| {
            restore_failed(format!("could not read restored SQLite row: {error}"))
        })?;
        result.insert(id, (cwd, rollout));
    }
    Ok(result)
}

fn register_projects(
    plan: &RestorePlan,
    options: &RestoreOptions,
    verified: &VerifiedPackage,
    registrar: &mut dyn FnMut(SourceOs, &Path) -> RegistrationStatus,
) -> Vec<ProjectRegistration> {
    if !options.register_projects {
        return Vec::new();
    }
    let target_os = if cfg!(target_os = "macos") {
        SourceOs::Macos
    } else {
        SourceOs::Windows
    };
    verified
        .preview
        .manifest
        .projects
        .iter()
        .map(|project| {
            let project_path = plan.projects_root.join(&project.name);
            let status = registrar(target_os, &project_path);
            ProjectRegistration {
                project_id: project.project_id,
                project_path,
                status,
            }
        })
        .collect()
}

fn data_verification_passed(report: &VerificationReport) -> bool {
    report.package_checksum_valid
        && report.files_valid
        && report.sessions_valid
        && report.session_index_valid
        && report.sqlite_threads_valid
        && report.path_mapping_valid
        && report.forbidden_files_absent
        && report.project_files_valid
}

fn hash_optional_file(path: &Path) -> Result<Option<String>, RehomeError> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(restore_failed(format!(
                "could not read restored file {}: {error}",
                path.display()
            )))
        }
    };
    Ok(Some(format!("{:x}", Sha256::digest(bytes))))
}

#[cfg(windows)]
fn metadata_is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_type().is_symlink()
        || metadata.file_attributes()
            & windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT
            != 0
}

#[cfg(not(windows))]
fn metadata_is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

fn timestamp() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn restore_failed(message: impl Into<String>) -> RehomeError {
    RehomeError::new(ErrorCode::RestoreFailed, message)
}

#[cfg(test)]
#[allow(dead_code)]
#[path = "../../tests/common/mod.rs"]
mod preflight_fixtures;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{
        models::{ContentCounts, CreatePackageRequest, PlannedOperation, TargetInventory},
        package::{create_package, inspect_package},
        planner::build_restore_plan,
    };
    use std::collections::BTreeSet;

    static APP_DATA_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn preflight_storage(
        plan: &RestorePlan,
        backup_root: &Path,
        available_space: &mut dyn FnMut(&Path) -> Result<u64, RehomeError>,
    ) -> Result<(), RehomeError> {
        super::preflight_storage(
            plan,
            &inspect_package_for_planning(&plan.package_path)?,
            backup_root,
            available_space,
        )
    }

    fn storage_fixture() -> (
        preflight_fixtures::SyntheticCodexFixture,
        RestorePlan,
        RestoreOptions,
    ) {
        storage_fixture_with_rows(0, "/p")
    }

    fn storage_fixture_with_rows(
        rows: usize,
        cwd: &str,
    ) -> (
        preflight_fixtures::SyntheticCodexFixture,
        RestorePlan,
        RestoreOptions,
    ) {
        let fixture = preflight_fixtures::synthetic_codex_fixture().unwrap();
        if rows > 0 {
            let mut session = fs::OpenOptions::new()
                .append(true)
                .open(&fixture.session_path)
                .unwrap();
            let row = format!(
                "{}\n",
                serde_json::json!({"type": "turn_context", "payload": {"cwd": cwd}})
            );
            session.write_all(row.repeat(rows).as_bytes()).unwrap();
        }
        let package = fixture.root.join("synthetic.rehome");
        create_package(CreatePackageRequest {
            codex_home: fixture.codex_home.clone(),
            project_paths: vec![fixture.project_path.clone()],
            conversation_ids: vec![Uuid::parse_str(preflight_fixtures::THREAD_ID).unwrap()],
            output_path: package.clone(),
            source_device_id: Uuid::nil(),
            skill_paths: vec![],
            plugin_paths: vec![],
            generated_image_paths: vec![],
        })
        .unwrap();
        let codex_home = fixture.root.join("target/.codex");
        fs::create_dir_all(&codex_home).unwrap();
        fs::copy(&fixture.state_db_path, codex_home.join("state_5.sqlite")).unwrap();
        Connection::open(codex_home.join("state_5.sqlite"))
            .unwrap()
            .execute("DELETE FROM threads", [])
            .unwrap();
        let plan = build_restore_plan(
            &inspect_package(&package).unwrap(),
            &TargetInventory {
                codex_home,
                target_os: SourceOs::Windows,
                target_arch: "x86_64".into(),
                counts: ContentCounts::default(),
                projects: vec![],
                conversations: vec![],
            },
            &fixture.root.join("target/projects"),
        )
        .unwrap();
        let options = RestoreOptions {
            codex_closed_confirmed: true,
            backup_root: fixture.root.join("backup"),
            register_projects: false,
            continuation_probe: None,
        };
        (fixture, plan, options)
    }

    #[derive(Default)]
    struct Events(Vec<RestoreProgressEvent>);
    impl RestoreProgressSink for Events {
        fn update(&mut self, event: RestoreProgressEvent) {
            self.0.push(event);
        }
    }

    #[test]
    fn storage_preflight_insufficient_space_precedes_transaction_and_target_writes() {
        let _lock = APP_DATA_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let (fixture, plan, options) = storage_fixture();
        let before = fs::read(plan.target_codex_home.join("state_5.sqlite")).unwrap();
        let old_app_data = std::env::var_os("LOCALAPPDATA");
        std::env::set_var("LOCALAPPDATA", fixture.root.join("app-data"));
        let mut events = Events::default();
        let mut queried = Vec::new();
        let result = apply_server_plan(
            plan.clone(),
            options.clone(),
            &mut |_, _| unreachable!(),
            None,
            &mut events,
            &mut |path| {
                queried.push(path.to_owned());
                Ok(0)
            },
        );
        if let Some(old) = old_app_data {
            std::env::set_var("LOCALAPPDATA", old);
        } else {
            std::env::remove_var("LOCALAPPDATA");
        }
        let error = result.unwrap_err();
        assert_eq!(error.code, ErrorCode::DiskSpaceInsufficient);
        assert!(!queried.is_empty());
        assert_eq!(
            events.0,
            [RestoreProgressEvent::Stage(MigrationJobStage::Preflight)]
        );
        assert!(!options.backup_root.exists());
        assert!(!fixture.root.join("app-data").exists());
        assert_eq!(fs::read_dir(&plan.target_codex_home).unwrap().count(), 1);
        assert_eq!(
            fs::read(plan.target_codex_home.join("state_5.sqlite")).unwrap(),
            before
        );
        assert!(!plan.projects_root.exists());
    }

    #[test]
    fn repeated_rewrite_space_rejects_before_transaction_or_target_changes() {
        let _lock = APP_DATA_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let (_fixture, mut plan, options) = storage_fixture_with_rows(50_000, "/p");
        let to = format!("/{}", "p".repeat(222));
        plan.reference_rewrites
            .push(crate::core::models::ReferenceRewrite {
                source_task_id: plan.sessions[0].source_task_id,
                package_source: plan.sessions[0].package_source.clone(),
                kind: ReferenceRewriteKind::ProjectPath,
                from: "/p".into(),
                to,
            });
        // Each of 50,000 rows gains 221 ASCII bytes. This lower bound excludes
        // the session header and other payloads; the output alone cannot fit.
        let rewritten_rows = 50_000
            * (b"{\"type\":\"turn_context\",\"payload\":{\"cwd\":\"/p\"}}\n".len() as u64 + 221);
        let available = 12_000_000;
        assert!(plan.required_bytes < available);
        assert!(rewritten_rows > available);
        let before = fs::read(plan.target_codex_home.join("state_5.sqlite")).unwrap();
        let local_app_data = tempfile::tempdir().unwrap();
        let application_root = local_app_data.path().join("com.rehome.desktop");
        let old_app_data = std::env::var_os("LOCALAPPDATA");
        std::env::set_var("LOCALAPPDATA", local_app_data.path());
        let mut events = Events::default();
        let result = apply_server_plan(
            plan.clone(),
            options.clone(),
            &mut |_, _| unreachable!(),
            None,
            &mut events,
            &mut |_| Ok(available),
        );
        if let Some(old) = old_app_data {
            std::env::set_var("LOCALAPPDATA", old);
        } else {
            std::env::remove_var("LOCALAPPDATA");
        }
        assert_eq!(result.unwrap_err().code, ErrorCode::DiskSpaceInsufficient);
        assert_eq!(
            events.0,
            [RestoreProgressEvent::Stage(MigrationJobStage::Preflight)]
        );
        assert!(!options.backup_root.exists());
        assert!(!application_root.exists());
        assert!(!plan.projects_root.exists());
        assert!(!plan.sessions[0].target.exists());
        assert_eq!(
            fs::read(plan.target_codex_home.join("state_5.sqlite")).unwrap(),
            before
        );
    }

    #[test]
    fn shrinking_rewrite_keeps_original_staging_and_copy_budget() {
        let original_cwd = format!("/{}", "p".repeat(222));
        let (_fixture, mut plan, options) = storage_fixture_with_rows(5_000, &original_cwd);
        plan.reference_rewrites
            .push(crate::core::models::ReferenceRewrite {
                source_task_id: plan.sessions[0].source_task_id,
                package_source: plan.sessions[0].package_source.clone(),
                kind: ReferenceRewriteKind::ProjectPath,
                from: original_cwd,
                to: "/p".into(),
            });
        // Even when serialized output shrinks, original staging + copy allowance
        // plus the fixed floor still leaves no room for old targets and metadata.
        let available = plan.required_bytes * 4 + 1024 * 1024;
        let error =
            preflight_storage(&plan, &options.backup_root, &mut |_| Ok(available)).unwrap_err();
        assert_eq!(error.code, ErrorCode::DiskSpaceInsufficient);
        assert!(!options.backup_root.exists());
        assert!(!plan.projects_root.exists());
        assert!(!plan.sessions[0].target.exists());
    }

    #[test]
    fn storage_preflight_checks_all_roots_parents_backup_and_staging_without_leaks() {
        let (fixture, mut plan, mut options) = storage_fixture();
        fs::create_dir_all(&plan.projects_root).unwrap();
        let skills = fixture.root.join("target/.agents/skills");
        fs::create_dir_all(skills.join("shared")).unwrap();
        let sentinel = skills.join("shared/SKILL.md");
        fs::write(&sentinel, b"original shared skill").unwrap();
        plan.operations.push(PlannedOperation {
            package_source: "agents/skills/shared/SKILL.md".into(),
            target: sentinel.clone(),
            expected_previous_hash: None,
            action: ChangeKind::Update,
            rollback_required: true,
        });
        let backups = fixture.root.join("existing-backups");
        fs::create_dir(&backups).unwrap();
        options.backup_root = backups.join("missing/backup");
        let mut queried = BTreeSet::new();
        preflight_storage(&plan, &options.backup_root, &mut |path| {
            queried.insert(path.to_path_buf());
            Ok(u64::MAX)
        })
        .unwrap();
        for expected in [
            &plan.target_codex_home,
            &plan.projects_root,
            &skills,
            &skills.join("shared"),
            &backups,
            &std::env::temp_dir(),
        ] {
            assert!(
                queried.contains(&fs::canonicalize(expected).unwrap()),
                "missing query: {expected:?}"
            );
        }
        assert_eq!(fs::read(&sentinel).unwrap(), b"original shared skill");
        assert_eq!(fs::read_dir(skills.join("shared")).unwrap().count(), 1);
        assert_eq!(fs::read_dir(backups).unwrap().count(), 0);
        assert!(!options.backup_root.exists());
        assert_eq!(fs::read_dir(&plan.target_codex_home).unwrap().count(), 1);
        assert_eq!(fs::read_dir(&plan.projects_root).unwrap().count(), 0);
    }

    #[test]
    fn storage_preflight_budget_includes_backup_and_temporary_copy_capacity() {
        let (_fixture, plan, options) = storage_fixture();
        // Raw incoming bytes alone cannot cover staging, atomic replacement and rollback copies.
        let error = preflight_storage(
            &plan,
            &options.backup_root,
            &mut |_| Ok(plan.required_bytes),
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::DiskSpaceInsufficient);
        assert!(!options.backup_root.exists());
    }

    #[test]
    fn storage_preflight_rejects_overflow_before_query_or_write() {
        let (_fixture, mut plan, options) = storage_fixture();
        plan.required_bytes = u64::MAX;
        let error = preflight_storage(&plan, &options.backup_root, &mut |_| {
            panic!("overflow must fail before query")
        })
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::DiskSpaceInsufficient);
        assert!(!options.backup_root.exists());
    }

    #[test]
    fn storage_preflight_propagates_native_query_failure() {
        let (_fixture, plan, options) = storage_fixture();
        let error = preflight_storage(&plan, &options.backup_root, &mut |_| {
            Err(restore_failed("synthetic native query failure"))
        })
        .unwrap_err();
        assert!(error.message.contains("synthetic native query failure"));
        assert!(!options.backup_root.exists());
    }

    #[test]
    fn storage_preflight_refuses_file_as_backup_ancestor_without_truncation() {
        let (fixture, plan, _) = storage_fixture();
        let blocker = fixture.root.join("blocker");
        fs::write(&blocker, b"keep me").unwrap();
        assert!(preflight_storage(&plan, &blocker.join("backups"), &mut |_| Ok(u64::MAX)).is_err());
        assert_eq!(fs::read(blocker).unwrap(), b"keep me");
    }

    #[cfg(windows)]
    #[test]
    fn storage_preflight_readonly_target_fails_without_changing_or_truncating_it() {
        let (_fixture, plan, options) = storage_fixture();
        let target = &plan.operations[0].target;
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(target, b"do not truncate").unwrap();
        let writable = fs::metadata(target).unwrap().permissions();
        let mut readonly = writable.clone();
        readonly.set_readonly(true);
        fs::set_permissions(target, readonly).unwrap();
        let result = preflight_storage(&plan, &options.backup_root, &mut |_| Ok(u64::MAX));
        fs::set_permissions(target, writable).unwrap();
        assert!(result.is_err());
        assert_eq!(fs::read(target).unwrap(), b"do not truncate");
        assert!(!options.backup_root.exists());
    }

    #[test]
    fn storage_preflight_native_query_reports_available_bytes() {
        let temporary = tempfile::tempdir().unwrap();
        let available = available_disk_space(temporary.path()).unwrap();
        assert!(available > 0 && available < u64::MAX);
    }

    #[test]
    fn probe_rejects_empty_stored_sessions_before_transaction() {
        let (_fixture, mut plan, mut options) = storage_fixture();
        plan.sessions.clear();
        crate::core::plan_store::store(&plan).unwrap();
        options.continuation_probe = Some(crate::core::models::ContinuationProbeOptions {
            probe_thread_id: Uuid::new_v4(),
            online_usage_confirmed: true,
        });
        let error = apply_restore_with_services(
            plan,
            options.clone(),
            &mut |_, _| unreachable!(),
            None,
            &mut NoProgress,
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::CodexVerificationFailed);
        assert!(!options.backup_root.exists());
    }
}
