import { useEffect, useRef, useState, type RefObject } from "react";
import {
  AlertTriangle,
  CheckCircle2,
  Circle,
  FileArchive,
  FolderOpen,
  HardDrive,
  LoaderCircle,
  Play,
  ShieldCheck,
  XCircle,
} from "lucide-react";

import {
  applyRestore,
  buildRestorePlan,
  inspectPackage,
  openRestoredThread,
  selectRestoreDestinations,
  startMigrateAndConnect,
  getMigrationJob,
} from "../../lib/api";
import { useI18n } from "../../lib/i18n";
import "../migration.css";
import {
  errorMessage,
  registrationIsComplete,
  type CodexInventory,
  type FileConflictResolution,
  type PackagePreview,
  type ProjectRegistration,
  type RegistrationStatus,
  type RestoreLocationSelection,
  type RestorePlan,
  type RestoreReport,
  type MigrationJobSnapshot,
} from "../../lib/types";

interface ReceivePageProps {
  headingRef: RefObject<HTMLHeadingElement | null>;
  inventory: CodexInventory | null;
  onOperationStart: () => void;
  onOperationEnd: () => void;
  operationBusy?: boolean;
  initialJobId?: string | null;
  onJobIdChange?: (jobId: string | null) => void;
  onOpenHistory?: () => void;
}

const verificationLabels: Array<[keyof RestoreReport["verification"], string]> = [
  ["package_checksum_valid", "迁移包校验"],
  ["files_valid", "文件完整性"],
  ["sessions_valid", "对话文件"],
  ["session_index_valid", "会话索引"],
  ["sqlite_threads_valid", "线程数据库"],
  ["path_mapping_valid", "跨平台路径"],
  ["forbidden_files_absent", "禁用文件隔离"],
  ["project_files_valid", "项目文件"],
  ["app_registration_valid", "Codex 项目登记"],
  ["app_visible_ready", "Codex 可见状态"],
];

export default function ReceivePage({
  headingRef,
  inventory,
  onOperationStart,
  onOperationEnd,
  operationBusy = false,
  initialJobId = null,
  onJobIdChange,
  onOpenHistory,
}: ReceivePageProps) {
  const { t } = useI18n();
  const [preview, setPreview] = useState<PackagePreview | null>(null);
  const [locations, setLocations] = useState<RestoreLocationSelection | null>(null);
  const [plan, setPlan] = useState<RestorePlan | null>(null);
  const [conflictResolution, setConflictResolution] = useState<FileConflictResolution | null>(null);
  const [codexClosed, setCodexClosed] = useState(false);
  const [report, setReport] = useState<RestoreReport | null>(null);
  const [phase, setPhase] = useState<"idle" | "inspecting" | "selecting" | "planning" | "restoring">("idle");
  const [error, setError] = useState<string | null>(null);
  const [registrationStatuses, setRegistrationStatuses] = useState<Record<string, string>>({});
  const requestGeneration = useRef(0);
  const [mode, setMode] = useState<"connect" | "files">("connect");
  const [probeThreadId, setProbeThreadId] = useState("");
  const [onlineUsage, setOnlineUsage] = useState(false);
  const [jobId, setJobId] = useState<string | null>(initialJobId);
  const [snapshot, setSnapshot] = useState<MigrationJobSnapshot | null>(null);
  const [pollError, setPollError] = useState<unknown>(null);
  const [startError, setStartError] = useState<unknown>(null);
  // App callbacks change identity on every render; activity belongs to the job,
  // not to a polling effect or a particular callback instance.
  const callbacks = useRef({ onOperationStart, onOperationEnd, onJobIdChange });
  callbacks.current = { onOperationStart, onOperationEnd, onJobIdChange };
  const migrationActive = useRef(false);
  const jobRunning = Boolean(jobId && (!snapshot || snapshot.status === "running"));
  const controlsBusy = operationBusy || phase !== "idle" || jobRunning;
  const mappedConversations = (preview?.manifest.conversations ?? [])
    .flatMap(conversation => {
      const session = plan?.sessions.find(item => item.source_task_id === conversation.task_id);
      return session ? [{ ...conversation, targetId: session.target_task_id }] : [];
    })
    .sort((a, b) => Date.parse(b.updated_at) - Date.parse(a.updated_at));

  function beginMigration() {
    if (migrationActive.current) return;
    migrationActive.current = true;
    callbacks.current.onOperationStart();
  }

  function endMigration() {
    if (!migrationActive.current) return;
    migrationActive.current = false;
    callbacks.current.onOperationEnd();
  }

  useEffect(() => {
    if (!jobId || (snapshot && snapshot.status !== "running")) return;
    beginMigration();
    let disposed = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    async function poll() {
      try {
        const current = await getMigrationJob(jobId!);
        if (disposed) return;
        setSnapshot(current);
        setPollError(null);
        if (current.status !== "running") {
          endMigration();
          return;
        }
      } catch (caught) {
        if (disposed) return;
        setPollError(caught);
      }
      if (!disposed) timer = setTimeout(() => void poll(), 400);
    }
    void poll();
    return () => { disposed = true; clearTimeout(timer); };
  }, [jobId]);

  // Navigation keeps this page mounted. An actual unmount releases this view's
  // activity count; a new view with the known job ID reacquires it and polls.
  useEffect(() => () => endMigration(), []);

  async function choosePackage() {
    if (controlsBusy) return;
    const generation = ++requestGeneration.current;
    setError(null);
    setPhase("inspecting");
    onOperationStart();
    try {
      const inspected = await inspectPackage();
      if (generation !== requestGeneration.current) return;
      if (inspected) {
        setPreview(inspected);
        clearRestoreSelection();
        if (inspected.manifest.counts.projects === 0) {
          const selected = await selectRestoreDestinations(inspected.selection_id);
          if (generation !== requestGeneration.current) return;
          setLocations(selected);
        } else {
          setLocations(null);
        }
      }
    } catch (caught) {
      if (generation !== requestGeneration.current) return;
      setPreview(null);
      setError(errorMessage(caught, t));
    } finally {
      if (generation === requestGeneration.current) setPhase("idle");
      onOperationEnd();
    }
  }

  async function chooseLocations() {
    if (!preview || controlsBusy) return;
    const generation = ++requestGeneration.current;
    setError(null);
    setPhase("selecting");
    onOperationStart();
    try {
      const selected = await selectRestoreDestinations(preview.selection_id);
      if (generation !== requestGeneration.current) return;
      if (selected) {
        setLocations(selected);
        clearRestoreSelection();
      }
    } catch (caught) {
      if (generation !== requestGeneration.current) return;
      setError(errorMessage(caught, t));
    } finally {
      if (generation === requestGeneration.current) setPhase("idle");
      onOperationEnd();
    }
  }

  async function handlePlan(resolution: FileConflictResolution | null = null) {
    if (!preview || !locations || controlsBusy) return;
    const generation = ++requestGeneration.current;
    setError(null);
    setReport(null);
    setPhase("planning");
    onOperationStart();
    try {
      const nextPlan = await buildRestorePlan(
        preview.selection_id,
        locations.selection_id,
        resolution ?? undefined,
      );
      if (generation === requestGeneration.current) {
        clearRestoreSelection();
        setPlan(nextPlan);
        setConflictResolution(resolution);
        const newest = [...preview.manifest.conversations]
          .sort((a, b) => Date.parse(b.updated_at) - Date.parse(a.updated_at))
          .map(conversation => nextPlan.sessions.find(session => session.source_task_id === conversation.task_id))
          .find(Boolean);
        setMode(newest ? "connect" : "files");
        setProbeThreadId(newest?.target_task_id ?? "");
      }
    } catch (caught) {
      if (generation !== requestGeneration.current) return;
      setPlan(null);
      setError(errorMessage(caught, t));
    } finally {
      if (generation === requestGeneration.current) setPhase("idle");
      onOperationEnd();
    }
  }

  function clearRestoreSelection() {
    setPlan(null);
    setConflictResolution(null);
    setReport(null);
    setCodexClosed(false);
    setRegistrationStatuses({});
    setOnlineUsage(false);
    setProbeThreadId("");
    setJobId(null);
    setSnapshot(null);
    setPollError(null);
    setStartError(null);
    callbacks.current.onJobIdChange?.(null);
  }

  async function handleRestore() {
    if (!canRestore || !plan) return;
    setError(null);
    setPhase("restoring");
    onOperationStart();
    try {
      setReport(await applyRestore(plan.plan_id, {
        codex_closed_confirmed: true,
        register_projects: true,
      }));
    } catch (caught) {
      // A failed attempt consumes its capability and may have rolled back writes.
      // Re-plan against the current files instead of retrying the stale snapshot.
      clearRestoreSelection();
      setError(`${errorMessage(caught, t)} ${t("请重新预览导入内容后重试；如提示回滚失败，请先在迁移记录中恢复。")}`);
    } finally {
      setPhase("idle");
      onOperationEnd();
    }
  }

  async function handleMigration() {
    if (!canRestore || !plan || !onlineUsage || !mappedConversations.some(item => item.targetId === probeThreadId) || migrationActive.current) return;
    setError(null);
    setStartError(null);
    setPhase("restoring");
    beginMigration();
    try {
      const current = await startMigrateAndConnect({
        plan_id: plan.plan_id, codex_closed_confirmed: true, register_projects: true,
        probe_thread_id: probeThreadId, online_usage_confirmed: true,
      });
      setJobId(current.job_id);
      callbacks.current.onJobIdChange?.(current.job_id);
      setSnapshot(current);
      if (current.status !== "running") endMigration();
    } catch (caught) {
      clearRestoreSelection();
      setStartError(caught);
      endMigration();
    } finally {
      setPhase("idle");
    }
  }

  async function handleOpenRestored(registration: ProjectRegistration) {
    if (controlsBusy) return;
    setError(null);
    onOperationStart();
    try {
      const status = await openRestoredThread(registration.project_path, report!.transaction_id);
      setRegistrationStatuses((current) => ({
        ...current,
        [registration.project_id]: registrationStatusMessage(status, t),
      }));
    } catch (caught) {
      setError(errorMessage(caught, t));
    } finally {
      onOperationEnd();
    }
  }

  const canPlan = Boolean(preview && locations && !controlsBusy);
  const projectLocationRequired = Boolean(preview && preview.manifest.counts.projects > 0);
  const canRestore = Boolean(
    plan && plan.conflict_count === 0 && codexClosed && !controlsBusy && !report && !jobId,
  );
  const planOperations = plan
    ? [...plan.operations].sort(
        (left, right) => Number(right.action === "conflict") - Number(left.action === "conflict"),
      )
    : [];
  const manualRegistration = report?.registrations.some(
    (registration) => !registrationIsComplete(registration.status),
  );

  return (
    <div className="page migration-page receive-page">
      <header className="page-header">
        <p className="eyebrow">IMPORT</p>
        <h1 ref={headingRef} tabIndex={-1}>{t("导入 ReHome 迁移包")}</h1>
        <p className="page-description">{t("检查 .rehome 包和目标位置。“仅恢复文件”不验证续聊；“迁移并接入 Codex”检查计划内所有对话是否被识别，并仅验证所选对话临时分支能否继续。")}</p>
      </header>

      <section className="workflow-section" aria-labelledby="receive-package-title">
        <div className="section-title-row"><div><span className="step-number">1</span><h2 id="receive-package-title">{t("选择迁移包")}</h2></div></div>
        <div className="form-row"><div className="form-label"><FileArchive aria-hidden="true" /><span><strong>{t("ReHome 迁移包")}</strong><small>{preview?.package_path ?? t("尚未选择")}</small></span></div><button className="secondary-button" type="button" onClick={() => void choosePackage()} disabled={controlsBusy}>{phase === "inspecting" ? <LoaderCircle className="spin" aria-hidden="true" /> : <FolderOpen aria-hidden="true" />}{t("选择迁移包")}</button></div>
        {preview && (
          <div className="preview-band">
            <div className="preview-facts">
              <span><small>{t("来源系统")}</small><strong>{sourceOsLabel(preview.manifest.source_os)}</strong></span>
              <span><small>{t("项目")}</small><strong>{preview.manifest.counts.projects}</strong></span>
              <span><small>{t("对话")}</small><strong>{t("{count} 个对话", { count: preview.manifest.counts.conversations })}</strong></span>
              <span><small>{t("技能 / 插件 / 图片")}</small><strong>{preview.manifest.counts.skills} / {preview.manifest.counts.plugins} / {preview.manifest.counts.generated_images}</strong></span>
            </div>
            <div className="integrity-row">
              <span className={preview.checksum_valid ? "status status-success" : "status status-error"}>{preview.checksum_valid ? <CheckCircle2 aria-hidden="true" /> : <XCircle aria-hidden="true" />}{t(preview.checksum_valid ? "校验通过" : "校验失败")}</span>
              <code className="hash-text">{preview.archive_hash}</code>
              <span>{t("禁用文件 {count}", { count: preview.forbidden_files_total })}</span>
            </div>
          </div>
        )}
      </section>

      <section className="workflow-section" aria-labelledby="receive-target-title">
        <div className="section-title-row"><div><span className="step-number">2</span><h2 id="receive-target-title">{t("选择保存位置")}</h2></div></div>
        <PathPicker icon={HardDrive} label={t("Codex 数据位置")} value={locations?.target_codex_home ?? inventory?.codex_home ?? t("未检测")} />
        <PathPicker
          icon={FolderOpen}
          label={t("项目保存位置")}
          value={projectLocationRequired
            ? locations?.projects_root ?? t("尚未选择")
            : preview
              ? t("迁移包不含项目文件，无需选择")
              : t("尚未选择")}
          buttonLabel={projectLocationRequired ? t("选择项目保存位置") : undefined}
          onClick={projectLocationRequired ? chooseLocations : undefined}
          disabled={!preview || controlsBusy}
        />
        <div className="command-row"><p>{t("安全备份由 ENHE 自动管理")}</p><button className="command-button" type="button" disabled={!canPlan} onClick={() => void handlePlan()}>{phase === "planning" ? <LoaderCircle className="spin" aria-hidden="true" /> : <ShieldCheck aria-hidden="true" />}{t("预览导入内容")}</button></div>
      </section>

      {plan && (
        <section className="workflow-section" aria-labelledby="restore-plan-title">
          <div className="section-title-row"><div><span className="step-number">3</span><h2 id="restore-plan-title">{t("确认导入内容")}</h2></div><div className="plan-badges"><span>{t("需要 {size}", { size: formatBytes(plan.required_bytes) })}</span><span className={plan.conflict_count ? "status status-error" : "status status-success"}>{plan.conflict_count ? <AlertTriangle aria-hidden="true" /> : <CheckCircle2 aria-hidden="true" />}{t("冲突 {count}", { count: plan.conflict_count })}</span></div></div>
          {projectLocationRequired && <div className="destination-line"><span>{t("目标项目目录")}</span><code>{plan.projects_root}</code></div>}
          <div className="table-wrap">
            <table className="conflict-table">
              <thead><tr><th>{t("包内来源")}</th><th>{t("目标位置")}</th><th>{t("变更")}</th></tr></thead>
              <tbody>{planOperations.map((operation) => <tr key={`${operation.package_source}-${operation.target}`}><td><code>{operation.package_source}</code></td><td><code>{operation.target}</code></td><td><span className={`change change-${operation.action}`}>{changeLabel(operation.action, t, conflictResolution !== null)}</span></td></tr>)}</tbody>
            </table>
          </div>
          {plan.conflict_count > 0 && (
            <ConflictResolutionPanel
              count={plan.conflict_count}
              resolution={conflictResolution}
              busy={controlsBusy}
              onResolve={handlePlan}
            />
          )}
          {plan.conflict_count === 0 && conflictResolution && (
            <p className="inline-state status-success" role="status">
              <CheckCircle2 aria-hidden="true" />
              {t(conflictResolution === "keep_existing"
                ? "已选择保留新电脑上的不同文件。"
                : "已选择使用迁移包文件；被替换的文件会自动备份。")}
            </p>
          )}
          {plan.conflict_count === 0 && <>
            <fieldset className="restore-modes" disabled={controlsBusy || Boolean(report) || Boolean(jobId)}>
              <legend>{t("恢复方式")}</legend>
              <label className="restore-mode"><input type="radio" name="restore-mode" value="connect" checked={mode === "connect"} disabled={!mappedConversations.length} onChange={() => setMode("connect")} /><span>{t("迁移并接入 Codex（推荐）")}</span></label>
              <label className="restore-mode"><input type="radio" name="restore-mode" value="files" checked={mode === "files"} onChange={() => setMode("files")} /><span>{t("仅恢复文件")}</span></label>
            </fieldset>
            {!mappedConversations.length && <p className="help-text">{t("没有可映射到恢复计划的对话，无法验证接入；仍可仅恢复文件。")}</p>}
            {mode === "connect" && <>
              <label className="probe-choice"><span>{t("用于临时分支验证的对话")}</span><select value={probeThreadId} onChange={event => setProbeThreadId(event.target.value)} disabled={controlsBusy || Boolean(jobId)}>
                {mappedConversations.map(item => <option key={item.task_id} value={item.targetId}>{item.title || item.task_id}</option>)}
              </select></label>
              <p className="help-text">{t("检查全部计划内对话是否被 Codex 识别；仅在所选对话的临时分支发送固定验证消息，不向原对话添加验证消息。")}</p>
              <label className="confirmation-row"><input type="checkbox" aria-label={t("同意在线验证及用量")} checked={onlineUsage} disabled={controlsBusy || Boolean(jobId)} onChange={event => setOnlineUsage(event.target.checked)} /><span><strong>{t("同意在线验证及用量")}</strong><small>{t("临时分支会使用所选对话上下文联系已配置模型服务，并发送固定验证消息，可能产生用量；不是仅发送孤立的中性提示。本地回滚后，服务端操作和用量无法撤销。")}</small></span></label>
            </>}
          </>}
          <label className="confirmation-row"><input type="checkbox" checked={codexClosed} disabled={controlsBusy || Boolean(report) || Boolean(jobId)} onChange={(event) => setCodexClosed(event.target.checked)} aria-label={t("确认已保存工作并完全退出 Codex")} /><span><strong>{t("已保存工作并完全退出 Codex")}</strong><small>{t(mode === "connect" ? "迁移会写入上方 Codex 数据位置，并启动验证服务。请保持应用打开直到任务结束。" : mappedConversations.length ? "导入会写入上方 Codex 数据位置。完成后重新打开 Codex，并单独验证原对话是否可见、能否继续。" : "导入会写入上方 Codex 数据位置；本次仅恢复文件，不验证对话续聊。")}</small></span></label>
          <div className="command-row">{mode === "files" && <ProgressSteps active={phase === "restoring"} complete={Boolean(report)} />}<button className="command-button" type="button" disabled={!canRestore || (mode === "connect" && (!onlineUsage || !probeThreadId))} onClick={() => void (mode === "connect" ? handleMigration() : handleRestore())}>{phase === "restoring" || jobRunning ? <LoaderCircle className="spin" aria-hidden="true" /> : <Play aria-hidden="true" />}{t(mode === "connect" ? "开始迁移并接入" : phase === "restoring" ? "正在导入" : "导入到 Codex")}</button></div>
        </section>
      )}

      {error && <p className="inline-state status-error page-error" role="alert"><XCircle aria-hidden="true" />{error}</p>}
      {startError != null && <div className="inline-state status-error page-error" role="alert"><XCircle aria-hidden="true" /><div>{errorMessage(startError, t)}{onOpenHistory && <p><button className="secondary-button" type="button" onClick={onOpenHistory}>{t("前往迁移记录")}</button></p>}</div></div>}
      {jobId && <section className="result-panel" aria-labelledby="migration-result-title">
        <h2 id="migration-result-title">{t("Codex 接入验证")}</h2>
        <MigrationProgress snapshot={snapshot} />
        {pollError != null && <div className="manual-status" role="status"><AlertTriangle aria-hidden="true" /><div>{t("暂时无法读取任务状态，不代表迁移失败或已回滚；将继续查询，请勿重复启动。")}{"\n"}{errorMessage(pollError, t)}{onOpenHistory && <p><button className="secondary-button" type="button" onClick={onOpenHistory}>{t("前往迁移记录")}</button></p>}</div></div>}
        {snapshot?.status === "succeeded" && <div className="inline-state status-success" role="status"><CheckCircle2 aria-hidden="true" /><div><strong>{t("迁移并接入完成")}</strong><p>{t("验证消息未添加到原对话。")}</p><p>{t("已验证全部计划内对话可被识别，以及所选对话的临时分支可继续；其他对话未逐一测试续聊。")}</p></div></div>}
        {snapshot && snapshot.status !== "running" && snapshot.status !== "succeeded" && <div className="inline-state status-error" role="alert"><XCircle aria-hidden="true" /><div>
          <strong>{t(snapshot.status === "rolled_back" ? "本地更改已回滚" : snapshot.status === "failed_before_write" ? "未写入本地数据" : "本地回滚未完成，需要手动恢复。")}</strong>
          <p>{errorMessage(snapshot.error ?? { code: "restore_failed", message: "" }, t)}</p>
          {snapshot.transaction_id && <p>{t("事务编号")}：<code>{snapshot.transaction_id}</code></p>}
          {snapshot.status === "rolled_back" && <p>{t("此前的文件验证只表示当时通过检查，不表示回滚后文件仍保留。服务端处理和用量不能撤销。")}</p>}
          {snapshot.status === "rollback_failed" && <p>{t("请保留现有文件和自动备份，完全关闭 Codex 及辅助进程，再查看迁移记录中的恢复状态；不要覆盖较新的数据。")}</p>}
          {onOpenHistory && <button className="secondary-button" type="button" onClick={onOpenHistory}>{t("前往迁移记录")}</button>}
        </div></div>}
      </section>}

      {report && (
        <section className="result-panel" aria-labelledby="restore-result-title">
          <div className="section-title-row"><div><CheckCircle2 aria-hidden="true" /><h2 id="restore-result-title">{t("导入完成")}</h2></div><span className="status status-success">{t("{count} 个文件", { count: report.restored_files })}</span></div>
          <div className="verification-list">
            {verificationLabels.map(([key, label]) => {
              const passed = report.verification[key];
              return <span key={key} className={passed ? "verification-pass" : "verification-fail"}>{passed ? <CheckCircle2 aria-hidden="true" /> : <AlertTriangle aria-hidden="true" />}{t(key === "app_visible_ready" && !passed ? "对话可见性待确认" : label)}</span>;
            })}
          </div>
          {!mappedConversations.length
            ? <p className="manual-status" role="status"><CheckCircle2 aria-hidden="true" />{t("文件已恢复。本次计划没有可恢复的对话，未执行对话识别或续聊验证。")}</p>
            : !report.verification.app_visible_ready && <p className="manual-status" role="status"><AlertTriangle aria-hidden="true" />{t("文件和索引已导入。请重启 Codex，打开原对话并继续发送一条消息，确认可以使用。")}</p>}
          {manualRegistration && <p className="manual-status" role="status"><AlertTriangle aria-hidden="true" />{t("项目文件已导入，需要在 Codex 中手动打开")}</p>}
          {report.registrations.map((registration) => {
            const message = registrationStatuses[registration.project_id]
              ?? (typeof registration.status === "object" ? registration.status.invocation_failed.message : null);
            return <div className="registration-row" key={registration.project_id}><code>{registration.project_path}</code><button className="secondary-button" type="button" disabled={controlsBusy} onClick={() => void handleOpenRestored(registration)}><FolderOpen aria-hidden="true" />{t("在 Codex 中打开")}</button>{message && <span role="status">{message}</span>}</div>;
          })}
        </section>
      )}
    </div>
  );
}

function PathPicker({ icon: Icon, label, value, buttonLabel, onClick, disabled }: { icon: typeof FolderOpen; label: string; value: string; buttonLabel?: string; onClick?: () => Promise<void>; disabled?: boolean }) {
  return <div className="form-row"><div className="form-label"><Icon aria-hidden="true" /><span><strong>{label}</strong><small>{value}</small></span></div>{buttonLabel && onClick && <button className="secondary-button" type="button" disabled={disabled} onClick={() => void onClick()}><FolderOpen aria-hidden="true" />{buttonLabel}</button>}</div>;
}

function ConflictResolutionPanel({
  count,
  resolution,
  busy,
  onResolve,
}: {
  count: number;
  resolution: FileConflictResolution | null;
  busy: boolean;
  onResolve: (resolution: FileConflictResolution) => Promise<void>;
}) {
  const { t } = useI18n();
  const unresolvedAfterChoice = resolution !== null;
  return (
    <div className="conflict-resolution-panel" role="alert">
      <div className="conflict-resolution-copy">
        <AlertTriangle aria-hidden="true" />
        <span>
          <strong>{t(unresolvedAfterChoice
            ? "仍有 {count} 个无法自动处理的结构冲突。"
            : "发现 {count} 个同名但内容不同的文件。", { count })}</strong>
          <small>{t(unresolvedAfterChoice
            ? "请查看上表中的冲突路径，移开对应文件或目录，或重新选择一个空的项目保存位置后再预览。"
            : "请选择如何处理这些普通文件冲突。")}</small>
        </span>
      </div>
      <div className="conflict-resolution-actions" role="group" aria-label={t("冲突处理方式")}>
        <button
          className="secondary-button"
          type="button"
          aria-pressed={resolution === "keep_existing"}
          disabled={busy || resolution === "keep_existing"}
          onClick={() => void onResolve("keep_existing")}
        >
          <ShieldCheck aria-hidden="true" />{t("保留新电脑文件（推荐）")}
        </button>
        <button
          className="secondary-button"
          type="button"
          aria-pressed={resolution === "use_package"}
          disabled={busy || resolution === "use_package"}
          onClick={() => void onResolve("use_package")}
        >
          <FileArchive aria-hidden="true" />{t("使用迁移包文件")}
        </button>
      </div>
      <p>{t("保留会跳过同名文件；替换会先自动备份新电脑上的原文件。")}</p>
    </div>
  );
}

function ProgressSteps({ active, complete }: { active: boolean; complete: boolean }) {
  const { t } = useI18n();
  const labels = ["检查", "备份", "导入", "完成"];
  return <div className="progress-steps" aria-label={t("导入进度")}>{labels.map((label, index) => <span key={label} className={complete ? "complete" : active && index < 2 ? "active" : ""}>{complete ? <CheckCircle2 aria-hidden="true" /> : <Circle aria-hidden="true" />}{t(label)}</span>)}</div>;
}

function MigrationProgress({ snapshot }: { snapshot: MigrationJobSnapshot | null }) {
  const { t } = useI18n();
  const stage = snapshot?.stage ?? "preflight";
  const succeeded = snapshot?.status === "succeeded";
  const stopped = Boolean(snapshot && snapshot.status !== "running" && !succeeded);
  const states = [
    ["文件已恢复", ["files_verified", "recognizing_threads", "probing_continuation", "committing"].includes(stage), ["preflight", "restoring_files"].includes(stage)],
    ["Codex 已识别对话", ["probing_continuation", "committing"].includes(stage), stage === "recognizing_threads"],
    ["临时分支可继续发送消息", stage === "committing", stage === "probing_continuation"],
  ] as const;
  return <div role="status" aria-label={t("接入验证进度")}>
    <ol className="access-steps">{states.map(([label, passed, active]) => {
      const state = succeeded ? "passed" : stopped || stage === "rolling_back" ? "unconfirmed" : passed ? "passed" : active ? "running" : "waiting";
      const Icon = state === "passed" ? CheckCircle2 : state === "running" ? LoaderCircle : Circle;
      return <li key={label} className={state}><Icon aria-hidden="true" className={state === "running" ? "spin" : undefined} /><span>{t(label)}</span><small>{t({ passed: "已通过", running: "进行中", waiting: "等待中", unconfirmed: "未确认" }[state])}</small></li>;
    })}</ol>
    {stage === "preflight" && <p>{t("正在检查迁移条件")}</p>}
    {stage === "committing" && <p>{t("正在保存迁移结果")}</p>}
    {stage === "rolling_back" && <p>{t("正在回滚本地更改，最终结果待确认。")}</p>}
  </div>;
}

function sourceOsLabel(os: "windows" | "macos"): string {
  return os === "macos" ? "macOS" : "Windows";
}

function changeLabel(
  change: RestorePlan["operations"][number]["action"],
  t: (key: string) => string,
  structuralConflict: boolean,
): string {
  if (change === "conflict" && structuralConflict) return t("结构冲突");
  return t({ add: "新增", update: "更新", unchanged: "不变", preserve: "保留本机", conflict: "冲突" }[change]);
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

function registrationStatusMessage(status: RegistrationStatus, t: (key: string) => string): string {
  if (status === "registered") return t("已在 Codex 中登记");
  if (typeof status === "object") return status.invocation_failed.message;
  return t("项目文件已导入，需要在 Codex 中手动打开");
}
