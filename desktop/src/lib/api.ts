import { invoke } from "@tauri-apps/api/core";

import type {
  CodexInventory,
  CreatePackageReport,
  CreatePackageRequest,
  FileConflictResolution,
  PackagePreview,
  RegistrationStatus,
  RestoreOptions,
  RestoreLocationSelection,
  RestorePlan,
  RestoreReport,
  RollbackAction,
  RollbackReport,
  TransactionHistory,
  AppConfig,
  CloudConfig,
  CloudConfigStart,
  CloudOperationResult,
  CloudUploadRequest,
  LocalBackupListRequest,
  LocalBackupRequest as LocalBackupCommandRequest,
  LocalRestoreReport,
  LocalRestoreRequest,
  LocalSnapshot,
  LocalSnapshotSummary,
  LocalDiscoveryResult,
  LocalProjectCandidate,
  SchedulerStatus,
  MigrationJobSnapshot,
  StartMigrationSelection,
} from "./types";

export function discoverCodex(codexHome?: string | null): Promise<CodexInventory> {
  return invoke("discover_codex", { codexHome: codexHome ?? null });
}

export function pickDirectory(title: string): Promise<string | null> {
  return invoke("pick_directory", { title });
}

export function discoverLocalCandidates(roots?: string[]): Promise<LocalDiscoveryResult> {
  return invoke("discover_local_candidates", { roots: roots ?? null });
}

export function requestAdminLocalDiscovery(): Promise<LocalDiscoveryResult> {
  return invoke("request_admin_local_discovery");
}

export function cancelLocalDiscovery(): Promise<void> {
  return invoke("cancel_local_discovery");
}

export function countProjectFiles(paths: string[]): Promise<LocalProjectCandidate[]> {
  return invoke("count_project_files", { paths });
}

export function createPackage(selection: CreatePackageRequest): Promise<CreatePackageReport | null> {
  return invoke("create_package", { selection });
}

export function inspectPackage(): Promise<PackagePreview | null> {
  return invoke("inspect_package");
}

export async function selectRestoreDestinations(
  packageSelectionId: string,
): Promise<RestoreLocationSelection | null> {
  const response = await invoke<
    | ({ action: "destinations" } & RestoreLocationSelection)
    | { action: "plan"; plan: RestorePlan }
    | null
  >("build_restore_plan", {
    request: { action: "select_destinations", package_selection_id: packageSelectionId },
  });
  return response?.action === "destinations" ? response : null;
}

export async function buildRestorePlan(
  packageSelectionId: string,
  destinationSelectionId: string,
  conflictResolution?: FileConflictResolution,
): Promise<RestorePlan> {
  const response = await invoke<{ action: "plan"; plan: RestorePlan } | null>(
    "build_restore_plan",
    {
      request: {
        action: "build",
        package_selection_id: packageSelectionId,
        destination_selection_id: destinationSelectionId,
        conflict_resolution: conflictResolution,
      },
    },
  );
  if (response?.action !== "plan") throw new Error("恢复位置选择已取消");
  return response.plan;
}

export function applyRestore(planId: string, options: RestoreOptions): Promise<RestoreReport> {
  return invoke("apply_restore", { selection: { plan_id: planId, ...options } });
}

export function listTransactions(): Promise<TransactionHistory> {
  return invoke("list_transactions");
}

export function rollbackTransaction(
  transactionId: string,
  action: RollbackAction,
): Promise<RollbackReport> {
  return invoke("rollback_transaction", {
    selection: { transaction_id: transactionId, action },
  });
}

export function openPath(pathOrObjectId: string, transactionId?: string): Promise<void> {
  const selection = transactionId
    ? { kind: "transaction", path: pathOrObjectId, transaction_id: transactionId }
    : { kind: "granted", object_id: pathOrObjectId };
  return invoke("open_path", { selection });
}

export function openRestoredThread(
  path: string,
  transactionId: string,
): Promise<RegistrationStatus> {
  return invoke("open_restored_thread", {
    selection: { path, transaction_id: transactionId },
  });
}

export function startMigrateAndConnect(selection: StartMigrationSelection): Promise<MigrationJobSnapshot> {
  return invoke("start_migrate_and_connect", { selection });
}

export function getMigrationJob(jobId: string): Promise<MigrationJobSnapshot> {
  return invoke("get_migration_job", { jobId });
}

export function getAppConfig(): Promise<AppConfig> {
  return invoke("get_app_config");
}

export function saveAppConfig(config: AppConfig): Promise<AppConfig> {
  return invoke("save_app_config", { config });
}

export function runLocalBackup(request: LocalBackupCommandRequest): Promise<LocalSnapshot> {
  return invoke("run_local_backup", { request });
}

export function listLocalBackups(request: LocalBackupListRequest): Promise<LocalSnapshotSummary[]> {
  return invoke("list_local_backups", { request });
}

export function restoreLocalBackup(request: LocalRestoreRequest): Promise<LocalRestoreReport> {
  return invoke("restore_local_backup", { request });
}

export function getSchedulerStatus(): Promise<SchedulerStatus> {
  return invoke("get_scheduler_status");
}

export function setScheduler(enabled: boolean, intervalMinutes: number): Promise<SchedulerStatus> {
  return invoke("set_scheduler", {
    request: { enabled, interval_minutes: intervalMinutes },
  });
}

export function startCloudConfiguration(
  configFile: string,
  remoteName: string,
): Promise<CloudConfigStart> {
  return invoke("start_cloud_configuration", {
    request: { config_file: configFile, remote_name: remoteName },
  });
}

export function continueCloudConfiguration(
  configFile: string,
  remoteName: string,
  state: string,
  result: string,
): Promise<CloudConfigStart> {
  return invoke("continue_cloud_configuration", {
    request: { config_file: configFile, remote_name: remoteName, state, result },
  });
}

export function testCloudConnection(config: CloudConfig): Promise<CloudOperationResult> {
  return invoke("test_cloud_connection", { config });
}

export function uploadLocalSnapshot(request: CloudUploadRequest): Promise<CloudOperationResult> {
  return invoke("upload_local_snapshot", { request });
}
