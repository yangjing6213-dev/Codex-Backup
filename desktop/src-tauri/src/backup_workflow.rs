use crate::core::{
    app_config::{self, AppConfig},
    backup::managed_backup_root,
    cloud::{self, CloudConfig, CloudConfigStart, CloudOperationResult},
    error::{ErrorCode, RehomeError},
    paths::user_facing_path,
    restic::{self, LocalBackupRequest, LocalRestoreReport, LocalSnapshot, LocalSnapshotSummary},
    scheduler::{self, SchedulerStatus},
};
use serde::{Deserialize, Serialize};
use std::{
    env,
    fs::{self, OpenOptions},
    io::ErrorKind,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
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
pub fn run_local_backup(request: LocalBackupCommandRequest) -> Result<LocalSnapshot, RehomeError> {
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
}

#[tauri::command]
pub fn list_local_backups(
    request: LocalBackupListRequest,
) -> Result<Vec<LocalSnapshotSummary>, RehomeError> {
    let password = resolve_password(&request.recovery_password)?;
    restic::list_local_snapshots(&request.repository, &password, &restic_path())
}

#[tauri::command]
pub fn restore_local_backup(
    request: LocalRestoreCommandRequest,
) -> Result<LocalRestoreReport, RehomeError> {
    let password = resolve_password(&request.recovery_password)?;
    restic::restore_local(
        &request.snapshot_id,
        &request.repository,
        &password,
        &request.target,
        &restic_path(),
    )
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

struct WorkerLock {
    path: PathBuf,
}

impl WorkerLock {
    fn acquire(path: &Path) -> Result<Self, RehomeError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                RehomeError::new(
                    ErrorCode::SchedulerUnavailable,
                    format!("could not create worker lock directory: {error}"),
                )
            })?;
        }
        match OpenOptions::new().write(true).create_new(true).open(path) {
            Ok(file) => {
                let _ = serde_json::to_writer(
                    file,
                    &serde_json::json!({ "pid": std::process::id(), "started_at": chrono::Utc::now() }),
                );
                Ok(Self {
                    path: path.to_path_buf(),
                })
            }
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                let stale = fs::metadata(path)
                    .and_then(|metadata| metadata.modified())
                    .ok()
                    .and_then(|modified| SystemTime::now().duration_since(modified).ok())
                    .is_some_and(|age| age > Duration::from_secs(24 * 60 * 60));
                if stale {
                    let _ = fs::remove_file(path);
                    return Self::acquire(path);
                }
                Err(RehomeError::new(
                    ErrorCode::SchedulerUnavailable,
                    "another scheduled backup is already running",
                ))
            }
            Err(error) => Err(RehomeError::new(
                ErrorCode::SchedulerUnavailable,
                format!("could not create worker lock: {error}"),
            )),
        }
    }
}

impl Drop for WorkerLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn worker_lock_path() -> Result<PathBuf, RehomeError> {
    Ok(app_config::default_config_path()?
        .parent()
        .ok_or_else(|| {
            RehomeError::new(
                ErrorCode::SchedulerUnavailable,
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
