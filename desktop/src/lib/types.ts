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
  projects_file_counts_known?: boolean;
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

export interface PlannedSession {
  package_source: string;
  target: string;
  source_task_id: string;
  target_task_id: string;
  title: string;
  source_content_hash: string;
  expected_final_content_hash: string;
  action: "skip" | "import" | "import_as_branch";
}

export interface RestorePlan {
  plan_id: string;
  package_path: string;
  package_id: string;
  archive_hash: string;
  target_codex_home: string;
  projects_root: string;
  operations: PlannedOperation[];
  sessions: PlannedSession[];
  reference_rewrites: unknown[];
  bridge_verification: {
    session_index: string | null;
    sqlite_database: string | null;
  };
  conflict_count: number;
  required_bytes: number;
}

export interface ContinuationProbeOptions {
  probe_thread_id: string;
  online_usage_confirmed: boolean;
}

export interface StartMigrationSelection extends ContinuationProbeOptions {
  plan_id: string;
  codex_closed_confirmed: boolean;
  register_projects: boolean;
}

export interface CodexAccessVerification {
  required_threads: number;
  recognized_threads: number;
  probe_thread_id: string | null;
  threads_recognized: boolean;
  continuation_probe_valid: boolean;
  ephemeral_fork: boolean;
}

export interface RestoreOptions {
  codex_closed_confirmed: boolean;
  register_projects: boolean;
  continuation_probe?: ContinuationProbeOptions | null;
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
  codex_access: CodexAccessVerification;
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

export type MigrationJobStage =
  | "preflight"
  | "restoring_files"
  | "files_verified"
  | "recognizing_threads"
  | "probing_continuation"
  | "committing"
  | "rolling_back"
  | "finished";

export type MigrationJobStatus =
  | "running"
  | "succeeded"
  | "failed_before_write"
  | "rolled_back"
  | "rollback_failed";

export interface MigrationJobSnapshot {
  job_id: string;
  plan_id: string;
  /** Assigned only after the restore journal is durably prepared. */
  transaction_id: string | null;
  stage: MigrationJobStage;
  status: MigrationJobStatus;
  report: RestoreReport | null;
  /** Sanitized by the backend before publication. */
  error: RehomeError | null;
  updated_at: string;
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
  file_count: number;
  file_count_complete: boolean;
  skipped_entries: number;
}

export interface LocalDiscoveryResult {
  candidates: LocalProjectCandidate[];
  codex_homes: string[];
  conversation_count: number;
  scanned_roots: string[];
  skipped_roots: string[];
  warnings: string[];
  permission_denied_count: number;
  other_warning_count: number;
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
  automatic_project_scan: boolean;
  project_scan_roots: string[];
  project_selection_initialized: boolean;
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

export type BackupIssueKind =
  | "security_exclusion"
  | "rebuildable_dependency"
  | "runtime_ephemeral"
  | "test_artifact"
  | "jsonl_integrity"
  | "missing_source"
  | "enumeration_failure"
  | "copy_failure"
  | "filesystem_redirect"
  | "unsupported_entry"
  | "legacy_unknown";

export type BackupIntegrityStatus = "complete" | "warning" | "partial";

export interface BackupIssue {
  path: string;
  reason: string;
  bytes: number;
  kind?: BackupIssueKind;
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
  notices?: BackupIssue[];
  integrity_warnings?: BackupIssue[];
  missing: BackupIssue[];
  integrity_status: BackupIntegrityStatus;
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
  integrity_status: BackupIntegrityStatus;
  complete: boolean;
}

export interface LocalRestoreReport {
  restic_snapshot_id: string;
  restored_root: string;
  restored_files: number;
  restored_bytes: number;
  complete: boolean;
  exclusions: BackupIssue[];
  notices?: BackupIssue[];
  integrity_warnings?: BackupIssue[];
  integrity_status: BackupIntegrityStatus;
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
  backup_repository_invalid: "备份仓库无效，请选择新的空目录或有效仓库。",
  backup_password_required: "恢复密码错误或缺失，无法打开仓库。",
  backup_failed: "本地备份未完整完成；原始项目文件未被修改。",
  unsafe_path: "路径不符合安全要求，请重新选择本地目录。",
  config_invalid: "设置无效，请检查字段后重试。",
  admin_scan_unavailable: "管理员扫描未完成，请检查 Windows UAC 提示。",
  scheduler_unavailable: "计划任务不可用，本地手动备份仍可继续。",
  cloud_disabled: "云端备份已关闭，未执行远端操作。",
  cloud_configuration: "云端配置不完整，请完成 OneDrive 配置后重试。",
  cloud_unavailable: "云端操作失败，本地备份不受影响。",
  codex_not_found: "未找到 Codex 数据，请在数据页选择数据位置。",
  project_conflict: "恢复目标已存在，未覆盖任何资料。",
  restore_failed: "恢复未完成，请检查操作状态。",
  package_invalid: "迁移包无效，请重新选择并预览。",
  checksum_mismatch: "迁移包校验失败，未写入目标。",
  unsupported_schema: "迁移包格式暂不支持。",
  codex_running: "Codex 仍在运行，请保存工作后退出再重试。",
  disk_space_insufficient: "磁盘空间不足，既有资料保持不变。",
  rollback_failed: "回滚失败，请保留现场并查看迁移记录。",
  registration_incomplete: "Codex 注册未完成，请按提示手动打开恢复后的项目。",
  codex_app_server_unavailable: "Codex 验证服务不可用。",
  codex_authentication_required: "Codex 需要登录。",
  codex_verification_failed: "Codex 接入验证未完成。",
  codex_cleanup_unconfirmed: "Codex 辅助进程退出状态未确认。",
  migration_job_not_found: "迁移任务不存在或已过期。",
};

const localizedErrorGuidance: Record<string, { cause: string; solution: string }> = {
  backup_engine_unavailable: {
    cause: "原因：应用无法启动随安装包提供的 restic。",
    solution: "解决方法：请重新安装应用；若仍失败，请保留技术详情。",
  },
  backup_repository_invalid: {
    cause: "原因：所选路径不是可用的 ENHE/restic 备份仓库，或仓库结构无法读取。",
    solution: "解决方法：请选择新的空目录，或选择可正常打开的旧备份仓库，并根据技术详情检查权限或损坏情况。",
  },
  backup_password_required: {
    cause: "原因：恢复密码缺失、错误，或与该仓库不匹配。",
    solution: "解决方法：请输入创建该仓库时使用的恢复密码后重试。",
  },
  backup_failed: {
    cause: "原因：备份引擎未能完成操作。",
    solution: "解决方法：请检查备份目录权限和剩余空间，确认恢复密码后重试。",
  },
  unsafe_path: {
    cause: "原因：所选路径可能存在目录重叠、路径越界、格式无效或不支持的名称。",
    solution: "解决方法：请重新选择互不重叠的绝对本地目录；若仍失败，请根据技术详情修正路径。",
  },
  restore_failed: {
    cause: "原因：恢复请求未能完成，可能是其他操作仍在进行、计划已过期或恢复过程出错。",
    solution: "解决方法：等待其他操作结束，并查看迁移记录；如没有相关记录，请重新预览以重建计划。本地快照恢复请检查仓库、密码和空目标目录。",
  },
  codex_app_server_unavailable: {
    cause: "原因：无法启动或连接 Codex App Server。",
    solution: "解决方法：检查 Codex 安装及版本是否支持 App Server，修复后完全退出 Codex，再重新预览。",
  },
  codex_authentication_required: {
    cause: "原因：已配置模型服务的身份验证未通过。",
    solution: "解决方法：打开 Codex 完成登录并检查模型服务配置，保存工作并完全退出后重新预览。",
  },
  codex_verification_failed: {
    cause: "原因：对话识别或临时分支续聊验证未完成。",
    solution: "解决方法：检查联网同意、所选对话、模型服务和网络；确认 Codex 版本支持临时分支。若启用了 MCP 或其他集成，请在 Codex 中手动检查并处理，或明确选择“仅恢复文件”。处理后重新预览，不会自动改用仅恢复文件。",
  },
  codex_cleanup_unconfirmed: {
    cause: "原因：辅助进程可能仍在运行，未尝试检查点或回滚写入。",
    solution: "解决方法：完全关闭 Codex 及辅助进程，查看迁移记录和保留副本，再判断如何恢复。继续回滚不保证成功，请勿覆盖较新的数据。",
  },
  migration_job_not_found: {
    cause: "原因：当前应用无法找到此任务的状态记录。",
    solution: "解决方法：查看迁移记录确认实际结果；状态查询失败不代表迁移失败，也不能证明已回滚。",
  },
  rollback_failed: {
    cause: "原因：本地恢复未能完成，需检查事务记录和保留副本。",
    solution: "解决方法：完全关闭 Codex 及辅助进程，前往迁移记录检查恢复状态；保留副本，不要覆盖较新的数据。",
  },
  disk_space_insufficient: {
    cause: "原因：目标磁盘没有足够空间完成操作。",
    solution: "解决方法：请释放磁盘空间，或选择空间充足的目录后重试。",
  },
};

export function errorMessage(error: unknown, translate?: (key: string) => string): string {
  if (translate && typeof error === "object" && error !== null && "code" in error) {
    const code = String((error as { code: unknown }).code);
    const key = localizedErrorKeys[code];
    if (key) {
      const translated = translate(key);
      const detail = "message" in error ? String((error as { message: unknown }).message) : "";
      const guidance = localizedErrorGuidance[code];
      if (guidance) {
        return [
          translated,
          translate(guidance.cause),
          translate(guidance.solution),
          detail ? `${translate("技术详情：")}${detail}` : "",
        ].filter(Boolean).join("\n");
      }
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
