import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import App from "./App";

const api = vi.hoisted(() => ({
  discoverCodex: vi.fn(),
  discoverLocalCandidates: vi.fn(),
  countProjectFiles: vi.fn(),
  requestAdminLocalDiscovery: vi.fn(),
  getAppConfig: vi.fn(),
  getSchedulerStatus: vi.fn(),
  listLocalBackups: vi.fn(),
  restoreLocalBackup: vi.fn(),
  createPackage: vi.fn(),
  openPath: vi.fn(),
  saveAppConfig: vi.fn(),
  pickDirectory: vi.fn(),
  runLocalBackup: vi.fn(),
  startCloudConfiguration: vi.fn(),
  continueCloudConfiguration: vi.fn(),
  testCloudConnection: vi.fn(),
  uploadLocalSnapshot: vi.fn(),
  setScheduler: vi.fn(),
}));

vi.mock("./lib/api", () => api);

const inventory = {
  codex_home: "C:\\Users\\Me\\.codex",
  source_os: "windows",
  source_arch: "x86_64",
  source_device_id: "11111111-1111-1111-1111-111111111111",
  counts: {
    projects: 1,
    project_files: 3,
    conversations: 2,
    skills: 0,
    plugins: 0,
    generated_images: 0,
    sqlite_threads: 2,
  },
  projects: [{
    project_id: "22222222-2222-2222-2222-222222222222",
    name: "demo",
    source_path: "C:\\Work\\demo",
    source_available: true,
    archive_path: "projects/demo",
    file_count: 3,
    content_bytes: 120,
    git_remote: null,
    git_branch: "main",
    git_head: null,
  }],
  project_paths: ["C:\\Work\\demo"],
  conversations: [],
  conversation_paths: [],
  session_index_path: null,
  state_db_path: null,
  skill_paths: [],
  plugin_paths: [],
  generated_image_paths: [],
  skills: [],
  plugins: [],
  generated_images: [],
  warnings: [],
};

const config = {
  config_version: 1,
  codex_home: "C:\\Users\\Me\\.codex",
  local_repository: "D:\\ENHE\\backups",
  selected_project_paths: ["C:\\Work\\demo"],
  frequency_minutes: 15,
  retention: { high_frequency_hours: 48, daily_days: 30, weekly_weeks: 12 },
  cloud: { enabled: false, remote_name: null, remote_path: null, repository_path: null, config_file: null },
  locale: "zh-CN",
  appearance: "system",
  automatic_backup_enabled: false,
  automatic_project_scan: true,
  project_scan_roots: [],
  project_selection_initialized: true,
};

beforeEach(() => {
  vi.resetAllMocks();
  window.localStorage.clear();
  api.discoverCodex.mockImplementation(async (codexHome) => ({ ...inventory, codex_home: codexHome ?? inventory.codex_home }));
  api.discoverLocalCandidates.mockResolvedValue({
    candidates: [],
    codex_homes: [],
    conversation_count: 0,
    scanned_roots: ["C:\\"],
    skipped_roots: [],
    warnings: [],
    permission_denied_count: 0,
    other_warning_count: 0,
    cancelled: false,
  });
  api.getAppConfig.mockResolvedValue(config);
  api.countProjectFiles.mockImplementation(async (paths: string[]) => paths.map(path => ({ path, name: path.split(/[\\/]/).pop(), markers: [], file_count: 3, file_count_complete: true, skipped_entries: 0 })));
  api.getSchedulerStatus.mockResolvedValue({ enabled: false, task_name: "ENHE Codex Backup - Current User" });
  api.listLocalBackups.mockResolvedValue([]);
  api.saveAppConfig.mockImplementation(async (next) => next);
  api.pickDirectory.mockResolvedValue(null);
  api.requestAdminLocalDiscovery.mockResolvedValue({
    candidates: [], codex_homes: [], conversation_count: 0, scanned_roots: [], skipped_roots: [],
    warnings: [], permission_denied_count: 0, other_warning_count: 0, cancelled: false,
  });
  api.startCloudConfiguration.mockReset();
  api.continueCloudConfiguration.mockReset();
});

describe("ENHE Codex Backup shell", () => {
  it("shows exact scan folder names and counts separately from historical paths", async () => {
    const user = userEvent.setup();
    api.discoverLocalCandidates.mockResolvedValue({ candidates: [{ path: "F:\\Projects\\Product-video（推广视频生成）", name: "Product-video（推广视频生成）", markers: [], file_count: 1234, file_count_complete: true, skipped_entries: 0 }], codex_homes: [], conversation_count: 0, scanned_roots: ["F:\\Projects"], skipped_roots: [], warnings: [], permission_denied_count: 0, other_warning_count: 0, cancelled: false });
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "前往项目" }));
    expect(await screen.findByText("1,234 文件")).toBeVisible();
    expect(screen.getByRole("heading", { name: "本次扫描的项目" })).toBeVisible();
    expect(screen.getByRole("heading", { name: "手动与历史项目" })).toBeVisible();
    expect(screen.getByText("Product-video（推广视频生成）")).toBeVisible();
    expect(screen.queryByText("文件数将在备份时统计")).not.toBeInTheDocument();
    expect(screen.getByText(/包含隐藏文件、Git、依赖、构建产物及敏感文件/)).toBeVisible();
  });

  it("keeps a pending count across navigation and labels partial counts honestly", async () => {
    const user = userEvent.setup();
    let finish!: (value: unknown) => void;
    api.countProjectFiles.mockImplementation(() => new Promise(resolve => { finish = resolve; }));
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "前往项目" }));
    expect(await screen.findByText("正在统计文件…")).toBeVisible();
    await user.click(screen.getByRole("button", { name: "前往数据" }));
    await user.click(screen.getByRole("button", { name: "前往项目" }));
    expect(api.countProjectFiles).toHaveBeenCalledTimes(1);
    await act(async () => finish([{path: "C:\\Work\\demo", name: "demo", markers: [], file_count: 17, file_count_complete: false, skipped_entries: 2}]));
    expect(await screen.findByText("已统计 17 文件 · 部分统计，跳过 2 项")).toBeVisible();
  });

  it("distinguishes a count error from a verified empty folder", async () => {
    const user = userEvent.setup();
    api.countProjectFiles.mockRejectedValue({ message: "synthetic read failure" });
    api.discoverLocalCandidates.mockResolvedValue({ candidates: [{ path: "F:\\Projects\\EmptyFolder", name: "EmptyFolder", markers: [], file_count: 0, file_count_complete: true, skipped_entries: 0 }], codex_homes: [], conversation_count: 0, scanned_roots: ["F:\\Projects"], skipped_roots: [], warnings: [], permission_denied_count: 0, other_warning_count: 0, cancelled: false });
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "前往项目" }));
    expect(await screen.findByText("统计失败，请重新扫描")).toBeVisible();
    expect(screen.getByText("0 文件")).toBeVisible();
    await user.click(screen.getByRole("button", { name: "切换为英文" }));
    expect(screen.getByText("Count failed; scan again")).toBeVisible();
    expect(screen.getByText("0 files")).toBeVisible();
    expect(screen.getByText(/Full local project backup includes hidden files/)).toBeVisible();
  });

  it("serializes file counts when a second project is added during a pending count", async () => {
    const user = userEvent.setup();
    let finish!: (value: unknown) => void;
    api.countProjectFiles.mockImplementationOnce(() => new Promise(resolve => { finish = resolve; }));
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "前往项目" }));
    await screen.findByText("正在统计文件…");
    await user.type(screen.getByRole("textbox", { name: "手动添加项目目录" }), "C:\\Work\\second");
    await user.click(screen.getByRole("button", { name: "添加目录" }));
    expect(api.countProjectFiles).toHaveBeenCalledTimes(1);
    await act(async () => finish([{ path: "C:\\Work\\demo", name: "demo", markers: [], file_count: 4, file_count_complete: true, skipped_entries: 0 }]));
    await waitFor(() => expect(api.countProjectFiles).toHaveBeenCalledTimes(2));
    expect(api.countProjectFiles).toHaveBeenLastCalledWith(["C:\\Work\\second"]);
    expect(await screen.findByText("4 文件")).toBeVisible();
    expect(await screen.findByText("3 文件")).toBeVisible();
  });

  it("does not restart an outstanding file count until it finishes after rescan", async () => {
    const user = userEvent.setup();
    let finish!: (value: unknown) => void;
    api.countProjectFiles.mockImplementationOnce(() => new Promise(resolve => { finish = resolve; }));
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "前往项目" }));
    await screen.findByText("正在统计文件…");
    await user.click(screen.getByRole("button", { name: "重新扫描" }));
    await screen.findByText("本机项目扫描已完成");
    expect(api.countProjectFiles).toHaveBeenCalledTimes(1);
    await act(async () => finish([{ path: "C:\\Work\\demo", name: "demo", markers: [], file_count: 99, file_count_complete: true, skipped_entries: 0 }]));
    await waitFor(() => expect(api.countProjectFiles).toHaveBeenCalledTimes(2));
    expect(await screen.findByText("3 文件")).toBeVisible();
    expect(screen.queryByText("99 文件")).not.toBeInTheDocument();
  });

  it("does not let slow startup discovery replace a newer manual scan or its settings", async () => {
    const user = userEvent.setup();
    let finish!: (value: unknown) => void;
    api.discoverCodex.mockImplementationOnce(() => new Promise(resolve => { finish = resolve; }));
    api.getAppConfig.mockResolvedValue({ ...config, project_scan_roots: ["C:\\OLD"] });
    api.discoverLocalCandidates.mockResolvedValue({ candidates: [{ path: "C:\\NEW\\current", name: "current", markers: [], file_count: 7, file_count_complete: true, skipped_entries: 0 }], codex_homes: [], conversation_count: 0, scanned_roots: ["C:\\NEW"], skipped_roots: [], warnings: [], permission_denied_count: 0, other_warning_count: 0, cancelled: false });
    render(<App />);
    await waitFor(() => expect(api.discoverCodex).toHaveBeenCalledTimes(1));
    await user.click(screen.getByRole("button", { name: "前往项目" }));
    const roots = screen.getByRole("textbox", { name: "项目扫描目录（每行一个；留空扫描所有本地磁盘）" });
    await user.clear(roots);
    await user.type(roots, "C:\\NEW");
    await user.click(screen.getByRole("button", { name: "重新扫描" }));
    await screen.findByText("7 文件");
    await act(async () => finish(inventory));
    expect(api.discoverLocalCandidates).toHaveBeenCalledTimes(1);
    expect(api.discoverLocalCandidates).toHaveBeenLastCalledWith(["C:\\NEW"]);
    await user.click(screen.getByRole("button", { name: "前往概览" }));
    await user.click(screen.getByRole("button", { name: "前往项目" }));
    expect(screen.getByRole("textbox", { name: "项目扫描目录（每行一个；留空扫描所有本地磁盘）" })).toHaveValue("C:\\NEW");
  });

  it("preserves loaded settings when a manual scan is requested before config loading finishes", async () => {
    const user = userEvent.setup();
    let finish!: (value: unknown) => void;
    api.getAppConfig.mockImplementationOnce(() => new Promise(resolve => { finish = resolve; }));
    render(<App />);
    await user.click(screen.getByRole("button", { name: "前往项目" }));
    await user.type(screen.getByRole("textbox", { name: "项目扫描目录（每行一个；留空扫描所有本地磁盘）" }), "C:\\NEW");
    await user.click(screen.getByRole("button", { name: "重新扫描" }));
    expect(api.saveAppConfig).not.toHaveBeenCalled();
    await act(async () => finish({
      ...config,
      codex_home: "C:\\Users\\Preserved\\.codex",
      local_repository: "E:\\Preserved\\backups",
      frequency_minutes: 90,
      automatic_backup_enabled: true,
      automatic_project_scan: false,
      project_scan_roots: ["C:\\OLD"],
    }));
    await screen.findByText("本机项目扫描已完成");
    expect(api.discoverCodex).not.toHaveBeenCalled();
    expect(api.discoverLocalCandidates).toHaveBeenCalledTimes(1);
    expect(api.saveAppConfig).toHaveBeenCalledWith(expect.objectContaining({
      codex_home: "C:\\Users\\Preserved\\.codex",
      local_repository: "E:\\Preserved\\backups",
      frequency_minutes: 90,
      automatic_backup_enabled: true,
      automatic_project_scan: false,
      project_scan_roots: ["C:\\NEW"],
      selected_project_paths: ["C:\\Work\\demo"],
    }));
    expect(await screen.findByText("本机已就绪")).toBeVisible();
    expect(screen.getByRole("textbox", { name: "项目扫描目录（每行一个；留空扫描所有本地磁盘）" })).toHaveValue("C:\\NEW");
    await user.click(screen.getByRole("button", { name: "前往备份与迁移" }));
    expect(screen.getByRole("button", { name: "开始本地备份" })).toBeVisible();
  });

  it("does not announce complete scanning when a project count is partial", async () => {
    api.discoverLocalCandidates.mockResolvedValue({ candidates: [{ path: "F:\\Projects\\Partial", name: "Partial", markers: [], file_count: 9, file_count_complete: false, skipped_entries: 1 }], codex_homes: [], conversation_count: 0, scanned_roots: ["F:\\Projects"], skipped_roots: [], warnings: [], permission_denied_count: 0, other_warning_count: 0, cancelled: false });
    render(<App />);
    expect(await screen.findByText("本机项目扫描已部分完成")).toBeVisible();
    expect(screen.queryByText("本机项目扫描已完成")).not.toBeInTheDocument();
    expect(screen.getByText("至少 9")).toBeVisible();
  });
  it("does not let a slow data discovery overwrite a newer project selection", async () => {
    const user = userEvent.setup();
    let finish!: (value: unknown) => void;
    render(<App />);
    await screen.findByText("本机已就绪");
    await user.click(screen.getByRole("button", { name:"前往数据" }));
    api.discoverCodex.mockImplementationOnce(() => new Promise(resolve => { finish = resolve; }));
    await user.click(screen.getByRole("button", { name:"保存数据设置" }));
    await user.click(screen.getByRole("button", { name:"前往项目" }));
    await user.click(screen.getByRole("checkbox", { name:"选择项目 demo" }));
    await user.click(screen.getByRole("button", { name:"保存项目选择" }));
    await act(async () => finish(inventory));
    expect(api.saveAppConfig).toHaveBeenLastCalledWith(expect.objectContaining({selected_project_paths:[]}));
    expect(screen.getByRole("checkbox", { name:"选择项目 demo" })).not.toBeChecked();
  });

  it("retains a migration report completed while viewing another page", async () => {
    const user = userEvent.setup();
    let finish!: (value: unknown) => void;
    api.createPackage.mockImplementationOnce(() => new Promise(resolve => { finish = resolve; }));
    api.openPath.mockResolvedValue(undefined);
    render(<App />);
    await user.click(await screen.findByRole("button", { name:"前往备份与迁移" }));
    await user.click(screen.getByRole("tab", { name:"导出 ReHome 迁移包" }));
    await user.click(screen.getByRole("checkbox", { name:"选择项目 demo" }));
    await user.click(screen.getByRole("button", { name:"创建迁移包" }));
    await user.click(screen.getByRole("button", { name:"前往项目" }));
    await act(async () => finish({package_path:"F:\\Synthetic\\preserved.rehome",reveal_id:"synthetic",counts:inventory.counts,bytes_written:200,archive_hash:"synthetic-checksum",warnings:[]}));
    await user.click(screen.getByRole("button", { name:"前往备份与迁移" }));
    expect(screen.getByRole("tab", { name:"本地备份" })).toHaveAttribute("aria-selected", "true");
    await user.click(screen.getByRole("tab", { name:"导出 ReHome 迁移包" }));
    expect(screen.getByText("F:\\Synthetic\\preserved.rehome")).toBeVisible();
  });
  it("refreshes data after saving a different Codex source", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("本机已就绪");
    await user.click(screen.getByRole("button", { name: "前往数据" }));
    api.pickDirectory.mockResolvedValue("F:\\Synthetic\\.codex");
    api.discoverCodex.mockResolvedValue({ ...inventory, codex_home: "F:\\Synthetic\\.codex", counts: {...inventory.counts, conversations: 27} });
    await user.click(screen.getByRole("button", { name: "选择 Codex 数据位置" }));
    await user.click(screen.getByRole("button", { name: "保存数据设置" }));
    expect(await screen.findByText("27")).toBeVisible();
    expect(api.discoverCodex).toHaveBeenLastCalledWith("F:\\Synthetic\\.codex");
  });

  it("shows English scan, migration and local/device restore instructions", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("本机已就绪");
    await user.click(screen.getByRole("button", { name: "切换为英文" }));
    await user.click(screen.getByRole("button", { name: "Go to Projects" }));
    expect(screen.getByRole("checkbox", { name: "Scan projects automatically at startup" })).toBeChecked();
    expect(await screen.findByText("3 files")).toBeVisible();
    await user.click(screen.getByRole("button", { name: "Go to Backups & Migration" }));
    await user.click(screen.getByRole("tab", { name: "Export a ReHome migration package" }));
    expect(screen.getByRole("heading", { name: "Export a ReHome migration package" })).toBeVisible();
    await user.click(screen.getByRole("button", { name: "Back to local backup" }));
    await user.click(screen.getByRole("button", { name: "Go to How it works" }));
    expect(screen.getByRole("heading", { name: "Restore on this device" })).toBeVisible();
    expect(screen.getByRole("heading", { name: "Move to another device" })).toBeVisible();
    expect(screen.getByRole("heading", { name: "Restored files do not guarantee session continuation" })).toBeVisible();
  });

  it("does not claim a partial restore is complete", async () => {
    const user = userEvent.setup();
    api.listLocalBackups.mockResolvedValue([{restic_snapshot_id:"partial-fixture",created_at:"2026-09-19T00:00:00Z",file_count:2,complete:false}]);
    api.restoreLocalBackup.mockResolvedValue({restored_root:"F:\\Synthetic\\restored",restored_files:2,complete:false,missing:[{path:"synthetic-locked.txt",reason:"synthetic locked-file detail",bytes:3}]});
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "前往备份与迁移" }));
    await user.click(screen.getByRole("button", { name: "刷新本地备份" }));
    const target = screen.getByLabelText("恢复目标目录");
    await user.type(target, "F:\\Synthetic\\empty");
    await user.click(screen.getByRole("button", { name: "恢复" }));
    await waitFor(() => expect(api.restoreLocalBackup).toHaveBeenCalledTimes(1));
    expect(screen.getAllByText(/恢复已结束，但备份中存在缺失内容/).length).toBeGreaterThan(0);
    expect(screen.queryByText(/^恢复完成/)).not.toBeInTheDocument();
    await user.click(screen.getByText("查看恢复缺失清单"));
    expect(screen.getByText(/synthetic-locked.txt/)).toBeVisible();
    expect(screen.getByText(/synthetic locked-file detail/)).toBeVisible();
    expect(screen.getByText(/manifest.json/)).toBeVisible();
  });
  it("returns from both migration pages and defaults to local backup on reentry", async () => {
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "前往备份与迁移" }));
    for (const name of ["导出 ReHome 迁移包", "导入 ReHome 迁移包"]) {
      document.documentElement.scrollTop = 400;
      await user.click(screen.getByRole("tab", { name }));
      expect(document.documentElement.scrollTop).toBe(0);
      await user.click(screen.getByRole("button", { name: "返回本地备份" }));
      expect(screen.getByRole("tab", { name: "本地备份" })).toHaveAttribute("aria-selected", "true");
    }
    await user.click(screen.getByRole("tab", { name: "导出 ReHome 迁移包" }));
    await user.click(screen.getByRole("button", { name: "前往项目" }));
    await user.click(screen.getByRole("button", { name: "前往备份与迁移" }));
    expect(screen.getByRole("tab", { name: "本地备份" })).toHaveAttribute("aria-selected", "true");
  });

  it("keeps an in-flight backup and its result across navigation without resetting settings", async () => {
    const user = userEvent.setup();
    let finish!: (value: unknown) => void;
    api.runLocalBackup.mockImplementation(() => new Promise((resolve) => { finish = resolve; }));
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "前往备份与迁移" }));
    await user.type(screen.getByLabelText("恢复密码"), "synthetic-password");
    await user.click(screen.getByRole("button", { name: "开始本地备份" }));
    await user.click(screen.getByRole("button", { name: "前往数据" }));
    expect(screen.getByRole("heading", { name: "数据" })).toBeVisible();
    expect(screen.getByRole("button", { name: "查看进行中的任务" })).toBeVisible();
    await user.click(screen.getByRole("button", { name: "前往备份与迁移" }));
    expect(screen.getByRole("button", { name: "备份进行中" })).toBeDisabled();
    expect(screen.getByLabelText("恢复密码")).toHaveValue("synthetic-password");
    await user.click(screen.getByRole("button", { name: "前往项目" }));
    await user.click(screen.getByRole("checkbox", { name: "选择项目 demo" }));
    await user.click(screen.getByRole("button", { name: "保存项目选择" }));
    await act(async () => finish({
      logical_backup_id: "fixture-backup", restic_snapshot_id: "fixture-snapshot", complete: true,
      manifest: { created_at: "2026-09-19T00:00:00Z", file_count: 4, byte_count: 300 },
    }));
    await user.click(screen.getByRole("button", { name: "前往备份与迁移" }));
    expect(screen.getByText(/本地备份已完成 · 4/)).toBeVisible();
    expect(api.runLocalBackup).toHaveBeenCalledTimes(1);
    await user.click(screen.getByRole("button", { name: "前往项目" }));
    expect(screen.getByRole("checkbox", { name: "选择项目 demo" })).not.toBeChecked();
  });

  it("deduplicates Windows aliases and allows deselecting an unavailable saved project", async () => {
    const user = userEvent.setup();
    api.discoverCodex.mockResolvedValue({ ...inventory, projects: [{ ...inventory.projects[0], source_path: "\\\\?\\C:\\Work\\demo", source_available: false }] });
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "前往项目" }));
    const checkbox = screen.getByRole("checkbox", { name: "选择项目 demo" });
    expect(checkbox).toBeChecked();
    expect(checkbox).toBeEnabled();
    await user.click(checkbox);
    await user.click(screen.getByRole("button", { name: "保存项目选择" }));
    expect(api.saveAppConfig).toHaveBeenLastCalledWith(expect.objectContaining({ selected_project_paths: [], project_selection_initialized: true }));
    await user.click(screen.getByRole("button", { name: "前往概览" }));
    await user.click(screen.getByRole("button", { name: "前往项目" }));
    expect(screen.getByRole("checkbox", { name: "选择项目 demo" })).not.toBeChecked();
  });

  it("preserves an explicitly empty selection after restart and backs up only Codex data", async () => {
    const user = userEvent.setup();
    api.getAppConfig.mockResolvedValue({ ...config, selected_project_paths: [] });
    api.runLocalBackup.mockRejectedValue({ message: "synthetic engine error" });
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "前往项目" }));
    expect(screen.getByRole("checkbox", { name: "选择项目 demo" })).not.toBeChecked();
    await user.click(screen.getByRole("button", { name: "前往备份与迁移" }));
    await user.type(screen.getByLabelText("恢复密码"), "synthetic-password");
    await user.click(screen.getByRole("button", { name: "开始本地备份" }));
    expect(api.runLocalBackup).toHaveBeenCalledWith(expect.objectContaining({ project_paths: [] }));
    expect(await screen.findByText("synthetic engine error")).toBeVisible();
  });

  it("respects disabled startup scanning and replaces previous rescan candidates", async () => {
    const user = userEvent.setup();
    api.getAppConfig.mockResolvedValue({ ...config, automatic_project_scan: false, selected_project_paths: [], project_scan_roots: ["F:\\Projects"] });
    render(<App />);
    await screen.findByText("本机已就绪");
    expect(api.discoverLocalCandidates).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "前往项目" }));
    expect(screen.getByRole("checkbox", { name: "启动时自动扫描项目" })).not.toBeChecked();
    api.discoverLocalCandidates.mockResolvedValue({ candidates: [{path:"F:\\Projects\\first",name:"first",markers:[".git"]}], codex_homes:[], conversation_count:0, scanned_roots:["F:\\Projects"], skipped_roots:[],warnings:[],permission_denied_count:0,other_warning_count:0,cancelled:false });
    await user.click(screen.getByRole("button", { name: "重新扫描" }));
    expect(await screen.findByText("first")).toBeVisible();
    expect(api.discoverLocalCandidates).toHaveBeenLastCalledWith(["F:\\Projects"]);
    api.discoverLocalCandidates.mockResolvedValue({ candidates: [{path:"F:\\Projects\\second",name:"second",markers:[".git"]}], codex_homes:[], conversation_count:0, scanned_roots:["F:\\Projects"], skipped_roots:[],warnings:[],permission_denied_count:0,other_warning_count:0,cancelled:false });
    await user.click(screen.getByRole("button", { name: "重新扫描" }));
    expect(await screen.findByText("second")).toBeVisible();
    expect(screen.queryByText("first")).not.toBeInTheDocument();
    expect(screen.queryByText("demo")).not.toBeInTheDocument();
  });

  it("does not present an uncounted discovery as zero files", async () => {
    const user = userEvent.setup();
    api.discoverCodex.mockResolvedValue({ ...inventory, projects: [{...inventory.projects[0], file_count:0}], counts:{...inventory.counts,project_files:0} });
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "前往项目" }));
    expect(await screen.findByText("3 文件")).toBeVisible();
    expect(screen.queryByText(/^0 文件/)).not.toBeInTheDocument();
  });

  it("exposes the primary navigation areas and a local-only state", async () => {
    render(<App />);

    expect(await screen.findByRole("navigation", { name: "主导航" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "前往概览" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "前往项目" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "前往数据" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "前往备份与迁移" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "前往设置" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "前往操作说明" })).toBeInTheDocument();
    expect(screen.getAllByText("云端备份已关闭").length).toBeGreaterThan(0);
    expect(screen.getByText("v0.1.3")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Codex 数据备份&迁移" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "开始本地备份" })).toBeInTheDocument();
  });

  it("provides guided backup actions from the overview", async () => {
    const user = userEvent.setup();
    render(<App />);

    expect(await screen.findByRole("button", { name: "项目备份设置" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "数据备份设置" })).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "项目备份设置" }));
    expect(screen.getByRole("heading", { name: "项目" })).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "前往概览" }));
    await user.click(screen.getByRole("button", { name: "开始本地备份" }));
    expect(screen.getByRole("heading", { name: "备份与迁移" })).toBeInTheDocument();
  });

  it("opens data backup settings and saves the Codex data location", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "数据备份设置" }));

    expect(screen.getByRole("heading", { name: "数据" })).toBeInTheDocument();
    const codexHome = screen.getByRole("textbox", { name: "Codex 数据位置" });
    expect(codexHome).toHaveValue("C:\\Users\\Me\\.codex");
    await user.clear(codexHome);
    await user.type(codexHome, "D:\\CodexData");
    await user.click(screen.getByRole("button", { name: "保存数据设置" }));

    expect(api.saveAppConfig).toHaveBeenCalledWith(expect.objectContaining({ codex_home: "D:\\CodexData" }));
  });

  it("opens a bilingual operation guide with an accessible flow", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "前往操作说明" }));

    expect(screen.getByRole("heading", { name: "操作说明" })).toBeInTheDocument();
    expect(screen.getByRole("list", { name: "操作流程" })).toBeInTheDocument();
    expect(screen.getByText("自动扫描并确认路径")).toBeInTheDocument();
    expect(screen.getByText("选择项目")).toBeInTheDocument();
    expect(screen.getByText("恢复或离线迁移")).toBeInTheDocument();
  });

  it("summarizes skipped permission directories and lets the user request an elevated scan", async () => {
    const user = userEvent.setup();
    api.discoverLocalCandidates.mockResolvedValue({
      candidates: [{ path: "F:\\Projects\\found", name: "found", markers: [".git"] }],
      codex_homes: [], conversation_count: 0, scanned_roots: ["C:\\", "F:\\"], skipped_roots: [],
      warnings: [], permission_denied_count: 37, other_warning_count: 0, cancelled: false,
    });
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "前往项目" }));
    expect(await screen.findByText(/37/)).toBeInTheDocument();
    expect(screen.queryByText(/os error 5/)).not.toBeInTheDocument();
    expect(screen.getByText("found")).toBeInTheDocument();
    await user.click(screen.getByRole("checkbox", { name: "扫描受限目录时申请管理员权限" }));
    await user.click(screen.getByRole("button", { name: "以管理员权限重新扫描" }));
    expect(api.requestAdminLocalDiscovery).toHaveBeenCalledTimes(1);
  });

  it("switches the complete primary flow to English", async () => {
    const user = userEvent.setup();
    render(<App />);

    await screen.findByText("本机已就绪");
    await user.click(screen.getByRole("button", { name: "切换为英文" }));

    expect(screen.getByRole("button", { name: "Go to Overview" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Go to Backups & Migration" })).toBeInTheDocument();
    expect(screen.getAllByText("Cloud backup is off").length).toBeGreaterThan(0);
    expect(screen.getByRole("heading", { name: "Codex Data Backup & Migration" })).toBeInTheDocument();
  });

  it("keeps the projects and settings workflows reachable", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "前往项目" }));
    expect(screen.getByRole("heading", { name: "项目" })).toBeInTheDocument();
    expect(screen.getByText("demo")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "前往设置" }));
    expect(screen.getByRole("heading", { name: "设置" })).toBeInTheDocument();
    expect(screen.getByText("外观")).toBeInTheDocument();
    expect(screen.getByText("云端备份（可选）")).toBeInTheDocument();
  });

  it("loads configuration before discovery and passes the saved Codex path", async () => {
    const order: string[] = [];
    api.getAppConfig.mockImplementation(async () => {
      order.push("config");
      return config;
    });
    api.discoverCodex.mockImplementation(async (codexHome: string) => {
      order.push("discovery");
      expect(codexHome).toBe(config.codex_home);
      return inventory;
    });

    render(<App />);

    await screen.findByText("本机已就绪");
    expect(order.slice(0, 2)).toEqual(["config", "discovery"]);
  });

  it("opens native folder selection from local path controls", async () => {
    const user = userEvent.setup();
    api.pickDirectory.mockResolvedValueOnce("C:\\Users\\Me\\.codex");
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "前往数据" }));
    await user.click(screen.getByRole("button", { name: "选择 Codex 数据位置" }));

    expect(api.pickDirectory).toHaveBeenCalledWith("选择 Codex 数据位置");
  });

  it("shows the attempted path and a recovery action when Codex discovery fails", async () => {
    const user = userEvent.setup();
    api.discoverCodex.mockRejectedValue({
      code: "codex_not_found",
      message: "Codex home was not found (attempted Codex data path: C:\\Missing\\.codex)",
    });
    render(<App />);

    expect(await screen.findByText(/C:\\Missing\\\.codex/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "选择 Codex 数据位置" }));
    expect(api.pickDirectory).toHaveBeenCalledWith("选择 Codex 数据位置");
  });

  it("retries Codex discovery with an automatically found local home", async () => {
    const configured = { ...config, codex_home: "C:\\Missing\\.codex", selected_project_paths: [] };
    const discoveredHome = "F:\\Users\\Me\\.codex";
    let attempts = 0;
    api.getAppConfig.mockResolvedValue(configured);
    api.discoverCodex.mockImplementation(async () => {
      attempts += 1;
      if (attempts === 1) {
        throw { code: "codex_not_found", message: "missing configured home" };
      }
      return { ...inventory, codex_home: discoveredHome, project_paths: [] };
    });
    api.discoverLocalCandidates.mockResolvedValue({
      candidates: [],
      codex_homes: [discoveredHome],
      conversation_count: 4,
      scanned_roots: ["C:\\", "F:\\"],
      skipped_roots: [],
      warnings: [],
      cancelled: false,
    });

    render(<App />);

    expect(await screen.findByText("本机已就绪")).toBeInTheDocument();
    expect(api.discoverCodex).toHaveBeenNthCalledWith(2, discoveredHome);
    expect(api.saveAppConfig).toHaveBeenCalledWith(expect.objectContaining({ codex_home: discoveredHome }));
  });

  it("shows metadata-only local project candidates without selecting them automatically", async () => {
    const user = userEvent.setup();
    api.discoverLocalCandidates.mockResolvedValue({
      candidates: [{ path: "F:\\Projects\\found", name: "found", markers: [".git"] }],
      codex_homes: [],
      conversation_count: 0,
      scanned_roots: ["C:\\", "F:\\"],
      skipped_roots: [],
      warnings: [],
      cancelled: false,
    });
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "前往项目" }));
    expect(await screen.findByText("found")).toBeInTheDocument();
    expect(screen.getByRole("checkbox", { name: /F:\\Projects\\found/ })).not.toBeChecked();
  });

  it("allows adding a regular local folder to the backup scope", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "前往项目" }));
    await user.type(screen.getByLabelText("手动添加项目目录"), "F:\\Notes\\shared");
    await user.click(screen.getByRole("button", { name: "添加目录" }));

    expect(screen.getByText("F:\\Notes\\shared")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "保存项目选择" }));
    expect(api.saveAppConfig).toHaveBeenCalledWith(expect.objectContaining({
      selected_project_paths: expect.arrayContaining(["F:\\Notes\\shared"]),
    }));
  });

  it("does not present a partial local backup as complete", async () => {
    const user = userEvent.setup();
    api.runLocalBackup.mockResolvedValue({
      logical_backup_id: "33333333-3333-3333-3333-333333333333",
      restic_snapshot_id: "0123456789abcdef0123456789abcdef",
      complete: false,
      manifest: {
        format: "enhe-codex-backup",
        schema_version: 1,
        logical_backup_id: "33333333-3333-3333-3333-333333333333",
        batch_id: "44444444-4444-4444-4444-444444444444",
        created_at: "2026-09-16T00:00:00Z",
        app_version: "0.1.2",
        restic_version: "restic 0.19.1",
        source_device_id: inventory.source_device_id,
        source_codex_home: inventory.codex_home,
        project_paths: inventory.project_paths,
        git_metadata_paths: [],
        file_count: 1,
        byte_count: 10,
        fingerprint: "fingerprint",
        exclusions: [],
        missing: [{ path: "C:\\Work\\missing", reason: "missing", bytes: 0 }],
        integrity_status: "partial",
      },
    });

    render(<App />);
    await user.click(await screen.findByRole("button", { name: "前往备份与迁移" }));
    await user.type(screen.getByLabelText("恢复密码"), "synthetic-password");
    await user.click(screen.getByRole("button", { name: "开始本地备份" }));

    expect((await screen.findAllByText(/本地备份已完成但存在缺失内容/)).length).toBeGreaterThanOrEqual(2);
    expect(screen.queryByText(/^本地备份已完成 ·/)).not.toBeInTheDocument();
  });

  it("renders and submits a real rclone configuration question", async () => {
    const user = userEvent.setup();
    api.startCloudConfiguration.mockResolvedValue({
      config_file: "C:\\Users\\Me\\rclone.conf",
      remote_name: "enhe-onedrive",
      provider: "onedrive",
      completed: false,
      question: {
        state: "state-1",
        name: "config_type",
        help: "Choose the account type",
        default_value: "onedrive",
        is_password: false,
        examples: ["onedrive"],
        error: null,
      },
    });
    api.continueCloudConfiguration.mockResolvedValue({
      config_file: "C:\\Users\\Me\\rclone.conf",
      remote_name: "enhe-onedrive",
      provider: "onedrive",
      completed: true,
      question: null,
    });

    render(<App />);
    await user.click(await screen.findByRole("button", { name: "前往设置" }));
    await user.type(screen.getByLabelText("OneDrive 远端名称"), "enhe-onedrive");
    await user.type(screen.getByLabelText("rclone 配置文件"), "C:\\Users\\Me\\rclone.conf");
    await user.click(screen.getByRole("button", { name: "开始 OneDrive 配置" }));
    expect(await screen.findByRole("heading", { name: "OneDrive 配置问题" })).toBeInTheDocument();
    expect(screen.getByDisplayValue("onedrive")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "提交配置答案" }));
    expect(api.continueCloudConfiguration).toHaveBeenCalledWith(
      "C:\\Users\\Me\\rclone.conf",
      "enhe-onedrive",
      "state-1",
      "onedrive",
    );
    expect(await screen.findByText("OneDrive 配置已完成")).toBeInTheDocument();
  });
});
