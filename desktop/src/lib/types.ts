export type SourceOs = "windows" | "macos";
export type RecoveryStatus =
  | "prepared"
  | "applying"
  | "verifying"
  | "committed"
  | "rolling_back"
  | "rolled_back"
  | "rollback_failed";
export type ChangeKind = "add" | "update" | "unchanged" | "preserve" | "conflict";
export type FileConflictResolution = "keep_existing" | "use_package";
export type RegistrationStatus =
  | "registered"
  | "command_unavailable"
  | "manual_open_required"
  | { invocation_failed: { message: string } };

export interface RehomeError {
  code: string;
  message: string;
}

export interface ContentCounts {
  projects: number;
  project_files: number;
  conversations: number;
  skills: number;
  plugins: number;
  generated_images: number;
  sqlite_threads: number;
}

export interface ProjectEntry {
  project_id: string;
  name: string;
  source_path: string;
  source_available: boolean;
  archive_path: string;
  file_count: number;
  content_bytes: number;
  git_remote: string | null;
  git_branch: string | null;
  git_head: string | null;
}

export interface ConversationEntry {
  task_id: string;
  project_id: string | null;
  title: string;
  updated_at: string;
  content_hash: string;
  archive_path: string;
  classification: {
    parent_task_id: string | null;
    agent_path: string | null;
    agent_nickname: string | null;
    depth: number | null;
  } | null;
}

export interface OptionalContentEntry {
  content_id: string;
  name: string;
  source_path: string;
  relative_path: string;
  size_bytes: number;
  thumbnail_data_url: string | null;
  reveal_id: string | null;
}

export interface CodexInventory {
  codex_home: string;
  source_os: SourceOs;
  source_arch: string;
  source_device_id: string;
  counts: ContentCounts;
  projects: ProjectEntry[];
  project_paths: string[];
  conversations: ConversationEntry[];
  conversation_paths: string[];
  session_index_path: string | null;
  state_db_path: string | null;
  skill_paths: string[];
  plugin_paths: string[];
  generated_image_paths: string[];
  skills: OptionalContentEntry[];
  plugins: OptionalContentEntry[];
  generated_images: OptionalContentEntry[];
  warnings: string[];
}

export interface CreatePackageRequest {
  project_ids: string[];
  conversation_ids: string[];
  skill_ids: string[];
  plugin_ids: string[];
  generated_image_ids: string[];
}

export interface CreatePackageReport {
  package_path: string;
  package_id: string;
  bytes_written: number;
  counts: ContentCounts;
  archive_hash: string;
  reveal_id: string;
}

export interface PackageManifest {
  format: string;
  schema_version: number;
  package_id: string;
  created_at: string;
  source_os: SourceOs;
  source_arch: string;
  source_device_id: string;
  mode: "full";
  parent_checkpoint: string | null;
  counts: ContentCounts;
  projects: ProjectEntry[];
  conversations: ConversationEntry[];
  exclusions: {
    excluded_files: number;
    excluded_bytes: number;
    rules: string[];
  };
}

export interface PackagePreview {
  selection_id: string;
  package_path: string;
  archive_hash: string;
  manifest: PackageManifest;
  checksum_valid: boolean;
  entries: string[];
  forbidden_files_total: number;
}

export interface PlannedOperation {
  package_source: string;
  target: string;
  expected_previous_hash: string | null;
  action: ChangeKind;
  rollback_required: boolean;
}

export interface RestorePlan {
  plan_id: string;
  package_path: string;
  package_id: string;
  archive_hash: string;
  target_codex_home: string;
  projects_root: string;
  operations: PlannedOperation[];
  sessions: unknown[];
  reference_rewrites: unknown[];
  bridge_verification: {
    session_index: string | null;
    sqlite_database: string | null;
  };
  conflict_count: number;
  required_bytes: number;
}

export interface RestoreOptions {
  codex_closed_confirmed: boolean;
  register_projects: boolean;
}

export interface RestoreLocationSelection {
  selection_id: string;
  target_codex_home: string;
  projects_root: string;
  backup_root: string;
}

export interface VerificationReport {
  package_checksum_valid: boolean;
  files_valid: boolean;
  sessions_valid: boolean;
  session_index_valid: boolean;
  sqlite_threads_valid: boolean;
  path_mapping_valid: boolean;
  forbidden_files_absent: boolean;
  project_files_valid: boolean;
  app_registration_valid: boolean;
  app_visible_ready: boolean;
}

export interface ProjectRegistration {
  project_id: string;
  project_path: string;
  status: RegistrationStatus;
}

export interface RestoreReport {
  transaction_id: string;
  package_id: string;
  completed_at: string;
  restored_files: number;
  restored_bytes: number;
  registrations: ProjectRegistration[];
  verification: VerificationReport;
}

export interface RollbackReport {
  transaction_id: string;
  completed_at: string;
  restored_files: number;
  success: boolean;
}

export interface TransactionSummary {
  transaction_id: string;
  package_id: string;
  created_at: string;
  status: RecoveryStatus;
  backup_root: string;
  transaction_backup_path: string;
  target_codex_home: string;
  projects_root: string;
  restored_project_paths: string[];
  changed_files: number;
}

export interface TransactionHistory {
  transactions: TransactionSummary[];
  warnings: string[];
}

export interface LocalProjectCandidate {
  path: string;
  name: string;
  markers: string[];
}

export interface LocalDiscoveryResult {
  candidates: LocalProjectCandidate[];
  codex_homes: string[];
  conversation_count: number;
  scanned_roots: string[];
  skipped_roots: string[];
  warnings: string[];
  cancelled: boolean;
}

export type Appearance = "light" | "dark" | "system";

export interface RetentionPolicy {
  high_frequency_hours: number;
  daily_days: number;
  weekly_weeks: number;
}

export interface CloudConfig {
  enabled: boolean;
  remote_name: string | null;
  remote_path: string | null;
  repository_path: string | null;
  config_file: string | null;
}

export interface AppConfig {
  config_version: number;
  codex_home: string | null;
  local_repository: string | null;
  selected_project_paths: string[];
  frequency_minutes: number;
  retention: RetentionPolicy;
  cloud: CloudConfig;
  locale: "zh-CN" | "en";
  appearance: Appearance;
  automatic_backup_enabled: boolean;
}

export interface SchedulerStatus {
  enabled: boolean;
  task_name: string;
  worker_path?: string | null;
  interval_minutes?: number | null;
  next_run_time?: string | null;
  last_result?: string | null;
  message?: string | null;
}

export interface BackupIssue {
  path: string;
  reason: string;
  bytes: number;
}

export interface BackupManifest {
  format: string;
  schema_version: number;
  logical_backup_id: string;
  batch_id: string;
  created_at: string;
  app_version: string;
  restic_version: string;
  source_device_id: string;
  source_codex_home: string;
  project_paths: string[];
  git_metadata_paths: string[];
  file_count: number;
  byte_count: number;
  fingerprint: string;
  exclusions: BackupIssue[];
  missing: BackupIssue[];
  integrity_status: "complete" | "partial";
}

export interface LocalSnapshot {
  logical_backup_id: string;
  restic_snapshot_id: string;
  manifest: BackupManifest;
  complete: boolean;
}

export interface LocalSnapshotSummary {
  logical_backup_id: string | null;
  restic_snapshot_id: string;
  created_at: string;
  file_count: number;
  byte_count: number;
  complete: boolean;
}

export interface LocalRestoreReport {
  restic_snapshot_id: string;
  restored_root: string;
  restored_files: number;
  restored_bytes: number;
  complete: boolean;
  missing: BackupIssue[];
}

export interface LocalBackupRequest {
  codex_home: string;
  project_paths: string[];
  repository: string;
  recovery_password: string;
  remember_password: boolean;
  source_device_id?: string;
}

export interface LocalBackupListRequest {
  repository: string;
  recovery_password: string;
}

export interface LocalRestoreRequest {
  snapshot_id: string;
  repository: string;
  recovery_password: string;
  target: string;
}

export interface CloudOperationResult {
  state: "off" | "configuring" | "on" | "needs_attention";
  message: string;
  remote_path: string | null;
}

export interface CloudConfigStart {
  config_file: string;
  remote_name: string;
  provider: string;
  completed: boolean;
  question: CloudConfigQuestion | null;
}

export interface CloudConfigQuestion {
  state: string;
  name: string | null;
  help: string | null;
  default_value: string | null;
  is_password: boolean;
  examples: string[];
  error: string | null;
}

export interface CloudUploadRequest {
  config: CloudConfig;
  source_repository: string;
  source_password: string;
  target_password: string;
  snapshot_id: string;
}

export type RollbackAction = "rollback" | "resume";

const localizedErrorKeys: Record<string, string> = {
  backup_engine_unavailable: "备份引擎不可用，请检查安装包中的 restic。",
  backup_repository_invalid: "备份仓库无效，请选择 ENHE 创建的仓库目录。",
  backup_password_required: "恢复密码错误或缺失，无法打开仓库。",
  backup_failed: "本地备份失败，既有版本保持不变。",
  unsafe_path: "路径不安全，请选择互不重叠的本地目录。",
  config_invalid: "设置无效，请检查字段后重试。",
  scheduler_unavailable: "计划任务不可用，本地手动备份仍可继续。",
  cloud_disabled: "云端备份已关闭，未执行远端操作。",
  cloud_configuration: "云端配置不完整，请完成 OneDrive 配置后重试。",
  cloud_unavailable: "云端操作失败，本地备份不受影响。",
  codex_not_found: "未找到 Codex 数据，请在设置中选择数据位置。",
  project_conflict: "恢复目标已存在，未覆盖任何资料。",
  restore_failed: "本地备份失败，既有版本保持不变。",
  package_invalid: "迁移包无效，请重新选择并预览。",
  checksum_mismatch: "迁移包校验失败，未写入目标。",
  unsupported_schema: "迁移包格式暂不支持。",
  codex_running: "Codex 仍在运行，请保存工作后退出再重试。",
  disk_space_insufficient: "磁盘空间不足，既有资料保持不变。",
  rollback_failed: "回滚失败，请保留现场并查看迁移记录。",
  registration_incomplete: "Codex 注册未完成，请按提示手动打开恢复后的项目。",
};

export function errorMessage(error: unknown, translate?: (key: string) => string): string {
  if (translate && typeof error === "object" && error !== null && "code" in error) {
    const code = String((error as { code: unknown }).code);
    const key = localizedErrorKeys[code];
    if (key) {
      const translated = translate(key);
      const detail = "message" in error ? String((error as { message: unknown }).message) : "";
      return code === "codex_not_found" && detail ? `${translated} ${detail}` : translated;
    }
  }
  if (typeof error === "object" && error !== null && "message" in error) {
    return String((error as { message: unknown }).message);
  }
  return String(error);
}

export function registrationIsComplete(status: RegistrationStatus): boolean {
  return status === "registered";
}
