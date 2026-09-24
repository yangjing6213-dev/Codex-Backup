import { CheckCircle2, TriangleAlert } from "lucide-react";

import { useI18n } from "../../lib/i18n";
import type { BackupIntegrityStatus, BackupIssue, BackupIssueKind } from "../../lib/types";

type BackupResultData = {
  exclusions?: BackupIssue[];
  notices?: BackupIssue[];
  integrity_warnings?: BackupIssue[];
  missing?: BackupIssue[];
  integrity_status?: BackupIntegrityStatus;
};

type BackupResultTone = "success" | "warning" | "error";

const issueText: Record<BackupIssueKind, { impact: string; solution: string }> = {
  security_exclusion: {
    impact: "登录凭据未包含在备份中",
    solution: "恢复后重新登录",
  },
  rebuildable_dependency: {
    impact: "可重新安装的依赖未包含在备份中",
    solution: "恢复后重新安装依赖",
  },
  runtime_ephemeral: {
    impact: "运行时临时文件未包含在备份中",
    solution: "启动应用后自动重新生成",
  },
  test_artifact: {
    impact: "测试产物未包含在备份中",
    solution: "需要时重新运行测试",
  },
  jsonl_integrity: {
    impact: "数据文件可能不完整",
    solution: "保留文件并检查受影响的会话",
  },
  missing_source: {
    impact: "源目录未进入本次备份",
    solution: "检查或重新选择正确的项目路径，必要时重新连接磁盘；仅在确认不再需要该项目时取消选择",
  },
  enumeration_failure: {
    impact: "目录内容未能完整读取",
    solution: "检查目录权限后重试",
  },
  copy_failure: {
    impact: "文件未能完整复制",
    solution: "关闭占用文件的程序后重试",
  },
  filesystem_redirect: {
    impact: "文件系统链接目标未包含在备份中",
    solution: "检查链接目标后重试",
  },
  unsupported_entry: {
    impact: "此文件类型未包含在备份中",
    solution: "转换为普通文件后重试",
  },
  legacy_unknown: {
    impact: "旧版结果未提供问题分类",
    solution: "检查受影响路径后重试",
  },
};

export function backupResultTone(result: BackupResultData): BackupResultTone {
  if ((result.missing?.length ?? 0) > 0 || result.integrity_status === "partial") return "error";
  if ((result.notices?.length ?? 0) > 0 || (result.integrity_warnings?.length ?? 0) > 0 || result.integrity_status === "warning") return "warning";
  return "success";
}

function countByKind(issues: BackupIssue[]): Map<BackupIssueKind, number> {
  const counts = new Map<BackupIssueKind, number>();
  for (const issue of issues) {
    const kind = issue.kind ?? "legacy_unknown";
    counts.set(kind, (counts.get(kind) ?? 0) + 1);
  }
  return counts;
}

function statusKey(operation: "backup" | "restore", tone: BackupResultTone): string {
  if (operation === "restore") {
    if (tone === "error") return "恢复已部分完成";
    if (tone === "warning") return "恢复完成，存在注意事项";
    return "恢复完整完成";
  }
  if (tone === "error") return "备份已部分完成";
  if (tone === "warning") return "备份完成，存在注意事项";
  return "备份完整完成";
}

export function BackupResultSummary({ manifest, operation = "backup" }: {
  manifest: BackupResultData;
  operation?: "backup" | "restore";
}) {
  const { t } = useI18n();
  const tone = backupResultTone(manifest);
  const status = t(statusKey(operation, tone));
  const exclusions = countByKind(manifest.exclusions ?? []);
  const notices = countByKind(manifest.notices ?? []);
  const actionable = [
    ...(manifest.integrity_warnings ?? []).map(issue => ({ issue, label: "数据警告" })),
    ...(manifest.missing ?? []).map(issue => ({ issue, label: "文件缺失" })),
  ];
  const hasDetails = exclusions.size > 0 || notices.size > 0 || actionable.length > 0;
  const StatusIcon = tone === "success" ? CheckCircle2 : TriangleAlert;

  return (
    <div className={`backup-result-summary ${tone}`} role="status" aria-label={status}>
      <div className="backup-result-status">
        <StatusIcon aria-hidden="true" />
        <strong>{status}</strong>
      </div>
      {hasDetails && <p>{[
        ["安全排除", manifest.exclusions?.length ?? 0],
        ["自动处理", manifest.notices?.length ?? 0],
        ["数据警告", manifest.integrity_warnings?.length ?? 0],
        ["文件缺失", manifest.missing?.length ?? 0],
      ].filter(([, count]) => Number(count) > 0).map(([label, count]) => t("{label} {count} 项", { label: t(String(label)), count })).join(" · ")}</p>}
      {hasDetails && <details className="backup-result-details">
        <summary>{t("查看详细结果")}</summary>
        {[...exclusions].map(([kind, count]) => <IssueSummary key={`exclusion-${kind}`} kind={kind} count={count} label="安全排除" />)}
        {[...notices].map(([kind, count]) => <IssueSummary key={`notice-${kind}`} kind={kind} count={count} label="自动处理" />)}
        {actionable.map(({ issue, label }, index) => {
          const text = issueText[issue.kind ?? "legacy_unknown"];
          return <div className="backup-result-issue" key={`${issue.path}-${index}`}>
            <strong>{t(label)}</strong>
            <code>{issue.path}</code>
            <p><span>{t("影响")}</span>{t(text.impact)}</p>
            <p><span>{t("建议解决方法")}</span>{t(text.solution)}</p>
          </div>;
        })}
      </details>}
    </div>
  );
}

function IssueSummary({ kind, count, label }: { kind: BackupIssueKind; count: number; label: string }) {
  const { t } = useI18n();
  const text = issueText[kind];
  return <div className="backup-result-issue aggregate">
    <strong>{t("{label} {count} 项", { label: t(label), count })}</strong>
    <p><span>{t("影响")}</span>{t(text.impact)}</p>
    <p><span>{t("建议解决方法")}</span>{t(text.solution)}</p>
  </div>;
}
