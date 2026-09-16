import { useCallback, useEffect, useMemo, useRef, useState, type RefObject } from "react";
import {
  Archive,
  CheckCircle2,
  CloudOff,
  CloudUpload,
  FolderKanban,
  FolderOpen,
  HardDrive,
  Languages,
  Laptop,
  LoaderCircle,
  LockKeyhole,
  Moon,
  Palette,
  RefreshCw,
  RotateCcw,
  Settings2,
  Sun,
  TriangleAlert,
} from "lucide-react";

import ReceivePage from "./features/receive/ReceivePage";
import SendPage from "./features/send/SendPage";
import {
  discoverCodex,
  getAppConfig,
  getSchedulerStatus,
  listLocalBackups,
  restoreLocalBackup,
  runLocalBackup,
  saveAppConfig,
  setScheduler,
  startCloudConfiguration,
  continueCloudConfiguration,
  discoverLocalCandidates,
  cancelLocalDiscovery,
  pickDirectory,
  testCloudConnection,
  uploadLocalSnapshot,
} from "./lib/api";
import { I18nProvider, useI18n, type Locale } from "./lib/i18n";
import {
  errorMessage,
  type AppConfig,
  type Appearance,
  type CloudConfig,
  type CloudConfigQuestion,
  type CodexInventory,
  type LocalSnapshot,
  type LocalSnapshotSummary,
  type LocalDiscoveryResult,
  type LocalProjectCandidate,
  type SchedulerStatus,
} from "./lib/types";
import "./App.css";

export type View = "overview" | "projects" | "backups" | "settings";
type BackupView = "local" | "export" | "import";
type LocalScanState = "idle" | "running" | "complete" | "partial" | "failed";

const views: Array<{
  id: View;
  label: string;
  accessibleLabel: string;
  icon: typeof HardDrive;
}> = [
  { id: "overview", label: "概览", accessibleLabel: "前往概览", icon: HardDrive },
  { id: "projects", label: "项目", accessibleLabel: "前往项目", icon: FolderKanban },
  { id: "backups", label: "备份与迁移", accessibleLabel: "前往备份与迁移", icon: Archive },
  { id: "settings", label: "设置", accessibleLabel: "前往设置", icon: Settings2 },
];

const viewTitles: Record<View, string> = {
  overview: "概览",
  projects: "项目",
  backups: "备份与迁移",
  settings: "设置",
};

function defaultConfig(): AppConfig {
  return {
    config_version: 1,
    codex_home: null,
    local_repository: null,
    selected_project_paths: [],
    frequency_minutes: 15,
    retention: { high_frequency_hours: 48, daily_days: 30, weekly_weeks: 12 },
    cloud: {
      enabled: false,
      remote_name: null,
      remote_path: null,
      repository_path: null,
      config_file: null,
    },
    locale: "zh-CN",
    appearance: "system",
    automatic_backup_enabled: false,
  };
}

export default function App() {
  return (
    <I18nProvider>
      <AppContent />
    </I18nProvider>
  );
}

function AppContent() {
  const { locale, setLocale, t } = useI18n();
  const [view, setView] = useState<View>("overview");
  const [config, setConfig] = useState<AppConfig>(defaultConfig);
  const [inventory, setInventory] = useState<CodexInventory | null>(null);
  const [localDiscovery, setLocalDiscovery] = useState<LocalDiscoveryResult | null>(null);
  const [localScanState, setLocalScanState] = useState<LocalScanState>("idle");
  const [scheduler, setSchedulerStatus] = useState<SchedulerStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [codexScanFailed, setCodexScanFailed] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [activeOperations, setActiveOperations] = useState(0);
  const headingRef = useRef<HTMLHeadingElement>(null);
  const previousViewRef = useRef(view);

  useEffect(() => {
    let active = true;
    async function initialize() {
      let loaded = defaultConfig();
      let fastDiscoverySucceeded = false;
      try {
        loaded = await getAppConfig();
        if (active) {
          setConfig(loaded);
          setLocale(loaded.locale);
        }
      } catch (caught) {
        if (active) setError(errorMessage(caught, t));
      }

      try {
        const detected = await discoverCodex(loaded.codex_home);
        fastDiscoverySucceeded = true;
        if (active) {
          setInventory(detected);
          const next = mergeAutomaticConfig(loaded, detected);
          setConfig(next);
          let saveFailed = false;
          if (hasConfigChanges(loaded, next)) {
            try {
              const saved = await saveAppConfig(next);
              if (active) setConfig(saved);
            } catch (caught) {
              saveFailed = true;
              if (active) setError(errorMessage(caught, t));
            }
          }
          if (!saveFailed) {
            setError(null);
            setCodexScanFailed(false);
          }
        }
      } catch (caught) {
        if (active) {
          setCodexScanFailed(true);
          setError(errorMessage(caught, t));
        }
      } finally {
        if (active) setLoading(false);
      }

      if (!active) return;
      setLocalScanState("running");
      try {
        const result = await discoverLocalCandidates();
        if (active) {
          setLocalDiscovery(result);
          setLocalScanState(result.cancelled || result.warnings.length > 0 ? "partial" : "complete");

          if (!fastDiscoverySucceeded && result.codex_homes.length > 0) {
            try {
              const detected = await discoverCodex(result.codex_homes[0]);
              const next = mergeAutomaticConfig(
                { ...loaded, codex_home: result.codex_homes[0] },
                detected,
              );
              setInventory(detected);
              setConfig(next);
              let saveFailed = false;
              if (hasConfigChanges(loaded, next)) {
                try {
                  const saved = await saveAppConfig(next);
                  if (active) setConfig(saved);
                } catch (caught) {
                  saveFailed = true;
                  if (active) setError(errorMessage(caught, t));
                }
              }
              setCodexScanFailed(false);
              if (!saveFailed) setError(null);
            } catch (caught) {
              setCodexScanFailed(true);
              setError(errorMessage(caught, t));
            }
          }
        }
      } catch (caught) {
        if (active) {
          setLocalScanState("failed");
          setError(errorMessage(caught, t));
        }
      }
    }
    void initialize();
    void getSchedulerStatus()
      .then((status) => {
        if (active) setSchedulerStatus(status);
      })
      .catch(() => undefined);
    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    if (previousViewRef.current !== view) {
      headingRef.current?.focus();
      previousViewRef.current = view;
    }
  }, [view]);

  useEffect(() => {
    document.documentElement.dataset.appearance = config.appearance;
  }, [config.appearance]);

  useEffect(() => {
    if (!inventory) return;
    setConfig((current) => ({
      ...current,
      codex_home: current.codex_home ?? inventory.codex_home,
      selected_project_paths:
        current.selected_project_paths.length > 0
          ? current.selected_project_paths
          : inventory.project_paths,
    }));
  }, [inventory]);

  const persistConfig = useCallback(
    async (next: AppConfig, message = "设置已保存") => {
      const saved = await saveAppConfig(next);
      setConfig(saved);
      setLocale(saved.locale);
      setNotice(message);
    },
    [setLocale],
  );

  async function chooseCodexHome() {
    try {
      const selected = await pickDirectory(t("选择 Codex 数据位置"));
      if (!selected) return;
      const detected = await discoverCodex(selected);
      const next = mergeAutomaticConfig({ ...config, codex_home: selected }, detected);
      setInventory(detected);
      setConfig(next);
      await saveAppConfig(next);
      setCodexScanFailed(false);
      setError(null);
      setNotice(t("设置已保存"));
    } catch (caught) {
      setError(errorMessage(caught, t));
    }
  }

  function operationStarted() {
    setActiveOperations((current) => current + 1);
  }

  function operationFinished() {
    setActiveOperations((current) => Math.max(0, current - 1));
  }

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <button className="brand" type="button" onClick={() => setView("overview")} aria-label={t("ENHE Codex Backup")}>
          <span className="brand-mark" aria-hidden="true">E</span>
          <span className="brand-copy"><strong>ENHE</strong><small>Codex Backup</small></span>
        </button>

        <nav className="navigation" aria-label={t("主导航")}>
          {views.map(({ id, label, accessibleLabel, icon: Icon }) => (
            <button
              className="nav-item"
              data-active={view === id}
              type="button"
              aria-label={t(accessibleLabel)}
              aria-current={view === id ? "page" : undefined}
              onClick={() => setView(id)}
              key={id}
            >
              <Icon aria-hidden="true" />
              <span>{t(label)}</span>
            </button>
          ))}
        </nav>

        <button
          className="language-toggle"
          type="button"
          aria-label={locale === "en" ? "Switch to Chinese" : "切换为英文"}
          onClick={() => {
            const next: Locale = locale === "en" ? "zh-CN" : "en";
            setLocale(next);
            setConfig((current) => ({ ...current, locale: next }));
          }}
        >
          <Languages aria-hidden="true" />
          <span>{locale === "en" ? "中文" : "English"}</span>
        </button>

        <div className="sidebar-meta">
          <Laptop aria-hidden="true" />
          <span>{t("云端备份已关闭")}</span>
        </div>
      </aside>

      <main className="workspace" data-view={view} aria-busy={activeOperations > 0}>
        <header className="topbar">
          <span className="topbar-title">{t(viewTitles[view])}</span>
          {loading ? (
            <span className="machine-status"><LoaderCircle className="spin" aria-hidden="true" />{t("本机扫描中")}</span>
          ) : error ? (
            <span className="machine-status machine-error"><TriangleAlert aria-hidden="true" />{t("本机扫描失败")}</span>
          ) : (
            <span className="machine-status"><CheckCircle2 aria-hidden="true" />{t("本机已就绪")}</span>
          )}
        </header>

        {notice && <div className="notice" role="status">{notice}<button type="button" onClick={() => setNotice(null)} aria-label={t("关闭")}>×</button></div>}
        {error && <div className="global-error" role="alert"><span>{error}</span>{codexScanFailed && <button className="text-button" type="button" onClick={() => void chooseCodexHome()}>{t("选择 Codex 数据位置")}</button>}</div>}

        {view === "overview" && (
          <OverviewPage headingRef={headingRef} inventory={inventory} localConversationCount={localDiscovery?.conversation_count ?? 0} config={config} scheduler={scheduler} localScanState={localScanState} onNavigate={setView} />
        )}
        {view === "projects" && (
          <ProjectsPage
            headingRef={headingRef}
            inventory={inventory}
            localCandidates={localDiscovery?.candidates ?? []}
            localWarnings={localDiscovery?.warnings ?? []}
            scannedRootCount={localDiscovery?.scanned_roots.length ?? 0}
            localScanState={localScanState}
            onCancelLocalScan={() => { void cancelLocalDiscovery(); }}
            config={config}
            onSave={async (next) => {
              try {
                await persistConfig(next, t("项目选择已保存"));
              } catch (caught) {
                setError(errorMessage(caught, t));
              }
            }}
            onError={setError}
          />
        )}
        {view === "backups" && (
          <BackupsPage
            headingRef={headingRef}
            inventory={inventory}
            config={config}
            onConfigChange={setConfig}
            onNavigate={setView}
            onOperationStart={operationStarted}
            onOperationEnd={operationFinished}
            onNotice={setNotice}
            onError={setError}
          />
        )}
        {view === "settings" && (
          <SettingsPage
            headingRef={headingRef}
            config={config}
            scheduler={scheduler}
            onSave={async (next) => {
              try {
                const task = await setScheduler(next.automatic_backup_enabled, next.frequency_minutes);
                const saved = await saveAppConfig(next);
                setConfig(saved);
                setLocale(saved.locale);
                setSchedulerStatus(task);
                setNotice(t("设置已保存"));
              } catch (caught) {
                setError(errorMessage(caught, t));
              }
            }}
            onConfigChange={setConfig}
            onNotice={setNotice}
            onError={setError}
          />
        )}
      </main>
    </div>
  );
}

interface OverviewPageProps {
  headingRef: RefObject<HTMLHeadingElement | null>;
  inventory: CodexInventory | null;
  localConversationCount: number;
  config: AppConfig;
  scheduler: SchedulerStatus | null;
  localScanState: LocalScanState;
  onNavigate: (view: View) => void;
}

function OverviewPage({ headingRef, inventory, localConversationCount, config, scheduler, localScanState, onNavigate }: OverviewPageProps) {
  const { t } = useI18n();
  return (
    <div className="page">
      <header className="page-header">
        <p className="eyebrow">ENHE CODEX BACKUP</p>
        <h1 ref={headingRef} tabIndex={-1}>{t("Codex 数据备份&迁移")}</h1>
        <p className="page-description">{t("没有云端也能备份、查看和恢复资料。")}</p>
      </header>

      <section className="status-banner local-banner">
        <div className="status-icon"><HardDrive aria-hidden="true" /></div>
        <div><strong>{t("云端备份已关闭")}</strong><p>{t("云端关闭时不会启动远端连接，也不会上传资料。")}</p></div>
        <button className="primary-button" type="button" onClick={() => onNavigate("backups")}>{t("开始本地备份")}</button>
      </section>

      <section className="metric-grid" aria-label={t("本机检测")}>
        <Metric label={t("项目总数")} value={inventory?.counts.projects ?? 0} />
        <Metric label={t("对话总数")} value={inventory?.counts.conversations ?? localConversationCount} />
        <Metric label={t("文件总数")} value={inventory?.counts.project_files ?? 0} />
        <Metric label={t("计划任务状态")} value={scheduler?.enabled ? t("已启用") : t("未启用")} text />
      </section>

      {localScanState === "running" && <div className="scan-status" role="status"><LoaderCircle className="spin" aria-hidden="true" />{t("正在扫描本机项目")}</div>}
      {(localScanState === "complete" || localScanState === "partial") && <div className="scan-status" role="status"><CheckCircle2 aria-hidden="true" />{t(localScanState === "complete" ? "本机项目扫描已完成" : "本机项目扫描已部分完成")}</div>}

      <section className="card two-column">
        <div>
          <div className="section-heading"><LockKeyhole aria-hidden="true" /><h2>{t("完整本地备份")}</h2></div>
          <p>{t("选择的项目会与 Git 元数据、未提交修改和 worktree 一起进入完整备份。")}</p>
          <div className="path-value"><span>{t("Codex 数据位置")}</span><code>{config.codex_home ?? t("未检测")}</code></div>
          <div className="path-value"><span>{t("备份目录")}</span><code>{config.local_repository ?? t("未检测")}</code></div>
        </div>
        <div className="how-it-works">
          <div className="section-heading"><CheckCircle2 aria-hidden="true" /><h2>{t("操作说明")}</h2></div>
          <p>{t("选择本地目录后，应用用 restic 加密、去重并校验快照。恢复会先进入独立目录，不覆盖现有资料。")}</p>
          <p className="muted">{t("离线恢复")}: {t("把仓库目录和恢复密码带到新设备，安装应用后选择恢复目标即可。")}</p>
        </div>
      </section>
    </div>
  );
}

function Metric({ label, value, text = false }: { label: string; value: number | string; text?: boolean }) {
  return <div className="metric"><span>{label}</span><strong className={text ? "metric-text" : undefined}>{value}</strong></div>;
}

interface ProjectsPageProps {
  headingRef: RefObject<HTMLHeadingElement | null>;
  inventory: CodexInventory | null;
  localCandidates: LocalProjectCandidate[];
  localWarnings: string[];
  scannedRootCount: number;
  localScanState: LocalScanState;
  onCancelLocalScan: () => void;
  config: AppConfig;
  onSave: (config: AppConfig) => Promise<void>;
  onError: (message: string | null) => void;
}

function ProjectsPage({ headingRef, inventory, localCandidates, localWarnings, scannedRootCount, localScanState, onCancelLocalScan, config, onSave, onError }: ProjectsPageProps) {
  const { t } = useI18n();
  const [selected, setSelected] = useState(() => new Set(config.selected_project_paths));
  const [manualPath, setManualPath] = useState("");
  const projects = inventory?.projects ?? [];
  const discoveredPaths = useMemo(() => new Set(projects.map((project) => project.source_path)), [projects]);
  const candidatePaths = useMemo(() => new Set(localCandidates.map((candidate) => candidate.path)), [localCandidates]);
  const manualPaths = [...selected].filter((path) => !discoveredPaths.has(path) && !candidatePaths.has(path));
  useEffect(() => {
    setSelected(new Set(config.selected_project_paths));
  }, [config.selected_project_paths]);
  const toggleProject = (path: string) => {
    setSelected((current) => {
      const next = new Set(current);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
  };
  return (
    <div className="page">
      <header className="page-header page-header-with-action">
        <div><p className="eyebrow">PROJECTS</p><h1 ref={headingRef} tabIndex={-1}>{t("项目")}</h1><p className="page-description">{t("选择的项目会与 Git 元数据、未提交修改和 worktree 一起进入完整备份。")}</p></div>
        <button className="primary-button" type="button" onClick={() => void onSave({ ...config, selected_project_paths: [...selected] })}>{t("保存项目选择")}</button>
      </header>
      <section className="card project-list" aria-label={t("项目文件夹")}>
        <div className="manual-project form-grid">
          <PathField label={t("手动添加项目目录")} value={manualPath} onChange={setManualPath} title={t("选择项目目录")} placeholder="F:\\Notes\\shared" onError={onError} />
          <div><p className="help-text">{t("允许添加非 Git 普通目录。")}</p><button className="secondary-button" type="button" onClick={() => { const path = manualPath.trim(); if (path) { setSelected((current) => new Set(current).add(path)); setManualPath(""); } }} disabled={!manualPath.trim()}>{t("添加目录")}</button></div>
        </div>
        {localScanState === "running" && <div className="scan-status" role="status"><LoaderCircle className="spin" aria-hidden="true" />{t("正在扫描本机项目")}<button className="text-button" type="button" onClick={onCancelLocalScan}>{t("取消扫描")}</button></div>}
        {localScanState === "complete" && <p className="help-text">{t("本机项目扫描已完成")}</p>}
        {localScanState === "partial" && <p className="help-text">{t("本机项目扫描已部分完成")}</p>}
        {localScanState === "failed" && <p className="inline-error" role="alert">{t("本机项目扫描失败")}</p>}
        {localScanState !== "idle" && localScanState !== "running" && <p className="help-text">{t("扫描位置数量")}: {scannedRootCount} · {t("候选项目数量")}: {localCandidates.length}</p>}
        {localWarnings.length > 0 && <div className="scan-warning" role="status"><TriangleAlert aria-hidden="true" /><span>{t("扫描提示")}: {localWarnings.join(" · ")}</span></div>}
        {projects.length === 0 ? <p className="empty-state">{t("当前没有可扫描的项目。")}</p> : projects.map((project) => (
          <label className="project-row" key={project.project_id}>
            <input type="checkbox" checked={selected.has(project.source_path)} onChange={() => toggleProject(project.source_path)} aria-label={`选择项目 ${project.name}`} disabled={!project.source_available} />
            <span className="project-copy"><strong>{project.name}</strong><code>{project.source_path}</code></span>
            <span className="project-meta">{project.file_count} {t("文件")}{project.git_branch ? ` · ${project.git_branch}` : ""}</span>
          </label>
        ))}
        {localCandidates.filter((candidate) => !discoveredPaths.has(candidate.path)).map((candidate) => (
          <label className="project-row" key={candidate.path}>
            <input type="checkbox" checked={selected.has(candidate.path)} onChange={() => toggleProject(candidate.path)} aria-label={`${t("选择项目")} ${candidate.path}`} />
            <span className="project-copy"><strong>{candidate.name}</strong><code>{candidate.path}</code></span>
            <span className="project-meta">{t("自动发现")} · {candidate.markers.join(", ")}</span>
          </label>
        ))}
        {manualPaths.map((path) => (
          <label className="project-row" key={path}>
            <input type="checkbox" checked onChange={() => toggleProject(path)} aria-label={`${t("选择项目")} ${path}`} />
            <span className="project-copy"><strong>{t("手动目录")}</strong><code>{path}</code></span>
            <span className="project-meta">{t("已纳入本地备份")}</span>
          </label>
        ))}
      </section>
    </div>
  );
}

interface BackupsPageProps {
  headingRef: RefObject<HTMLHeadingElement | null>;
  inventory: CodexInventory | null;
  config: AppConfig;
  onConfigChange: (config: AppConfig) => void;
  onNavigate: (view: View) => void;
  onOperationStart: () => void;
  onOperationEnd: () => void;
  onNotice: (message: string | null) => void;
  onError: (message: string | null) => void;
}

function BackupsPage({ headingRef, inventory, config, onConfigChange, onNavigate, onOperationStart, onOperationEnd, onNotice, onError }: BackupsPageProps) {
  const { t, locale } = useI18n();
  const [subview, setSubview] = useState<BackupView>("local");
  const [repository, setRepository] = useState(config.local_repository ?? "");
  const [password, setPassword] = useState("");
  const [restoreTarget, setRestoreTarget] = useState("");
  const [rememberPassword, setRememberPassword] = useState(false);
  const [snapshots, setSnapshots] = useState<LocalSnapshotSummary[]>([]);
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<LocalSnapshot | null>(null);
  const [restoreResult, setRestoreResult] = useState<string | null>(null);

  useEffect(() => {
    setRepository(config.local_repository ?? "");
  }, [config.local_repository]);

  const codexHome = config.codex_home ?? inventory?.codex_home ?? "";
  const projectPaths = config.selected_project_paths.length > 0 ? config.selected_project_paths : inventory?.project_paths ?? [];

  async function startBackup() {
    onError(null);
    onNotice(null);
    if (!repository.trim()) return onError(t("需要先填写完整的本地备份设置。"));
    if (!codexHome || projectPaths.length === 0) return onError(t("需要先选择至少一个项目。"));
    if (!password) return onError(t("请输入恢复密码。"));
    setBusy(true);
    onOperationStart();
    try {
      const backup = await runLocalBackup({ codex_home: codexHome, project_paths: projectPaths, repository, recovery_password: password, remember_password: rememberPassword, source_device_id: inventory?.source_device_id });
      setResult(backup);
      setSnapshots((current) => [summaryFromSnapshot(backup), ...current.filter((item) => item.restic_snapshot_id !== backup.restic_snapshot_id)]);
      onConfigChange({ ...config, codex_home: codexHome, local_repository: repository, selected_project_paths: projectPaths });
      onNotice(t(backup.complete ? "本地备份已完成" : "本地备份已完成但存在缺失内容"));
    } catch (caught) {
      onError(errorMessage(caught, t));
    } finally {
      setBusy(false);
      onOperationEnd();
    }
  }

  async function refreshBackups() {
    onError(null);
    if (!repository.trim()) return onError(t("需要先填写完整的本地备份设置。"));
    setBusy(true);
    onOperationStart();
    try {
      setSnapshots(await listLocalBackups({ repository, recovery_password: password }));
      onNotice(t("最近的本地备份"));
    } catch (caught) {
      onError(errorMessage(caught, t));
    } finally {
      setBusy(false);
      onOperationEnd();
    }
  }

  async function restore(snapshot: LocalSnapshotSummary) {
    onError(null);
    if (!repository.trim() || !restoreTarget.trim()) return onError(t("需要先填写完整的本地备份设置。"));
    setBusy(true);
    onOperationStart();
    try {
      const report = await restoreLocalBackup({ snapshot_id: snapshot.restic_snapshot_id, repository, recovery_password: password, target: restoreTarget });
      setRestoreResult(`${report.restored_root} · ${report.restored_files} ${t("文件")}`);
      onNotice(t("恢复完成"));
    } catch (caught) {
      onError(errorMessage(caught, t));
    } finally {
      setBusy(false);
      onOperationEnd();
    }
  }

  async function upload(snapshot: LocalSnapshotSummary) {
    onError(null);
    if (!config.cloud.enabled) return onError(t("云端备份已关闭"));
    if (!password.trim()) return onError(t("请输入恢复密码。"));
    setBusy(true);
    onOperationStart();
    try {
      const result = await uploadLocalSnapshot({
        config: config.cloud,
        source_repository: repository,
        source_password: password,
        target_password: password,
        snapshot_id: snapshot.restic_snapshot_id,
      });
      onNotice(cloudResultMessage(result.message, t));
    } catch (caught) {
      onError(errorMessage(caught, t));
    } finally {
      setBusy(false);
      onOperationEnd();
    }
  }

  if (subview === "export") return <SendPage headingRef={headingRef} inventory={inventory} onOperationStart={onOperationStart} onOperationEnd={onOperationEnd} />;
  if (subview === "import") return <ReceivePage headingRef={headingRef} inventory={inventory} onOperationStart={onOperationStart} onOperationEnd={onOperationEnd} />;

  return (
    <div className="page">
      <header className="page-header page-header-with-action">
        <div><p className="eyebrow">BACKUP & MIGRATION</p><h1 ref={headingRef} tabIndex={-1}>{t("备份与迁移")}</h1><p className="page-description">{t("本地备份不会访问云端。")}</p></div>
        <button className="secondary-button" type="button" onClick={() => void refreshBackups()} disabled={busy}><RefreshCw aria-hidden="true" />{t("刷新本地备份")}</button>
      </header>

      <div className="subnav" role="tablist" aria-label={t("迁移能力")}>
        <button role="tab" aria-selected={subview === "local"} className={subview === "local" ? "subnav-item active" : "subnav-item"} type="button" onClick={() => setSubview("local")}>{t("本地备份")}</button>
        <button role="tab" aria-selected={false} className="subnav-item" type="button" onClick={() => setSubview("export")}>{t("导出 ReHome 迁移包")}</button>
        <button role="tab" aria-selected={false} className="subnav-item" type="button" onClick={() => setSubview("import")}>{t("导入 ReHome 迁移包")}</button>
      </div>

      <section className="card form-card">
        <div className="section-heading"><HardDrive aria-hidden="true" /><h2>{t("完整本地备份")}</h2></div>
        <p className="help-text">{t("操作说明")}: {t("选择本地目录后，应用用 restic 加密、去重并校验快照。恢复会先进入独立目录，不覆盖现有资料。")}</p>
        <div className="form-grid">
          <PathField label={t("备份目录")} value={repository} onChange={setRepository} title={t("选择备份目录")} placeholder="D:\\ENHE\\backups" onError={onError} />
          <label><span>{t("恢复密码")}</span><input type="password" value={password} onChange={(event) => setPassword(event.target.value)} autoComplete="new-password" /></label>
        </div>
        <label className="checkbox-row"><input type="checkbox" checked={rememberPassword} onChange={(event) => setRememberPassword(event.target.checked)} /><span><strong>{t("记住密码（仅此 Windows 用户）")}</strong><small>{t("使用 DPAPI 保护密码，供计划任务使用。")}</small></span></label>
        <div className="path-line"><span>{t("Codex 数据位置")}</span><code>{codexHome || t("未检测")}</code></div>
        <div className="path-line"><span>{t("项目")}</span><code>{projectPaths.length} · {projectPaths.join("; ") || t("未检测")}</code></div>
        <button className="primary-button" type="button" onClick={() => void startBackup()} disabled={busy}>{busy ? <LoaderCircle className="spin" aria-hidden="true" /> : <LockKeyhole aria-hidden="true" />}{busy ? t("备份进行中") : t("开始本地备份")}</button>
        {result && <div className={result.complete ? "result success" : "result warning"} role="status">{result.complete ? <CheckCircle2 aria-hidden="true" /> : <TriangleAlert aria-hidden="true" />}<span>{t(result.complete ? "本地备份已完成" : "本地备份已完成但存在缺失内容")} · {result.manifest.file_count} {t("文件")} · {result.restic_snapshot_id}</span></div>}
      </section>

      <section className="card">
        <div className="section-heading"><Archive aria-hidden="true" /><h2>{t("最近的本地备份")}</h2></div>
        {snapshots.length === 0 ? <p className="empty-state">{t("还没有本地备份")}</p> : <div className="snapshot-list">{snapshots.map((snapshot) => (
          <div className="snapshot-row" key={snapshot.restic_snapshot_id}>
            <div><strong>{formatDate(snapshot.created_at, locale)}</strong><code>{snapshot.restic_snapshot_id}</code></div>
            <span className={snapshot.complete ? "tag complete" : "tag partial"}>{snapshot.complete ? t("完整") : t("部分完成")}</span>
            <span>{snapshot.file_count} {t("文件")}</span>
            <div className="snapshot-actions"><button className="secondary-button small" type="button" disabled={busy} onClick={() => void restore(snapshot)}><RotateCcw aria-hidden="true" />{t("恢复")}</button>{config.cloud.enabled && <button className="secondary-button small" type="button" disabled={busy} onClick={() => void upload(snapshot)}><CloudUpload aria-hidden="true" />{t("上传此快照")}</button>}</div>
          </div>
        ))}</div>}
        <PathField className="restore-target" label={t("恢复目标目录")} value={restoreTarget} onChange={setRestoreTarget} title={t("选择恢复目标目录")} placeholder="D:\\ENHE\\restored" onError={onError} />
        {restoreResult && <div className="result success" role="status"><CheckCircle2 aria-hidden="true" />{t("恢复完成")} · {restoreResult}</div>}
      </section>

      <section className="migration-note"><CloudOff aria-hidden="true" /><div><strong>{t("云端备份已关闭")}</strong><p>{t("云端关闭时不会启动远端连接，也不会上传资料。")}</p></div><button type="button" className="text-button" onClick={() => onNavigate("settings")}>{t("设置")}</button></section>
    </div>
  );
}

function summaryFromSnapshot(snapshot: LocalSnapshot): LocalSnapshotSummary {
  return { logical_backup_id: snapshot.logical_backup_id, restic_snapshot_id: snapshot.restic_snapshot_id, created_at: snapshot.manifest.created_at, file_count: snapshot.manifest.file_count, byte_count: snapshot.manifest.byte_count, complete: snapshot.complete };
}

function mergeAutomaticConfig(config: AppConfig, inventory: CodexInventory): AppConfig {
  return {
    ...config,
    codex_home: config.codex_home?.trim() ? config.codex_home : inventory.codex_home,
    selected_project_paths: config.selected_project_paths.length > 0 ? config.selected_project_paths : inventory.project_paths,
  };
}

function hasConfigChanges(before: AppConfig, after: AppConfig): boolean {
  return before.codex_home !== after.codex_home
    || before.selected_project_paths.join("\0") !== after.selected_project_paths.join("\0");
}

interface PathFieldProps {
  className?: string;
  label: string;
  value: string;
  onChange: (value: string) => void;
  title: string;
  placeholder?: string;
  disabled?: boolean;
  onError?: (message: string | null) => void;
}

function PathField({ className, label, value, onChange, title, placeholder, disabled, onError }: PathFieldProps) {
  const { t } = useI18n();
  async function choosePath() {
    if (disabled) return;
    onError?.(null);
    try {
      const selected = await pickDirectory(title);
      if (selected) onChange(selected);
    } catch (caught) {
      onError?.(errorMessage(caught, t));
    }
  }

  return (
    <label className={className ? `path-field ${className}` : "path-field"}>
      <span>{label}</span>
      <div className="path-input-row">
        <input
          aria-label={label}
          value={value}
          onChange={(event) => onChange(event.target.value)}
          onClick={() => void choosePath()}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              event.preventDefault();
              void choosePath();
            }
          }}
          placeholder={placeholder}
          disabled={disabled}
          title={title}
        />
        <button className="path-picker-button" type="button" onClick={() => void choosePath()} disabled={disabled} aria-label={title}>
          <FolderOpen aria-hidden="true" />
        </button>
      </div>
    </label>
  );
}

interface SettingsPageProps {
  headingRef: RefObject<HTMLHeadingElement | null>;
  config: AppConfig;
  scheduler: SchedulerStatus | null;
  onSave: (config: AppConfig) => Promise<void>;
  onConfigChange: (config: AppConfig) => void;
  onNotice: (message: string | null) => void;
  onError: (message: string | null) => void;
}

function SettingsPage({ headingRef, config, scheduler, onSave, onConfigChange, onNotice, onError }: SettingsPageProps) {
  const { t } = useI18n();
  const [draft, setDraft] = useState(config);
  const [cloudBusy, setCloudBusy] = useState(false);
  const [cloudQuestion, setCloudQuestion] = useState<CloudConfigQuestion | null>(null);
  const [cloudAnswer, setCloudAnswer] = useState("");
  useEffect(() => {
    setDraft(config);
  }, [config]);
  const update = <K extends keyof AppConfig>(key: K, value: AppConfig[K]) => setDraft((current) => ({ ...current, [key]: value }));
  const updateCloud = (value: Partial<CloudConfig>) => setDraft((current) => ({ ...current, cloud: { ...current.cloud, ...value } }));

  async function configureCloud() {
    const configFile = draft.cloud.config_file?.trim();
    const remoteName = draft.cloud.remote_name?.trim();
    if (!configFile || !remoteName) return onError(t("云端默认关闭；启用前需要完成 OneDrive 配置。"));
    setCloudBusy(true);
    onError(null);
    try {
      const started = await startCloudConfiguration(configFile, remoteName);
      updateCloud({ config_file: started.config_file, enabled: false });
      setCloudQuestion(started.completed ? null : started.question);
      setCloudAnswer(started.question?.default_value ?? "");
      onNotice(started.completed ? t("OneDrive 配置已完成") : t("OneDrive 配置需要回答下一步问题"));
    } catch (caught) {
      onError(errorMessage(caught, t));
    } finally {
      setCloudBusy(false);
    }
  }

  async function continueCloud() {
    if (!cloudQuestion) return;
    const configFile = draft.cloud.config_file?.trim();
    const remoteName = draft.cloud.remote_name?.trim();
    if (!configFile || !remoteName) return onError(t("云端默认关闭；启用前需要完成 OneDrive 配置。"));
    setCloudBusy(true);
    onError(null);
    try {
      const next = await continueCloudConfiguration(configFile, remoteName, cloudQuestion.state, cloudAnswer);
      updateCloud({ config_file: next.config_file, enabled: false });
      setCloudQuestion(next.completed ? null : next.question);
      setCloudAnswer(next.question?.default_value ?? "");
      onNotice(next.completed ? t("OneDrive 配置已完成") : t("OneDrive 配置需要回答下一步问题"));
    } catch (caught) {
      onError(errorMessage(caught, t));
    } finally {
      setCloudBusy(false);
    }
  }

  async function testCloud() {
    setCloudBusy(true);
    onError(null);
    try {
      const result = await testCloudConnection(draft.cloud);
      onNotice(cloudResultMessage(result.message, t));
    } catch (caught) {
      onError(errorMessage(caught, t));
    } finally {
      setCloudBusy(false);
    }
  }

  return (
    <div className="page">
      <header className="page-header"><p className="eyebrow">SETTINGS</p><h1 ref={headingRef} tabIndex={-1}>{t("设置")}</h1><p className="page-description">{t("云端默认关闭；启用前需要完成 OneDrive 配置。")}</p></header>

      <section className="card settings-card">
        <div className="section-heading"><Languages aria-hidden="true" /><h2>{t("语言")}</h2></div>
        <div className="choice-grid"><label className="choice"><input type="radio" name="locale" checked={draft.locale === "zh-CN"} onChange={() => update("locale", "zh-CN")} /><span>{t("简体中文")}</span></label><label className="choice"><input type="radio" name="locale" checked={draft.locale === "en"} onChange={() => update("locale", "en")} /><span>{t("English")}</span></label></div>
      </section>

      <section className="card settings-card">
        <div className="section-heading"><Palette aria-hidden="true" /><h2>{t("外观")}</h2></div>
        <div className="choice-grid appearance-grid">
          <AppearanceChoice value="light" current={draft.appearance} label={t("浅色")} icon={<Sun aria-hidden="true" />} onChange={(value) => update("appearance", value)} />
          <AppearanceChoice value="dark" current={draft.appearance} label={t("深色")} icon={<Moon aria-hidden="true" />} onChange={(value) => update("appearance", value)} />
          <AppearanceChoice value="system" current={draft.appearance} label={t("跟随系统")} icon={<Laptop aria-hidden="true" />} onChange={(value) => update("appearance", value)} />
        </div>
      </section>

      <section className="card settings-card">
        <div className="section-heading"><RefreshCw aria-hidden="true" /><h2>{t("自动本地备份")}</h2></div>
        <label className="checkbox-row"><input type="checkbox" checked={draft.automatic_backup_enabled} onChange={(event) => update("automatic_backup_enabled", event.target.checked)} /><span><strong>{t("启用当前用户计划任务")}</strong><small>{t("仅支持 Windows 11 x64 当前用户任务计划。")}</small></span></label>
        <label className="compact-field"><span>{t("频率（分钟）")}</span><input type="number" min="1" max="10080" value={draft.frequency_minutes} onChange={(event) => update("frequency_minutes", Number(event.target.value))} /></label>
        <div className="scheduler-state"><span>{t("计划任务状态")}</span><strong>{scheduler?.enabled ? t("已启用") : scheduler?.message ? t("无法读取计划任务") : t("未启用")}</strong></div>
      </section>

      <section className="card settings-card">
        <div className="section-heading"><HardDrive aria-hidden="true" /><h2>{t("本地数据位置")}</h2></div>
        <p className="help-text">{t("这些位置只用于本地备份；云端关闭时不会触发任何远端调用。")}</p>
        <div className="form-grid">
          <PathField label={t("Codex 数据位置")} value={draft.codex_home ?? ""} onChange={(value) => update("codex_home", value || null)} title={t("选择 Codex 数据位置")} placeholder="%USERPROFILE%\\.codex" onError={onError} />
          <PathField label={t("备份目录")} value={draft.local_repository ?? ""} onChange={(value) => update("local_repository", value || null)} title={t("选择备份目录")} placeholder="D:\\ENHE\\backups" onError={onError} />
        </div>
      </section>

      <section className="card settings-card">
        <div className="section-heading"><CloudUpload aria-hidden="true" /><h2>{t("云端备份（可选）")}</h2></div>
        <p className="help-text">{t("配置完成后才会产生远端调用；取消或失败不会影响本地备份。")}</p>
        <label className="checkbox-row cloud-toggle"><input type="checkbox" checked={draft.cloud.enabled} onChange={(event) => updateCloud({ enabled: event.target.checked })} /><span><strong>{t("启用云端备份")}</strong><small>{t("云端默认关闭；启用前需要完成 OneDrive 配置。")}</small></span></label>
        <div className="form-grid"><label><span>{t("OneDrive 远端名称")}</span><input value={draft.cloud.remote_name ?? ""} onChange={(event) => updateCloud({ remote_name: event.target.value })} placeholder="enhe-onedrive" /></label><label><span>{t("OneDrive 路径")}</span><input value={draft.cloud.remote_path ?? ""} onChange={(event) => updateCloud({ remote_path: event.target.value })} placeholder="ENHE/Codex Backup" /></label><label><span>{t("云端仓库路径")}</span><input value={draft.cloud.repository_path ?? ""} onChange={(event) => updateCloud({ repository_path: event.target.value })} placeholder="restic" /></label><label><span>{t("rclone 配置文件")}</span><input value={draft.cloud.config_file ?? ""} onChange={(event) => updateCloud({ config_file: event.target.value })} placeholder="%APPDATA%\\rclone\\rclone.conf" /></label></div>
        <div className="button-row"><button className="secondary-button" type="button" onClick={() => void configureCloud()} disabled={cloudBusy}><CloudUpload aria-hidden="true" />{t("开始 OneDrive 配置")}</button><button className="secondary-button" type="button" onClick={() => void testCloud()} disabled={cloudBusy || !draft.cloud.enabled}><CheckCircle2 aria-hidden="true" />{t("测试 OneDrive 连接")}</button></div>
        {cloudQuestion && <div className="cloud-question" role="group" aria-labelledby="cloud-question-title">
          <h3 id="cloud-question-title">{t("OneDrive 配置问题")}</h3>
          {cloudQuestion.name && <strong>{cloudQuestion.name}</strong>}
          {cloudQuestion.help && <p>{cloudQuestion.help}</p>}
          {cloudQuestion.error && <p className="inline-error" role="alert">{cloudQuestion.error}</p>}
          <label><span>{t("配置答案")}</span><input type={cloudQuestion.is_password ? "password" : "text"} value={cloudAnswer} onChange={(event) => setCloudAnswer(event.target.value)} autoComplete={cloudQuestion.is_password ? "new-password" : "off"} /></label>
          {cloudQuestion.examples.length > 0 && <div className="example-choices" aria-label={t("可选答案")}>{cloudQuestion.examples.map((example) => <button className="text-button" type="button" key={example} onClick={() => setCloudAnswer(example)}>{example}</button>)}</div>}
          <div className="button-row"><button className="primary-button" type="button" onClick={() => void continueCloud()} disabled={cloudBusy}>{cloudBusy ? <LoaderCircle className="spin" aria-hidden="true" /> : <CheckCircle2 aria-hidden="true" />}{t("提交配置答案")}</button><button className="text-button" type="button" onClick={() => { setCloudQuestion(null); setCloudAnswer(""); }}>{t("取消配置")}</button></div>
        </div>}
      </section>

      <section className="settings-footer"><button className="primary-button" type="button" onClick={() => { onConfigChange(draft); void onSave(draft); }}>{t("保存设置")}</button><span>{t("当前版本")} 0.1.1</span></section>
    </div>
  );
}

function AppearanceChoice({ value, current, label, icon, onChange }: { value: Appearance; current: Appearance; label: string; icon: React.ReactNode; onChange: (value: Appearance) => void }) {
  return <label className={current === value ? "choice appearance-choice selected" : "choice appearance-choice"}><input type="radio" name="appearance" checked={current === value} onChange={() => onChange(value)} />{icon}<span>{label}</span></label>;
}

function formatDate(value: string, locale: Locale): string {
  const date = new Date(value);
  if (Number.isNaN(date.valueOf())) return value;
  return new Intl.DateTimeFormat(locale === "en" ? "en-US" : "zh-CN", { dateStyle: "medium", timeStyle: "short" }).format(date);
}

function cloudResultMessage(message: string, translate: (key: string) => string): string {
  if (message === "OneDrive connection test completed") {
    return translate("OneDrive 连接测试已完成");
  }
  if (message === "local snapshot copied to OneDrive") {
    return translate("本地快照已复制到 OneDrive");
  }
  return message;
}
