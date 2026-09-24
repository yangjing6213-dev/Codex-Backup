import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { createRef } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { I18nProvider, useI18n } from "../../lib/i18n";
import type { MigrationJobSnapshot, PackagePreview, RestorePlan, RestoreReport } from "../../lib/types";
import ReceivePage from "./ReceivePage";

const api = vi.hoisted(() => ({
  inspectPackage: vi.fn(), selectRestoreDestinations: vi.fn(), buildRestorePlan: vi.fn(),
  applyRestore: vi.fn(), openRestoredThread: vi.fn(),
  startMigrateAndConnect: vi.fn(), getMigrationJob: vi.fn(),
}));
vi.mock("../../lib/api", () => api);

const conversations = [
  { task_id: "source-old", title: "Older conversation", updated_at: "2026-09-19T00:00:00Z" },
  { task_id: "source-new", title: "Newest conversation", updated_at: "2026-09-20T00:00:00Z" },
  { task_id: "unmapped", title: "Unmapped conversation", updated_at: "2026-09-21T00:00:00Z" },
].map(item => ({ ...item, project_id: null, archive_path: `sessions/${item.task_id}.jsonl`, content_hash: "synthetic", classification: null }));
const preview: PackagePreview = {
  selection_id: "package-selection", package_path: "C:/Synthetic/demo.rehome", archive_hash: "synthetic",
  checksum_valid: true, entries: [], forbidden_files_total: 0,
  manifest: {
    format: "rehome", schema_version: 1, package_id: "package-1", created_at: "2026-09-20T00:00:00Z",
    source_os: "windows", source_arch: "x86_64", source_device_id: "synthetic-device", mode: "full", parent_checkpoint: null,
    counts: { projects: 0, project_files: 0, conversations: 3, skills: 0, plugins: 0, generated_images: 0, sqlite_threads: 3 },
    projects: [], conversations, exclusions: { excluded_files: 0, excluded_bytes: 0, rules: [] },
  },
};
const plan: RestorePlan = {
  plan_id: "plan-1", package_id: "package-1", package_path: preview.package_path, archive_hash: "synthetic",
  target_codex_home: "C:/Synthetic/target", projects_root: "C:/Synthetic/projects", operations: [],
  sessions: conversations.slice(0, 2).map(item => ({
    package_source: item.archive_path, target: `C:/Synthetic/target/${item.task_id}.jsonl`,
    source_task_id: item.task_id, target_task_id: item.task_id.replace("source", "target"), title: item.title,
    source_content_hash: "synthetic", expected_final_content_hash: "synthetic-remapped", action: "import_as_branch",
  })),
  reference_rewrites: [], bridge_verification: { session_index: null, sqlite_database: null }, conflict_count: 0, required_bytes: 100,
};
const restored: RestoreReport = {
  transaction_id: "transaction-1", package_id: "package-1", completed_at: "2026-09-20T00:00:00Z", restored_files: 2,
  restored_bytes: 100, registrations: [], verification: {
    package_checksum_valid: true, files_valid: true, sessions_valid: true, session_index_valid: true,
    sqlite_threads_valid: true, path_mapping_valid: true, forbidden_files_absent: true, project_files_valid: true,
    app_registration_valid: true, app_visible_ready: false,
    codex_access: { required_threads: 2, recognized_threads: 2, probe_thread_id: "target-new", threads_recognized: true, continuation_probe_valid: true, ephemeral_fork: true },
  },
};
function job(overrides: Partial<MigrationJobSnapshot> = {}): MigrationJobSnapshot {
  return { job_id: "job-1", plan_id: "plan-1", transaction_id: null, stage: "preflight", status: "running", report: null, error: null, updated_at: "2026-09-20T00:00:00Z", ...overrides };
}
function LanguageSwitch() {
  const { setLocale } = useI18n();
  return <button onClick={() => setLocale("en")}>English</button>;
}
const headingRef = createRef<HTMLHeadingElement>();
function renderPage(props: Partial<React.ComponentProps<typeof ReceivePage>> = {}) {
  const callbacks = { onOperationStart: vi.fn(), onOperationEnd: vi.fn() };
  const view = render(<I18nProvider><LanguageSwitch /><ReceivePage headingRef={headingRef} inventory={null} {...callbacks} {...props} /></I18nProvider>);
  return { ...view, ...callbacks };
}
async function click(name: string) {
  await act(async () => { fireEvent.click(screen.getByRole("button", { name })); });
}
async function prepare() {
  await click("选择迁移包");
  await click("预览导入内容");
}
function consent() {
  fireEvent.click(screen.getByRole("checkbox", { name: "确认已保存工作并完全退出 Codex" }));
  fireEvent.click(screen.getByRole("checkbox", { name: "同意在线验证及用量" }));
}
async function poll() { await act(async () => { await vi.advanceTimersByTimeAsync(400); }); }

beforeEach(() => {
  vi.resetAllMocks();
  window.localStorage.clear();
  api.inspectPackage.mockResolvedValue(preview);
  api.selectRestoreDestinations.mockResolvedValue({ selection_id: "destinations", target_codex_home: plan.target_codex_home, projects_root: plan.projects_root, backup_root: "C:/Synthetic/backups" });
  api.buildRestorePlan.mockResolvedValue(plan);
  api.startMigrateAndConnect.mockResolvedValue(job());
  api.getMigrationJob.mockResolvedValue(job());
  api.applyRestore.mockResolvedValue(restored);
});
afterEach(() => vi.useRealTimers());

describe("migrate and connect wizard", () => {
  it("explains both restore modes and the selected-branch verification scope in both languages", async () => {
    renderPage();
    expect(screen.getByText(/“仅恢复文件”不验证续聊/, { selector: ".page-description" })).toHaveTextContent("检查计划内所有对话是否被识别，并仅验证所选对话临时分支能否继续");
    await click("English");
    expect(screen.getByText(/“Restore files only” does not verify continuation/, { selector: ".page-description" })).toHaveTextContent("checks recognition of all planned conversations and tests continuation only on a temporary branch of the selected conversation");
  });

  it("defaults to the newest mapped target, permits choosing an older target and requires both consents", async () => {
    renderPage();
    await prepare();
    expect(screen.getByRole("radio", { name: /迁移并接入 Codex/ })).toBeChecked();
    expect(screen.getByRole("radio", { name: "仅恢复文件" })).not.toBeChecked();
    const choice = screen.getByRole("combobox", { name: "用于临时分支验证的对话" });
    expect(choice).toHaveValue("target-new");
    expect(within(choice).getAllByRole("option").map(item => item.textContent)).toEqual(["Newest conversation", "Older conversation"]);
    const start = screen.getByRole("button", { name: "开始迁移并接入" });
    expect(start).toBeDisabled();
    fireEvent.click(screen.getByRole("checkbox", { name: "确认已保存工作并完全退出 Codex" }));
    expect(start).toBeDisabled();
    fireEvent.click(screen.getByRole("checkbox", { name: "同意在线验证及用量" }));
    fireEvent.change(choice, { target: { value: "target-old" } });
    expect(start).toBeEnabled();
    expect(screen.getByText(/服务端操作和用量无法撤销/)).toBeVisible();
    expect(screen.getByText(/使用所选对话上下文联系已配置模型服务/)).toBeVisible();
    expect(screen.getByText(/全部计划内对话.*仅.*临时分支/)).toBeVisible();
    await click("开始迁移并接入");
    expect(api.startMigrateAndConnect).toHaveBeenCalledExactlyOnceWith({ plan_id: "plan-1", codex_closed_confirmed: true, register_projects: true, probe_thread_id: "target-old", online_usage_confirmed: true });
    expect(api.applyRestore).not.toHaveBeenCalled();
  });

  it("disables connect for zero mapped conversations but leaves file-only restore usable", async () => {
    api.buildRestorePlan.mockResolvedValue({ ...plan, sessions: [] });
    api.applyRestore.mockResolvedValue({ ...restored, verification: {
      ...restored.verification,
      codex_access: { required_threads: 0, recognized_threads: 0, probe_thread_id: null, threads_recognized: false, continuation_probe_valid: false, ephemeral_fork: false },
    } });
    renderPage();
    await prepare();
    expect(screen.getByRole("radio", { name: /迁移并接入 Codex/ })).toBeDisabled();
    expect(screen.getByRole("radio", { name: "仅恢复文件" })).toBeChecked();
    expect(screen.getByText(/没有可映射到恢复计划的对话/)).toBeVisible();
    fireEvent.click(screen.getByRole("checkbox", { name: "确认已保存工作并完全退出 Codex" }));
    await click("导入到 Codex");
    expect(api.applyRestore).toHaveBeenCalledExactlyOnceWith("plan-1", { codex_closed_confirmed: true, register_projects: true });
    expect(api.startMigrateAndConnect).not.toHaveBeenCalled();
    expect(screen.getByText("文件已恢复。本次计划没有可恢复的对话，未执行对话识别或续聊验证。")).toBeVisible();
    expect(screen.queryByText(/文件和索引已导入/)).not.toBeInTheDocument();
    expect(screen.queryByText(/打开原对话|单独验证原对话/)).not.toBeInTheDocument();
    await click("English");
    expect(screen.getByText("Files were restored. This plan contains no conversations to restore; conversation recognition and continuation were not tested.")).toBeVisible();
    expect(screen.queryByText(/Files and indexes have been imported|open the original conversation|separately verify whether original conversations/i)).not.toBeInTheDocument();
  });

  it("retains the bilingual manual continuation reminder for file-only restore with mapped conversations", async () => {
    api.applyRestore.mockResolvedValue({ ...restored, verification: {
      ...restored.verification,
      codex_access: { required_threads: 0, recognized_threads: 0, probe_thread_id: null, threads_recognized: false, continuation_probe_valid: false, ephemeral_fork: false },
    } });
    renderPage(); await prepare();
    fireEvent.click(screen.getByRole("radio", { name: "仅恢复文件" }));
    fireEvent.click(screen.getByRole("checkbox", { name: "确认已保存工作并完全退出 Codex" }));
    await click("导入到 Codex");
    expect(screen.getByText("文件和索引已导入。请重启 Codex，打开原对话并继续发送一条消息，确认可以使用。")).toBeVisible();
    expect(screen.queryByText(/本次计划没有可恢复的对话/)).not.toBeInTheDocument();
    await click("English");
    expect(screen.getByText("Files and indexes have been imported. Restart Codex, open the original conversation, and send another message to confirm it works.")).toBeVisible();
    expect(api.startMigrateAndConnect).not.toHaveBeenCalled();
  });

  it("explains selected-context model contact and irreversible server usage in English", async () => {
    renderPage(); await prepare(); await click("English");
    expect(screen.getByRole("checkbox", { name: "Consent to online verification and usage" })).not.toBeChecked();
    expect(screen.getByText(/use the selected conversation's context to contact the configured model service/)).toBeVisible();
    expect(screen.getByText(/Local rollback cannot undo server processing or usage/)).toBeVisible();
    expect(screen.getByRole("button", { name: "Start migration and connection" })).toBeDisabled();
  });

  it("disables every import action while another global operation is busy", async () => {
    const callbacks = { onOperationStart: vi.fn(), onOperationEnd: vi.fn() };
    const { rerender } = renderPage(callbacks);
    await prepare();
    consent();
    rerender(<I18nProvider><LanguageSwitch /><ReceivePage headingRef={headingRef} inventory={null} {...callbacks} operationBusy /></I18nProvider>);
    for (const name of ["选择迁移包", "预览导入内容", "开始迁移并接入"]) expect(screen.getByRole("button", { name })).toBeDisabled();
  });

  it("advances three evidence stages without optimistic recognition and stops polling at success", async () => {
    const callbacks = renderPage();
    await prepare();
    consent();
    callbacks.onOperationStart.mockClear(); callbacks.onOperationEnd.mockClear();
    vi.useFakeTimers();
    await click("开始迁移并接入");
    const progress = () => screen.getByRole("status", { name: "接入验证进度" });
    api.getMigrationJob.mockResolvedValue(job({ stage: "recognizing_threads", transaction_id: "transaction-1" }));
    await poll();
    expect(within(progress()).getByText("文件已恢复").closest("li")).toHaveTextContent("已通过");
    expect(within(progress()).getByText("Codex 已识别对话").closest("li")).toHaveTextContent("进行中");
    expect(within(progress()).getByText("临时分支可继续发送消息").closest("li")).toHaveTextContent("等待中");
    api.getMigrationJob.mockResolvedValue(job({ stage: "probing_continuation" }));
    await poll();
    expect(within(progress()).getByText("Codex 已识别对话").closest("li")).toHaveTextContent("已通过");
    expect(within(progress()).getByText("临时分支可继续发送消息").closest("li")).toHaveTextContent("进行中");
    api.getMigrationJob.mockResolvedValue(job({ stage: "finished", status: "succeeded", transaction_id: "transaction-1", report: restored }));
    await poll();
    expect(within(progress()).getAllByText("已通过")).toHaveLength(3);
    expect(screen.getByText("验证消息未添加到原对话。")).toBeVisible();
    expect(callbacks.onOperationStart).toHaveBeenCalledTimes(1);
    expect(callbacks.onOperationEnd).toHaveBeenCalledTimes(1);
    const polls = api.getMigrationJob.mock.calls.length;
    await poll();
    expect(api.getMigrationJob).toHaveBeenCalledTimes(polls);
    expect(screen.getByRole("button", { name: "开始迁移并接入" })).toBeDisabled();
  });

  it("resumes a known job, tolerates poll errors, and ignores callback identity changes", async () => {
    vi.useFakeTimers();
    api.getMigrationJob.mockRejectedValueOnce({ code: "migration_job_not_found", message: "expired" });
    const first = { onOperationStart: vi.fn(), onOperationEnd: vi.fn() };
    const view = renderPage({ ...first, initialJobId: "job-1" });
    await act(async () => {});
    expect(api.getMigrationJob).toHaveBeenCalledWith("job-1");
    expect(screen.getByText(/无法读取任务状态.*不代表迁移失败/)).toBeVisible();
    expect(screen.queryByText("本地更改已回滚")).not.toBeInTheDocument();
    expect(first.onOperationEnd).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "选择迁移包" })).toBeDisabled();
    const next = { onOperationStart: vi.fn(), onOperationEnd: vi.fn() };
    view.rerender(<I18nProvider><LanguageSwitch /><ReceivePage headingRef={headingRef} inventory={null} initialJobId="job-1" {...next} /></I18nProvider>);
    api.getMigrationJob.mockResolvedValue(job({ status: "failed_before_write", stage: "finished", error: { code: "codex_app_server_unavailable", message: "fixed category" } }));
    await poll();
    expect(first.onOperationStart).toHaveBeenCalledTimes(1);
    expect(next.onOperationStart).not.toHaveBeenCalled();
    expect(next.onOperationEnd).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("alert")).toHaveTextContent("未写入本地数据");
    expect(within(screen.getByRole("status", { name: "接入验证进度" })).getAllByText("未确认")).toHaveLength(3);
    expect(api.startMigrateAndConnect).not.toHaveBeenCalled();
    view.unmount();
    const polls = api.getMigrationJob.mock.calls.length;
    await poll();
    expect(api.getMigrationJob).toHaveBeenCalledTimes(polls);
  });

  it("clears polling on actual unmount and resumes only the known job after remount", async () => {
    vi.useFakeTimers();
    api.getMigrationJob.mockResolvedValue(job({ stage: "probing_continuation" }));
    const first = renderPage({ initialJobId: "job-1" });
    await act(async () => {});
    expect(api.getMigrationJob).toHaveBeenCalledTimes(1);
    first.unmount();
    const calls = api.getMigrationJob.mock.calls.length;
    await poll();
    expect(api.getMigrationJob).toHaveBeenCalledTimes(calls);
    api.getMigrationJob.mockResolvedValue(job({ stage: "finished", status: "rolled_back", transaction_id: "transaction-1", error: { code: "codex_verification_failed", message: "fixed category" } }));
    renderPage({ initialJobId: "job-1" });
    await act(async () => {});
    expect(screen.getByRole("alert")).toHaveTextContent("本地更改已回滚");
    expect(within(screen.getByRole("status", { name: "接入验证进度" })).getAllByText("未确认")).toHaveLength(3);
    expect(screen.queryByText("已通过")).not.toBeInTheDocument();
    expect(api.startMigrateAndConnect).not.toHaveBeenCalled();
  });

  it.each([
    ["rolled_back", "codex_verification_failed", "本地更改已回滚"],
    ["rollback_failed", "rollback_failed", "本地回滚未完成，需要手动恢复。"],
    ["rollback_failed", "codex_cleanup_unconfirmed", "Codex 辅助进程退出状态未确认。"],
    ["failed_before_write", "codex_authentication_required", "Codex 需要登录。"],
    ["failed_before_write", "codex_app_server_unavailable", "Codex 验证服务不可用。"],
    ["failed_before_write", "restore_failed", "恢复未完成，请检查操作状态。"],
  ] as const)("renders %s/%s truthfully with a reachable recovery action and live English translation", async (status, code, expected) => {
    const onOpenHistory = vi.fn();
    api.getMigrationJob.mockResolvedValue(job({ stage: "finished", status, transaction_id: status === "failed_before_write" ? null : "transaction-1", error: { code, message: "fixed category" } }));
    renderPage({ initialJobId: "job-1", onOpenHistory });
    await act(async () => {});
    expect(screen.getByRole("alert")).toHaveTextContent(expected);
    if (status !== "failed_before_write") expect(screen.getByRole("alert")).toHaveTextContent("transaction-1");
    // All backend terminal snapshots have stage=finished, including failures.
    expect(within(screen.getByRole("status", { name: "接入验证进度" })).getAllByText("未确认")).toHaveLength(3);
    expect(screen.queryByText("已通过")).not.toBeInTheDocument();
    if (status !== "rolled_back") expect(screen.queryByText("本地更改已回滚")).not.toBeInTheDocument();
    if (code === "codex_cleanup_unconfirmed") expect(screen.getByRole("alert")).toHaveTextContent("未尝试检查点或回滚写入");
    await click("前往迁移记录");
    expect(onOpenHistory).toHaveBeenCalledOnce();
    await click("English");
    expect(screen.getByRole("alert").textContent).not.toMatch(/[\u3400-\u9fff]/);
    expect(screen.getByRole("alert")).toHaveTextContent("Solution:");
    expect(api.applyRestore).not.toHaveBeenCalled();
  });

  it("never falls back to file restore when fork verification is unsupported", async () => {
    api.startMigrateAndConnect.mockRejectedValue({ code: "codex_verification_failed", message: "unsupported fork" });
    renderPage(); await prepare(); consent(); await click("开始迁移并接入");
    expect(screen.getByRole("alert")).toHaveTextContent("Codex 接入验证未完成");
    expect(screen.getByRole("alert")).toHaveTextContent("临时分支");
    expect(api.applyRestore).not.toHaveBeenCalled();
    expect(screen.queryByText("本地更改已回滚")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "开始迁移并接入" })).not.toBeInTheDocument();
  });
});
