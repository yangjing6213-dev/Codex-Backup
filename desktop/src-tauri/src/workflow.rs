use crate::core::{
    app_config,
    app_server::SystemCodexAccessVerifier,
    backup::managed_backup_root,
    bridge::register_project_with_detected_cli,
    discovery::discover_codex as core_discover_codex,
    error::{ErrorCode, RehomeError},
    local_discovery::{
        count_project_files as core_count_project_files,
        discover_local_candidates as core_discover_local_candidates, LocalDiscoveryResult,
        LocalProjectCandidate, LocalScanRequest,
    },
    models::{
        CodexInventory, ContinuationProbeOptions, CreatePackageReport, CreatePackageRequest,
        FileConflictResolution, MigrationJobSnapshot, MigrationJobStage, MigrationJobStatus,
        PackagePreview, RecoveryStatus, RegistrationStatus, RestoreOptions, RestorePlan,
        RestoreReport, RollbackReport, SourceOs, TargetInventory, TransactionHistory,
        TransactionSummary,
    },
    package::{
        create_package_replacing as core_create_package_replacing,
        inspect_package as core_inspect_package,
    },
    paths::user_facing_path,
    planner::build_restore_plan_with_conflict_resolution as core_build_restore_plan,
    restore::{
        apply_restore_by_id, apply_restore_with_services,
        list_transaction_history as core_list_transaction_history, rollback as core_rollback,
        transaction_summary as core_transaction_summary, RestoreProgressEvent, RestoreProgressSink,
    },
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    env, fs,
    path::{Component, Path, PathBuf, Prefix},
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tauri::{AppHandle, State, WebviewWindow};
use tauri_plugin_dialog::{DialogExt, FilePath};
use tauri_plugin_opener::OpenerExt;
use uuid::Uuid;

const GRANT_TTL: Duration = Duration::from_secs(15 * 60);

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreatePackageSelection {
    pub project_ids: Vec<Uuid>,
    pub conversation_ids: Vec<Uuid>,
    pub skill_ids: Vec<Uuid>,
    pub plugin_ids: Vec<Uuid>,
    pub generated_image_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CreatedPackage {
    #[serde(flatten)]
    pub report: CreatePackageReport,
    pub archive_hash: String,
    pub reveal_id: Uuid,
}

#[derive(Debug, Clone, Serialize)]
pub struct InspectedPackage {
    pub selection_id: Uuid,
    #[serde(flatten)]
    pub preview: PackagePreview,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum BuildRestorePlanRequest {
    SelectDestinations {
        package_selection_id: Uuid,
    },
    Build {
        package_selection_id: Uuid,
        destination_selection_id: Uuid,
        conflict_resolution: Option<FileConflictResolution>,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum BuildRestorePlanResponse {
    Destinations {
        selection_id: Uuid,
        target_codex_home: PathBuf,
        projects_root: PathBuf,
        backup_root: PathBuf,
    },
    Plan {
        plan: RestorePlan,
    },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplyRestoreSelection {
    pub plan_id: Uuid,
    pub codex_closed_confirmed: bool,
    pub register_projects: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MigrateAndConnectSelection {
    pub plan_id: Uuid,
    pub codex_closed_confirmed: bool,
    pub register_projects: bool,
    pub probe_thread_id: Uuid,
    pub online_usage_confirmed: bool,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RollbackAction {
    Rollback,
    Resume,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RollbackSelection {
    pub transaction_id: Uuid,
    pub action: RollbackAction,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum OpenPathSelection {
    Granted { object_id: Uuid },
    Transaction { path: PathBuf, transaction_id: Uuid },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpenRestoredThreadSelection {
    pub path: PathBuf,
    pub transaction_id: Uuid,
}

#[derive(Clone)]
pub struct WorkflowState {
    inner: Arc<Mutex<WorkflowGrants>>,
    migration_jobs: MigrationJobs,
    local_discovery_cancel: Arc<AtomicBool>,
}

impl Default for WorkflowState {
    fn default() -> Self {
        Self {
            inner: Arc::new(Mutex::new(WorkflowGrants::default())),
            migration_jobs: MigrationJobs::default(),
            local_discovery_cancel: Arc::new(AtomicBool::new(false)),
        }
    }
}

#[derive(Clone, Default)]
struct MigrationJobs {
    inner: Arc<Mutex<MigrationJobStore>>,
}

#[derive(Default)]
struct MigrationJobStore {
    records: HashMap<Uuid, MigrationJobRecord>,
    terminal_order: VecDeque<Uuid>,
}

struct MigrationJobRecord {
    snapshot: MigrationJobSnapshot,
    rollback_outcome: Option<MigrationJobStatus>,
}

impl MigrationJobs {
    fn store(&self) -> std::sync::MutexGuard<'_, MigrationJobStore> {
        self.inner.lock().unwrap_or_else(|error| error.into_inner())
    }

    fn start(&self, plan_id: Uuid) -> MigrationJobSnapshot {
        let snapshot = MigrationJobSnapshot {
            job_id: Uuid::new_v4(),
            plan_id,
            transaction_id: None,
            stage: MigrationJobStage::Preflight,
            status: MigrationJobStatus::Running,
            report: None,
            error: None,
            updated_at: chrono::Utc::now().to_rfc3339(),
        };
        self.store().records.insert(
            snapshot.job_id,
            MigrationJobRecord {
                snapshot: snapshot.clone(),
                rollback_outcome: None,
            },
        );
        snapshot
    }

    fn get(&self, job_id: Uuid) -> Result<MigrationJobSnapshot, RehomeError> {
        self.store()
            .records
            .get(&job_id)
            .map(|record| record.snapshot.clone())
            .ok_or_else(migration_job_not_found)
    }

    fn record_progress(
        &self,
        job_id: Uuid,
        event: RestoreProgressEvent,
    ) -> Result<(), RehomeError> {
        let mut store = self.store();
        let record = store
            .records
            .get_mut(&job_id)
            .ok_or_else(migration_job_not_found)?;
        if record.snapshot.status != MigrationJobStatus::Running {
            return Ok(());
        }
        match event {
            RestoreProgressEvent::Stage(stage) => record.snapshot.stage = stage,
            RestoreProgressEvent::TransactionPrepared(id) => {
                record.snapshot.transaction_id = Some(id)
            }
            RestoreProgressEvent::RollbackCompleted => {
                record.rollback_outcome = Some(MigrationJobStatus::RolledBack)
            }
            RestoreProgressEvent::RollbackFailed => {
                record.rollback_outcome = Some(MigrationJobStatus::RollbackFailed)
            }
        }
        record.snapshot.updated_at = chrono::Utc::now().to_rfc3339();
        Ok(())
    }

    fn finish(
        &self,
        job_id: Uuid,
        result: Result<RestoreReport, RehomeError>,
    ) -> Result<(), RehomeError> {
        let mut store = self.store();
        let record = store
            .records
            .get_mut(&job_id)
            .ok_or_else(migration_job_not_found)?;
        if record.snapshot.status != MigrationJobStatus::Running {
            return Ok(());
        }
        match result {
            Ok(report) => {
                record.snapshot.status = MigrationJobStatus::Succeeded;
                record.snapshot.report = Some(report);
            }
            Err(error) => {
                record.snapshot.status = if record.snapshot.transaction_id.is_none() {
                    MigrationJobStatus::FailedBeforeWrite
                } else {
                    record
                        .rollback_outcome
                        .unwrap_or(MigrationJobStatus::RollbackFailed)
                };
                record.snapshot.error = Some(sanitize_migration_error(error));
            }
        }
        record.snapshot.stage = MigrationJobStage::Finished;
        record.snapshot.updated_at = chrono::Utc::now().to_rfc3339();
        store.terminal_order.push_back(job_id);
        while store.terminal_order.len() > 20 {
            if let Some(oldest) = store.terminal_order.pop_front() {
                store.records.remove(&oldest);
            }
        }
        Ok(())
    }
}

fn migration_job_not_found() -> RehomeError {
    RehomeError::new(
        ErrorCode::MigrationJobNotFound,
        "Migration job was not found or has expired.",
    )
}

fn sanitize_migration_error(error: RehomeError) -> RehomeError {
    // Core/OS errors can embed paths or external payloads. Publish only a known
    // cause description, keeping the original typed cause and separate job evidence.
    let message = match error.code {
        ErrorCode::CodexRunning => "Confirm Codex is fully closed before retrying.",
        ErrorCode::CodexAppServerUnavailable => "The Codex verification service is unavailable.",
        ErrorCode::CodexAuthenticationRequired => "Codex authentication is required.",
        ErrorCode::CodexVerificationFailed => "Codex access verification could not be completed. Confirm online consent and the selected thread.",
        ErrorCode::CodexCleanupUnconfirmed => "Codex child termination could not be confirmed. Check migration history for manual recovery.",
        ErrorCode::DiskSpaceInsufficient => "There is insufficient disk space for migration.",
        ErrorCode::PackageInvalid | ErrorCode::ChecksumMismatch | ErrorCode::UnsupportedSchema => "The migration package could not be validated.",
        ErrorCode::ProjectConflict => "A project conflict prevents migration.",
        ErrorCode::UnsafePath => "A migration path failed safety validation.",
        ErrorCode::RollbackFailed => "Recovery could not be completed. Check migration history.",
        _ => "Migration could not be completed. Check migration history before retrying.",
    };
    RehomeError::new(error.code, message)
}

struct MigrationWorker {
    claim: PlanClaim,
    jobs: MigrationJobs,
    job_id: Uuid,
}

impl MigrationWorker {
    fn run(
        mut self,
        check_process: impl FnOnce() -> Result<(), RehomeError>,
        restore: impl FnOnce(&Path, &mut dyn RestoreProgressSink) -> Result<RestoreReport, RehomeError>,
    ) {
        let backup_root = self.claim.backup_root.clone();
        let result = check_process().and_then(|()| restore(&backup_root, &mut self));
        let _ = self.jobs.finish(self.job_id, result);
    }
}

impl RestoreProgressSink for MigrationWorker {
    fn update(&mut self, event: RestoreProgressEvent) {
        let _ = self.jobs.record_progress(self.job_id, event);
    }
}

impl Drop for MigrationWorker {
    fn drop(&mut self) {
        // Also runs on worker unwind (join failure) or a dropped queued closure.
        // finish is idempotent; publish terminal evidence before PlanClaim drops.
        let _ = self.jobs.finish(
            self.job_id,
            Err(RehomeError::new(
                ErrorCode::RestoreFailed,
                "Migration worker stopped before completion.",
            )),
        );
    }
}

fn prepare_migration(
    workflow: &WorkflowState,
    selection: &MigrateAndConnectSelection,
    check_process: impl FnOnce() -> Result<(), RehomeError>,
) -> Result<(MigrationJobSnapshot, MigrationWorker), RehomeError> {
    if !selection.codex_closed_confirmed {
        return Err(RehomeError::new(
            ErrorCode::CodexRunning,
            "Codex closure confirmation is required.",
        ));
    }
    if !selection.online_usage_confirmed {
        return Err(RehomeError::new(
            ErrorCode::CodexVerificationFailed,
            "Online usage confirmation is required.",
        ));
    }
    check_process()?;
    let claim = workflow.claim_plan(selection.plan_id)?;
    let initial = workflow.migration_jobs.start(selection.plan_id);
    let worker = MigrationWorker {
        claim,
        jobs: workflow.migration_jobs.clone(),
        job_id: initial.job_id,
    };
    Ok((initial, worker))
}

#[derive(Default)]
struct WorkflowGrants {
    packages: HashMap<Uuid, Timed<PackageGrant>>,
    reveal_paths: HashMap<Uuid, Timed<PathBuf>>,
    restore_locations: HashMap<Uuid, Timed<RestoreLocationGrant>>,
    plans: HashMap<Uuid, Timed<RestorePlanGrant>>,
    rollbacks_in_flight: HashSet<Uuid>,
}

struct Timed<T> {
    value: T,
    expires_at: Instant,
}

struct RestoreLocationGrant {
    package_selection_id: Uuid,
    projects_root: PathBuf,
    backup_root: PathBuf,
}

#[derive(Clone)]
struct PackageGrant {
    path: PathBuf,
    archive_hash: String,
    file_identity: String,
}

struct RestorePlanGrant {
    backup_root: PathBuf,
    state: GrantState,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum GrantState {
    Available,
    InFlight,
}

pub(crate) struct PlanClaim {
    workflow: WorkflowState,
    plan_id: Uuid,
    pub(crate) backup_root: PathBuf,
    finished: bool,
}

impl PlanClaim {
    pub(crate) fn restore_available(mut self) {
        self.workflow.finish_plan(self.plan_id, true);
        self.finished = true;
    }
}

impl Drop for PlanClaim {
    fn drop(&mut self) {
        if !self.finished {
            self.workflow.finish_plan(self.plan_id, false);
        }
    }
}

pub(crate) struct RollbackClaim {
    workflow: WorkflowState,
    transaction_id: Uuid,
}

impl Drop for RollbackClaim {
    fn drop(&mut self) {
        self.workflow
            .grants()
            .rollbacks_in_flight
            .remove(&self.transaction_id);
    }
}

impl WorkflowState {
    fn grant_reveal_path(&self, path: PathBuf) -> Uuid {
        let id = Uuid::new_v4();
        self.grants().reveal_paths.insert(id, timed(path));
        id
    }

    fn resolve_granted_path(&self, id: Uuid) -> Result<PathBuf, RehomeError> {
        let mut grants = self.grants();
        grants.prune();
        if let Some(grant) = grants.packages.get(&id) {
            validate_package_file_identity(&grant.value)?;
            return Ok(grant.value.path.clone());
        }
        let granted = grants.reveal_paths.get(&id).ok_or_else(|| {
            selection_failed(
                ErrorCode::RestoreFailed,
                "file location permission expired or was not found",
            )
        })?;
        let canonical = canonical_existing_file(&granted.value)?;
        if canonical != granted.value {
            return Err(open_failed("granted file path changed"));
        }
        Ok(canonical)
    }

    pub(crate) fn grant_inspected_package(
        &self,
        path: PathBuf,
        archive_hash: String,
    ) -> Result<Uuid, RehomeError> {
        let file_identity = package_file_identity(&path)?;
        let id = Uuid::new_v4();
        self.grants().packages.insert(
            id,
            timed(PackageGrant {
                path,
                archive_hash,
                file_identity,
            }),
        );
        Ok(id)
    }

    pub(crate) fn resolve_package(&self, id: Uuid) -> Result<PathBuf, RehomeError> {
        let mut grants = self.grants();
        grants.prune();
        let grant = grants.packages.get(&id).ok_or_else(|| {
            selection_failed(
                ErrorCode::PackageInvalid,
                "package selection expired or was not found",
            )
        })?;
        validate_package_file_identity(&grant.value)?;
        Ok(grant.value.path.clone())
    }

    pub(crate) fn validate_package_grant(
        &self,
        id: Uuid,
        archive_hash: &str,
    ) -> Result<PathBuf, RehomeError> {
        let mut grants = self.grants();
        grants.prune();
        let grant = grants.packages.get(&id).ok_or_else(|| {
            selection_failed(
                ErrorCode::PackageInvalid,
                "package selection expired or was not found",
            )
        })?;
        validate_package_file_identity(&grant.value)?;
        if !grant.value.archive_hash.eq_ignore_ascii_case(archive_hash) {
            return Err(selection_failed(
                ErrorCode::PackageInvalid,
                "selected package archive hash changed after inspection",
            ));
        }
        Ok(grant.value.path.clone())
    }

    pub(crate) fn grant_restore_locations(
        &self,
        package_selection_id: Uuid,
        projects_root: PathBuf,
        backup_root: PathBuf,
    ) -> Uuid {
        let id = Uuid::new_v4();
        self.grants().restore_locations.insert(
            id,
            timed(RestoreLocationGrant {
                package_selection_id,
                projects_root,
                backup_root,
            }),
        );
        id
    }

    pub(crate) fn resolve_restore_locations(
        &self,
        package_selection_id: Uuid,
        id: Uuid,
    ) -> Result<(PathBuf, PathBuf), RehomeError> {
        let mut grants = self.grants();
        grants.prune();
        let grant = grants.restore_locations.get(&id).ok_or_else(|| {
            selection_failed(
                ErrorCode::RestoreFailed,
                "restore location selection expired or was not found",
            )
        })?;
        if grant.value.package_selection_id != package_selection_id {
            return Err(selection_failed(
                ErrorCode::RestoreFailed,
                "restore locations do not belong to the selected package",
            ));
        }
        Ok((
            grant.value.projects_root.clone(),
            grant.value.backup_root.clone(),
        ))
    }

    pub(crate) fn grant_plan(
        &self,
        plan_id: Uuid,
        backup_root: PathBuf,
    ) -> Result<(), RehomeError> {
        let mut grants = self.grants();
        grants.prune();
        if grants
            .plans
            .get(&plan_id)
            .is_some_and(|grant| grant.value.state == GrantState::InFlight)
        {
            return Err(selection_failed(
                ErrorCode::RestoreFailed,
                "restore plan is already being applied",
            ));
        }
        grants.plans.insert(
            plan_id,
            timed(RestorePlanGrant {
                backup_root,
                state: GrantState::Available,
            }),
        );
        Ok(())
    }

    pub(crate) fn claim_plan(&self, plan_id: Uuid) -> Result<PlanClaim, RehomeError> {
        let mut grants = self.grants();
        grants.prune();
        if !grants.rollbacks_in_flight.is_empty()
            || grants
                .plans
                .values()
                .any(|grant| grant.value.state == GrantState::InFlight)
        {
            return Err(selection_failed(
                ErrorCode::RestoreFailed,
                "a restore or rollback is already in progress",
            ));
        }
        let grant = grants.plans.get_mut(&plan_id).ok_or_else(|| {
            selection_failed(
                ErrorCode::RestoreFailed,
                "restore plan capability expired or was not found",
            )
        })?;
        grant.value.state = GrantState::InFlight;
        Ok(PlanClaim {
            workflow: self.clone(),
            plan_id,
            backup_root: grant.value.backup_root.clone(),
            finished: false,
        })
    }

    fn finish_plan(&self, plan_id: Uuid, restore_available: bool) {
        let mut grants = self.grants();
        if restore_available {
            if let Some(grant) = grants.plans.get_mut(&plan_id) {
                if grant.value.state == GrantState::InFlight {
                    grant.value.state = GrantState::Available;
                    grant.expires_at = Instant::now() + GRANT_TTL;
                }
            }
        } else {
            grants.plans.remove(&plan_id);
        }
    }

    pub(crate) fn claim_rollback(
        &self,
        transaction_id: Uuid,
    ) -> Result<RollbackClaim, RehomeError> {
        let mut grants = self.grants();
        if grants
            .plans
            .values()
            .any(|grant| grant.value.state == GrantState::InFlight)
        {
            return Err(selection_failed(
                ErrorCode::RollbackFailed,
                "a restore is already in progress",
            ));
        }
        if !grants.rollbacks_in_flight.insert(transaction_id) {
            return Err(selection_failed(
                ErrorCode::RollbackFailed,
                "transaction rollback is already in progress",
            ));
        }
        Ok(RollbackClaim {
            workflow: self.clone(),
            transaction_id,
        })
    }

    fn grants(&self) -> std::sync::MutexGuard<'_, WorkflowGrants> {
        self.inner.lock().unwrap_or_else(|error| error.into_inner())
    }
}

impl WorkflowGrants {
    fn prune(&mut self) {
        let now = Instant::now();
        self.packages.retain(|_, grant| grant.expires_at > now);
        self.reveal_paths.retain(|_, grant| grant.expires_at > now);
        self.restore_locations
            .retain(|_, grant| grant.expires_at > now);
        self.plans
            .retain(|_, grant| grant.value.state == GrantState::InFlight || grant.expires_at > now);
    }
}

fn timed<T>(value: T) -> Timed<T> {
    Timed {
        value,
        expires_at: Instant::now() + GRANT_TTL,
    }
}

#[tauri::command]
pub async fn discover_codex(
    state: State<'_, WorkflowState>,
    codex_home: Option<PathBuf>,
) -> Result<CodexInventory, RehomeError> {
    let state = state.inner().clone();
    let attempted = codex_home
        .clone()
        .or_else(default_codex_home_hint)
        .unwrap_or_else(|| PathBuf::from("<environment default>"));
    run_blocking(ErrorCode::CodexNotFound, move || {
        let mut inventory = core_discover_codex(codex_home)
            .map_err(|error| discovery_diagnostic(error, &attempted))?;
        for image in &mut inventory.generated_images {
            let canonical = canonical_existing_file(&image.source_path)?;
            image.reveal_id = Some(state.grant_reveal_path(canonical));
        }
        Ok(inventory)
    })
    .await
}

#[tauri::command]
pub async fn pick_directory(
    app: AppHandle,
    window: WebviewWindow,
    title: String,
) -> Result<Option<PathBuf>, RehomeError> {
    run_blocking(ErrorCode::ConfigInvalid, move || {
        let Some(selected) = app
            .dialog()
            .file()
            .set_parent(&window)
            .set_title(title.as_str())
            .blocking_pick_folder()
        else {
            return Ok(None);
        };
        let path = selected_path(selected)?;
        Ok(Some(user_facing_path(&canonical_existing_directory(
            &path,
        )?)))
    })
    .await
}

#[tauri::command]
pub async fn discover_local_candidates(
    state: State<'_, WorkflowState>,
    roots: Option<Vec<PathBuf>>,
) -> Result<LocalDiscoveryResult, RehomeError> {
    state.local_discovery_cancel.store(false, Ordering::Relaxed);
    let cancel = state.local_discovery_cancel.clone();
    run_blocking(ErrorCode::ConfigInvalid, move || {
        let config = app_config::load_config(&app_config::default_config_path()?)?;
        Ok(core_discover_local_candidates(
            LocalScanRequest::from_config(&config, roots),
            cancel,
        ))
    })
    .await
}

#[tauri::command]
pub async fn count_project_files(
    paths: Vec<PathBuf>,
) -> Result<Vec<LocalProjectCandidate>, RehomeError> {
    run_blocking(ErrorCode::ConfigInvalid, move || {
        Ok(core_count_project_files(paths))
    })
    .await
}

#[tauri::command]
pub async fn request_admin_local_discovery() -> Result<LocalDiscoveryResult, RehomeError> {
    run_blocking(
        ErrorCode::AdminScanUnavailable,
        request_admin_local_discovery_sync,
    )
    .await
}

#[tauri::command]
pub fn cancel_local_discovery(state: State<'_, WorkflowState>) {
    state.local_discovery_cancel.store(true, Ordering::Relaxed);
}

#[cfg(windows)]
fn request_admin_local_discovery_sync() -> Result<LocalDiscoveryResult, RehomeError> {
    use std::{ffi::OsStr, os::windows::ffi::OsStrExt, thread};
    use windows_sys::Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_HIDE};

    let config = app_config::load_config(&app_config::default_config_path()?)?;
    let scan_directory = tempfile::Builder::new()
        .prefix("enhe-codex-backup-admin-scan-")
        .tempdir()
        .map_err(|error| RehomeError::new(ErrorCode::AdminScanUnavailable, error.to_string()))?;
    let output_path = scan_directory.path().join("result.json");
    let request = LocalScanRequest::from_config(&config, None);
    let request_bytes = serde_json::to_vec(&request)
        .map_err(|error| RehomeError::new(ErrorCode::AdminScanUnavailable, error.to_string()))?;
    fs::write(output_path.with_extension("request.json"), request_bytes)
        .map_err(|error| RehomeError::new(ErrorCode::AdminScanUnavailable, error.to_string()))?;
    let executable = env::current_exe().map_err(|error| {
        RehomeError::new(
            ErrorCode::AdminScanUnavailable,
            format!("could not locate the application for administrator scan: {error}"),
        )
    })?;
    let arguments = format!("--admin-local-scan \"{}\"", output_path.display());
    let wide = |value: &OsStr| {
        value
            .encode_wide()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>()
    };
    let verb = wide(OsStr::new("runas"));
    let executable = wide(executable.as_os_str());
    let arguments = wide(OsStr::new(&arguments));
    let launched = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            executable.as_ptr(),
            arguments.as_ptr(),
            std::ptr::null(),
            SW_HIDE,
        )
    };
    if (launched as usize) <= 32 {
        return Err(RehomeError::new(
            ErrorCode::AdminScanUnavailable,
            "administrator scan was cancelled or Windows denied the request",
        ));
    }

    let deadline = Instant::now() + Duration::from_secs(90);
    loop {
        if output_path.is_file() {
            let bytes = fs::read(&output_path).map_err(|error| {
                RehomeError::new(
                    ErrorCode::AdminScanUnavailable,
                    format!("could not read administrator scan result: {error}"),
                )
            });
            let _ = fs::remove_file(&output_path);
            return bytes.and_then(|bytes| {
                serde_json::from_slice(&bytes).map_err(|error| {
                    RehomeError::new(
                        ErrorCode::AdminScanUnavailable,
                        format!("administrator scan returned invalid data: {error}"),
                    )
                })
            });
        }
        if Instant::now() >= deadline {
            let _ = fs::remove_file(&output_path);
            return Err(RehomeError::new(
                ErrorCode::AdminScanUnavailable,
                "administrator scan timed out; the normal scan result is still safe to use",
            ));
        }
        thread::sleep(Duration::from_millis(250));
    }
}

#[cfg(not(windows))]
fn request_admin_local_discovery_sync() -> Result<LocalDiscoveryResult, RehomeError> {
    Err(RehomeError::new(
        ErrorCode::AdminScanUnavailable,
        "administrator scan is only available on Windows",
    ))
}

#[tauri::command]
pub async fn create_package(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, WorkflowState>,
    selection: CreatePackageSelection,
) -> Result<Option<CreatedPackage>, RehomeError> {
    let state = state.inner().clone();
    run_blocking(ErrorCode::PackageInvalid, move || {
        let Some(selected) = app
            .dialog()
            .file()
            .set_parent(&window)
            .set_title("保存 ReHome 包")
            .set_file_name("handoff.rehome")
            .add_filter("ReHome 包", &["rehome"])
            .blocking_save_file()
        else {
            return Ok(None);
        };
        let output_path = canonical_save_path(selected)?;
        let inventory = core_discover_codex(None)?;
        let request = resolve_create_package_request(&inventory, selection, output_path)?;
        // The native save dialog only returns an existing filename after the user confirms replace.
        let report = core_create_package_replacing(request)?;
        let preview = core_inspect_package(&report.package_path)?;
        let canonical = canonical_existing_file(&report.package_path)?;
        let reveal_id = state.grant_inspected_package(canonical, preview.archive_hash.clone())?;
        Ok(Some(CreatedPackage {
            report,
            archive_hash: preview.archive_hash,
            reveal_id,
        }))
    })
    .await
}

#[tauri::command]
pub async fn inspect_package(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, WorkflowState>,
) -> Result<Option<InspectedPackage>, RehomeError> {
    let state = state.inner().clone();
    run_blocking(ErrorCode::PackageInvalid, move || {
        let Some(selected) = app
            .dialog()
            .file()
            .set_parent(&window)
            .set_title("选择 ReHome 包")
            .add_filter("ReHome 包", &["rehome"])
            .blocking_pick_file()
        else {
            return Ok(None);
        };
        let path = canonical_existing_file(&selected_path(selected)?)?;
        if !has_rehome_extension(&path) {
            return Err(selection_failed(
                ErrorCode::PackageInvalid,
                "selected package must use the .rehome extension",
            ));
        }
        let preview = core_inspect_package(&path)?;
        let selection_id = state.grant_inspected_package(path, preview.archive_hash.clone())?;
        Ok(Some(InspectedPackage {
            selection_id,
            preview,
        }))
    })
    .await
}

#[tauri::command]
pub async fn build_restore_plan(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, WorkflowState>,
    request: BuildRestorePlanRequest,
) -> Result<Option<BuildRestorePlanResponse>, RehomeError> {
    let state = state.inner().clone();
    run_blocking(ErrorCode::RestoreFailed, move || match request {
        BuildRestorePlanRequest::SelectDestinations {
            package_selection_id,
        } => {
            let package_path = state.resolve_package(package_selection_id)?;
            let package = core_inspect_package(&package_path)?;
            state.validate_package_grant(package_selection_id, &package.archive_hash)?;
            let inventory = core_discover_codex(None)?;
            let projects_root = if package.manifest.projects.is_empty() {
                default_unused_projects_root(&inventory.codex_home)?
            } else {
                let Some(projects) = app
                    .dialog()
                    .file()
                    .set_parent(&window)
                    .set_title("选择项目目录")
                    .blocking_pick_folder()
                else {
                    return Ok(None);
                };
                canonical_existing_directory(&selected_path(projects)?)?
            };
            let backup_root = managed_backup_root()?;
            validate_restore_location_separation(&projects_root, &backup_root)?;
            let selection_id = state.grant_restore_locations(
                package_selection_id,
                projects_root.clone(),
                backup_root.clone(),
            );
            Ok(Some(BuildRestorePlanResponse::Destinations {
                selection_id,
                target_codex_home: inventory.codex_home,
                projects_root,
                backup_root,
            }))
        }
        BuildRestorePlanRequest::Build {
            package_selection_id,
            destination_selection_id,
            conflict_resolution,
        } => {
            let package_path = state.resolve_package(package_selection_id)?;
            let (projects_root, backup_root) =
                state.resolve_restore_locations(package_selection_id, destination_selection_id)?;
            let package = core_inspect_package(&package_path)?;
            state.validate_package_grant(package_selection_id, &package.archive_hash)?;
            let inventory = core_discover_codex(None)?;
            let target = TargetInventory {
                codex_home: inventory.codex_home,
                target_os: inventory.source_os,
                target_arch: inventory.source_arch,
                counts: inventory.counts,
                projects: inventory.projects,
                conversations: inventory.conversations,
            };
            let plan =
                core_build_restore_plan(&package, &target, &projects_root, conflict_resolution)?;
            state.grant_plan(plan.plan_id, backup_root)?;
            Ok(Some(BuildRestorePlanResponse::Plan { plan }))
        }
    })
    .await
}

#[tauri::command]
pub async fn start_migrate_and_connect(
    state: State<'_, WorkflowState>,
    selection: MigrateAndConnectSelection,
) -> Result<MigrationJobSnapshot, RehomeError> {
    let state = state.inner().clone();
    let admission = selection.clone();
    let (initial, worker) = run_blocking(ErrorCode::RestoreFailed, move || {
        prepare_migration(&state, &admission, ensure_codex_desktop_is_closed)
    })
    .await
    .map_err(sanitize_migration_error)?;
    // The worker owns both the job and its plan claim independently of this IPC call.
    tauri::async_runtime::spawn_blocking(move || {
        worker.run(ensure_codex_desktop_is_closed, |backup_root, progress| {
            let plan = crate::core::plan_store::load(selection.plan_id)?;
            apply_restore_with_services(
                plan,
                RestoreOptions {
                    codex_closed_confirmed: selection.codex_closed_confirmed,
                    backup_root: backup_root.to_path_buf(),
                    register_projects: selection.register_projects,
                    continuation_probe: Some(ContinuationProbeOptions {
                        probe_thread_id: selection.probe_thread_id,
                        online_usage_confirmed: selection.online_usage_confirmed,
                    }),
                },
                &mut register_project_with_detected_cli,
                Some(&mut SystemCodexAccessVerifier),
                progress,
            )
        });
    });
    Ok(initial)
}

#[tauri::command]
pub async fn get_migration_job(
    state: State<'_, WorkflowState>,
    job_id: Uuid,
) -> Result<MigrationJobSnapshot, RehomeError> {
    state.migration_jobs.get(job_id)
}

#[tauri::command]
pub async fn apply_restore(
    state: State<'_, WorkflowState>,
    selection: ApplyRestoreSelection,
) -> Result<RestoreReport, RehomeError> {
    let state = state.inner().clone();
    run_blocking(ErrorCode::RestoreFailed, move || {
        run_file_restore(
            &state,
            selection,
            ensure_codex_desktop_is_closed,
            |plan_id, options| apply_restore_by_id(plan_id, options),
        )
    })
    .await
}

fn run_file_restore(
    state: &WorkflowState,
    selection: ApplyRestoreSelection,
    check_process: impl FnOnce() -> Result<(), RehomeError>,
    restore: impl FnOnce(Uuid, RestoreOptions) -> Result<RestoreReport, RehomeError>,
) -> Result<RestoreReport, RehomeError> {
    let claim = state.claim_plan(selection.plan_id)?;
    let result = check_process().and_then(|()| {
        restore(
            selection.plan_id,
            RestoreOptions {
                codex_closed_confirmed: selection.codex_closed_confirmed,
                backup_root: claim.backup_root.clone(),
                register_projects: selection.register_projects,
                continuation_probe: None,
            },
        )
    });
    match result {
        Err(error) if error.code == ErrorCode::CodexRunning => {
            claim.restore_available();
            Err(error)
        }
        result => result,
    }
}

#[tauri::command]
pub async fn list_transactions() -> Result<TransactionHistory, RehomeError> {
    run_blocking(ErrorCode::RollbackFailed, core_list_transaction_history).await
}

#[tauri::command]
pub async fn rollback_transaction(
    state: State<'_, WorkflowState>,
    selection: RollbackSelection,
) -> Result<RollbackReport, RehomeError> {
    let state = state.inner().clone();
    run_blocking(ErrorCode::RollbackFailed, move || {
        ensure_codex_desktop_is_closed()?;
        let _claim = state.claim_rollback(selection.transaction_id)?;
        let transaction = rollback_transaction_by_id(selection.transaction_id)?;
        validate_rollback_action(transaction.status, selection.action)?;
        core_rollback(selection.transaction_id)
    })
    .await
}

#[tauri::command]
pub async fn open_path(
    app: AppHandle,
    state: State<'_, WorkflowState>,
    selection: OpenPathSelection,
) -> Result<(), RehomeError> {
    let state = state.inner().clone();
    run_blocking(ErrorCode::RestoreFailed, move || {
        let canonical = match selection {
            OpenPathSelection::Granted { object_id } => {
                let granted = state.resolve_granted_path(object_id)?;
                let canonical = canonical_existing(&granted)?;
                if canonical != granted {
                    return Err(open_failed("granted package path changed"));
                }
                canonical
            }
            OpenPathSelection::Transaction {
                path,
                transaction_id,
            } => authorize_open_path(&path, transaction_id, false)?,
        };
        app.opener()
            .reveal_item_in_dir(canonical)
            .map_err(|error| open_failed(format!("could not reveal path: {error}")))
    })
    .await
}

#[tauri::command]
pub async fn open_restored_thread(
    selection: OpenRestoredThreadSelection,
) -> Result<RegistrationStatus, RehomeError> {
    run_blocking(ErrorCode::RegistrationIncomplete, move || {
        let canonical = authorize_open_path(&selection.path, selection.transaction_id, true)?;
        Ok(register_project_with_detected_cli(
            current_source_os(),
            &canonical,
        ))
    })
    .await
}

pub(crate) fn resolve_create_package_request(
    inventory: &CodexInventory,
    selection: CreatePackageSelection,
    output_path: PathBuf,
) -> Result<CreatePackageRequest, RehomeError> {
    let selected_projects = selection
        .project_ids
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    if selected_projects.len() != selection.project_ids.len() {
        return Err(selection_failed(
            ErrorCode::ProjectConflict,
            "project selection contains duplicates",
        ));
    }
    let projects_by_id = inventory
        .projects
        .iter()
        .map(|project| (project.project_id, project))
        .collect::<HashMap<_, _>>();
    let project_paths = selection
        .project_ids
        .iter()
        .map(|project_id| {
            projects_by_id
                .get(project_id)
                .and_then(|project| {
                    project
                        .source_available
                        .then(|| PathBuf::from(&project.source_path))
                })
                .ok_or_else(|| {
                    selection_failed(
                        ErrorCode::ProjectConflict,
                        format!(
                            "selected project {project_id} is missing or is not available in fresh discovery; rescan and select its conversations instead"
                        ),
                    )
                })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let conversations_by_id = inventory
        .conversations
        .iter()
        .map(|conversation| (conversation.task_id, conversation))
        .collect::<HashMap<_, _>>();
    let mut seen_conversations = HashSet::new();
    for conversation_id in &selection.conversation_ids {
        if !seen_conversations.insert(*conversation_id) {
            return Err(selection_failed(
                ErrorCode::ProjectConflict,
                "conversation selection contains duplicates",
            ));
        }
        conversations_by_id.get(conversation_id).ok_or_else(|| {
            selection_failed(
                ErrorCode::ProjectConflict,
                format!("selected conversation {conversation_id} is not in fresh discovery"),
            )
        })?;
    }

    Ok(CreatePackageRequest {
        codex_home: inventory.codex_home.clone(),
        project_paths,
        conversation_ids: selection.conversation_ids,
        output_path,
        source_device_id: inventory.source_device_id,
        skill_paths: resolve_optional_paths(&selection.skill_ids, &inventory.skills, "skill")?,
        plugin_paths: resolve_optional_paths(&selection.plugin_ids, &inventory.plugins, "plugin")?,
        generated_image_paths: resolve_optional_paths(
            &selection.generated_image_ids,
            &inventory.generated_images,
            "generated image",
        )?,
    })
}

fn resolve_optional_paths(
    selected_ids: &[Uuid],
    entries: &[crate::core::models::OptionalContentEntry],
    kind: &str,
) -> Result<Vec<PathBuf>, RehomeError> {
    let available = entries
        .iter()
        .map(|entry| (entry.content_id, &entry.source_path))
        .collect::<HashMap<_, _>>();
    let mut seen = HashSet::new();
    selected_ids
        .iter()
        .map(|id| {
            if !seen.insert(*id) {
                return Err(selection_failed(
                    ErrorCode::ProjectConflict,
                    format!("{kind} selection contains duplicates"),
                ));
            }
            available
                .get(id)
                .map(|path| (*path).clone())
                .ok_or_else(|| {
                    selection_failed(
                        ErrorCode::ProjectConflict,
                        format!("selected {kind} {id} is not in fresh discovery"),
                    )
                })
        })
        .collect()
}

pub(crate) fn validate_rollback_action(
    status: RecoveryStatus,
    action: RollbackAction,
) -> Result<(), RehomeError> {
    let valid = match action {
        RollbackAction::Rollback => status == RecoveryStatus::Committed,
        RollbackAction::Resume => matches!(
            status,
            RecoveryStatus::Prepared
                | RecoveryStatus::Applying
                | RecoveryStatus::Verifying
                | RecoveryStatus::RollingBack
                | RecoveryStatus::RollbackFailed
        ),
    };
    if valid {
        Ok(())
    } else {
        Err(selection_failed(
            ErrorCode::RollbackFailed,
            "rollback action does not match the transaction status",
        ))
    }
}

pub(crate) fn rollback_transaction_by_id(
    transaction_id: Uuid,
) -> Result<TransactionSummary, RehomeError> {
    core_transaction_summary(transaction_id)?.ok_or_else(|| {
        selection_failed(
            ErrorCode::RollbackFailed,
            "transaction was not found for rollback",
        )
    })
}

pub(crate) fn open_transaction_by_id(
    transaction_id: Uuid,
) -> Result<TransactionSummary, RehomeError> {
    core_transaction_summary(transaction_id)
        .map_err(|error| open_failed(error.message))?
        .ok_or_else(|| open_failed("transaction was not found for open operation"))
}

fn authorize_open_path(
    path: &Path,
    transaction_id: Uuid,
    restored_only: bool,
) -> Result<PathBuf, RehomeError> {
    let canonical = canonical_existing(path)?;
    let transaction = open_transaction_by_id(transaction_id)?;
    authorize_transaction_path(&canonical, &transaction, restored_only)?;
    Ok(canonical)
}

pub(crate) fn authorize_transaction_path(
    canonical: &Path,
    transaction: &TransactionSummary,
    restored_only: bool,
) -> Result<(), RehomeError> {
    let exact_restored_project = transaction.restored_project_paths.iter().any(|path| {
        fs::canonicalize(path).is_ok_and(|canonical_project| canonical_project == canonical)
    });
    let exact_transaction_backup = !restored_only
        && fs::canonicalize(&transaction.transaction_backup_path)
            .is_ok_and(|canonical_backup| canonical_backup == canonical);

    if exact_restored_project || exact_transaction_backup {
        Ok(())
    } else {
        Err(open_failed(
            "path is not an exact object owned by the selected transaction",
        ))
    }
}

fn canonical_save_path(selected: FilePath) -> Result<PathBuf, RehomeError> {
    let mut path = selected_path(selected)?;
    if !has_rehome_extension(&path) {
        path.set_extension("rehome");
    }
    validate_local_dialog_path(&path)?;
    let parent = path
        .parent()
        .ok_or_else(|| selection_failed(ErrorCode::PackageInvalid, "save path has no parent"))?;
    let file_name = path
        .file_name()
        .ok_or_else(|| selection_failed(ErrorCode::PackageInvalid, "save path has no file name"))?;
    let parent = canonical_existing_directory(parent)?;
    let output = parent.join(file_name);
    validate_local_dialog_path(&output)?;
    Ok(output)
}

fn selected_path(selected: FilePath) -> Result<PathBuf, RehomeError> {
    selected.into_path().map_err(|error| {
        selection_failed(
            ErrorCode::RestoreFailed,
            format!("native selection is not a local filesystem path: {error}"),
        )
    })
}

pub(crate) fn validate_local_dialog_path(path: &Path) -> Result<(), RehomeError> {
    let supported = if !path.is_absolute() {
        false
    } else {
        match path.components().next() {
            Some(Component::Prefix(prefix)) => {
                matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_))
            }
            Some(Component::RootDir) => true,
            _ => false,
        }
    };
    if !supported {
        return Err(selection_failed(
            ErrorCode::RestoreFailed,
            "native selection must be an absolute local path",
        ));
    }
    Ok(())
}

fn canonical_existing_file(path: &Path) -> Result<PathBuf, RehomeError> {
    let canonical = canonical_existing(path)?;
    if !canonical.is_file() {
        return Err(selection_failed(
            ErrorCode::PackageInvalid,
            "selected path is not a regular file",
        ));
    }
    Ok(canonical)
}

fn canonical_existing_directory(path: &Path) -> Result<PathBuf, RehomeError> {
    validate_local_dialog_path(path)?;
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        selection_failed(
            ErrorCode::RestoreFailed,
            format!("could not inspect selected directory: {error}"),
        )
    })?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(selection_failed(
            ErrorCode::RestoreFailed,
            "selected path is not a regular directory",
        ));
    }
    let canonical = fs::canonicalize(path).map_err(|error| {
        selection_failed(
            ErrorCode::RestoreFailed,
            format!("could not canonicalize selected directory: {error}"),
        )
    })?;
    validate_local_dialog_path(&canonical)?;
    Ok(canonical)
}

fn validate_restore_location_separation(
    projects_root: &Path,
    backup_root: &Path,
) -> Result<(), RehomeError> {
    if projects_root.starts_with(backup_root) || backup_root.starts_with(projects_root) {
        return Err(selection_failed(
            ErrorCode::RestoreFailed,
            "项目目录和事务备份目录必须是两个互不包含的目录",
        ));
    }
    Ok(())
}

fn default_unused_projects_root(codex_home: &Path) -> Result<PathBuf, RehomeError> {
    let parent = codex_home.parent().ok_or_else(|| {
        selection_failed(
            ErrorCode::RestoreFailed,
            "could not derive an unused project location from the Codex data path",
        )
    })?;
    Ok(parent.join("ReHome Projects"))
}

fn canonical_existing(path: &Path) -> Result<PathBuf, RehomeError> {
    validate_local_dialog_path(path)?;
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| open_failed(format!("could not inspect path: {error}")))?;
    if (!metadata.is_file() && !metadata.is_dir()) || metadata.file_type().is_symlink() {
        return Err(open_failed("path is not a regular file or directory"));
    }
    let canonical = fs::canonicalize(path)
        .map_err(|error| open_failed(format!("could not canonicalize path: {error}")))?;
    validate_local_dialog_path(&canonical)?;
    Ok(canonical)
}

fn validate_package_file_identity(grant: &PackageGrant) -> Result<(), RehomeError> {
    let current = package_file_identity(&grant.path)?;
    if current != grant.file_identity {
        return Err(selection_failed(
            ErrorCode::PackageInvalid,
            "selected package file identity changed after inspection",
        ));
    }
    Ok(())
}

#[cfg(windows)]
fn package_file_identity(path: &Path) -> Result<String, RehomeError> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
    };

    let file = fs::File::open(path).map_err(|error| {
        selection_failed(
            ErrorCode::PackageInvalid,
            format!("could not open selected package identity: {error}"),
        )
    })?;
    let mut information = BY_HANDLE_FILE_INFORMATION::default();
    let result = unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut information) };
    if result == 0 {
        return Err(selection_failed(
            ErrorCode::PackageInvalid,
            format!(
                "could not inspect selected package identity: {}",
                std::io::Error::last_os_error()
            ),
        ));
    }
    Ok(format!(
        "{}:{:08x}{:08x}",
        information.dwVolumeSerialNumber, information.nFileIndexHigh, information.nFileIndexLow
    ))
}

#[cfg(unix)]
fn package_file_identity(path: &Path) -> Result<String, RehomeError> {
    use std::os::unix::fs::MetadataExt;

    let file = fs::File::open(path).map_err(|error| {
        selection_failed(
            ErrorCode::PackageInvalid,
            format!("could not open selected package identity: {error}"),
        )
    })?;
    let metadata = file.metadata().map_err(|error| {
        selection_failed(
            ErrorCode::PackageInvalid,
            format!("could not inspect selected package identity: {error}"),
        )
    })?;
    Ok(format!("{}:{}", metadata.dev(), metadata.ino()))
}

#[cfg(not(any(windows, unix)))]
fn package_file_identity(path: &Path) -> Result<String, RehomeError> {
    let metadata = fs::metadata(path).map_err(|error| {
        selection_failed(
            ErrorCode::PackageInvalid,
            format!("could not inspect selected package identity: {error}"),
        )
    })?;
    Ok(format!("{}:{:?}", metadata.len(), metadata.modified().ok()))
}

fn has_rehome_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("rehome"))
}

async fn run_blocking<T, F>(code: ErrorCode, operation: F) -> Result<T, RehomeError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, RehomeError> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(operation)
        .await
        .map_err(|error| selection_failed(code, format!("background operation failed: {error}")))?
}

fn current_source_os() -> SourceOs {
    if cfg!(target_os = "macos") {
        SourceOs::Macos
    } else {
        SourceOs::Windows
    }
}

fn default_codex_home_hint() -> Option<PathBuf> {
    env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
        .or_else(|| {
            env::var_os("USERPROFILE")
                .or_else(|| env::var_os("HOME"))
                .map(PathBuf::from)
                .map(|home| home.join(".codex"))
        })
}

fn discovery_diagnostic(error: RehomeError, attempted: &Path) -> RehomeError {
    RehomeError::new(
        error.code,
        format!(
            "{} (attempted Codex data path: {})",
            error.message,
            attempted.display()
        ),
    )
}

fn selection_failed(code: ErrorCode, message: impl Into<String>) -> RehomeError {
    RehomeError::new(code, message)
}

fn open_failed(message: impl Into<String>) -> RehomeError {
    RehomeError::new(ErrorCode::RestoreFailed, message)
}

fn ensure_codex_desktop_is_closed() -> Result<(), RehomeError> {
    if codex_desktop_is_running()? {
        return Err(RehomeError::new(
            ErrorCode::CodexRunning,
            "Codex 或相关后台进程仍在运行。请完全退出 Codex Desktop、ChatGPT 和相关扩展进程后，再开始恢复或回滚。",
        ));
    }
    Ok(())
}

#[cfg(windows)]
const WINDOWS_CODEX_PROCESS_NAMES: &[&str] = &[
    "codex.exe",
    "codex-code-mode-host.exe",
    "ChatGPT.exe",
    "extension-host.exe",
];

#[cfg(windows)]
fn codex_desktop_is_running() -> Result<bool, RehomeError> {
    for process_name in WINDOWS_CODEX_PROCESS_NAMES {
        if tasklist_reports_process(process_name)? {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(windows)]
fn tasklist_reports_process(process_name: &str) -> Result<bool, RehomeError> {
    let filter = format!("IMAGENAME eq {process_name}");
    let output = Command::new("tasklist")
        .args(["/FI", &filter, "/FO", "CSV", "/NH"])
        .output()
        .map_err(|error| {
            RehomeError::new(
                ErrorCode::CodexRunning,
                format!("could not check whether Codex is running: {error}"),
            )
        })?;
    if !output.status.success() {
        return Err(RehomeError::new(
            ErrorCode::CodexRunning,
            "could not check whether Codex is running",
        ));
    }
    Ok(tasklist_csv_has_process(
        &String::from_utf8_lossy(&output.stdout),
        process_name,
    ))
}

#[cfg(windows)]
fn tasklist_csv_has_process(output: &str, process_name: &str) -> bool {
    let expected = format!("\"{}\"", process_name.to_ascii_lowercase());
    output
        .to_ascii_lowercase()
        .lines()
        .any(|line| line.trim_start().starts_with(&expected))
}

#[cfg(target_os = "macos")]
fn codex_desktop_is_running() -> Result<bool, RehomeError> {
    let output = Command::new("pgrep")
        .args(["-f", "/Codex.app/"])
        .output()
        .map_err(|error| {
            RehomeError::new(
                ErrorCode::CodexRunning,
                format!("could not check whether Codex is running: {error}"),
            )
        })?;
    Ok(output.status.success())
}

#[cfg(not(any(windows, target_os = "macos")))]
fn codex_desktop_is_running() -> Result<bool, RehomeError> {
    Ok(false)
}

#[cfg(test)]
mod migration_tests {
    use super::*;
    use crate::core::{
        models::{MigrationJobStage, MigrationJobStatus},
        restore::RestoreProgressEvent,
    };
    use std::sync::mpsc;

    fn selection(workflow: &WorkflowState) -> MigrateAndConnectSelection {
        let plan_id = Uuid::new_v4();
        workflow
            .grant_plan(plan_id, PathBuf::from("C:/Synthetic/backups"))
            .unwrap();
        MigrateAndConnectSelection {
            plan_id,
            codex_closed_confirmed: true,
            register_projects: false,
            probe_thread_id: Uuid::new_v4(),
            online_usage_confirmed: true,
        }
    }

    fn failure() -> RehomeError {
        RehomeError::new(
            ErrorCode::CodexVerificationFailed,
            "synthetic raw payload SECRET",
        )
    }

    fn report(transaction_id: Uuid) -> RestoreReport {
        serde_json::from_value(serde_json::json!({
            "transaction_id": transaction_id, "package_id": Uuid::new_v4(),
            "completed_at": "2026-09-23T00:00:00Z", "restored_files": 1,
            "restored_bytes": 10, "registrations": [],
            "verification": {
                "package_checksum_valid": true, "files_valid": true, "sessions_valid": true,
                "session_index_valid": true, "sqlite_threads_valid": true, "path_mapping_valid": true,
                "forbidden_files_absent": true, "project_files_valid": true,
                "app_registration_valid": true, "app_visible_ready": false
            }
        })).unwrap()
    }

    #[test]
    fn migration_input_rejects_arbitrary_probe_prompts_and_paths() {
        let base = serde_json::json!({
            "plan_id": Uuid::new_v4(), "codex_closed_confirmed": true,
            "register_projects": false, "probe_thread_id": Uuid::new_v4(),
            "online_usage_confirmed": true
        });
        assert!(serde_json::from_value::<MigrateAndConnectSelection>(base.clone()).is_ok());
        for field in ["probe_prompt", "target_codex_home", "backup_root"] {
            let mut value = base.clone();
            value[field] = "untrusted".into();
            assert!(serde_json::from_value::<MigrateAndConnectSelection>(value).is_err());
        }
    }

    #[test]
    fn confirmations_and_initial_process_check_precede_job_and_plan_claim() {
        let workflow = WorkflowState::default();
        for (closed, online, code) in [
            (false, true, ErrorCode::CodexRunning),
            (true, false, ErrorCode::CodexVerificationFailed),
        ] {
            let mut input = selection(&workflow);
            input.codex_closed_confirmed = closed;
            input.online_usage_confirmed = online;
            let error =
                prepare_migration(&workflow, &input, || panic!("must validate consent first"))
                    .err()
                    .unwrap();
            assert_eq!(error.code, code);
            assert!(workflow.claim_plan(input.plan_id).is_ok());
        }
        let input = selection(&workflow);
        let error = prepare_migration(&workflow, &input, || {
            Err(RehomeError::new(ErrorCode::CodexRunning, "running"))
        })
        .err()
        .unwrap();
        assert_eq!(error.code, ErrorCode::CodexRunning);
        assert!(workflow.claim_plan(input.plan_id).is_ok());
        assert!(workflow
            .migration_jobs
            .inner
            .lock()
            .unwrap()
            .records
            .is_empty());
    }

    #[test]
    fn worker_rechecks_process_before_restore() {
        let workflow = WorkflowState::default();
        let input = selection(&workflow);
        let mut checks = 0;
        let mut check = || {
            checks += 1;
            if checks == 1 {
                Ok(())
            } else {
                Err(RehomeError::new(ErrorCode::CodexRunning, "running"))
            }
        };
        let (initial, worker) = prepare_migration(&workflow, &input, &mut check).unwrap();
        worker.run(&mut check, |_, _| panic!("restore must not be called"));
        assert_eq!(checks, 2);
        let snapshot = workflow.migration_jobs.get(initial.job_id).unwrap();
        assert_eq!(snapshot.status, MigrationJobStatus::FailedBeforeWrite);
        assert_eq!(snapshot.transaction_id, None);
        assert_eq!(snapshot.error.unwrap().code, ErrorCode::CodexRunning);
    }

    #[test]
    fn structured_rollback_state_survives_frontend_polling() {
        for (prepared, events, code, expected) in [
            (
                false,
                vec![],
                ErrorCode::RollbackFailed,
                MigrationJobStatus::FailedBeforeWrite,
            ),
            (
                true,
                vec![],
                ErrorCode::CodexVerificationFailed,
                MigrationJobStatus::RollbackFailed,
            ),
            (
                true,
                vec![RestoreProgressEvent::RollbackCompleted],
                ErrorCode::RollbackFailed,
                MigrationJobStatus::RolledBack,
            ),
            (
                true,
                vec![RestoreProgressEvent::RollbackFailed],
                ErrorCode::CodexCleanupUnconfirmed,
                MigrationJobStatus::RollbackFailed,
            ),
            (
                true,
                vec![
                    RestoreProgressEvent::RollbackCompleted,
                    RestoreProgressEvent::RollbackFailed,
                ],
                ErrorCode::RestoreFailed,
                MigrationJobStatus::RollbackFailed,
            ),
            (
                true,
                vec![
                    RestoreProgressEvent::RollbackFailed,
                    RestoreProgressEvent::RollbackCompleted,
                ],
                ErrorCode::RestoreFailed,
                MigrationJobStatus::RolledBack,
            ),
        ] {
            let jobs = MigrationJobs::default();
            let initial = jobs.start(Uuid::new_v4());
            let transaction_id = Uuid::new_v4();
            if prepared {
                jobs.record_progress(
                    initial.job_id,
                    RestoreProgressEvent::TransactionPrepared(transaction_id),
                )
                .unwrap();
                jobs.record_progress(
                    initial.job_id,
                    RestoreProgressEvent::Stage(MigrationJobStage::RollingBack),
                )
                .unwrap();
            }
            for event in events {
                jobs.record_progress(initial.job_id, event).unwrap();
            }
            jobs.finish(
                initial.job_id,
                Err(RehomeError::new(
                    code,
                    "rollback succeeded; SECRET raw payload",
                )),
            )
            .unwrap();
            let snapshot = jobs.clone().get(initial.job_id).unwrap();
            assert_eq!(snapshot.status, expected);
            assert_eq!(snapshot.stage, MigrationJobStage::Finished);
            assert_eq!(snapshot.transaction_id, prepared.then_some(transaction_id));
            let error = snapshot.error.as_ref().unwrap();
            assert_eq!(error.code, code);
            assert!(!error.message.contains("SECRET"));
            assert!(!error.message.contains("rollback succeeded"));
            assert!(snapshot.report.is_none());
            jobs.record_progress(
                initial.job_id,
                RestoreProgressEvent::Stage(MigrationJobStage::Preflight),
            )
            .unwrap();
            jobs.finish(initial.job_id, Err(failure())).unwrap();
            assert_eq!(jobs.get(initial.job_id).unwrap(), snapshot);
        }
    }

    #[test]
    fn terminal_storage_prunes_oldest_completion_and_never_running_jobs() {
        let jobs = MigrationJobs::default();
        let running: Vec<_> = (0..23).map(|_| jobs.start(Uuid::new_v4())).collect();
        let late = jobs.start(Uuid::new_v4());
        let oldest_terminal = jobs.start(Uuid::new_v4());
        jobs.finish(oldest_terminal.job_id, Err(failure())).unwrap();
        jobs.finish(late.job_id, Err(failure())).unwrap();
        let retained: Vec<_> = (0..19)
            .map(|_| {
                let job = jobs.start(Uuid::new_v4());
                jobs.finish(job.job_id, Err(failure())).unwrap();
                job.job_id
            })
            .collect();
        assert_eq!(
            jobs.get(oldest_terminal.job_id).unwrap_err().code,
            ErrorCode::MigrationJobNotFound
        );
        assert!(jobs.get(late.job_id).is_ok());
        for job in retained {
            assert!(jobs.get(job).is_ok());
        }
        for job in running {
            assert_eq!(
                jobs.get(job.job_id).unwrap().status,
                MigrationJobStatus::Running
            );
        }
        assert_eq!(jobs.inner.lock().unwrap().records.len(), 43);
        assert_eq!(
            jobs.get(Uuid::new_v4()).unwrap_err().code,
            ErrorCode::MigrationJobNotFound
        );
    }

    #[test]
    fn running_job_is_pollable_and_outlives_its_caller() {
        let workflow = WorkflowState::default();
        let input = selection(&workflow);
        let other = selection(&workflow);
        let (initial, worker) = prepare_migration(&workflow, &input, || Ok(())).unwrap();
        let transaction_id = Uuid::new_v4();
        let expected = report(transaction_id);
        let result = expected.clone();
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let (finished_tx, finished_rx) = mpsc::channel();
        let handle = tauri::async_runtime::spawn_blocking(move || {
            worker.run(
                || Ok(()),
                |root, sink| {
                    assert_eq!(root, Path::new("C:/Synthetic/backups"));
                    sink.update(RestoreProgressEvent::TransactionPrepared(transaction_id));
                    sink.update(RestoreProgressEvent::Stage(
                        MigrationJobStage::RecognizingThreads,
                    ));
                    entered_tx.send(()).unwrap();
                    release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
                    Ok(result)
                },
            );
            finished_tx.send(()).unwrap();
        });
        drop(handle);
        entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        let polling_state = workflow.clone();
        let snapshot = polling_state.migration_jobs.get(initial.job_id).unwrap();
        assert_eq!(snapshot.status, MigrationJobStatus::Running);
        assert_eq!(snapshot.stage, MigrationJobStage::RecognizingThreads);
        assert_eq!(snapshot.transaction_id, Some(transaction_id));
        assert_ne!(snapshot.updated_at, initial.updated_at);
        assert!(chrono::DateTime::parse_from_rfc3339(&snapshot.updated_at).is_ok());
        assert!(workflow.claim_plan(input.plan_id).is_err());
        assert!(workflow.claim_plan(other.plan_id).is_err());
        assert!(workflow.claim_rollback(transaction_id).is_err());
        release_tx.send(()).unwrap();
        finished_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        let snapshot = polling_state.migration_jobs.get(initial.job_id).unwrap();
        assert_eq!(snapshot.status, MigrationJobStatus::Succeeded);
        assert_eq!(snapshot.stage, MigrationJobStage::Finished);
        assert_eq!(snapshot.report, Some(expected));
        assert!(snapshot.error.is_none());
        assert!(workflow.claim_plan(input.plan_id).is_err());
        assert!(workflow.claim_plan(other.plan_id).is_ok());
    }

    #[test]
    fn worker_panic_terminalizes_from_recorded_evidence_and_releases_guard() {
        for prepared in [false, true] {
            let workflow = WorkflowState::default();
            let input = selection(&workflow);
            let other = selection(&workflow);
            let (initial, worker) = prepare_migration(&workflow, &input, || Ok(())).unwrap();
            let transaction_id = Uuid::new_v4();
            let handle = tauri::async_runtime::spawn_blocking(move || {
                worker.run(
                    || Ok(()),
                    |_, sink| {
                        if prepared {
                            sink.update(RestoreProgressEvent::TransactionPrepared(transaction_id));
                        }
                        panic!("synthetic worker failure");
                    },
                );
            });
            assert!(tauri::async_runtime::block_on(handle).is_err());
            let snapshot = workflow.migration_jobs.get(initial.job_id).unwrap();
            assert_eq!(
                snapshot.status,
                if prepared {
                    MigrationJobStatus::RollbackFailed
                } else {
                    MigrationJobStatus::FailedBeforeWrite
                }
            );
            assert_eq!(snapshot.transaction_id, prepared.then_some(transaction_id));
            assert_eq!(snapshot.error.unwrap().code, ErrorCode::RestoreFailed);
            assert!(workflow.claim_plan(other.plan_id).is_ok());
        }
    }

    #[test]
    fn abandoned_worker_terminalizes_before_releasing_plan_claim() {
        let workflow = WorkflowState::default();
        let input = selection(&workflow);
        let (initial, worker) = prepare_migration(&workflow, &input, || Ok(())).unwrap();
        drop(worker);
        assert_eq!(
            workflow.migration_jobs.get(initial.job_id).unwrap().status,
            MigrationJobStatus::FailedBeforeWrite
        );
        assert!(workflow.claim_rollback(Uuid::new_v4()).is_ok());
    }

    #[test]
    fn file_only_restore_checks_process_before_core_and_keeps_retry_capability() {
        let workflow = WorkflowState::default();
        let input = selection(&workflow);
        let error = run_file_restore(
            &workflow,
            ApplyRestoreSelection {
                plan_id: input.plan_id,
                codex_closed_confirmed: true,
                register_projects: false,
            },
            || Err(RehomeError::new(ErrorCode::CodexRunning, "running")),
            |_, _| panic!("must not restore"),
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::CodexRunning);
        assert!(workflow.claim_plan(input.plan_id).is_ok());
    }
}

#[cfg(test)]
mod grant_tests {
    use super::*;

    fn granted_plan(workflow: &WorkflowState) -> Uuid {
        let plan_id = Uuid::new_v4();
        workflow
            .grant_plan(plan_id, PathBuf::from("C:/Synthetic/backups"))
            .unwrap();
        plan_id
    }

    #[test]
    fn claimed_plan_prevents_a_second_migration_job() {
        let workflow = WorkflowState::default();
        let plan_id = granted_plan(&workflow);
        let claim = workflow.claim_plan(plan_id).unwrap();
        assert_eq!(
            workflow.claim_plan(plan_id).err().unwrap().code,
            ErrorCode::RestoreFailed
        );
        drop(claim);
        assert!(workflow.claim_plan(plan_id).is_err());
    }

    #[test]
    fn different_restore_plans_are_exclusive_until_guard_release() {
        let workflow = WorkflowState::default();
        let first = granted_plan(&workflow);
        let second = granted_plan(&workflow);
        let claim = workflow.claim_plan(first).unwrap();
        assert_eq!(
            workflow.claim_plan(second).err().unwrap().code,
            ErrorCode::RestoreFailed
        );
        claim.restore_available();
        drop(workflow.claim_plan(second).unwrap());
        assert!(workflow.claim_plan(first).is_ok());
    }

    #[test]
    fn restore_and_manual_rollback_exclude_each_other_in_both_orders() {
        let workflow = WorkflowState::default();
        let first = granted_plan(&workflow);
        let second = granted_plan(&workflow);
        let transaction_id = Uuid::new_v4();
        let plan = workflow.claim_plan(first).unwrap();
        assert_eq!(
            workflow.claim_rollback(transaction_id).err().unwrap().code,
            ErrorCode::RollbackFailed
        );
        drop(plan);
        let rollback = workflow.claim_rollback(transaction_id).unwrap();
        assert_eq!(
            workflow.claim_plan(second).err().unwrap().code,
            ErrorCode::RestoreFailed
        );
        assert!(workflow.claim_rollback(transaction_id).is_err());
        drop(rollback);
        assert!(workflow.claim_plan(second).is_ok());
    }

    #[test]
    fn pruning_expired_capabilities_keeps_in_flight_restore_plans() {
        let plan_id = Uuid::new_v4();
        let mut grants = WorkflowGrants::default();
        grants.plans.insert(
            plan_id,
            Timed {
                value: RestorePlanGrant {
                    backup_root: PathBuf::from("C:\\backups"),
                    state: GrantState::InFlight,
                },
                expires_at: Instant::now() - Duration::from_secs(1),
            },
        );

        grants.prune();

        assert!(grants.plans.contains_key(&plan_id));
    }

    #[test]
    fn restore_locations_must_not_overlap() {
        let root = PathBuf::from("/restore");
        assert!(validate_restore_location_separation(&root, &root).is_err());
        assert!(validate_restore_location_separation(&root, &root.join("backups")).is_err());
        assert!(validate_restore_location_separation(
            &root.join("projects"),
            &root.join("backups")
        )
        .is_ok());
    }

    #[test]
    fn conversation_only_restore_uses_a_non_overlapping_placeholder_project_root() {
        let parent = PathBuf::from("root").join("user");
        let codex_home = parent.join(".codex");
        let projects_root = default_unused_projects_root(&codex_home).unwrap();

        assert_eq!(projects_root, parent.join("ReHome Projects"));
        assert!(!projects_root.starts_with(&codex_home));
        assert!(!codex_home.starts_with(&projects_root));
    }

    #[test]
    fn discovery_diagnostic_includes_the_attempted_codex_path() {
        let error = RehomeError::new(ErrorCode::CodexNotFound, "Codex home was not found");
        let diagnostic = discovery_diagnostic(error, Path::new("C:\\Users\\Me\\.codex"));

        assert!(diagnostic.message.contains("C:\\Users\\Me\\.codex"));
        assert_eq!(diagnostic.code, ErrorCode::CodexNotFound);
    }

    #[cfg(windows)]
    #[test]
    fn tasklist_process_detection_uses_the_csv_image_name() {
        assert!(tasklist_csv_has_process(
            "\"codex.exe\",\"123\",\"Console\",\"1\",\"100 K\"\r\n",
            "codex.exe"
        ));
        assert!(!tasklist_csv_has_process(
            "INFO: No tasks are running which match the specified criteria.\r\n",
            "codex.exe"
        ));
        assert!(WINDOWS_CODEX_PROCESS_NAMES.contains(&"ChatGPT.exe"));
        assert!(WINDOWS_CODEX_PROCESS_NAMES.contains(&"extension-host.exe"));
    }
}
