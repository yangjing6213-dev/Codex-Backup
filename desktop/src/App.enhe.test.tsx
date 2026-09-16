import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import App from "./App";

const api = vi.hoisted(() => ({
  discoverCodex: vi.fn(),
  discoverLocalCandidates: vi.fn(),
  getAppConfig: vi.fn(),
  getSchedulerStatus: vi.fn(),
  listLocalBackups: vi.fn(),
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
};

beforeEach(() => {
  vi.clearAllMocks();
  window.localStorage.clear();
  api.discoverCodex.mockResolvedValue(inventory);
  api.discoverLocalCandidates.mockResolvedValue({
    candidates: [],
    codex_homes: [],
    conversation_count: 0,
    scanned_roots: ["C:\\"],
    skipped_roots: [],
    warnings: [],
    cancelled: false,
  });
  api.getAppConfig.mockResolvedValue(config);
  api.getSchedulerStatus.mockResolvedValue({ enabled: false, task_name: "ENHE Codex Backup - Current User" });
  api.listLocalBackups.mockResolvedValue([]);
  api.saveAppConfig.mockResolvedValue(config);
  api.pickDirectory.mockResolvedValue(null);
  api.startCloudConfiguration.mockReset();
  api.continueCloudConfiguration.mockReset();
});

describe("ENHE Codex Backup shell", () => {
  it("exposes exactly the four primary navigation areas and a local-only state", async () => {
    render(<App />);

    expect(await screen.findByRole("navigation", { name: "主导航" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "前往概览" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "前往项目" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "前往备份与迁移" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "前往设置" })).toBeInTheDocument();
    expect(screen.getAllByText("云端备份已关闭").length).toBeGreaterThan(0);
    expect(screen.getByRole("button", { name: "开始本地备份" })).toBeInTheDocument();
  });

  it("switches the complete primary flow to English", async () => {
    const user = userEvent.setup();
    render(<App />);

    await screen.findByText("本机已就绪");
    await user.click(screen.getByRole("button", { name: "切换为英文" }));

    expect(screen.getByRole("button", { name: "Go to Overview" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Go to Backups & Migration" })).toBeInTheDocument();
    expect(screen.getAllByText("Cloud backup is off").length).toBeGreaterThan(0);
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

    await user.click(await screen.findByRole("button", { name: "前往设置" }));
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
        app_version: "0.1.0",
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
