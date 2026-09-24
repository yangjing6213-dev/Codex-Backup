import { describe, expect, expectTypeOf, it } from "vitest";
import { renderHook } from "@testing-library/react";

import { I18nProvider, useI18n } from "./i18n";
import { errorMessage } from "./types";
import type { BackupIntegrityStatus, BackupIssueKind, LocalRestoreReport } from "./types";
import type {
  CodexAccessVerification,
  ContinuationProbeOptions,
  MigrationJobSnapshot,
  MigrationJobStage,
  MigrationJobStatus,
  PlannedSession,
  RehomeError,
  RestoreOptions,
  RestorePlan,
  RestoreReport,
} from "./types";

describe("migration contracts", () => {
  it("keeps legacy restore requests probe-free and types explicit consent", () => {
    const legacy = {
      codex_closed_confirmed: true,
      register_projects: false,
    } satisfies RestoreOptions;
    const probe = {
      probe_thread_id: "11111111-1111-4111-8111-111111111111",
      online_usage_confirmed: false,
    } satisfies ContinuationProbeOptions;
    const options = { ...legacy, continuation_probe: probe } satisfies RestoreOptions;

    expectTypeOf<ContinuationProbeOptions>().toEqualTypeOf<{
      probe_thread_id: string;
      online_usage_confirmed: boolean;
    }>();
    expectTypeOf<RestoreOptions["continuation_probe"]>()
      .toEqualTypeOf<ContinuationProbeOptions | null | undefined>();
    expect(legacy).not.toHaveProperty("continuation_probe");
    expect(JSON.parse(JSON.stringify(options)).continuation_probe).toEqual({
      probe_thread_id: "11111111-1111-4111-8111-111111111111",
      online_usage_confirmed: false,
    });
  });

  it("exposes the full planned-session contract for selecting a probe", () => {
    expectTypeOf<RestorePlan["sessions"]>().toEqualTypeOf<PlannedSession[]>();
    expectTypeOf<PlannedSession>().toEqualTypeOf<{
      package_source: string;
      target: string;
      source_task_id: string;
      target_task_id: string;
      title: string;
      source_content_hash: string;
      expected_final_content_hash: string;
      action: "skip" | "import" | "import_as_branch";
    }>();
  });

  it("matches backend stages, statuses and structured result fields exactly", () => {
    expectTypeOf<MigrationJobStage>().toEqualTypeOf<
      "preflight" | "restoring_files" | "files_verified" | "recognizing_threads"
      | "probing_continuation" | "committing" | "rolling_back" | "finished"
    >();
    expectTypeOf<MigrationJobStatus>().toEqualTypeOf<
      "running" | "succeeded" | "failed_before_write" | "rolled_back" | "rollback_failed"
    >();
    expectTypeOf<MigrationJobSnapshot>().toEqualTypeOf<{
      job_id: string;
      plan_id: string;
      transaction_id: string | null;
      stage: MigrationJobStage;
      status: MigrationJobStatus;
      report: RestoreReport | null;
      error: RehomeError | null;
      updated_at: string;
    }>();
    expectTypeOf<CodexAccessVerification>().toEqualTypeOf<{
      required_threads: number;
      recognized_threads: number;
      probe_thread_id: string | null;
      threads_recognized: boolean;
      continuation_probe_valid: boolean;
      ephemeral_fork: boolean;
    }>();
    expectTypeOf<RestoreReport["verification"]["codex_access"]>()
      .toEqualTypeOf<CodexAccessVerification>();
  });

  it("represents every terminal outcome without deriving transaction identity from messages", () => {
    const transactionId = "22222222-2222-4222-8222-222222222222";
    const running = {
      job_id: "33333333-3333-4333-8333-333333333333",
      plan_id: "44444444-4444-4444-8444-444444444444",
      transaction_id: null,
      stage: "preflight",
      status: "running",
      report: null,
      error: null,
      updated_at: "2026-09-20T00:00:00Z",
    } satisfies MigrationJobSnapshot;
    const report = {
      transaction_id: transactionId,
      package_id: "55555555-5555-4555-8555-555555555555",
      completed_at: "2026-09-20T00:00:00Z",
      restored_files: 2,
      restored_bytes: 128,
      registrations: [],
      verification: {
        package_checksum_valid: true,
        files_valid: true,
        sessions_valid: true,
        session_index_valid: true,
        sqlite_threads_valid: true,
        path_mapping_valid: true,
        forbidden_files_absent: true,
        project_files_valid: true,
        app_registration_valid: false,
        app_visible_ready: false,
        codex_access: {
          required_threads: 3,
          recognized_threads: 3,
          probe_thread_id: "11111111-1111-4111-8111-111111111111",
          threads_recognized: true,
          continuation_probe_valid: true,
          ephemeral_fork: true,
        },
      },
    } satisfies RestoreReport;
    const finished = { ...running, stage: "finished" } as const;
    const outcomes = {
      succeeded: { ...finished, status: "succeeded", transaction_id: transactionId, report },
      failed_before_write: {
        ...finished, status: "failed_before_write",
        error: { code: "codex_app_server_unavailable", message: "App Server unavailable" },
      },
      rolled_back: {
        ...finished, status: "rolled_back", transaction_id: transactionId,
        error: { code: "codex_authentication_required", message: "Authentication required" },
      },
      rollback_failed: {
        ...finished, status: "rollback_failed", transaction_id: transactionId,
        error: { code: "rollback_failed", message: "Manual recovery required" },
      },
    } satisfies Record<Exclude<MigrationJobStatus, "running">, MigrationJobSnapshot>;

    expect(Object.values(outcomes).map(({ status, transaction_id, report, error }) => ({
      status, transaction_id, hasReport: report !== null, errorCode: error?.code ?? null,
    }))).toEqual([
      { status: "succeeded", transaction_id: transactionId, hasReport: true, errorCode: null },
      { status: "failed_before_write", transaction_id: null, hasReport: false, errorCode: "codex_app_server_unavailable" },
      { status: "rolled_back", transaction_id: transactionId, hasReport: false, errorCode: "codex_authentication_required" },
      { status: "rollback_failed", transaction_id: transactionId, hasReport: false, errorCode: "rollback_failed" },
    ]);
    expect(report.verification.app_visible_ready).toBe(false);
  });
});

describe("backup result contracts", () => {
  it("preserves typed issues across JSON serialization", () => {
    const issueKind: BackupIssueKind = "jsonl_integrity";
    const integrityStatus: BackupIntegrityStatus = "warning";
    const report = {
      restic_snapshot_id: "12345678",
      restored_root: "C:/Synthetic/restore",
      restored_files: 1,
      restored_bytes: 7,
      complete: false,
      exclusions: [],
      notices: [],
      integrity_warnings: [
        { path: "sessions/partial.jsonl", reason: "invalid record", bytes: 7, kind: issueKind },
      ],
      integrity_status: integrityStatus,
      missing: [],
    } satisfies LocalRestoreReport;

    expect(JSON.parse(JSON.stringify(report))).toEqual(report);
  });
});

describe("errorMessage", () => {
  it.each([
    ["zh-CN", "原因：对话识别或临时分支续聊验证未完成。", "解决方法：检查联网同意、所选对话、模型服务和网络；确认 Codex 版本支持临时分支。若启用了 MCP 或其他集成，请在 Codex 中手动检查并处理，或明确选择“仅恢复文件”。处理后重新预览，不会自动改用仅恢复文件。"],
    ["en", "Cause: Conversation recognition or continuation on a temporary branch did not complete.", "Solution: Check online consent, the selected conversation, model service and network. Confirm the Codex version supports temporary branches. If MCP or other integrations are enabled, check them and resolve any issues manually in Codex, or explicitly choose “Restore files only”. Resolve the issue and preview again; file-only restore is not started automatically."],
  ])("conditionally suggests integration remedies using the real %s translation", (locale, cause, solution) => {
    window.localStorage.setItem("enhe-codex-backup.locale", locale);
    const { result, unmount } = renderHook(useI18n, { wrapper: I18nProvider });
    try {
      const message = errorMessage({ code: "codex_verification_failed", message: "fixed category" }, result.current.t);
      expect(message).toContain(cause);
      expect(message).toContain(solution);
    } finally {
      unmount();
      window.localStorage.removeItem("enhe-codex-backup.locale");
    }
  });

  it("shows a cause, solution and sanitized detail for a backup failure", () => {
    const message = errorMessage(
      { code: "backup_failed", message: "backup engine failed (exit 1): synthetic failure" },
      (key) => key,
    );

    expect(message).toContain("本地备份未完整完成；原始项目文件未被修改。");
    expect(message).toContain("原因：备份引擎未能完成操作。");
    expect(message).toContain("解决方法：请检查备份目录权限和剩余空间，确认恢复密码后重试。");
    expect(message).toContain("技术详情：backup engine failed (exit 1): synthetic failure");
  });

  it("keeps restore failures distinct from backup failures", () => {
    const restore = errorMessage(
      { code: "restore_failed", message: "synthetic restore" },
      (key) => key,
    );

    expect(restore).toContain("恢复未完成，请检查操作状态。");
    expect(restore).toContain("等待其他操作结束");
    expect(restore).toContain("没有相关记录");
    expect(restore).toContain("重新预览");
    expect(restore).not.toContain("保持不变");
    expect(restore).not.toContain("本地备份未完整完成");
  });

  it.each([
    ["codex_app_server_unavailable", "Codex 验证服务不可用", "安装"],
    ["codex_authentication_required", "Codex 需要登录", "登录"],
    ["codex_verification_failed", "Codex 接入验证未完成", "临时分支"],
    ["codex_cleanup_unconfirmed", "Codex 辅助进程退出状态未确认", "保留副本"],
    ["migration_job_not_found", "迁移任务不存在或已过期", "迁移记录"],
  ])("localizes %s with actionable guidance without claiming rollback", (code, cause, remedy) => {
    const message = errorMessage({ code, message: "fixed category" }, key => key);
    expect(message).toContain(cause);
    expect(message).toContain("解决方法：");
    expect(message).toContain(remedy);
    expect(message).not.toContain("本地更改已回滚");
    expect(message).not.toContain("保持不变");
  });
});
