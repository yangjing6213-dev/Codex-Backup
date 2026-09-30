use crate::core::{
    app_config::{self, AppConfig},
    backup::managed_backup_root,
    cloud::{self, CloudConfig, CloudConfigStart, CloudOperationResult},
    error::{ErrorCode, RehomeError},
    paths::user_facing_path,
    project_links::is_filesystem_redirect,
    restic::{self, LocalBackupRequest, LocalRestoreReport, LocalSnapshot, LocalSnapshotSummary},
    scheduler::{self, SchedulerStatus},
    stable_fs::PinnedParent,
};
use serde::{Deserialize, Serialize};
use std::{
    env,
    fs::{self, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalBackupCommandRequest {
    pub codex_home: PathBuf,
    pub project_paths: Vec<PathBuf>,
    pub repository: PathBuf,
    pub recovery_password: String,
    #[serde(default)]
    pub remember_password: bool,
    #[serde(default)]
    pub source_device_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalBackupListRequest {
    pub repository: PathBuf,
    pub recovery_password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalRestoreCommandRequest {
    pub snapshot_id: String,
    pub repository: PathBuf,
    pub recovery_password: String,
    pub target: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SchedulerCommandRequest {
    pub enabled: bool,
    pub interval_minutes: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CloudConfigStartRequest {
    pub config_file: PathBuf,
    pub remote_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CloudConfigAnswerRequest {
    pub config_file: PathBuf,
    pub remote_name: String,
    pub state: String,
    pub result: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CloudUploadCommandRequest {
    pub config: CloudConfig,
    pub source_repository: PathBuf,
    pub source_password: String,
    pub target_password: String,
    pub snapshot_id: String,
}

#[tauri::command]
pub fn get_app_config() -> Result<AppConfig, RehomeError> {
    let path = app_config::default_config_path()?;
    let original = app_config::load_config(&path)?;
    let mut config = normalize_user_paths(original.clone());
    if config.local_repository.is_none() {
        if let Ok(default_repository) = managed_backup_root() {
            config.local_repository = Some(user_facing_path(&default_repository));
        }
    }
    if config != original {
        app_config::save_config(&path, &config)?;
    }
    Ok(config)
}

#[tauri::command]
pub fn save_app_config(config: AppConfig) -> Result<AppConfig, RehomeError> {
    let config = normalize_user_paths(config);
    app_config::save_config(&app_config::default_config_path()?, &config)?;
    Ok(config)
}

fn normalize_user_paths(mut config: AppConfig) -> AppConfig {
    config.codex_home = config.codex_home.as_deref().map(user_facing_path);
    config.local_repository = config.local_repository.as_deref().map(user_facing_path);
    config.selected_project_paths = config
        .selected_project_paths
        .iter()
        .map(|path| user_facing_path(path))
        .collect();
    config
}

#[tauri::command]
pub async fn run_local_backup(
    request: LocalBackupCommandRequest,
) -> Result<LocalSnapshot, RehomeError> {
    run_blocking(ErrorCode::BackupFailed, move || {
        let _lock = WorkerLock::acquire(&worker_lock_path()?)?;
        let retention = app_config::load_config(&app_config::default_config_path()?)
            .map(|config| config.retention)
            .unwrap_or_default();
        if request.remember_password {
            app_config::protect_secret(
                &app_config::default_secret_path()?,
                &request.recovery_password,
            )?;
        }
        let source_device_id = request.source_device_id.unwrap_or_else(Uuid::new_v4);
        let repository = request.repository.clone();
        let password = request.recovery_password.clone();
        let snapshot = restic::backup_local(
            LocalBackupRequest {
                codex_home: request.codex_home,
                project_paths: request.project_paths,
                repository: request.repository,
                password: request.recovery_password,
                source_device_id,
            },
            &restic_path(),
        )?;
        if snapshot.complete {
            restic::apply_retention(
                &repository,
                &password,
                &restic_path(),
                retention.high_frequency_hours,
                retention.daily_days,
                retention.weekly_weeks,
            )?;
        }
        Ok(snapshot)
    })
    .await
}

#[tauri::command]
pub async fn list_local_backups(
    request: LocalBackupListRequest,
) -> Result<Vec<LocalSnapshotSummary>, RehomeError> {
    run_blocking(ErrorCode::BackupFailed, move || {
        let password = resolve_password(&request.recovery_password)?;
        restic::list_local_snapshots(&request.repository, &password, &restic_path())
    })
    .await
}

#[tauri::command]
pub async fn restore_local_backup(
    request: LocalRestoreCommandRequest,
) -> Result<LocalRestoreReport, RehomeError> {
    run_blocking(ErrorCode::RestoreFailed, move || {
        let _lock = WorkerLock::acquire(&worker_lock_path()?)?;
        let password = resolve_password(&request.recovery_password)?;
        restic::restore_local(
            &request.snapshot_id,
            &request.repository,
            &password,
            &request.target,
            &restic_path(),
        )
    })
    .await
}

#[tauri::command]
pub fn run_due_backup() -> Result<LocalSnapshot, RehomeError> {
    let _lock = WorkerLock::acquire(&worker_lock_path()?)?;
    let config = app_config::load_config(&app_config::default_config_path()?)?;
    if !config.automatic_backup_enabled {
        return Err(RehomeError::new(
            ErrorCode::ConfigInvalid,
            "scheduled backup is disabled",
        ));
    }
    let codex_home = config.codex_home.ok_or_else(|| {
        RehomeError::new(
            ErrorCode::ConfigInvalid,
            "Codex data location is not configured",
        )
    })?;
    let repository = config.local_repository.ok_or_else(|| {
        RehomeError::new(
            ErrorCode::ConfigInvalid,
            "local backup repository is not configured",
        )
    })?;
    let password = app_config::unprotect_secret(&app_config::default_secret_path()?)?;
    let snapshot = restic::backup_local(
        LocalBackupRequest {
            codex_home,
            project_paths: config.selected_project_paths,
            repository: repository.clone(),
            password: password.clone(),
            source_device_id: Uuid::new_v4(),
        },
        &restic_path(),
    )?;
    if snapshot.complete {
        restic::apply_retention(
            &repository,
            &password,
            &restic_path(),
            config.retention.high_frequency_hours,
            config.retention.daily_days,
            config.retention.weekly_weeks,
        )?;
    }
    Ok(snapshot)
}

#[tauri::command]
pub fn get_scheduler_status() -> Result<SchedulerStatus, RehomeError> {
    scheduler::query_current_user_task()
}

#[tauri::command]
pub fn set_scheduler(request: SchedulerCommandRequest) -> Result<SchedulerStatus, RehomeError> {
    let mut config = app_config::load_config(&app_config::default_config_path()?)?;
    let previous_config = config.clone();
    config.frequency_minutes = request.interval_minutes;
    config.automatic_backup_enabled = request.enabled;
    let status = if request.enabled {
        scheduler::register_current_user_task(&current_executable()?, request.interval_minutes)
    } else {
        scheduler::unregister_current_user_task()?;
        Ok(SchedulerStatus {
            enabled: false,
            task_name: scheduler::TASK_NAME.to_owned(),
            worker_path: None,
            interval_minutes: Some(request.interval_minutes),
            next_run_time: None,
            last_result: None,
            message: Some("scheduled backup is disabled".to_owned()),
        })
    }?;
    if let Err(error) = app_config::save_config(&app_config::default_config_path()?, &config) {
        if request.enabled {
            let _ = scheduler::unregister_current_user_task();
        } else if previous_config.automatic_backup_enabled {
            let _ = scheduler::register_current_user_task(
                &current_executable()?,
                previous_config.frequency_minutes,
            );
        }
        return Err(error);
    }
    Ok(status)
}

#[tauri::command]
pub fn start_cloud_configuration(
    request: CloudConfigStartRequest,
) -> Result<CloudConfigStart, RehomeError> {
    cloud::start_one_drive_configuration(&request.config_file, &request.remote_name, &rclone_path())
}

#[tauri::command]
pub fn continue_cloud_configuration(
    request: CloudConfigAnswerRequest,
) -> Result<CloudConfigStart, RehomeError> {
    cloud::continue_one_drive_configuration(
        &request.config_file,
        &request.remote_name,
        &request.state,
        &request.result,
        &rclone_path(),
    )
}

#[tauri::command]
pub fn test_cloud_connection(config: CloudConfig) -> Result<CloudOperationResult, RehomeError> {
    cloud::test_one_drive_connection(&config, &rclone_path())
}

#[tauri::command]
pub fn upload_local_snapshot(
    request: CloudUploadCommandRequest,
) -> Result<CloudOperationResult, RehomeError> {
    cloud::copy_local_snapshot_to_cloud(
        &request.config,
        &request.source_repository,
        &request.source_password,
        &request.target_password,
        &request.snapshot_id,
        &restic_path(),
        &rclone_path(),
    )
}

async fn run_blocking<T, F>(code: ErrorCode, operation: F) -> Result<T, RehomeError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, RehomeError> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(operation)
        .await
        .map_err(|error| RehomeError::new(code, format!("background operation failed: {error}")))?
}

fn resolve_password(explicit: &str) -> Result<String, RehomeError> {
    if !explicit.is_empty() {
        return Ok(explicit.to_owned());
    }
    app_config::unprotect_secret(&app_config::default_secret_path()?).map_err(|_| {
        RehomeError::new(
            ErrorCode::BackupPasswordRequired,
            "enter the recovery password or save one for this Windows user",
        )
    })
}

// Keep this guard inside the blocking worker, so dropping an async caller cannot
// release it while restic (including retention) is still mutating local data.
struct WorkerLock {
    _file: fs::File,
    _parent: PinnedParent,
}

const WORKER_LOCK_MARKER: &[u8] = b"\nENHE-CODEX-BACKUP-WORKER-LOCK-V2\n";

impl WorkerLock {
    fn acquire(path: &Path) -> Result<Self, RehomeError> {
        let parent = path
            .parent()
            .ok_or_else(|| lock_unavailable("worker lock directory is unavailable"))?;
        fs::create_dir_all(parent).map_err(|error| {
            lock_unavailable(format!("could not create worker lock directory: {error}"))
        })?;
        let parent = PinnedParent::open(parent).map_err(|error| {
            lock_unavailable(format!("could not pin worker lock directory: {error}"))
        })?;
        let mut options = OpenOptions::new();
        options.read(true).write(true).create_new(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            use windows_sys::Win32::Storage::FileSystem::{
                FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ, FILE_SHARE_WRITE,
            };
            options
                .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
                .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW);
        }
        let (mut file, created) = match options.open(path) {
            Ok(file) => (file, true),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                options.create_new(false);
                (
                    options.open(path).map_err(|error| {
                        lock_unavailable(format!("could not open worker lock: {error}"))
                    })?,
                    false,
                )
            }
            Err(error) => {
                return Err(lock_unavailable(format!(
                    "could not create worker lock: {error}"
                )))
            }
        };
        file.try_lock().map_err(|error| match error {
            fs::TryLockError::WouldBlock => operation_in_progress(),
            fs::TryLockError::Error(error) => {
                lock_unavailable(format!("could not acquire worker lock: {error}"))
            }
        })?;
        let metadata = file
            .metadata()
            .map_err(|error| lock_unavailable(format!("could not inspect worker lock: {error}")))?;
        if !metadata.is_file()
            || is_filesystem_redirect(&metadata)
            || worker_lock_link_count(&file)? != 1
        {
            return Err(lock_unavailable(
                "worker lock must be an ordinary, unshared file",
            ));
        }
        // Never rewrite metadata on ordinary reuse. A fixed format marker can
        // be completed after a partial initialization without truncating the
        // original legacy ownership evidence or replacing the locked file.
        let mut previous = Vec::new();
        Read::by_ref(&mut file)
            .take(4097)
            .read_to_end(&mut previous)
            .map_err(|error| lock_unavailable(format!("could not read worker lock: {error}")))?;
        if previous.len() > 4096 {
            return Err(lock_unavailable("worker lock metadata is too large"));
        }
        if !previous.ends_with(WORKER_LOCK_MARKER) {
            let mut marker_offset = 0;
            if !created {
                // An old release may have created this file but not written its
                // PID yet. An OS lock alone cannot establish legacy ownership.
                if previous.is_empty() {
                    return Err(lock_unavailable("existing empty worker lock may belong to an initializing older application; close older instances before repairing it"));
                }
                if !WORKER_LOCK_MARKER.starts_with(&previous) {
                    let mut legacy = serde_json::Deserializer::from_slice(&previous)
                        .into_iter::<serde_json::Value>();
                    let metadata = legacy.next().ok_or_else(|| lock_unavailable("worker lock metadata is invalid"))?
                        .map_err(|_| lock_unavailable("worker lock metadata is invalid; close older app instances before repairing the lock"))?;
                    marker_offset = legacy.byte_offset() as u64;
                    let tail = &previous[marker_offset as usize..];
                    if !WORKER_LOCK_MARKER.starts_with(tail) {
                        return Err(lock_unavailable(
                            "worker lock has unrecognized trailing metadata",
                        ));
                    }
                    let pid = metadata
                        .get("pid")
                        .and_then(|v| v.as_u64())
                        .and_then(|v| u32::try_from(v).ok())
                        .filter(|pid| *pid != 0)
                        .ok_or_else(|| {
                            lock_unavailable("legacy worker lock has no valid process ID")
                        })?;
                    let started_at = metadata
                        .get("started_at")
                        .and_then(|value| value.as_str())
                        .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
                        .ok_or_else(|| {
                            lock_unavailable("legacy worker lock has no valid start time")
                        })?;
                    if legacy_process_running(pid, started_at)? {
                        return Err(operation_in_progress());
                    }
                }
            }
            file.seek(SeekFrom::Start(marker_offset))
                .and_then(|_| file.write_all(WORKER_LOCK_MARKER))
                .and_then(|_| file.sync_all())
                .map_err(|error| {
                    lock_unavailable(format!("could not initialize worker lock marker: {error}"))
                })?;
        }
        // Do not unlink: deleting the path can let a second process lock a new
        // file while another worker still owns the original one.
        Ok(Self {
            _file: file,
            _parent: parent,
        })
    }
}

#[cfg(windows)]
fn worker_lock_link_count(file: &fs::File) -> Result<u64, RehomeError> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
    };
    let mut information = BY_HANDLE_FILE_INFORMATION::default();
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut information) } == 0 {
        return Err(lock_unavailable(format!(
            "could not inspect worker lock links: {}",
            std::io::Error::last_os_error()
        )));
    }
    Ok(u64::from(information.nNumberOfLinks))
}

#[cfg(unix)]
fn worker_lock_link_count(file: &fs::File) -> Result<u64, RehomeError> {
    use std::os::unix::fs::MetadataExt;
    file.metadata()
        .map(|metadata| metadata.nlink())
        .map_err(|error| lock_unavailable(format!("could not inspect worker lock links: {error}")))
}

fn lock_unavailable(message: impl Into<String>) -> RehomeError {
    RehomeError::new(ErrorCode::BackupLockUnavailable, message)
}

fn operation_in_progress() -> RehomeError {
    RehomeError::new(
        ErrorCode::OperationInProgress,
        "another backup or restore is already running",
    )
}

#[cfg(windows)]
fn legacy_process_running(
    pid: u32,
    started_at: chrono::DateTime<chrono::FixedOffset>,
) -> Result<bool, RehomeError> {
    use windows_sys::Win32::{
        Foundation::{
            CloseHandle, GetLastError, ERROR_INVALID_PARAMETER, FILETIME, WAIT_OBJECT_0,
            WAIT_TIMEOUT,
        },
        System::Threading::{
            GetProcessTimes, OpenProcess, WaitForSingleObject, PROCESS_QUERY_LIMITED_INFORMATION,
            PROCESS_SYNCHRONIZE,
        },
    };
    unsafe {
        let process = OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
            0,
            pid,
        );
        if process.is_null() {
            let error = GetLastError();
            return if error == ERROR_INVALID_PARAMETER {
                Ok(false)
            } else {
                Err(lock_unavailable(format!(
                    "could not verify legacy worker process: Windows error {error}"
                )))
            };
        }
        let state = WaitForSingleObject(process, 0);
        if state != WAIT_TIMEOUT {
            let error = GetLastError();
            CloseHandle(process);
            return if state == WAIT_OBJECT_0 {
                Ok(false)
            } else {
                Err(lock_unavailable(format!(
                    "could not verify legacy worker exit: Windows error {error}"
                )))
            };
        }
        let mut creation = FILETIME::default();
        let mut exit = FILETIME::default();
        let mut kernel = FILETIME::default();
        let mut user = FILETIME::default();
        let success = GetProcessTimes(process, &mut creation, &mut exit, &mut kernel, &mut user);
        let error = GetLastError();
        CloseHandle(process);
        if success == 0 {
            return Err(lock_unavailable(format!(
                "could not query legacy worker process: Windows error {error}"
            )));
        }
        let creation_ticks =
            (u64::from(creation.dwHighDateTime) << 32) | u64::from(creation.dwLowDateTime);
        let started_ticks = (i128::from(started_at.timestamp()) + 11_644_473_600) * 10_000_000
            + i128::from(started_at.timestamp_subsec_nanos() / 100);
        // lpExitTime is undefined for a live process. Liveness comes from the
        // zero-time wait above; creation time only excludes a reused PID.
        Ok(i128::from(creation_ticks) <= started_ticks)
    }
}

#[cfg(unix)]
fn legacy_process_running(
    pid: u32,
    _started_at: chrono::DateTime<chrono::FixedOffset>,
) -> Result<bool, RehomeError> {
    let Ok(pid) = i32::try_from(pid) else {
        return Ok(false);
    };
    if unsafe { libc::kill(pid, 0) } == 0 {
        return Ok(true);
    }
    let error = std::io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::ESRCH) {
        Ok(false)
    } else {
        Err(lock_unavailable(format!(
            "could not verify legacy worker process: {error}"
        )))
    }
}

fn worker_lock_path() -> Result<PathBuf, RehomeError> {
    Ok(app_config::default_config_path()?
        .parent()
        .ok_or_else(|| {
            RehomeError::new(
                ErrorCode::BackupLockUnavailable,
                "worker lock directory is unavailable",
            )
        })?
        .join("backup-worker.lock"))
}

fn current_executable() -> Result<PathBuf, RehomeError> {
    env::current_exe().map_err(|error| {
        RehomeError::new(
            ErrorCode::SchedulerUnavailable,
            format!("could not locate ENHE worker executable: {error}"),
        )
    })
}

fn restic_path() -> PathBuf {
    tool_path("ENHE_RESTIC_PATH", "restic.exe")
}

fn rclone_path() -> PathBuf {
    tool_path("ENHE_RCLONE_PATH", "rclone.exe")
}

fn tool_path(variable: &str, name: &str) -> PathBuf {
    if let Some(path) = env::var_os(variable).map(PathBuf::from) {
        return path;
    }
    let executable = env::current_exe().unwrap_or_else(|_| PathBuf::from(name));
    let candidates = [
        executable
            .parent()
            .map(|parent| parent.join("resources").join(name)),
        executable.parent().map(|parent| parent.join(name)),
        Some(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("resources")
                .join(name),
        ),
    ];
    candidates
        .into_iter()
        .flatten()
        .find(|path| path.is_file())
        .unwrap_or_else(|| Path::new(name).to_path_buf())
}

#[cfg(test)]
mod worker_lock_tests {
    use super::*;
    use std::time::{Duration, SystemTime};

    #[test]
    fn abandoned_lock_is_reusable_without_waiting_a_day() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("backup-worker.lock");
        fs::write(
            &path,
            r#"{"pid":4294967295,"started_at":"2026-09-29T00:00:00Z"}"#,
        )
        .unwrap();
        let first = WorkerLock::acquire(&path).expect("an exited owner cannot block retry");
        drop(first);
        let second = WorkerLock::acquire(&path).expect("a completed worker releases ownership");
        drop(second);
    }

    #[test]
    fn live_file_lock_is_never_removed_because_of_its_age() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("backup-worker.lock");
        let owner = WorkerLock::acquire(&path).unwrap();
        owner
            ._file
            .set_times(
                fs::FileTimes::new()
                    .set_modified(SystemTime::now() - Duration::from_secs(2 * 24 * 60 * 60)),
            )
            .unwrap();
        let error = WorkerLock::acquire(&path)
            .err()
            .expect("a live lock must stay exclusive");
        assert_eq!(
            serde_json::to_value(error.code).unwrap(),
            "operation_in_progress"
        );
        drop(owner);
        WorkerLock::acquire(&path).expect("ownership ends when its file handle closes");
    }

    #[test]
    fn live_legacy_owner_is_not_overwritten() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("backup-worker.lock");
        let bytes = serde_json::to_vec(
            &serde_json::json!({"pid": std::process::id(), "started_at": chrono::Utc::now()}),
        )
        .unwrap();
        fs::write(&path, &bytes).unwrap();
        let error = WorkerLock::acquire(&path)
            .err()
            .expect("an older live worker still owns this lock");
        assert_eq!(
            serde_json::to_value(error.code).unwrap(),
            "operation_in_progress"
        );
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }

    #[test]
    fn lock_access_failure_is_distinct_from_scheduler_failure() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("backup-worker.lock");
        fs::create_dir(&path).unwrap();
        let error = WorkerLock::acquire(&path).err().unwrap();
        assert_eq!(
            serde_json::to_value(error.code).unwrap(),
            "backup_lock_unavailable"
        );
    }

    #[test]
    fn hardlinked_lock_never_modifies_another_file() {
        let root = tempfile::tempdir().unwrap();
        let original = root.path().join("original.json");
        let path = root.path().join("backup-worker.lock");
        let bytes = br#"{"lock_version":2,"sentinel":"preserve this file"}"#;
        fs::write(&original, bytes).unwrap();
        fs::hard_link(&original, &path).unwrap();
        let error = WorkerLock::acquire(&path)
            .err()
            .expect("a lock must not overwrite a shared file");
        assert_eq!(
            serde_json::to_value(error.code).unwrap(),
            "backup_lock_unavailable"
        );
        assert_eq!(fs::read(&original).unwrap(), bytes);
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }

    #[cfg(windows)]
    #[test]
    fn live_lock_path_cannot_be_deleted_or_renamed() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("backup-worker.lock");
        let parked = root.path().join("parked.lock");
        let owner = WorkerLock::acquire(&path).unwrap();
        assert!(fs::remove_file(&path).is_err());
        assert!(fs::rename(&path, &parked).is_err());
        assert!(path.is_file());
        assert!(!parked.exists());
        drop(owner);
        fs::remove_file(&path).expect("the file is no longer pinned after the operation");
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_lock_never_modifies_another_file() {
        let root = tempfile::tempdir().unwrap();
        let original = root.path().join("original.json");
        let path = root.path().join("backup-worker.lock");
        let bytes = br#"{"lock_version":2,"sentinel":"preserve this file"}"#;
        fs::write(&original, bytes).unwrap();
        std::os::unix::fs::symlink(&original, &path).unwrap();
        let error = WorkerLock::acquire(&path).err().unwrap();
        assert_eq!(
            serde_json::to_value(error.code).unwrap(),
            "backup_lock_unavailable"
        );
        assert_eq!(fs::read(&original).unwrap(), bytes);
    }

    #[test]
    fn existing_empty_lock_is_not_claimed_during_legacy_initialization() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("backup-worker.lock");
        let mut legacy = fs::File::create(&path).unwrap();
        let error = WorkerLock::acquire(&path)
            .err()
            .expect("an existing empty file may belong to an initializing legacy worker");
        assert_eq!(error.code, ErrorCode::BackupLockUnavailable);
        assert!(fs::read(&path).unwrap().is_empty());
        let bytes = serde_json::to_vec(&serde_json::json!({
            "pid": std::process::id(), "started_at": chrono::Utc::now()
        }))
        .unwrap();
        legacy.write_all(&bytes).unwrap();
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }

    #[test]
    fn interrupted_current_lock_marker_can_be_repaired_without_replacing_the_file() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("backup-worker.lock");
        for written in 1..WORKER_LOCK_MARKER.len() {
            for legacy_prefix in [
                b"".as_slice(),
                br#"{"pid":4294967295,"started_at":"2026-09-29T00:00:00Z"}"#,
            ] {
                let bytes = [legacy_prefix, &WORKER_LOCK_MARKER[..written]].concat();
                fs::write(&path, &bytes).unwrap();
                let owner = WorkerLock::acquire(&path)
                    .expect("a recognized interrupted initialization must permit retry");
                drop(owner);
                assert!(fs::read(&path).unwrap().ends_with(WORKER_LOCK_MARKER));
                let completed = fs::read(&path).unwrap();
                drop(WorkerLock::acquire(&path).unwrap());
                assert_eq!(
                    fs::read(&path).unwrap(),
                    completed,
                    "reuse must not rewrite the marker"
                );
            }
        }
    }

    #[cfg(windows)]
    #[test]
    fn reused_legacy_pid_does_not_block_a_new_backup() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("backup-worker.lock");
        fs::write(
            &path,
            serde_json::to_vec(&serde_json::json!({
                "pid": std::process::id(), "started_at": "2000-01-01T00:00:00Z"
            }))
            .unwrap(),
        )
        .unwrap();
        WorkerLock::acquire(&path).expect("a process created after the legacy task cannot own it");
    }
}
