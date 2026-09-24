import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";

import { I18nProvider } from "../../lib/i18n";
import type { BackupIssue, BackupManifest } from "../../lib/types";
import { BackupResultSummary, backupResultTone } from "./BackupResultSummary";

function manifest(overrides: Partial<BackupManifest> = {}): BackupManifest {
  return {
    format: "enhe-codex-backup",
    schema_version: 1,
    logical_backup_id: "00000000-0000-0000-0000-000000000001",
    batch_id: "00000000-0000-0000-0000-000000000002",
    created_at: "2026-09-20T00:00:00Z",
    app_version: "0.1.6",
    restic_version: "restic synthetic",
    source_device_id: "00000000-0000-0000-0000-000000000003",
    source_codex_home: "C:/Synthetic/.codex",
    project_paths: ["C:/Synthetic/project"],
    git_metadata_paths: [],
    file_count: 1,
    byte_count: 7,
    fingerprint: "synthetic",
    exclusions: [],
    notices: [],
    integrity_warnings: [],
    missing: [],
    integrity_status: "complete",
    ...overrides,
  };
}

function issue(overrides: Partial<BackupIssue> = {}): BackupIssue {
  return {
    path: "C:/Synthetic/item",
    reason: "synthetic detail",
    bytes: 0,
    kind: "legacy_unknown",
    ...overrides,
  };
}

function renderSummary(value: BackupManifest, operation: "backup" | "restore" = "backup") {
  return render(<I18nProvider><BackupResultSummary manifest={value} operation={operation} /></I18nProvider>);
}

describe("BackupResultSummary", () => {
  beforeEach(() => window.localStorage.clear());

  it("derives success, warning and error from buckets and integrity status instead of reason text", () => {
    expect(backupResultTone(manifest())).toBe("success");
    expect(backupResultTone(manifest({
      notices: [issue({ kind: "copy_failure", reason: "source is unavailable" })],
    }))).toBe("warning");
    expect(backupResultTone(manifest({
      missing: [issue({ kind: "rebuildable_dependency", reason: "safe to rebuild" })],
    }))).toBe("error");
    expect(backupResultTone(manifest({ integrity_status: "warning" }))).toBe("warning");
    expect(backupResultTone(manifest({ integrity_status: "partial" }))).toBe("error");
  });

  it("aggregates rebuildable notices without listing every path", () => {
    const notices = Array.from({ length: 444 }, (_, index) => issue({
      path: `project/node_modules/pkg-${index}`,
      reason: "dependency link was not followed",
      kind: "rebuildable_dependency",
    }));

    const { container } = renderSummary(manifest({ notices, integrity_status: "warning" }));

    expect(screen.getByRole("status", { name: "备份完成，存在注意事项" })).toHaveClass("warning");
    expect(screen.getByText(/自动处理 444 项/, { selector: ".backup-result-summary > p" })).toBeVisible();
    expect(screen.queryByText("project/node_modules/pkg-443")).not.toBeInTheDocument();
    expect(screen.getByText(/重新安装依赖/)).toBeInTheDocument();
    expect(container.querySelector("details")).not.toHaveAttribute("open");
  });

  it("aggregates security exclusions by stable kind and explains that sign-in is required", () => {
    renderSummary(manifest({
      exclusions: [
        issue({ path: "C:/Synthetic/.codex/auth.json", kind: "security_exclusion" }),
        issue({ path: "C:/Synthetic/.codex/session.json", kind: "security_exclusion" }),
      ],
    }));

    expect(screen.getByText(/安全排除 2 项/, { selector: ".backup-result-summary > p" })).toBeVisible();
    expect(screen.getByText(/重新登录/)).toBeInTheDocument();
    expect(screen.queryByText("C:/Synthetic/.codex/auth.json")).not.toBeInTheDocument();
  });

  it("lists every actionable path with impact and a translated solution", () => {
    renderSummary(manifest({
      integrity_warnings: [issue({
        path: "C:/Synthetic/.codex/sessions/damaged.jsonl",
        kind: "jsonl_integrity",
      })],
      missing: [issue({
        path: "F:/Projects/missing-project",
        reason: "source is unavailable",
        kind: "missing_source",
      })],
      integrity_status: "partial",
    }));

    expect(screen.getByRole("status", { name: "备份已部分完成" })).toHaveClass("error");
    expect(screen.getByText("C:/Synthetic/.codex/sessions/damaged.jsonl")).toBeInTheDocument();
    expect(screen.getByText("F:/Projects/missing-project")).toBeInTheDocument();
    expect(screen.getAllByText("影响")).toHaveLength(2);
    expect(screen.getAllByText("建议解决方法")).toHaveLength(2);
    expect(screen.getByText("数据警告")).toBeInTheDocument();
    expect(screen.getByText("文件缺失")).toBeInTheDocument();
    expect(screen.getByText(/文件缺失 1 项/)).toBeVisible();
    expect(screen.getByText(/检查或重新选择正确的项目路径/)).toBeInTheDocument();
    expect(screen.getByText(/仅在确认不再需要该项目时取消选择/)).toBeInTheDocument();
  });

  it("normalizes optional arrays from older backends at the component boundary", () => {
    renderSummary(manifest({ notices: undefined, integrity_warnings: undefined }));

    expect(screen.getByRole("status", { name: "备份完整完成" })).toHaveClass("success");
    expect(screen.queryByText("查看详细结果")).not.toBeInTheDocument();
  });

  it("renders English status and remedies through the shared restore component", () => {
    window.localStorage.setItem("enhe-codex-backup.locale", "en");
    renderSummary(manifest({
      notices: [issue({ kind: "rebuildable_dependency" })],
      integrity_status: "warning",
    }), "restore");

    expect(screen.getByRole("status", { name: "Restore completed with items to review" })).toHaveClass("warning");
    expect(screen.getByText(/Reinstall dependencies/)).toBeInTheDocument();
  });
});
