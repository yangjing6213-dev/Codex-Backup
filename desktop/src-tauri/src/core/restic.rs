use crate::core::error::{ErrorCode, RehomeError};
use crate::core::local_discovery::{has_redirect_ancestor, is_filesystem_redirect};
use chrono::{SecondsFormat, Utc};
use rusqlite::{backup::Backup, Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs::{self, File},
    io::{self, Read},
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};
use tempfile::{Builder, NamedTempFile};
use uuid::Uuid;
use walkdir::WalkDir;

pub const BACKUP_FORMAT: &str = "enhe-codex-backup";
pub const BACKUP_SCHEMA_VERSION: u32 = 1;
const PAYLOAD_ROOT: &str = "enhe-payload";
const SQLITE_SIDECAR_ROOT: &str = ".enhe-sqlite-sidecars";
const RESTIC_TAG: &str = "enhe-codex-backup";
const HASH_BUFFER_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalBackupRequest {
    pub codex_home: PathBuf,
    pub project_paths: Vec<PathBuf>,
    pub repository: PathBuf,
    pub password: String,
    pub source_device_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BackupManifest {
    pub format: String,
    pub schema_version: u32,
    pub logical_backup_id: Uuid,
    pub batch_id: Uuid,
    pub created_at: String,
    pub app_version: String,
    pub restic_version: String,
    pub source_device_id: Uuid,
    pub source_codex_home: String,
    pub project_paths: Vec<String>,
    pub git_metadata_paths: Vec<String>,
    pub file_count: u64,
    pub byte_count: u64,
    pub fingerprint: String,
    pub exclusions: Vec<BackupIssue>,
    #[serde(default)]
    pub notices: Vec<BackupIssue>,
    #[serde(default)]
    pub integrity_warnings: Vec<BackupIssue>,
    pub missing: Vec<BackupIssue>,
    pub integrity_status: String,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BackupIssueKind {
    SecurityExclusion,
    RebuildableDependency,
    RuntimeEphemeral,
    TestArtifact,
    JsonlIntegrity,
    MissingSource,
    EnumerationFailure,
    CopyFailure,
    FilesystemRedirect,
    UnsupportedEntry,
    #[default]
    LegacyUnknown,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct BackupIssue {
    pub path: String,
    pub reason: String,
    pub bytes: u64,
    #[serde(default)]
    pub kind: BackupIssueKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalSnapshot {
    pub logical_backup_id: Uuid,
    pub restic_snapshot_id: String,
    pub manifest: BackupManifest,
    pub complete: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalSnapshotSummary {
    pub logical_backup_id: Option<Uuid>,
    pub restic_snapshot_id: String,
    pub created_at: String,
    pub file_count: u64,
    pub byte_count: u64,
    #[serde(default = "default_integrity_status")]
    pub integrity_status: String,
    pub complete: bool,
}

fn default_integrity_status() -> String {
    "complete".to_owned()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalRestoreReport {
    pub restic_snapshot_id: String,
    pub restored_root: PathBuf,
    pub restored_files: u64,
    pub restored_bytes: u64,
    pub complete: bool,
    pub exclusions: Vec<BackupIssue>,
    pub notices: Vec<BackupIssue>,
    pub integrity_warnings: Vec<BackupIssue>,
    pub integrity_status: String,
    pub missing: Vec<BackupIssue>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct BackupState {
    fingerprint: String,
    snapshot: LocalSnapshot,
}

#[derive(Debug, Clone, Default)]
struct StageStats {
    files: u64,
    bytes: u64,
    exclusions: Vec<BackupIssue>,
    notices: Vec<BackupIssue>,
    integrity_warnings: Vec<BackupIssue>,
    missing: Vec<BackupIssue>,
    git_metadata_paths: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CodexPathDisposition {
    Include,
    SecurityExclusion,
    Notice(BackupIssueKind),
}

fn lowercase_components(path: &Path) -> Vec<String> {
    path.iter()
        .map(|component| component.to_string_lossy().to_ascii_lowercase())
        .collect()
}

fn is_codex_credential_path(_components: &[String], name: &str) -> bool {
    matches!(
        name,
        "auth.json"
            | "cookies"
            | "cookies.sqlite"
            | "token.json"
            | "rclone.conf"
            | "credentials.json"
            | "credentials.db"
            | "secrets.json"
            | "id_rsa"
            | "id_ed25519"
            | "private.key"
            | "private.pem"
    ) || name == ".env"
        || (name.starts_with(".env.") && name != ".env.example")
}

fn is_codex_runtime_or_cache_path(components: &[String], name: &str) -> bool {
    name.ends_with(".pid")
        || name.ends_with(".sock")
        || name.ends_with(".lock.runtime")
        || match components {
            [directory, filename] if directory == "thread-writer-locks" => filename
                .strip_suffix(".lock")
                .is_some_and(|id| id.len() == 36 && Uuid::parse_str(id).is_ok()),
            [tmp, arg0, directory, filename] => {
                tmp == "tmp"
                    && arg0 == "arg0"
                    && directory
                        .strip_prefix("codex-arg0")
                        .is_some_and(|suffix| !suffix.is_empty())
                    && filename == ".lock"
            }
            _ => false,
        }
        || components.iter().any(|component| {
            matches!(
                component.as_str(),
                "node_modules"
                    | ".venv"
                    | "venv"
                    | "__pycache__"
                    | ".codex-cache"
                    | ".pytest_cache"
                    | ".mypy_cache"
            )
        })
}

fn codex_path_disposition(relative: &Path) -> CodexPathDisposition {
    let components = lowercase_components(relative);
    let name = components.last().map(String::as_str).unwrap_or_default();
    if is_codex_credential_path(&components, name) {
        CodexPathDisposition::SecurityExclusion
    } else if is_codex_runtime_or_cache_path(&components, name) {
        CodexPathDisposition::Notice(BackupIssueKind::RuntimeEphemeral)
    } else {
        CodexPathDisposition::Include
    }
}

/// Codex-home policy only. Full encrypted project backups use `SourcePolicy::Project`.
pub fn is_excluded_backup_path(path: &Path) -> bool {
    codex_path_disposition(path) != CodexPathDisposition::Include
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SourcePolicy {
    Project,
    Codex,
}

impl SourcePolicy {
    fn excludes(self, relative: &Path) -> bool {
        self == Self::Codex && codex_path_disposition(relative) != CodexPathDisposition::Include
    }
}

fn redirect_notice_kind(relative: &Path) -> Option<BackupIssueKind> {
    let components = lowercase_components(relative);
    if components
        .iter()
        .any(|component| component == ".local-audit")
    {
        Some(BackupIssueKind::TestArtifact)
    } else if components
        .iter()
        .any(|component| matches!(component.as_str(), "node_modules" | ".pnpm"))
    {
        Some(BackupIssueKind::RebuildableDependency)
    } else {
        None
    }
}

fn backup_issue(
    path: &Path,
    reason: impl Into<String>,
    bytes: u64,
    kind: BackupIssueKind,
) -> BackupIssue {
    BackupIssue {
        path: path.display().to_string(),
        reason: reason.into(),
        bytes,
        kind,
    }
}

pub fn payload_relative_path(
    project_id: &str,
    source_root: &Path,
    source: &Path,
) -> Result<PathBuf, RehomeError> {
    let relative = match source.strip_prefix(source_root) {
        Ok(relative) => relative.to_path_buf(),
        Err(_) => {
            let root = source_root
                .canonicalize()
                .map_err(|_| unsafe_path("source root cannot be resolved"))?;
            let candidate = source
                .canonicalize()
                .map_err(|_| unsafe_path("source path cannot be resolved"))?;
            candidate
                .strip_prefix(root)
                .map(Path::to_path_buf)
                .map_err(|_| unsafe_path("source path escapes its root"))?
        }
    };
    let relative = normalize_relative(&relative)?;
    Ok(PathBuf::from("projects").join(project_id).join(relative))
}

pub fn file_fingerprint(root: &Path) -> Result<String, RehomeError> {
    fingerprint_tree(root, SourcePolicy::Project)
}

fn fingerprint_tree(root: &Path, policy: SourcePolicy) -> Result<String, RehomeError> {
    if has_redirect_ancestor(root).map_err(|error| backup_io("inspect fingerprint root", error))?
        || !root.is_dir()
    {
        return Err(unsafe_path("fingerprint root is not a regular directory"));
    }
    let mut records = BTreeMap::new();
    let mut entries = WalkDir::new(root)
        .follow_links(false)
        .follow_root_links(false)
        .into_iter();
    while let Some(entry) = entries.next() {
        let entry = entry.map_err(|error| backup_io("fingerprint", error))?;
        let relative = entry
            .path()
            .strip_prefix(root)
            .map_err(|_| unsafe_path("fingerprint path escaped its root"))?;
        if relative.as_os_str().is_empty() {
            continue;
        }
        let metadata = fs::symlink_metadata(entry.path())
            .map_err(|error| backup_io("fingerprint metadata", error))?;
        if policy.excludes(relative) {
            if metadata.is_dir() {
                entries.skip_current_dir();
            }
            continue;
        }
        if is_filesystem_redirect(&metadata) || !(metadata.is_file() || metadata.is_dir()) {
            return Err(unsafe_path(
                "fingerprint contains an unsupported filesystem entry",
            ));
        }
        let digest = if metadata.is_file() {
            hash_file(entry.path())?
        } else {
            "directory".into()
        };
        let relative = normalize_relative(relative)?;
        records.insert(relative, digest);
    }

    let mut hasher = Sha256::new();
    for (path, digest) in records {
        hasher.update(path.as_bytes());
        hasher.update([0]);
        hasher.update(digest.as_bytes());
        hasher.update([0]);
    }
    Ok(hex_digest(hasher.finalize()))
}

pub fn build_backup_args(
    repository: &Path,
    password_file: &Path,
    payload_root: &Path,
    tags: &[String],
) -> Vec<OsString> {
    let mut args = vec![
        OsString::from("--repo"),
        repository.as_os_str().to_owned(),
        OsString::from("--password-file"),
        password_file.as_os_str().to_owned(),
        OsString::from("backup"),
        OsString::from("--json"),
    ];
    for tag in tags {
        args.push(OsString::from("--tag"));
        args.push(OsString::from(tag));
    }
    args.push(payload_root.as_os_str().to_owned());
    args
}

fn can_reuse_cached_snapshot(previous: &BackupState, fingerprint: &str) -> bool {
    let manifest = &previous.snapshot.manifest;
    previous.fingerprint == fingerprint
        && previous.snapshot.complete
        && manifest
            .exclusions
            .iter()
            .chain(&manifest.notices)
            .chain(&manifest.integrity_warnings)
            .chain(&manifest.missing)
            .all(|issue| issue.kind != BackupIssueKind::LegacyUnknown)
}

pub fn backup_local(
    request: LocalBackupRequest,
    executable: &Path,
) -> Result<LocalSnapshot, RehomeError> {
    validate_request(&request)?;
    let git_sources = related_git_sources(&request)?;
    // An unreadable/unsupported source must never hit the complete-snapshot
    // cache. Staging records the individual missing entries in a partial backup.
    let source_fingerprint =
        fingerprint_sources(&request.codex_home, &request.project_paths, &git_sources).ok();
    let fingerprint = source_fingerprint
        .clone()
        .unwrap_or_else(|| format!("partial-{}", Uuid::new_v4()));
    if request.repository.is_dir() && repository_has_entries(&request.repository)? {
        ensure_repository(&request.repository, &request.password, executable)?;
        if let Some(previous) = read_state(&request.repository)? {
            if source_fingerprint.is_some() && can_reuse_cached_snapshot(&previous, &fingerprint) {
                return Ok(previous.snapshot);
            }
        }
    }

    let staging = Builder::new()
        .prefix(".enhe-backup-")
        .tempdir()
        .map_err(|error| backup_io("create staging directory", error))?;
    let payload = staging.path().join(PAYLOAD_ROOT);
    fs::create_dir_all(&payload).map_err(|error| backup_io("create payload directory", error))?;

    let mut stats = StageStats::default();
    for (index, project) in request.project_paths.iter().enumerate() {
        let project_id = stable_project_id(project, index);
        let destination = payload.join("projects").join(project_id.to_string());
        stage_tree(project, &destination, &mut stats, SourcePolicy::Project)?;
    }
    for (source, destination) in &git_sources {
        stage_tree(
            source,
            &payload.join(destination),
            &mut stats,
            SourcePolicy::Project,
        )?;
        stats.git_metadata_paths.push(source.display().to_string());
    }
    stage_tree(
        &request.codex_home,
        &payload.join("codex"),
        &mut stats,
        SourcePolicy::Codex,
    )?;

    let logical_backup_id = Uuid::new_v4();
    let integrity_status = if !stats.missing.is_empty() {
        "partial".to_owned()
    } else if !stats.notices.is_empty() || !stats.integrity_warnings.is_empty() {
        "warning".to_owned()
    } else {
        "complete".to_owned()
    };
    let manifest = BackupManifest {
        format: BACKUP_FORMAT.to_owned(),
        schema_version: BACKUP_SCHEMA_VERSION,
        logical_backup_id,
        batch_id: Uuid::new_v4(),
        created_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
        app_version: env!("CARGO_PKG_VERSION").to_owned(),
        restic_version: restic_version(executable),
        source_device_id: request.source_device_id,
        source_codex_home: request.codex_home.display().to_string(),
        project_paths: request
            .project_paths
            .iter()
            .map(|path| path.display().to_string())
            .collect(),
        git_metadata_paths: stats.git_metadata_paths.clone(),
        file_count: stats.files,
        byte_count: stats.bytes,
        fingerprint: fingerprint.clone(),
        exclusions: stats.exclusions,
        notices: stats.notices,
        integrity_warnings: stats.integrity_warnings,
        missing: stats.missing,
        integrity_status,
    };
    let manifest_bytes = serde_json::to_vec_pretty(&manifest)
        .map_err(|error| backup_failed(format!("could not serialize backup manifest: {error}")))?;
    fs::write(payload.join("manifest.json"), manifest_bytes)
        .map_err(|error| backup_io("write backup manifest", error))?;

    ensure_repository(&request.repository, &request.password, executable)?;
    let tags = vec![
        RESTIC_TAG.to_owned(),
        format!("logical_backup_id={logical_backup_id}"),
        format!("source_device_id={}", request.source_device_id),
        format!("fingerprint={fingerprint}"),
        format!("integrity_status={}", manifest.integrity_status),
    ];
    let args = build_backup_args(
        &request.repository,
        Path::new("<temporary-password-file>"),
        Path::new(PAYLOAD_ROOT),
        &tags,
    );
    let output =
        run_restic_with_password_in_dir(executable, args, &request.password, staging.path())?;
    let snapshot_id = parse_snapshot_id(&output.stdout)?;
    verify_repository(&request.repository, &request.password, executable)?;
    let snapshot = LocalSnapshot {
        logical_backup_id,
        restic_snapshot_id: snapshot_id,
        complete: manifest.missing.is_empty(),
        manifest,
    };
    write_state(
        &request.repository,
        &BackupState {
            fingerprint,
            snapshot: snapshot.clone(),
        },
    )?;
    Ok(snapshot)
}

pub fn apply_retention(
    repository: &Path,
    password: &str,
    executable: &Path,
    high_frequency_hours: u32,
    daily_days: u32,
    weekly_weeks: u32,
) -> Result<(), RehomeError> {
    if high_frequency_hours == 0 || daily_days == 0 || weekly_weeks == 0 {
        return Err(RehomeError::new(
            ErrorCode::ConfigInvalid,
            "retention periods must be greater than zero",
        ));
    }
    let args = vec![
        OsString::from("--repo"),
        repository.as_os_str().to_owned(),
        OsString::from("forget"),
        OsString::from("--tag"),
        OsString::from(RESTIC_TAG),
        OsString::from("--keep-within"),
        OsString::from(format!("{high_frequency_hours}h")),
        OsString::from("--keep-daily"),
        OsString::from(daily_days.to_string()),
        OsString::from("--keep-weekly"),
        OsString::from(weekly_weeks.to_string()),
        OsString::from("--prune"),
    ];
    run_restic_with_password(executable, args, password).map(|_| ())
}

pub fn list_local_snapshots(
    repository: &Path,
    password: &str,
    executable: &Path,
) -> Result<Vec<LocalSnapshotSummary>, RehomeError> {
    if password.is_empty() {
        return Err(RehomeError::new(
            ErrorCode::BackupPasswordRequired,
            "a recovery password is required to list local backups",
        ));
    }
    let args = vec![
        OsString::from("--repo"),
        repository.as_os_str().to_owned(),
        OsString::from("snapshots"),
        OsString::from("--json"),
        OsString::from("--tag"),
        OsString::from(RESTIC_TAG),
    ];
    let output = run_restic_with_password(executable, args, password)?;
    let mut summaries = parse_snapshot_summaries(&output.stdout)?;
    for summary in &mut summaries {
        let args = vec![
            OsString::from("--repo"),
            repository.as_os_str().to_owned(),
            OsString::from("stats"),
            OsString::from("--json"),
            OsString::from(summary.restic_snapshot_id.as_str()),
        ];
        if let Ok(stats) = run_restic_with_password(executable, args, password) {
            if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&stats.stdout) {
                // `stats.total_file_count` includes directories and must not
                // overwrite the source-file count from snapshot metadata.
                summary.byte_count = value
                    .get("total_size")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(summary.byte_count);
            }
        }
    }
    Ok(summaries)
}

pub fn restore_local(
    snapshot: &str,
    repository: &Path,
    password: &str,
    target: &Path,
    executable: &Path,
) -> Result<LocalRestoreReport, RehomeError> {
    restore_local_inner(snapshot, repository, password, target, executable)
        .map_err(as_restore_error)
}

fn restore_local_inner(
    snapshot: &str,
    repository: &Path,
    password: &str,
    target: &Path,
    executable: &Path,
) -> Result<LocalRestoreReport, RehomeError> {
    validate_snapshot_id(snapshot)?;
    if password.is_empty() {
        return Err(RehomeError::new(
            ErrorCode::BackupPasswordRequired,
            "a recovery password is required to restore a local backup",
        ));
    }
    validate_target_directory(target, repository)?;
    let restore_staging = Builder::new()
        .prefix(".enhe-restore-")
        .tempdir()
        .map_err(|error| backup_io("create restore staging directory", error))?;
    let args = vec![
        OsString::from("--repo"),
        repository.as_os_str().to_owned(),
        OsString::from("restore"),
        OsString::from(snapshot),
        OsString::from("--target"),
        restore_staging.path().as_os_str().to_owned(),
    ];
    run_restic_with_password(executable, args, password)?;
    let payload = find_payload_root(restore_staging.path())?;
    let manifest = read_manifest(&payload)?;
    let output_root = target.join(format!("backup-{}", manifest.logical_backup_id));
    if output_root.exists() {
        return Err(RehomeError::new(
            ErrorCode::ProjectConflict,
            format!("restore target already exists: {}", output_root.display()),
        ));
    }
    let temporary_root = target.join(format!(".enhe-restore-{}.partial", Uuid::new_v4()));
    fs::create_dir_all(&temporary_root)
        .map_err(|error| backup_io("create restore staging target", error))?;
    let restored_files = match copy_tree_for_restore(&payload, &temporary_root) {
        Ok(restored_files) => restored_files,
        Err(error) => {
            let _ = fs::remove_dir_all(&temporary_root);
            return Err(error);
        }
    };
    if let Err(error) = fs::rename(&temporary_root, &output_root) {
        let _ = fs::remove_dir_all(&temporary_root);
        return Err(backup_io("commit restored target", error));
    }
    let restored_bytes = restored_files.1;
    Ok(LocalRestoreReport {
        restic_snapshot_id: snapshot.to_owned(),
        restored_root: output_root,
        restored_files: restored_files.0,
        restored_bytes,
        complete: manifest.missing.is_empty(),
        exclusions: manifest.exclusions,
        notices: manifest.notices,
        integrity_warnings: manifest.integrity_warnings,
        integrity_status: manifest.integrity_status,
        missing: manifest.missing,
    })
}

fn as_restore_error(error: RehomeError) -> RehomeError {
    if error.code == ErrorCode::BackupFailed {
        RehomeError::new(
            ErrorCode::RestoreFailed,
            error
                .message
                .replacen("backup engine failed", "restore engine failed", 1),
        )
    } else {
        error
    }
}

fn validate_request(request: &LocalBackupRequest) -> Result<(), RehomeError> {
    if request.password.is_empty() {
        return Err(RehomeError::new(
            ErrorCode::BackupPasswordRequired,
            "a recovery password is required before creating a local backup",
        ));
    }
    if !request.repository.is_absolute() {
        return Err(unsafe_path(
            "backup repository must be an absolute local path",
        ));
    }
    for source in request
        .project_paths
        .iter()
        .chain(std::iter::once(&request.codex_home))
    {
        if !source.is_absolute() {
            return Err(unsafe_path("backup sources must be absolute local paths"));
        }
        if paths_overlap(source, &request.repository) {
            return Err(unsafe_path("backup repository overlaps a selected source"));
        }
    }
    for project in &request.project_paths {
        if paths_overlap(project, &request.codex_home) {
            return Err(unsafe_path("project selection overlaps Codex data"));
        }
    }
    Ok(())
}

fn stage_tree(
    source: &Path,
    destination: &Path,
    stats: &mut StageStats,
    policy: SourcePolicy,
) -> Result<(), RehomeError> {
    if has_redirect_ancestor(source).unwrap_or(true) || !source.is_dir() {
        stats.missing.push(backup_issue(
            source,
            "source is unavailable, not a directory, or contains a filesystem redirect",
            0,
            BackupIssueKind::MissingSource,
        ));
        return Ok(());
    }
    fs::create_dir_all(destination).map_err(|error| backup_io("create staged root", error))?;
    let mut entries = WalkDir::new(source)
        .follow_links(false)
        .follow_root_links(false)
        .into_iter();
    while let Some(entry) = entries.next() {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                stats.missing.push(backup_issue(
                    error.path().unwrap_or(source),
                    "source entry could not be enumerated",
                    0,
                    BackupIssueKind::EnumerationFailure,
                ));
                continue;
            }
        };
        let relative = entry
            .path()
            .strip_prefix(source)
            .map_err(|_| unsafe_path("selected source escaped its root"))?;
        if relative.as_os_str().is_empty() {
            continue;
        }
        let metadata = match fs::symlink_metadata(entry.path()) {
            Ok(metadata) => metadata,
            Err(_) => {
                stats.missing.push(backup_issue(
                    entry.path(),
                    "source metadata could not be read",
                    0,
                    BackupIssueKind::EnumerationFailure,
                ));
                continue;
            }
        };
        if is_filesystem_redirect(&metadata) {
            if metadata.is_dir() {
                entries.skip_current_dir();
            }
            let kind = redirect_notice_kind(relative);
            let issue = backup_issue(
                entry.path(),
                "symbolic link or filesystem redirect was not followed",
                0,
                kind.unwrap_or(BackupIssueKind::FilesystemRedirect),
            );
            if kind.is_some() {
                stats.notices.push(issue);
            } else {
                stats.missing.push(issue);
            }
            continue;
        }
        if policy == SourcePolicy::Codex {
            let disposition = codex_path_disposition(relative);
            match disposition {
                CodexPathDisposition::SecurityExclusion => {
                    stats.exclusions.push(backup_issue(
                        relative,
                        "Codex credential data was excluded",
                        metadata.len(),
                        BackupIssueKind::SecurityExclusion,
                    ));
                }
                CodexPathDisposition::Notice(kind) => {
                    stats.notices.push(backup_issue(
                        relative,
                        "Codex cache or runtime data was not backed up",
                        metadata.len(),
                        kind,
                    ));
                }
                CodexPathDisposition::Include => {}
            }
            if disposition != CodexPathDisposition::Include {
                if metadata.is_dir() {
                    entries.skip_current_dir();
                }
                continue;
            }
        }
        if metadata.is_dir() {
            fs::create_dir_all(destination.join(relative))
                .map_err(|error| backup_io("create staged directory", error))?;
            continue;
        }
        if !metadata.is_file() {
            stats.missing.push(backup_issue(
                entry.path(),
                "unsupported filesystem entry",
                0,
                BackupIssueKind::UnsupportedEntry,
            ));
            continue;
        }
        let target = if policy == SourcePolicy::Codex
            && is_sqlite_sidecar(entry.path())
            && sqlite_base_exists(entry.path())?
        {
            destination.join(SQLITE_SIDECAR_ROOT).join(relative)
        } else {
            destination.join(relative)
        };
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|error| backup_io("create staged parent", error))?;
        }
        let copied = (|| {
            if policy == SourcePolicy::Codex && is_sqlite_database(entry.path())? {
                copy_sqlite_database(entry.path(), &target)
            } else {
                fs::copy(entry.path(), &target)
                    .map(|_| ())
                    .map_err(|error| backup_io("copy selected file to staging", error))
            }
        })();
        if copied.is_err() {
            let _ = fs::remove_file(&target);
            stats.missing.push(backup_issue(
                entry.path(),
                "source file could not be copied consistently",
                metadata.len(),
                BackupIssueKind::CopyFailure,
            ));
            continue;
        }
        if policy == SourcePolicy::Codex && is_jsonl_file(entry.path()) {
            match inspect_jsonl(&target, metadata.len()) {
                Ok(Some(mut issue)) => {
                    issue.path = entry.path().display().to_string();
                    stats.integrity_warnings.push(issue);
                }
                Ok(None) => {}
                Err(error) => stats.integrity_warnings.push(backup_issue(
                    entry.path(),
                    format!("JSONL could not be checked: {}", error.message),
                    metadata.len(),
                    BackupIssueKind::JsonlIntegrity,
                )),
            }
        }
        stats.files += 1;
        stats.bytes = stats.bytes.saturating_add(metadata.len());
    }
    Ok(())
}

fn related_git_sources(
    request: &LocalBackupRequest,
) -> Result<Vec<(PathBuf, PathBuf)>, RehomeError> {
    let mut sources = Vec::new();
    for (index, project) in request.project_paths.iter().enumerate() {
        if has_redirect_ancestor(project).unwrap_or(true) {
            continue;
        }
        let git_marker = project.join(".git");
        if !fs::symlink_metadata(&git_marker)
            .map(|metadata| metadata.is_file() && !is_filesystem_redirect(&metadata))
            .unwrap_or(false)
        {
            continue;
        }
        let marker = fs::read_to_string(&git_marker)
            .map_err(|error| backup_io("read Git worktree marker", error))?;
        let Some(pointer) = marker.lines().find_map(|line| line.strip_prefix("gitdir:")) else {
            continue;
        };
        let gitdir = PathBuf::from(pointer.trim());
        let gitdir = if gitdir.is_absolute() {
            gitdir
        } else {
            project.join(gitdir)
        };
        validate_git_source(&gitdir, request)?;
        let base = Path::new("git-metadata").join(stable_project_id(project, index).to_string());
        sources.push((gitdir.clone(), base.join("worktree")));
        let commondir_file = gitdir.join("commondir");
        if commondir_file.is_file() && !has_redirect_ancestor(&commondir_file).unwrap_or(true) {
            let commondir = fs::read_to_string(&commondir_file)
                .map_err(|error| backup_io("read Git common directory marker", error))?;
            let common = gitdir.join(commondir.trim());
            validate_git_source(&common, request)?;
            if resolved_path(&common)? != resolved_path(&gitdir)? {
                sources.push((common, base.join("common")));
            }
        }
    }
    Ok(sources)
}

fn validate_git_source(source: &Path, request: &LocalBackupRequest) -> Result<(), RehomeError> {
    if paths_overlap(source, &request.codex_home) || paths_overlap(source, &request.repository) {
        return Err(unsafe_path(
            "Git metadata overlaps protected Codex data or backup repository",
        ));
    }
    // An external Git pointer is allowed only for an actual Git metadata tree.
    // Missing/redirected trees are left to staging to report as partial.
    if !has_redirect_ancestor(source).unwrap_or(true)
        && source.is_dir()
        && (!source.join("HEAD").is_file()
            || !(source.join("objects").is_dir() || source.join("commondir").is_file()))
    {
        return Err(unsafe_path(
            "Git pointer does not identify a Git metadata directory",
        ));
    }
    Ok(())
}

fn copy_tree_for_restore(source: &Path, destination: &Path) -> Result<(u64, u64), RehomeError> {
    let mut files = 0_u64;
    let mut bytes = 0_u64;
    for entry in WalkDir::new(source).follow_links(false).into_iter() {
        let entry = entry.map_err(|error| backup_io("scan restored payload", error))?;
        let relative = entry
            .path()
            .strip_prefix(source)
            .map_err(|_| unsafe_path("restored payload escaped its root"))?;
        if relative.as_os_str().is_empty() {
            continue;
        }
        let internal_manifest = relative == Path::new("manifest.json");
        let relative = normalize_relative(relative)?;
        let target = destination.join(&relative);
        let metadata = fs::symlink_metadata(entry.path())
            .map_err(|error| backup_io("read restored payload metadata", error))?;
        if is_filesystem_redirect(&metadata) {
            return Err(unsafe_path(
                "restored payload contains a filesystem redirect",
            ));
        }
        if metadata.is_dir() {
            fs::create_dir_all(&target)
                .map_err(|error| backup_io("create restored directory", error))?;
        } else if metadata.is_file() {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)
                    .map_err(|error| backup_io("create restored parent", error))?;
            }
            let temporary = NamedTempFile::new_in(
                target
                    .parent()
                    .ok_or_else(|| unsafe_path("restored file has no parent"))?,
            )
            .map_err(|error| backup_io("create restored temporary file", error))?;
            fs::copy(entry.path(), temporary.path())
                .map_err(|error| backup_io("write restored file", error))?;
            temporary
                .persist_noclobber(&target)
                .map_err(|error| backup_io("commit restored file", error))?;
            // Retain recovery metadata without inflating ordinary source counts.
            // Project-owned manifest.json files live below projects/<id>/.
            if !internal_manifest {
                files += 1;
                bytes = bytes.saturating_add(metadata.len());
            }
        }
    }
    repair_worktree_layout(destination)?;
    Ok((files, bytes))
}

fn is_sqlite_database(path: &Path) -> Result<bool, RehomeError> {
    let mut file = File::open(path).map_err(|error| backup_io("open SQLite database", error))?;
    let mut header = [0_u8; 16];
    match file.read_exact(&mut header) {
        Ok(()) => Ok(&header == b"SQLite format 3\0"),
        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => Ok(false),
        Err(error) => Err(backup_io("read SQLite database header", error)),
    }
}

fn is_jsonl_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("jsonl"))
}

fn inspect_jsonl(path: &Path, bytes: u64) -> Result<Option<BackupIssue>, RehomeError> {
    let file = File::open(path).map_err(|error| backup_io("open JSONL session", error))?;
    let mut reader = std::io::BufReader::new(file);
    let mut line = Vec::with_capacity(4096);
    let mut line_number = 0_u64;
    loop {
        line.clear();
        let read = std::io::BufRead::read_until(&mut reader, b'\n', &mut line)
            .map_err(|error| backup_io("read JSONL session", error))?;
        if read == 0 {
            break;
        }
        line_number += 1;
        while line
            .last()
            .is_some_and(|byte| matches!(*byte, b'\n' | b'\r' | b' ' | b'\t'))
        {
            line.pop();
        }
        let start = line
            .iter()
            .position(|byte| !matches!(*byte, b' ' | b'\t'))
            .unwrap_or(line.len());
        let record = &line[start..];
        if record.is_empty() {
            continue;
        }
        if serde_json::from_slice::<serde_json::Value>(record).is_err() {
            return Ok(Some(backup_issue(
                path,
                format!("JSONL contains an incomplete or invalid record at line {line_number}"),
                bytes,
                BackupIssueKind::JsonlIntegrity,
            )));
        }
    }
    Ok(None)
}

fn is_sqlite_sidecar(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    ["-wal", "-shm", ".wal", ".shm"]
        .iter()
        .any(|suffix| name.ends_with(suffix))
}

fn sqlite_base_exists(sidecar: &Path) -> Result<bool, RehomeError> {
    let Some(name) = sidecar.file_name().and_then(|name| name.to_str()) else {
        return Ok(false);
    };
    let name = name.to_ascii_lowercase();
    let base_name = ["-wal", "-shm", ".wal", ".shm"]
        .iter()
        .find_map(|suffix| name.strip_suffix(suffix))
        .unwrap_or_default();
    if base_name.is_empty() {
        return Ok(false);
    }
    let original_name = sidecar
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let suffix_length = original_name.len().saturating_sub(base_name.len());
    let original_base = &original_name[..original_name.len().saturating_sub(suffix_length)];
    let base = sidecar.with_file_name(original_base);
    if !fs::symlink_metadata(&base)
        .map(|metadata| metadata.is_file() && !metadata.file_type().is_symlink())
        .unwrap_or(false)
    {
        return Ok(false);
    }
    is_sqlite_database(&base)
}

fn copy_sqlite_database(source: &Path, destination: &Path) -> Result<(), RehomeError> {
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let source_connection = Connection::open_with_flags(source, flags)
        .map_err(|error| backup_io("open SQLite source snapshot", error))?;
    source_connection
        .busy_timeout(Duration::from_secs(5))
        .map_err(|error| backup_io("configure SQLite source snapshot", error))?;
    let mut destination_connection = Connection::open(destination)
        .map_err(|error| backup_io("open SQLite destination snapshot", error))?;
    let backup = Backup::new(&source_connection, &mut destination_connection)
        .map_err(|error| backup_io("start SQLite online backup", error))?;
    backup
        .run_to_completion(128, Duration::from_millis(1), None)
        .map_err(|error| backup_io("complete SQLite online backup", error))?;
    drop(backup);
    destination_connection
        .close()
        .map_err(|(_, error)| backup_io("close SQLite destination snapshot", error))?;
    Ok(())
}

fn repair_worktree_layout(destination: &Path) -> Result<(), RehomeError> {
    let projects = destination.join("projects");
    let metadata_root = destination.join("git-metadata");
    if !projects.is_dir() {
        return Ok(());
    }
    for entry in
        fs::read_dir(&projects).map_err(|error| backup_io("scan restored projects", error))?
    {
        let entry = entry.map_err(|error| backup_io("read restored project entry", error))?;
        let project = entry.path();
        if !entry
            .file_type()
            .map_err(|error| backup_io("read restored project type", error))?
            .is_dir()
        {
            continue;
        }
        let project_id_name = entry.file_name();
        let project_id = project_id_name
            .to_str()
            .ok_or_else(|| unsafe_path("restored project id is not valid UTF-8"))?;
        if !project_id
            .chars()
            .all(|character| character.is_ascii_hexdigit() || character == '-')
        {
            return Err(unsafe_path(
                "restored project id contains unsafe characters",
            ));
        }
        let marker = project.join(".git");
        if !marker.is_file() {
            continue;
        }
        let marker_text = fs::read_to_string(&marker)
            .map_err(|error| backup_io("read restored Git worktree marker", error))?;
        if !marker_text
            .lines()
            .any(|line| line.trim_start().starts_with("gitdir:"))
        {
            continue;
        }
        let worktree = metadata_root.join(project_id).join("worktree");
        if !worktree.is_dir() {
            return Err(backup_failed(
                "restored Git worktree metadata is missing from the backup",
            ));
        }
        let common = metadata_root.join(project_id).join("common");
        if worktree.join("commondir").is_file() {
            if !common.is_dir() {
                return Err(backup_failed(
                    "restored Git common metadata is missing from the backup",
                ));
            }
            fs::write(worktree.join("commondir"), b"../common\n")
                .map_err(|error| backup_io("repair restored Git common marker", error))?;
        }
        if worktree.join("gitdir").is_file() {
            let gitdir = format!("../../../projects/{project_id}/.git\n");
            fs::write(worktree.join("gitdir"), gitdir)
                .map_err(|error| backup_io("repair restored Git worktree marker", error))?;
        }
        let restored_marker = format!("gitdir: ../../git-metadata/{project_id}/worktree\n");
        fs::write(&marker, restored_marker)
            .map_err(|error| backup_io("repair restored project Git pointer", error))?;
    }
    Ok(())
}

fn find_payload_root(root: &Path) -> Result<PathBuf, RehomeError> {
    let direct = root.join(PAYLOAD_ROOT);
    if direct.is_dir() {
        return Ok(direct);
    }
    Err(RehomeError::new(
        ErrorCode::BackupFailed,
        "restic restore did not produce the expected ENHE payload root",
    ))
}

fn read_manifest(payload: &Path) -> Result<BackupManifest, RehomeError> {
    let bytes = fs::read(payload.join("manifest.json"))
        .map_err(|error| backup_io("read restored backup manifest", error))?;
    let manifest: BackupManifest = serde_json::from_slice(&bytes)
        .map_err(|error| backup_failed(format!("backup manifest is invalid: {error}")))?;
    if manifest.format != BACKUP_FORMAT || manifest.schema_version != BACKUP_SCHEMA_VERSION {
        return Err(RehomeError::new(
            ErrorCode::UnsupportedSchema,
            "backup manifest format or schema is not supported",
        ));
    }
    Ok(manifest)
}

fn ensure_repository(
    repository: &Path,
    password: &str,
    executable: &Path,
) -> Result<(), RehomeError> {
    if repository.exists() && !repository.is_dir() {
        return Err(RehomeError::new(
            ErrorCode::BackupRepositoryInvalid,
            "the selected backup repository is not a directory",
        ));
    }
    fs::create_dir_all(repository).map_err(|error| backup_io("create backup repository", error))?;
    let has_entries = repository_has_entries(repository)?;
    if !has_entries {
        let args = vec![
            OsString::from("--repo"),
            repository.as_os_str().to_owned(),
            OsString::from("init"),
        ];
        run_restic_with_password(executable, args, password)?;
        return Ok(());
    }
    if !has_restic_repository_layout(repository) {
        return Err(RehomeError::new(
            ErrorCode::BackupRepositoryInvalid,
            format!(
                "the selected directory is not a restic repository: {}",
                repository.display()
            ),
        ));
    }
    let args = vec![
        OsString::from("--repo"),
        repository.as_os_str().to_owned(),
        OsString::from("cat"),
        OsString::from("config"),
    ];
    run_restic_with_password(executable, args, password).map(|_| ())
}

fn verify_repository(
    repository: &Path,
    password: &str,
    executable: &Path,
) -> Result<(), RehomeError> {
    let args = vec![
        OsString::from("--repo"),
        repository.as_os_str().to_owned(),
        OsString::from("check"),
    ];
    run_restic_with_password(executable, args, password).map(|_| ())
}

fn repository_has_entries(repository: &Path) -> Result<bool, RehomeError> {
    Ok(fs::read_dir(repository)
        .map_err(|error| backup_io("inspect backup repository", error))?
        .next()
        .transpose()
        .map_err(|error| backup_io("inspect backup repository", error))?
        .is_some())
}

fn has_restic_repository_layout(repository: &Path) -> bool {
    repository.join("config").is_file()
        && ["data", "index", "keys", "locks", "snapshots"]
            .iter()
            .all(|entry| repository.join(entry).is_dir())
}

fn run_restic_with_password(
    executable: &Path,
    args: Vec<OsString>,
    password: &str,
) -> Result<EngineOutput, RehomeError> {
    run_restic_with_password_in_dir(executable, args, password, Path::new("."))
}

fn run_restic_with_password_in_dir(
    executable: &Path,
    args: Vec<OsString>,
    password: &str,
    working_directory: &Path,
) -> Result<EngineOutput, RehomeError> {
    if password.is_empty() {
        return Err(RehomeError::new(
            ErrorCode::BackupPasswordRequired,
            "a recovery password is required for the backup engine",
        ));
    }
    let password_file =
        NamedTempFile::new().map_err(|error| backup_io("create temporary password file", error))?;
    fs::write(password_file.path(), password.as_bytes())
        .map_err(|error| backup_io("write temporary password file", error))?;
    let mut prepared = Vec::with_capacity(args.len() + 2);
    let mut has_password_file = false;
    let mut index = 0;
    while index < args.len() {
        if args[index] == OsString::from("--password-file") {
            has_password_file = true;
            prepared.push(args[index].clone());
            prepared.push(
                if args.get(index + 1) == Some(&OsString::from("<temporary-password-file>")) {
                    password_file.path().as_os_str().to_owned()
                } else {
                    args.get(index + 1).cloned().unwrap_or_default()
                },
            );
            index += 2;
            continue;
        }
        prepared.push(args[index].clone());
        index += 1;
    }
    if !has_password_file {
        prepared.splice(
            0..0,
            [
                OsString::from("--password-file"),
                password_file.path().as_os_str().to_owned(),
            ],
        );
    }
    let mut command = Command::new(executable);
    command.current_dir(working_directory);
    command.args(prepared);
    let output = command
        .output()
        .map_err(|error| engine_unavailable(executable, error))?;
    if !output.status.success() {
        return Err(backup_engine_failed(
            output.status.code(),
            &output.stderr,
            password_file.path(),
        ));
    }
    Ok(EngineOutput {
        stdout: output.stdout,
    })
}

#[derive(Debug)]
struct EngineOutput {
    stdout: Vec<u8>,
}

fn parse_snapshot_id(bytes: &[u8]) -> Result<String, RehomeError> {
    let mut snapshot = None;
    for line in bytes.split(|byte| *byte == b'\n') {
        if let Ok(value) = serde_json::from_slice::<serde_json::Value>(line) {
            if let Some(id) = value.get("snapshot_id").and_then(serde_json::Value::as_str) {
                snapshot = Some(id.to_owned());
            }
        }
    }
    snapshot
        .filter(|id| id.chars().all(|character| character.is_ascii_hexdigit()))
        .ok_or_else(|| backup_failed("restic did not return a snapshot id"))
}

fn parse_snapshot_summaries(bytes: &[u8]) -> Result<Vec<LocalSnapshotSummary>, RehomeError> {
    let values: Vec<serde_json::Value> = serde_json::from_slice(bytes)
        .map_err(|error| backup_failed(format!("restic snapshot list is invalid: {error}")))?;
    let mut summaries = Vec::new();
    for value in values {
        let snapshot_id = value
            .get("id")
            .or_else(|| value.get("short_id"))
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| backup_failed("restic snapshot list has no snapshot id"))?;
        let tags = value
            .get("tags")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(serde_json::Value::as_str)
            .collect::<Vec<_>>();
        let logical_backup_id = tags.iter().find_map(|tag| {
            tag.strip_prefix("logical_backup_id=")
                .and_then(|id| Uuid::parse_str(id).ok())
        });
        let integrity_status = tags
            .iter()
            .find_map(|tag| tag.strip_prefix("integrity_status="))
            .unwrap_or("complete")
            .to_owned();
        let complete = integrity_status != "partial";
        summaries.push(LocalSnapshotSummary {
            logical_backup_id,
            restic_snapshot_id: snapshot_id.to_owned(),
            created_at: value
                .get("time")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown")
                .to_owned(),
            file_count: value
                .get("summary")
                .and_then(|summary| summary.get("total_files_processed"))
                .and_then(serde_json::Value::as_u64)
                .unwrap_or_default()
                // Each ENHE payload contains one additional internal manifest.
                .saturating_sub(1),
            byte_count: value
                .get("summary")
                .and_then(|summary| summary.get("bytes_added"))
                .and_then(serde_json::Value::as_u64)
                .unwrap_or_default(),
            integrity_status,
            complete,
        });
    }
    Ok(summaries)
}

fn validate_snapshot_id(snapshot: &str) -> Result<(), RehomeError> {
    if !(8..=64).contains(&snapshot.len())
        || !snapshot
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err(unsafe_path("snapshot id is not a valid restic identifier"));
    }
    Ok(())
}

fn validate_target_directory(target: &Path, repository: &Path) -> Result<(), RehomeError> {
    if paths_overlap(target, repository) {
        return Err(unsafe_path("restore target overlaps the backup repository"));
    }
    if target.exists() && !target.is_dir() {
        return Err(unsafe_path("restore target is not a directory"));
    }
    fs::create_dir_all(target).map_err(|error| backup_io("create restore target parent", error))?;
    Ok(())
}

fn fingerprint_sources(
    codex_home: &Path,
    projects: &[PathBuf],
    git_sources: &[(PathBuf, PathBuf)],
) -> Result<String, RehomeError> {
    let roots = projects
        .iter()
        .map(|root| (root.as_path(), SourcePolicy::Project))
        .chain(
            git_sources
                .iter()
                .map(|(root, _)| (root.as_path(), SourcePolicy::Project)),
        )
        .chain(std::iter::once((codex_home, SourcePolicy::Codex)));
    let mut hasher = Sha256::new();
    hasher.update(b"full-project-policy-v1\0");
    for (root, policy) in roots {
        hasher.update(root.to_string_lossy().as_bytes());
        hasher.update([0]);
        hasher.update(fingerprint_tree(root, policy)?.as_bytes());
        hasher.update([0]);
    }
    Ok(hex_digest(hasher.finalize()))
}

fn stable_project_id(path: &Path, index: usize) -> Uuid {
    let seed = format!("{}#{index}", path.to_string_lossy().to_ascii_lowercase());
    Uuid::new_v5(&Uuid::NAMESPACE_URL, seed.as_bytes())
}

fn normalize_relative(path: &Path) -> Result<String, RehomeError> {
    let raw = path
        .to_str()
        .ok_or_else(|| unsafe_path("path is not valid UTF-8"))?;
    if raw.is_empty() || raw.contains('\0') {
        return Err(unsafe_path("path is empty or contains NUL"));
    }
    let normalized = raw.replace('\\', "/");
    let mut components = Vec::new();
    for component in normalized.split('/') {
        if component.is_empty() || component == "." || component == ".." {
            return Err(unsafe_path("path contains an ambiguous component"));
        }
        if component.contains(':')
            || component.chars().any(|character| {
                matches!(character, '<' | '>' | '"' | '|' | '?' | '*')
                    || ('\u{1}'..='\u{1f}').contains(&character)
            })
        {
            return Err(unsafe_path("path contains a Windows-forbidden component"));
        }
        components.push(component);
    }
    Ok(components.join("/"))
}

fn paths_overlap(left: &Path, right: &Path) -> bool {
    match (resolved_path(left), resolved_path(right)) {
        (Ok(left), Ok(right)) => left.starts_with(&right) || right.starts_with(&left),
        _ => true, // Unknown boundaries must fail closed.
    }
}

fn resolved_path(path: &Path) -> Result<PathBuf, RehomeError> {
    let absolute = std::path::absolute(path).map_err(|error| backup_io("resolve path", error))?;
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            std::path::Component::Prefix(_) => {
                normalized.push(component.as_os_str());
                continue;
            }
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            _ => normalized.push(component.as_os_str()),
        }
        // Resolve each existing prefix so a nonexistent final directory cannot
        // hide a junction/alias or an overlapping repository.
        match fs::canonicalize(&normalized) {
            Ok(path) => normalized = path,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(backup_io("resolve path boundary", error)),
        }
    }
    #[cfg(windows)]
    {
        normalized = PathBuf::from(normalized.to_string_lossy().to_ascii_lowercase());
    }
    Ok(normalized)
}

fn hash_file(path: &Path) -> Result<String, RehomeError> {
    let mut file = File::open(path).map_err(|error| backup_io("hash selected file", error))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; HASH_BUFFER_BYTES];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|error| backup_io("read selected file", error))?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(hex_digest(hasher.finalize()))
}

fn restic_version(executable: &Path) -> String {
    Command::new(executable)
        .arg("version")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .and_then(|output| output.lines().next().map(str::trim).map(str::to_owned))
        .unwrap_or_else(|| "unavailable".to_owned())
}

fn hex_digest(digest: impl AsRef<[u8]>) -> String {
    digest
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn state_path(repository: &Path) -> PathBuf {
    let name = repository
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("repository");
    repository
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!(".{name}.enhe-state.json"))
}

fn read_state(repository: &Path) -> Result<Option<BackupState>, RehomeError> {
    let path = state_path(repository);
    if !path.is_file() {
        return Ok(None);
    }
    let bytes = fs::read(path).map_err(|error| backup_io("read backup state", error))?;
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|error| backup_failed(format!("backup state is invalid: {error}")))
}

fn write_state(repository: &Path, state: &BackupState) -> Result<(), RehomeError> {
    let path = state_path(repository);
    let temporary = path.with_extension("enhe-state.json.partial");
    let bytes = serde_json::to_vec_pretty(state)
        .map_err(|error| backup_failed(format!("could not serialize backup state: {error}")))?;
    fs::write(&temporary, bytes).map_err(|error| backup_io("write backup state", error))?;
    fs::rename(&temporary, path).map_err(|error| backup_io("commit backup state", error))?;
    Ok(())
}

fn unsafe_path(message: impl Into<String>) -> RehomeError {
    RehomeError::new(ErrorCode::UnsafePath, message)
}

fn backup_failed(message: impl Into<String>) -> RehomeError {
    RehomeError::new(ErrorCode::BackupFailed, message)
}

fn backup_io(stage: &str, error: impl std::fmt::Display) -> RehomeError {
    RehomeError::new(ErrorCode::BackupFailed, format!("{stage} failed: {error}"))
}

fn engine_unavailable(executable: &Path, error: io::Error) -> RehomeError {
    RehomeError::new(
        ErrorCode::BackupEngineUnavailable,
        format!(
            "backup engine could not be started at {}: {error}",
            executable.display()
        ),
    )
}

fn backup_engine_failed(
    exit_code: Option<i32>,
    stderr: &[u8],
    password_file: &Path,
) -> RehomeError {
    let password_file = password_file.to_string_lossy();
    let redacted = String::from_utf8_lossy(stderr)
        .replace(password_file.as_ref(), "<temporary-password-file>");
    let collapsed = redacted.split_whitespace().collect::<Vec<_>>().join(" ");
    let normalized = collapsed.to_ascii_lowercase();
    let code = if normalized.contains("wrong password")
        || normalized.contains("no key found")
        || normalized.contains("unable to decrypt key")
    {
        ErrorCode::BackupPasswordRequired
    } else if normalized.contains("repository does not exist")
        || normalized.contains("unable to open config file")
        || normalized.contains("is there a repository at")
    {
        ErrorCode::BackupRepositoryInvalid
    } else if normalized.contains("no space left on device")
        || normalized.contains("not enough space on the disk")
        || normalized.contains("disk full")
    {
        ErrorCode::DiskSpaceInsufficient
    } else {
        ErrorCode::BackupFailed
    };
    let char_count = collapsed.chars().count();
    let detail = if char_count > 600 {
        format!(
            "…{}",
            collapsed
                .chars()
                .skip(char_count.saturating_sub(599))
                .collect::<String>()
        )
    } else {
        collapsed
    };
    let summary = format!(
        "backup engine failed{}",
        exit_code.map_or(String::new(), |code| format!(" (exit {code})"))
    );
    RehomeError::new(
        code,
        if detail.is_empty() {
            summary
        } else {
            format!("{summary}: {detail}")
        },
    )
}

#[cfg(test)]
mod tests {
    use super::{
        can_reuse_cached_snapshot, ensure_repository, inspect_jsonl, parse_snapshot_summaries,
        redirect_notice_kind, stage_tree, BackupIssue, BackupIssueKind, BackupManifest,
        BackupState, LocalSnapshot, SourcePolicy, StageStats, BACKUP_FORMAT, BACKUP_SCHEMA_VERSION,
    };
    use crate::core::error::ErrorCode;
    use std::{fs, path::Path};
    use tempfile::tempdir;
    use uuid::Uuid;

    #[test]
    fn observed_runtime_locks_are_notices_only_for_codex_home() {
        let source = tempdir().expect("source directory");
        let paths = [
            "thread-writer-locks/12345678-1234-4234-8234-123456789abc.lock",
            "tmp/arg0/codex-arg0Synthetic7/.lock",
        ];
        for relative in paths {
            let path = source.path().join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, b"synthetic runtime lock").unwrap();
        }

        for policy in [SourcePolicy::Codex, SourcePolicy::Project] {
            let destination = tempdir().expect("staging directory");
            let mut stats = StageStats::default();
            stage_tree(source.path(), destination.path(), &mut stats, policy).unwrap();

            assert!(stats.missing.is_empty());
            assert!(stats.exclusions.is_empty());
            assert!(stats.integrity_warnings.is_empty());
            if policy == SourcePolicy::Codex {
                assert_eq!(stats.notices.len(), 2);
                assert_eq!(stats.files, 0);
                assert_eq!(stats.bytes, 0);
                for relative in paths {
                    assert!(!destination.path().join(relative).exists());
                    assert!(stats.notices.iter().any(|issue| {
                        Path::new(&issue.path) == Path::new(relative)
                            && issue.kind == BackupIssueKind::RuntimeEphemeral
                    }));
                }
            } else {
                assert!(stats.notices.is_empty());
                assert_eq!(stats.files, 2);
                for relative in paths {
                    assert_eq!(
                        fs::read(destination.path().join(relative)).unwrap(),
                        b"synthetic runtime lock"
                    );
                }
            }
        }
    }

    #[test]
    fn runtime_lock_neighbors_are_retained() {
        let source = tempdir().expect("source directory");
        let paths = [
            "Cargo.lock",
            "package-lock.json",
            "pnpm-lock.yaml",
            "arbitrary.lock",
            "thread-writer-locks/.lock",
            "thread-writer-locks/not-a-uuid.lock",
            "thread-writer-locks/12345678-1234-4234-8234-123456789abg.lock",
            "thread-writer-locks/12345678123442348234123456789abc.lock",
            "thread-writer-locks/12345678-1234-4234-8234-123456789abc.txt",
            "thread-writer-locks/nested/12345678-1234-4234-8234-123456789abc.lock",
            "nested/thread-writer-locks/12345678-1234-4234-8234-123456789abc.lock",
            "other/12345678-1234-4234-8234-123456789abc.lock",
            "tmp/keep.txt",
            "tmp/arg0/.lock",
            "tmp/arg0/codex-arg0/.lock",
            "tmp/arg0/otherSynthetic7/.lock",
            "tmp/arg0/codex-arg0Synthetic7/keep.txt",
            "tmp/arg0/codex-arg0Synthetic7/other.lock",
            "tmp/arg0/codex-arg0Synthetic7/nested/.lock",
            "tmp/arg1/codex-arg0Synthetic7/.lock",
            "nested/tmp/arg0/codex-arg0Synthetic7/.lock",
        ];
        for relative in paths {
            let path = source.path().join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, b"retained content").unwrap();
        }

        for policy in [SourcePolicy::Codex, SourcePolicy::Project] {
            let destination = tempdir().expect("staging directory");
            let mut stats = StageStats::default();
            stage_tree(source.path(), destination.path(), &mut stats, policy).unwrap();

            assert_eq!(stats.files, paths.len() as u64);
            assert!(stats.notices.is_empty());
            assert!(stats.missing.is_empty());
            assert!(stats.exclusions.is_empty());
            assert!(stats.integrity_warnings.is_empty());
            for relative in paths {
                assert_eq!(
                    fs::read(destination.path().join(relative)).unwrap(),
                    b"retained content",
                    "fixture {relative} must be retained"
                );
            }
        }
    }

    #[test]
    fn legacy_manifest_defaults_new_issue_fields() {
        let manifest: BackupManifest = serde_json::from_value(serde_json::json!({
            "format": BACKUP_FORMAT,
            "schema_version": 1,
            "logical_backup_id": Uuid::nil(),
            "batch_id": Uuid::nil(),
            "created_at": "2026-09-20T00:00:00Z",
            "app_version": "0.1.6",
            "restic_version": "restic synthetic",
            "source_device_id": Uuid::nil(),
            "source_codex_home": "C:/Synthetic/.codex",
            "project_paths": ["C:/Synthetic/project"],
            "git_metadata_paths": [],
            "file_count": 1,
            "byte_count": 7,
            "fingerprint": "synthetic",
            "exclusions": [{"path":"auth.json","reason":"legacy exclusion","bytes":7}],
            "missing": [],
            "integrity_status": "complete"
        }))
        .expect("legacy manifest");

        assert!(manifest.notices.is_empty());
        assert!(manifest.integrity_warnings.is_empty());
        assert_eq!(manifest.exclusions[0].kind, BackupIssueKind::LegacyUnknown);
    }

    #[test]
    fn dependency_and_local_audit_redirects_are_notices() {
        assert_eq!(
            redirect_notice_kind(Path::new("node_modules/.pnpm/pkg/node_modules/pkg")),
            Some(BackupIssueKind::RebuildableDependency)
        );
        assert_eq!(
            redirect_notice_kind(Path::new(".local-audit/fixtures/reparse")),
            Some(BackupIssueKind::TestArtifact)
        );
    }

    #[test]
    fn ordinary_redirect_remains_missing() {
        assert_eq!(redirect_notice_kind(Path::new("src/shared")), None);
    }

    #[test]
    fn legacy_issue_kinds_invalidate_cache_reuse() {
        let legacy_issue = BackupIssue {
            path: "auth.json".into(),
            reason: "legacy exclusion".into(),
            bytes: 7,
            kind: BackupIssueKind::LegacyUnknown,
        };
        let mut previous = cached_state(BackupManifest {
            format: BACKUP_FORMAT.into(),
            schema_version: BACKUP_SCHEMA_VERSION,
            logical_backup_id: Uuid::nil(),
            batch_id: Uuid::nil(),
            created_at: "2026-09-20T00:00:00Z".into(),
            app_version: "0.1.6".into(),
            restic_version: "restic synthetic".into(),
            source_device_id: Uuid::nil(),
            source_codex_home: "C:/Synthetic/.codex".into(),
            project_paths: vec!["C:/Synthetic/project".into()],
            git_metadata_paths: vec![],
            file_count: 1,
            byte_count: 7,
            fingerprint: "synthetic".into(),
            exclusions: vec![legacy_issue.clone()],
            notices: vec![],
            integrity_warnings: vec![],
            missing: vec![],
            integrity_status: "complete".into(),
        });

        assert!(!can_reuse_cached_snapshot(&previous, "synthetic"));
        previous.snapshot.manifest.exclusions.clear();
        previous
            .snapshot
            .manifest
            .notices
            .push(legacy_issue.clone());
        assert!(!can_reuse_cached_snapshot(&previous, "synthetic"));
        previous.snapshot.manifest.notices.clear();
        previous
            .snapshot
            .manifest
            .integrity_warnings
            .push(legacy_issue.clone());
        assert!(!can_reuse_cached_snapshot(&previous, "synthetic"));
        previous.snapshot.manifest.integrity_warnings.clear();
        previous.snapshot.manifest.missing.push(legacy_issue);
        assert!(!can_reuse_cached_snapshot(&previous, "synthetic"));
    }

    #[test]
    fn current_typed_issue_kinds_allow_cache_reuse() {
        let mut previous = cached_state(BackupManifest {
            format: BACKUP_FORMAT.into(),
            schema_version: BACKUP_SCHEMA_VERSION,
            logical_backup_id: Uuid::nil(),
            batch_id: Uuid::nil(),
            created_at: "2026-09-20T00:00:00Z".into(),
            app_version: "0.1.6".into(),
            restic_version: "restic synthetic".into(),
            source_device_id: Uuid::nil(),
            source_codex_home: "C:/Synthetic/.codex".into(),
            project_paths: vec!["C:/Synthetic/project".into()],
            git_metadata_paths: vec![],
            file_count: 1,
            byte_count: 7,
            fingerprint: "synthetic".into(),
            exclusions: vec![BackupIssue {
                path: "auth.json".into(),
                reason: "credential excluded".into(),
                bytes: 7,
                kind: BackupIssueKind::SecurityExclusion,
            }],
            notices: vec![],
            integrity_warnings: vec![],
            missing: vec![],
            integrity_status: "complete".into(),
        });

        assert!(can_reuse_cached_snapshot(&previous, "synthetic"));
        previous.snapshot.manifest.exclusions.clear();
        assert!(can_reuse_cached_snapshot(&previous, "synthetic"));
    }

    fn cached_state(manifest: BackupManifest) -> BackupState {
        BackupState {
            fingerprint: manifest.fingerprint.clone(),
            snapshot: LocalSnapshot {
                logical_backup_id: manifest.logical_backup_id,
                restic_snapshot_id: "snapshot".into(),
                manifest,
                complete: true,
            },
        }
    }

    #[test]
    fn rejects_non_repository_directory_without_modifying_it() {
        let repository = tempdir().unwrap();
        let sentinel = repository.path().join("keep.txt");
        fs::write(
            repository.path().join("config"),
            b"not a complete repository",
        )
        .unwrap();
        fs::write(&sentinel, b"existing user data").unwrap();

        let error = ensure_repository(
            repository.path(),
            "synthetic-password",
            &repository.path().join("missing-restic.exe"),
        )
        .unwrap_err();

        assert_eq!(error.code, ErrorCode::BackupRepositoryInvalid);
        assert_eq!(fs::read(sentinel).unwrap(), b"existing user data");
    }

    #[test]
    fn valid_repository_layout_reaches_restic_validation() {
        let repository = tempdir().unwrap();
        fs::write(repository.path().join("config"), b"synthetic config").unwrap();
        for directory in ["data", "index", "keys", "locks", "snapshots"] {
            fs::create_dir(repository.path().join(directory)).unwrap();
        }

        let error = ensure_repository(
            repository.path(),
            "synthetic-password",
            &repository.path().join("missing-restic.exe"),
        )
        .unwrap_err();

        assert_eq!(error.code, ErrorCode::BackupEngineUnavailable);
    }

    #[test]
    fn classifies_restic_failures_and_redacts_the_password_file_path() {
        let password_file = std::path::Path::new(r"C:\Temp\secret-password.txt");
        for (stderr, expected) in [
            (
                "Fatal: wrong password or no key found",
                ErrorCode::BackupPasswordRequired,
            ),
            (
                "Fatal: repository does not exist: unable to open config file",
                ErrorCode::BackupRepositoryInvalid,
            ),
            (
                "write failed: There is not enough space on the disk",
                ErrorCode::DiskSpaceInsufficient,
            ),
        ] {
            let error = super::backup_engine_failed(Some(10), stderr.as_bytes(), password_file);
            assert_eq!(error.code, expected, "stderr: {stderr}");
        }

        let error = super::backup_engine_failed(
            Some(1),
            format!("unexpected failure reading {}", password_file.display()).as_bytes(),
            password_file,
        );
        assert_eq!(error.code, ErrorCode::BackupFailed);
        assert!(!error.message.contains(&password_file.display().to_string()));
        assert!(error.message.contains("<temporary-password-file>"));

        let delayed_failure = format!(
            "{} Fatal: wrong password or no key found",
            "non-fatal warning ".repeat(80)
        );
        let error = super::backup_engine_failed(Some(1), delayed_failure.as_bytes(), password_file);
        assert_eq!(error.code, ErrorCode::BackupPasswordRequired);
        assert!(error.message.contains("wrong password or no key found"));
    }

    #[test]
    fn restored_manifest_never_overwrites_an_existing_file() {
        let source = tempdir().unwrap();
        let destination = tempdir().unwrap();
        fs::write(source.path().join("manifest.json"), b"new manifest").unwrap();
        fs::write(
            destination.path().join("manifest.json"),
            b"existing manifest",
        )
        .unwrap();
        assert!(super::copy_tree_for_restore(source.path(), destination.path()).is_err());
        assert_eq!(
            fs::read(destination.path().join("manifest.json")).unwrap(),
            b"existing manifest"
        );
    }

    #[test]
    fn full_file_restore_preserves_files_named_like_temporary_outputs() {
        let source = tempdir().unwrap();
        let destination = tempdir().unwrap();
        fs::write(source.path().join("same.enhe-partial"), b"first").unwrap();
        fs::write(source.path().join("same.txt"), b"second").unwrap();
        super::copy_tree_for_restore(source.path(), destination.path()).unwrap();
        assert_eq!(
            fs::read(destination.path().join("same.enhe-partial")).unwrap(),
            b"first"
        );
        assert_eq!(
            fs::read(destination.path().join("same.txt")).unwrap(),
            b"second"
        );
    }

    #[test]
    fn snapshot_file_count_includes_unchanged_files_but_not_internal_manifest() {
        let summaries = parse_snapshot_summaries(
            br#"[{"id":"12345678","summary":{"files_new":1,"files_changed":2,"files_unmodified":8,"total_files_processed":11}}]"#,
        )
        .unwrap();
        assert_eq!(summaries[0].file_count, 10);
    }

    #[test]
    fn warning_snapshot_summary_remains_complete() {
        let summaries =
            parse_snapshot_summaries(br#"[{"id":"12345678","tags":["integrity_status=warning"]}]"#)
                .unwrap();

        assert_eq!(summaries[0].integrity_status, "warning");
        assert!(summaries[0].complete);
    }

    #[test]
    fn large_valid_jsonl_is_not_an_issue() {
        let root = tempdir().expect("temp directory");
        let path = root.path().join("large.jsonl");
        let value = serde_json::json!({"text": "x".repeat(8 * 1024 * 1024 + 1)});
        fs::write(&path, format!("{}\n", value)).expect("write large JSONL");

        assert_eq!(
            inspect_jsonl(&path, fs::metadata(&path).unwrap().len()).unwrap(),
            None
        );
    }

    #[test]
    fn invalid_jsonl_is_preserved_as_integrity_warning() {
        let root = tempdir().expect("source directory");
        let destination = tempdir().expect("staging directory");
        fs::write(
            root.path().join("session.jsonl"),
            br#"{"ok":true}
{"incomplete":"#,
        )
        .expect("write JSONL fixture");

        let mut stats = StageStats::default();
        stage_tree(
            root.path(),
            destination.path(),
            &mut stats,
            SourcePolicy::Codex,
        )
        .expect("stage JSONL");

        assert!(destination.path().join("session.jsonl").is_file());
        assert_eq!(stats.files, 1);
        assert!(stats.missing.is_empty());
        assert_eq!(stats.integrity_warnings.len(), 1);
        assert_eq!(
            stats.integrity_warnings[0].kind,
            BackupIssueKind::JsonlIntegrity
        );
    }
}
