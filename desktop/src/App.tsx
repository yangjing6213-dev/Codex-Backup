import { useCallback, useEffect, useRef, useState, type RefObject } from "react";
import {
  Archive,
  ArrowDown,
  BookOpen,
  CheckCircle2,
  CloudOff,
  CloudUpload,
  Database,
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
  ShieldCheck,
  Sun,
  TriangleAlert,
  UserRound,
} from "lucide-react";

import ReceivePage from "./features/receive/ReceivePage";
import HistoryPage from "./features/history/HistoryPage";
import SendPage from "./features/send/SendPage";
import { BackupResultSummary, backupResultTone } from "./features/backup/BackupResultSummary";
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
  countProjectFiles,
  cancelLocalDiscovery,
  requestAdminLocalDiscovery,
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
  type LocalRestoreReport,
  type LocalDiscoveryResult,
  type LocalProjectCandidate,
  type SchedulerStatus,
} from "./lib/types";
import "./App.css";

export type View = "overview" | "projects" | "data" | "backups" | "settings" | "guide" | "about";
type BackupView = "local" | "export" | "import" | "history";
type LocalScanState = "idle" | "running" | "complete" | "partial" | "failed";
type OperationStatus = "idle" | "running" | "success" | "partial" | "failed";
type ProjectFileCounts = Record<string, LocalProjectCandidate | "counting" | "failed">;
type ProjectRow = { path: string; name: string; label: string; available: boolean; current: boolean; count?: LocalProjectCandidate };

const views: Array<{
  id: View;
  label: string;
  accessibleLabel: string;
  icon: typeof HardDrive;
}> = [
  { id: "overview", label: "概览", accessibleLabel: "前往概览", icon: HardDrive },
  { id: "projects", label: "项目", accessibleLabel: "前往项目", icon: FolderKanban },
  { id: "data", label: "数据", accessibleLabel: "前往数据", icon: Database },
  { id: "backups", label: "备份与迁移", accessibleLabel: "前往备份与迁移", icon: Archive },
  { id: "settings", label: "设置", accessibleLabel: "前往设置", icon: Settings2 },
  { id: "guide", label: "操作说明", accessibleLabel: "前往操作说明", icon: BookOpen },
  { id: "about", label: "关于作者", accessibleLabel: "前往关于作者", icon: UserRound },
];

const viewTitles: Record<View, string> = {
  overview: "概览",
  projects: "项目",
  data: "数据",
  backups: "备份与迁移",
  settings: "设置",
  guide: "操作说明",
  about: "关于作者",
};

function defaultConfig(): AppConfig {
  return {
    config_version: 1,
    codex_home: null,
    local_repository: null,
    selected_project_paths: [],
    automatic_project_scan: true,
    project_scan_roots: [],
    project_selection_initialized: false,
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
  const [projectFileCounts, setProjectFileCounts] = useState<ProjectFileCounts>({});
  const requestedCounts = useRef(new Set<string>());
  const countQueue = useRef(new Map<string, string>());
  const countRunning = useRef(false);
  const countGeneration = useRef(0);
  const scanGeneration = useRef(0);
  const requestProjectCounts = useCallback((paths: string[]) => {
    const pending = paths.filter(path => !requestedCounts.current.has(pathKey(path)));
    if (!pending.length) return;
    pending.forEach(path => {
      requestedCounts.current.add(pathKey(path));
      countQueue.current.set(pathKey(path), path);
    });
    setProjectFileCounts(current => ({ ...current, ...Object.fromEntries(pending.map(path => [pathKey(path), "counting" as const])) }));
    if (countRunning.current) return;
    countRunning.current = true;
    void (async () => {
      try {
        while (countQueue.current.size) {
          const batch = [...countQueue.current.values()];
          countQueue.current.clear();
          const generation = countGeneration.current;
          const failed = Object.fromEntries(batch.map(path => [pathKey(path), "failed" as const]));
          try {
            const results = await countProjectFiles(batch);
            if (generation === countGeneration.current) setProjectFileCounts(current => ({ ...current, ...failed, ...Object.fromEntries(results.map(result => [pathKey(result.path), result])) }));
          } catch {
            if (generation === countGeneration.current) setProjectFileCounts(current => ({ ...current, ...failed }));
          }
        }
      } finally {
        countRunning.current = false;
      }
    })();
  }, []);
  const [scheduler, setSchedulerStatus] = useState<SchedulerStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [codexScanFailed, setCodexScanFailed] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [activeOperations, setActiveOperations] = useState(0);
  const [backupStatus, setBackupStatus] = useState<OperationStatus>("idle");
  const [migrationStatus, setMigrationStatus] = useState<OperationStatus>("idle");
  const headingRef = useRef<HTMLHeadingElement>(null);
  const backupHeadingRef = useRef<HTMLHeadingElement>(null);
  const configRef = useRef(config);
  const configLoadPromise = useRef<Promise<AppConfig> | null>(null);
  const configLoadResolved = useRef(false);
  const dataRequestGeneration = useRef(0);
  configRef.current = config;
  const previousViewRef = useRef(view);

  const loadConfig = useCallback(() => {
    if (!configLoadPromise.current) {
      configLoadPromise.current = getAppConfig().then((saved) => {
        const loaded = { ...defaultConfig(), ...saved };
        configRef.current = loaded;
        configLoadResolved.current = true;
        return loaded;
      });
    }
    return configLoadPromise.current;
  }, []);

  useEffect(() => {
    let active = true;
    const generation = scanGeneration.current;
    const isCurrent = () => active && generation === scanGeneration.current;
    async function initialize() {
      let loaded = defaultConfig();
      let fastDiscoverySucceeded = false;
      try {
        loaded = await loadConfig();
        if (isCurrent()) {
          setConfig(loaded);
          setLocale(loaded.locale);
        }
      } catch (caught) {
        if (isCurrent()) setError(errorMessage(caught, t));
      }

      if (!isCurrent()) {
        if (active) setLoading(false);
        return;
      }
      try {
        const detected = await discoverCodex(loaded.codex_home);
        fastDiscoverySucceeded = true;
        if (isCurrent()) {
          setInventory(detected);
          const next = mergeAutomaticConfig(loaded, detected);
          setConfig(next);
          let saveFailed = false;
          if (hasConfigChanges(loaded, next)) {
            try {
              const saved = await saveAppConfig(next);
              if (isCurrent()) setConfig(saved);
            } catch (caught) {
              saveFailed = true;
              if (isCurrent()) setError(errorMessage(caught, t));
            }
          }
          if (isCurrent() && !saveFailed) {
            setError(null);
            setCodexScanFailed(false);
          }
        }
      } catch (caught) {
        if (isCurrent()) {
          setCodexScanFailed(true);
          setError(errorMessage(caught, t));
        }
      } finally {
        if (active) setLoading(false);
      }

      if (!isCurrent() || !loaded.automatic_project_scan) return;
      setLocalScanState("running");
      try {
        const result = await discoverLocalCandidates(loaded.project_scan_roots);
        if (isCurrent()) {
          setLocalDiscovery(result);
          setLocalScanState(localScanStateFor(result));

          if (!fastDiscoverySucceeded && result.codex_homes.length > 0) {
            try {
              const detected = await discoverCodex(result.codex_homes[0]);
              if (!isCurrent()) return;
              const next = mergeAutomaticConfig(
                { ...configRef.current, codex_home: result.codex_homes[0] },
                detected,
              );
              setInventory(detected);
              setConfig(next);
              let saveFailed = false;
              if (hasConfigChanges(loaded, next)) {
                try {
                  const saved = await saveAppConfig(next);
                  if (isCurrent()) setConfig(saved);
                } catch (caught) {
                  saveFailed = true;
                  if (isCurrent()) setError(errorMessage(caught, t));
                }
              }
              if (isCurrent()) {
                setCodexScanFailed(false);
                if (!saveFailed) setError(null);
              }
            } catch (caught) {
              if (isCurrent()) {
                setCodexScanFailed(true);
                setError(errorMessage(caught, t));
              }
            }
          }
        }
      } catch (caught) {
        if (isCurrent()) {
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
  }, [loadConfig]);

  useEffect(() => {
    if (previousViewRef.current !== view) {
      (view === "backups" ? backupHeadingRef : headingRef).current?.focus();
      document.documentElement.scrollTop = 0;
      previousViewRef.current = view;
    }
  }, [view]);

  useEffect(() => {
    document.documentElement.dataset.appearance = config.appearance;
  }, [config.appearance]);

  const persistConfig = useCallback(
    async (next: AppConfig, message = "设置已保存") => {
      const saved = await saveAppConfig(next);
      configRef.current = saved;
      setConfig(saved);
      setLocale(saved.locale);
      setNotice(message);
      return saved;
    },
    [setLocale],
  );

  async function persistProjectConfig(next: AppConfig, message: string) {
    const configWasReady = configLoadResolved.current;
    await loadConfig();
    return persistConfig({
      ...configRef.current,
      automatic_project_scan: configWasReady ? next.automatic_project_scan : configRef.current.automatic_project_scan,
      project_scan_roots: next.project_scan_roots,
      selected_project_paths: configWasReady ? next.selected_project_paths : configRef.current.selected_project_paths,
      project_selection_initialized: configWasReady ? next.project_selection_initialized : configRef.current.project_selection_initialized,
    }, message);
  }

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

  async function rescanProjects(next: AppConfig, administrator: boolean) {
    if (localScanState === "running") return;
    const generation = ++scanGeneration.current;
    setError(null);
    setNotice(null);
    setLocalScanState("running");
    try {
      const saved = await persistProjectConfig(next, t("项目选择已保存"));
      if (generation !== scanGeneration.current) return;
      countGeneration.current += 1;
      requestedCounts.current.clear();
      countQueue.current.clear();
      setProjectFileCounts({});
      setLocalDiscovery(null);
      const result = administrator ? await requestAdminLocalDiscovery() : await discoverLocalCandidates(saved.project_scan_roots);
      if (generation !== scanGeneration.current) return;
      setLocalDiscovery(result);
      setLocalScanState(localScanStateFor(result));
      if (administrator) setNotice(t("管理员扫描已完成"));
    } catch (caught) {
      if (generation !== scanGeneration.current) return;
      setLocalScanState("failed");
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
          <img className="brand-mark" src="/app-icon.png" alt="" />
          <span className="brand-copy"><small className="brand-version">v0.1.9</small><strong>ENHE</strong><small>Codex Backup</small></span>
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

      <main className="workspace" data-view={view} aria-busy={loading}>
        <header className="topbar">
          <span className="topbar-title">{t(viewTitles[view])}</span>
          {loading ? (
            <span className="machine-status"><LoaderCircle className="spin" aria-hidden="true" />{t("本机扫描中")}</span>
          ) : error ? (
            <span className="machine-status machine-error"><TriangleAlert aria-hidden="true" />{t(codexScanFailed ? "本机扫描失败" : "操作未完成")}</span>
          ) : (
            <span className="machine-status"><CheckCircle2 aria-hidden="true" />{t("本机已就绪")}</span>
          )}
        </header>

        {notice && <div className="notice" role="status">{notice}<button type="button" onClick={() => setNotice(null)} aria-label={t("关闭")}>×</button></div>}
        {error && <div className="global-error" role="alert"><span>{error}</span>{codexScanFailed && <button className="text-button" type="button" onClick={() => void chooseCodexHome()}>{t("选择 Codex 数据位置")}</button>}</div>}
        {activeOperations > 0 && <div className="notice" role="status"><span><LoaderCircle className="spin" aria-hidden="true" /> {t("任务进行中，切换页面不会中断；请勿退出应用。")}</span><button type="button" onClick={() => setView("backups")}>{t("查看进行中的任务")}</button></div>}

        {view === "overview" && (
          <OverviewPage headingRef={headingRef} inventory={inventory} localDiscovery={localDiscovery} config={config} scheduler={scheduler} localScanState={localScanState} codexScanFailed={codexScanFailed} loading={loading} backupStatus={backupStatus} migrationStatus={migrationStatus} onNavigate={setView} />
        )}
        {view === "projects" && (
          <ProjectsPage
            headingRef={headingRef}
            inventory={inventory}
            localCandidates={localDiscovery?.candidates ?? []}
            fileCounts={projectFileCounts}
            onCountFiles={requestProjectCounts}
            localWarnings={localDiscovery?.warnings ?? []}
            permissionDeniedCount={localDiscovery?.permission_denied_count ?? 0}
            otherWarningCount={localDiscovery?.other_warning_count ?? 0}
            scanLimitReached={localDiscovery?.scan_limit_reached ?? false}
            scannedRootCount={localDiscovery?.scanned_roots.length ?? 0}
            localScanState={localScanState}
            onCancelLocalScan={() => { void cancelLocalDiscovery(); }}
            onRescan={rescanProjects}
            config={config}
            onSave={async (next) => {
              try {
                await persistProjectConfig(next, t("项目选择已保存"));
              } catch (caught) {
                setError(errorMessage(caught, t));
              }
            }}
            onError={setError}
          />
        )}
        {view === "data" && (
          <DataPage
            headingRef={headingRef}
            inventory={inventory}
            config={config}
            onSave={async (next) => {
              const generation = ++dataRequestGeneration.current;
              try {
                const detected = await discoverCodex(next.codex_home);
                if (generation !== dataRequestGeneration.current) return;
                await persistConfig({ ...configRef.current, codex_home: detected.codex_home }, t("数据设置已保存"));
                if (generation !== dataRequestGeneration.current) return;
                setInventory(detected);
                setCodexScanFailed(false);
                setError(null);
              } catch (caught) {
                if (generation === dataRequestGeneration.current) setError(errorMessage(caught, t));
              }
            }}
            onError={setError}
          />
        )}
        {!loading && <div hidden={view !== "backups"}>
          <BackupsPage
            headingRef={backupHeadingRef}
            visible={view === "backups"}
            operationBusy={activeOperations > 0}
            inventory={inventory}
            config={config}
            onRepositoryChange={async (repository) => { await persistConfig({ ...configRef.current, local_repository: repository }, t("设置已保存")); }}
            onNavigate={setView}
            onOperationStart={operationStarted}
            onOperationEnd={operationFinished}
            onBackupStatusChange={setBackupStatus}
            onMigrationStatusChange={setMigrationStatus}
            onNotice={setNotice}
            onError={setError}
          />
        </div>}
        {view === "settings" && (
          <fieldset className="migration-control-guard" disabled={activeOperations > 0}>
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
          </fieldset>
        )}
        {view === "guide" && <GuidePage headingRef={headingRef} />}
        {view === "about" && <AboutPage headingRef={headingRef} />}
      </main>
    </div>
  );
}

interface OverviewPageProps {
  headingRef: RefObject<HTMLHeadingElement | null>;
  inventory: CodexInventory | null;
  localDiscovery: LocalDiscoveryResult | null;
  config: AppConfig;
  scheduler: SchedulerStatus | null;
  localScanState: LocalScanState;
  codexScanFailed: boolean;
  loading: boolean;
  backupStatus: OperationStatus;
  migrationStatus: OperationStatus;
  onNavigate: (view: View) => void;
}

function OverviewPage({ headingRef, inventory, localDiscovery, config, scheduler, localScanState, codexScanFailed, loading, backupStatus, migrationStatus, onNavigate }: OverviewPageProps) {
  const { t } = useI18n();
  const candidates = localDiscovery?.candidates ?? [];
  const counted = localDiscovery !== null && localScanState === "complete" && candidates.every(candidate => candidate.file_count_complete);
  const fileCount = candidates.reduce((sum, candidate) => sum + (candidate.file_count ?? 0), 0);
  const projectStatus = operationStatusForScan(localScanState);
  const dataStatus: OperationStatus = loading ? "running" : codexScanFailed ? "failed" : inventory ? "success" : "idle";
  return (
    <div className="page">
      <header className="page-header">
        <p className="eyebrow">ENHE CODEX BACKUP</p>
        <h1 ref={headingRef} tabIndex={-1}>{t("Codex 数据备份&迁移")}</h1>
        <p className="page-description">{t("没有云端也能备份、查看和恢复资料。")}</p>
      </header>

      <section className="overview-actions" aria-label={t("备份快捷操作")}>
        <button className="secondary-button overview-action" type="button" aria-label={t("项目备份设置")} onClick={() => onNavigate("projects")}>
          <FolderKanban aria-hidden="true" />
          <span className="overview-action-copy"><strong>{t("项目备份设置")}</strong><small>{t("先确认要备份的项目")}</small></span>
        </button>
        <button className="secondary-button overview-action" type="button" aria-label={t("数据备份设置")} onClick={() => onNavigate("data")}>
          <Database aria-hidden="true" />
          <span className="overview-action-copy"><strong>{t("数据备份设置")}</strong><small>{t("确认 Codex 对话和数据")}</small></span>
        </button>
        <button className="primary-button overview-action" type="button" aria-label={t("开始本地备份")} onClick={() => onNavigate("backups")}>
          <LockKeyhole aria-hidden="true" />
          <span className="overview-action-copy"><strong>{t("开始本地备份")}</strong><small>{t("完成设置后开始")}</small></span>
        </button>
      </section>

      <section className="overview-status-grid" aria-label={t("备份状态") }>
        <OverviewStatusCard icon={FolderKanban} title={t("项目扫描")} status={t(operationStatusKey("project", projectStatus))} state={projectStatus} actionLabel={t("查看项目")} onAction={() => onNavigate("projects")} />
        <OverviewStatusCard icon={Database} title={t("Codex 数据扫描")} status={t(operationStatusKey("data", dataStatus))} state={dataStatus} actionLabel={t("查看数据")} onAction={() => onNavigate("data")} />
        <OverviewStatusCard icon={LockKeyhole} title={t("本地备份")} status={t(operationStatusKey("backup", backupStatus))} state={backupStatus} actionLabel={t("查看备份与迁移")} onAction={() => onNavigate("backups")} />
        <OverviewStatusCard icon={RotateCcw} title={t("迁移/恢复")} status={t(operationStatusKey("migration", migrationStatus))} state={migrationStatus} actionLabel={t("查看备份与迁移")} onAction={() => onNavigate("backups")} />
      </section>

      <section className="status-banner local-banner">
        <div className="status-icon"><HardDrive aria-hidden="true" /></div>
        <div><strong>{t("云端备份已关闭")}</strong><p>{t("云端关闭时不会启动远端连接，也不会上传资料。")}</p></div>
      </section>

      <section className="metric-grid" aria-label={t("本机检测")}>
        <Metric label={t("扫描项目数")} value={localDiscovery ? candidates.length : t("未扫描")} text={!localDiscovery} />
        <Metric label={t("对话总数")} value={inventory?.counts.conversations ?? localDiscovery?.conversation_count ?? 0} />
        <Metric label={t("扫描文件数")} value={counted ? fileCount.toLocaleString() : localDiscovery ? t("至少 {count}", { count: fileCount.toLocaleString() }) : t(localScanState === "running" ? "正在统计" : "未扫描")} text />
        <Metric label={t("计划任务状态")} value={scheduler?.enabled ? t("已启用") : t("未启用")} text />
      </section>

      <section className="metric-grid overview-data-metrics" aria-label={t("数据扫描结果")}>
        <Metric label={t("对话总数")} value={inventory?.counts.conversations ?? 0} />
        <Metric label={t("技能")} value={inventory?.counts.skills ?? 0} />
        <Metric label={t("插件")} value={inventory?.counts.plugins ?? 0} />
        <Metric label={t("生成图片")} value={inventory?.counts.generated_images ?? 0} />
      </section>

      <section className="card two-column">
        <div>
          <div className="section-heading"><LockKeyhole aria-hidden="true" /><h2>{t("完整本地备份")}</h2></div>
          <p>{t("选择的项目会与 Git 元数据、未提交修改和 worktree 一起进入完整备份。")}</p>
          <div className="path-value"><span>{t("Codex 数据位置")}</span><code>{config.codex_home ?? t("未检测")}</code></div>
          <div className="path-value"><span>{t("备份目录")}</span><code>{config.local_repository ?? t("未检测")}</code></div>
          <p className="help-text">{t("备份目录用于存放加密备份仓库，不是待备份的项目目录；恢复时请选择同一个仓库。")}</p>
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

interface DataPageProps {
  headingRef: RefObject<HTMLHeadingElement | null>;
  inventory: CodexInventory | null;
  config: AppConfig;
  onSave: (config: AppConfig) => Promise<void>;
  onError: (message: string | null) => void;
}

function DataPage({ headingRef, inventory, config, onSave, onError }: DataPageProps) {
  const { t } = useI18n();
  const [draft, setDraft] = useState(config);

  useEffect(() => {
    setDraft(config);
  }, [config]);

  return (
    <div className="page">
      <header className="page-header page-header-with-action">
        <div><p className="eyebrow">DATA</p><h1 ref={headingRef} tabIndex={-1}>{t("数据")}</h1><p className="page-description">{t("集中管理 Codex 对话、索引和相关数据的备份来源。")}</p></div>
        <button className="primary-button" type="button" onClick={() => void onSave(draft)}>{t("保存数据设置")}</button>
      </header>

      <section className="card settings-card">
        <div className="section-heading"><Database aria-hidden="true" /><h2>{t("Codex 数据")}</h2></div>
        <p className="help-text">{t("此路径是要读取和备份的 Codex 数据来源，不是备份文件保存目录。")}</p>
        <PathField label={t("Codex 数据位置")} value={draft.codex_home ?? ""} onChange={(value) => setDraft((current) => ({ ...current, codex_home: value || null }))} title={t("选择 Codex 数据位置")} placeholder="%USERPROFILE%\\.codex" onError={onError} />
      </section>

      <section className="card settings-card">
        <div className="section-heading"><HardDrive aria-hidden="true" /><h2>{t("已发现的数据")}</h2></div>
        <p className="help-text">{t("备份会读取下列 Codex 内容；项目文件请在项目页面选择。")}</p>
        <div className="metric-grid data-metric-grid">
          <Metric label={t("对话总数")} value={inventory?.counts.conversations ?? 0} />
          <Metric label={t("技能")} value={inventory?.counts.skills ?? 0} />
          <Metric label={t("插件")} value={inventory?.counts.plugins ?? 0} />
          <Metric label={t("生成图片")} value={inventory?.counts.generated_images ?? 0} />
        </div>
        <div className="path-value"><span>{t("会话索引")}</span><code>{inventory?.session_index_path ?? t("未检测")}</code></div>
        <div className="path-value"><span>{t("状态数据库")}</span><code>{inventory?.state_db_path ?? t("未检测")}</code></div>
      </section>
    </div>
  );
}

function Metric({ label, value, text = false }: { label: string; value: number | string; text?: boolean }) {
  return <div className="metric"><span>{label}</span><strong className={text ? "metric-text" : undefined}>{value}</strong></div>;
}

function OverviewStatusCard({ icon: Icon, title, status, state, actionLabel, onAction }: { icon: typeof CheckCircle2; title: string; status: string; state: OperationStatus; actionLabel: string; onAction: () => void }) {
  return (
    <section className="overview-status-card" data-state={state} role="status" aria-label={status}>
      <div className="overview-status-copy"><Icon aria-hidden="true" /><span><strong>{title}</strong><small>{status}</small></span></div>
      <button className="text-button" type="button" onClick={onAction}>{actionLabel}</button>
    </section>
  );
}

function operationStatusForScan(state: LocalScanState): OperationStatus {
  return state === "complete" ? "success" : state === "partial" ? "partial" : state === "failed" ? "failed" : state === "running" ? "running" : "idle";
}

function operationStatusKey(kind: "project" | "data" | "backup" | "migration", state: OperationStatus): string {
  const labels: Record<"project" | "data" | "backup" | "migration", Record<OperationStatus, string>> = {
    project: {
      idle: "本机项目扫描尚未开始",
      running: "正在扫描本机项目",
      success: "本机项目扫描已完成",
      partial: "本机项目扫描已部分完成",
      failed: "本机项目扫描失败",
    },
    data: {
      idle: "本机Codex数据扫描尚未开始",
      running: "本机Codex数据扫描正在进行",
      success: "本机Codex数据扫描已完成",
      partial: "本机Codex数据扫描已部分完成",
      failed: "本机Codex数据扫描失败",
    },
    backup: {
      idle: "本地备份尚未完成",
      running: "本地备份进行中",
      success: "本地备份已完成",
      partial: "本地备份已部分完成",
      failed: "本地备份失败",
    },
    migration: {
      idle: "迁移/恢复尚未完成",
      running: "迁移/恢复进行中",
      success: "迁移/恢复已完成",
      partial: "迁移/恢复已部分完成",
      failed: "迁移/恢复失败",
    },
  };
  return labels[kind][state];
}

function operationStatusForTone(tone: ReturnType<typeof backupResultTone>): OperationStatus {
  return tone === "success" ? "success" : tone === "warning" ? "partial" : "failed";
}

function localScanStateFor(result: LocalDiscoveryResult): LocalScanState {
  return result.cancelled || result.permission_denied_count > 0 || result.other_warning_count > 0 || result.warnings.length > 0 || result.candidates.some(candidate => candidate.file_count_complete === false)
    ? "partial"
    : "complete";
}

function GuidePage({ headingRef }: { headingRef: RefObject<HTMLHeadingElement | null> }) {
  const { t } = useI18n();
  const steps = [
    ["自动扫描并确认路径", "启动后自动发现可访问的 Codex 数据、对话和项目；无权限目录会安全跳过并汇总。"],
    ["选择项目", "在项目页勾选要备份的项目，也可以用文件夹按钮加入普通目录。"],
    ["设置备份目录和密码", "在备份与迁移页选择本地目录并设置恢复密码；云端默认关闭。"],
    ["开始本地备份并检查", "应用用 restic 创建加密快照，完成后可以刷新列表查看文件数量和状态。"],
    ["恢复或离线迁移", "恢复到新的独立目录；跨设备时携带仓库目录和密码，或使用 ReHome 迁移包。"],
  ] as const;

  return (
    <div className="page">
      <header className="page-header">
        <p className="eyebrow">HOW IT WORKS</p>
        <h1 ref={headingRef} tabIndex={-1}>{t("操作说明")}</h1>
        <p className="page-description">{t("按流程完成本地扫描、备份、检查和恢复；每一步都由你确认。")}</p>
      </header>
      <section className="card guide-card" aria-label={t("操作流程")}>
        <ol className="flowchart-list" aria-label={t("操作流程")}>
          {steps.map(([title, description], index) => (
            <li className="flowchart-step" key={title}>
              <div className="flowchart-node" aria-hidden="true">{index + 1}</div>
              <div className="flowchart-copy"><h2>{t(title)}</h2><p>{t(description)}</p></div>
              {index < steps.length - 1 && <ArrowDown className="flowchart-arrow" aria-hidden="true" />}
            </li>
          ))}
        </ol>
      </section>
      <section className="card guide-safety">
        <div className="section-heading"><ShieldCheck aria-hidden="true" /><h2>{t("安全提示")}</h2></div>
        <p>{t("本地优先模式不需要登录或云端配置；管理员权限只有在你勾选并点击重新扫描时才会请求。")}</p>
      </section>
      {[
        ["恢复到本机", [
          ["打开原备份仓库", "进入备份与迁移的本地备份页，选择存放加密备份的目录，输入创建备份时的恢复密码。"],
          ["刷新并选择快照", "点击刷新本地备份，按日期选择要恢复的快照；部分完成的备份可能缺少文件。"],
          ["选择空目录并恢复", "填写新的恢复目标目录，再点击快照旁的恢复。原有项目和正在使用的 Codex 不会被覆盖。"],
          ["检查恢复内容", "打开结果显示的 backup-… 目录。projects 保存项目，codex 保存 Codex 数据；保留整个目录及 git-metadata，避免破坏 worktree 关联。"],
        ]],
        ["迁移到另一台设备", [
          ["携带整个仓库和密码", "等待备份完成后，将整个备份目录复制到移动硬盘或新设备；不能只复制快照中的个别文件。恢复密码请另行妥善保管。"],
          ["安装并按本机流程恢复", "在新设备安装应用，不用登录、不用配置云端。选择带来的仓库，输入原密码，刷新列表并恢复到空目录。"],
          ["恢复文件不等于续接会话", "先验证项目文件和 Git。恢复出的 Codex 数据是独立副本，不会自动替换新设备的真实资料；原会话续接需要另行验证。"],
        ]],
      ].map(([title, flow]) => <section className="card guide-card" key={title as string}>
        <h2>{t(title as string)}</h2>
        <ol className="flowchart-list restore-flow">
          {(flow as string[][]).map(([step, description], index) => <li className="flowchart-step" key={step}>
            <div className="flowchart-node" aria-hidden="true">{index + 1}</div>
            <div className="flowchart-copy"><h3>{t(step)}</h3><p>{t(description)}</p></div>
            {index < (flow as string[][]).length - 1 && <ArrowDown className="flowchart-arrow" aria-hidden="true" />}
          </li>)}
        </ol>
      </section>)}
      <p className="help-text">{t("完整迁移优先使用本地备份仓库。ReHome 迁移包用于选择性导入导出，不能替代保留 Git/worktree 的完整备份。")}</p>
    </div>
  );
}

function AboutPage({ headingRef }: { headingRef: RefObject<HTMLHeadingElement | null> }) {
  const { t } = useI18n();
  return (
    <div className="page">
      <header className="page-header">
        <p className="eyebrow">ABOUT ENHE</p>
        <h1 ref={headingRef} tabIndex={-1}>{t("关于作者")}</h1>
        <p className="page-description">{t("用AI打造一个人公司。")}</p>
      </header>
      <section className="card author-card" aria-label={t("关于作者")}>
        <img className="author-image" src="/author-enhe.png" alt="Enhe（恩禾）" />
        <div className="author-profile">
          <div>
            <p className="eyebrow">ENHE</p>
            <h2>Enhe（恩禾）</h2>
            <p className="author-role">{t("产品设计师 · 一人公司实践者 · AI Builder")}</p>
            <p>{t("用AI打造一个人公司。")}</p>
          </div>
          <div className="author-contact">
            <h3>{t("联系方式")}</h3>
            <dl>
              <div><dt>GitHub</dt><dd><a href="https://github.com/yangjing6213-dev" target="_blank" rel="noreferrer">yangjing6213-dev</a></dd></div>
              <div><dt>X / Twitter</dt><dd><a href="https://x.com/Amenenhe_ai" target="_blank" rel="noreferrer">Amenenhe_ai</a></dd></div>
              <div><dt>{t("网站")}</dt><dd><a href="https://www.enhe-tech.com.cn/" target="_blank" rel="noreferrer">www.enhe-tech.com.cn</a></dd></div>
              <div><dt>{t("微信")}</dt><dd>Hu-Amen</dd></div>
              <div><dt>{t("邮箱")}</dt><dd><a href="mailto:amen.enhe@gmail.com">amen.enhe@gmail.com</a></dd></div>
            </dl>
          </div>
        </div>
      </section>
    </div>
  );
}

interface ProjectsPageProps {
  headingRef: RefObject<HTMLHeadingElement | null>;
  inventory: CodexInventory | null;
  localCandidates: LocalProjectCandidate[];
  fileCounts: ProjectFileCounts;
  onCountFiles: (paths: string[]) => void;
  localWarnings: string[];
  permissionDeniedCount: number;
  otherWarningCount: number;
  scanLimitReached: boolean;
  scannedRootCount: number;
  localScanState: LocalScanState;
  onCancelLocalScan: () => void;
  onRescan: (config: AppConfig, administrator: boolean) => Promise<void>;
  config: AppConfig;
  onSave: (config: AppConfig) => Promise<void>;
  onError: (message: string | null) => void;
}

function ProjectsPage({ headingRef, inventory, localCandidates, fileCounts, onCountFiles, localWarnings, permissionDeniedCount, otherWarningCount, scanLimitReached, scannedRootCount, localScanState, onCancelLocalScan, onRescan, config, onSave, onError }: ProjectsPageProps) {
  const { t } = useI18n();
  const [selected, setSelected] = useState(() => new Set(config.selected_project_paths.map(pathKey)));
  const [manualPath, setManualPath] = useState("");
  const [manualPaths, setManualPaths] = useState<string[]>([]);
  const savedSelected = new Set(config.selected_project_paths.map(pathKey));
  const [automaticScan, setAutomaticScan] = useState(config.automatic_project_scan);
  const [scanRoots, setScanRoots] = useState(config.project_scan_roots.join("\n"));
  const [requestAdmin, setRequestAdmin] = useState(false);
  const scanBusy = localScanState === "running";
  const scanConfigured = scanRoots.split(/\r?\n/).some((path) => path.trim());
  const rows = new Map<string, ProjectRow>();
  const folderName = (path: string) => displayPath(path).replace(/[\\/]+$/, "").split(/[\\/]/).pop() || displayPath(path);
  for (const project of inventory?.projects ?? []) {
    const key = pathKey(project.source_path);
    if (!savedSelected.has(key) && !manualPaths.some((path) => pathKey(path) === key)) continue;
    rows.set(key, { path: displayPath(project.source_path), name: folderName(project.source_path), label: folderName(project.source_path), available: project.source_available, current: false });
  }
  for (const candidate of localCandidates) {
    const key = pathKey(candidate.path);
    rows.set(key, { path: displayPath(candidate.path), name: candidate.name, label: rows.get(key)?.label ?? displayPath(candidate.path), available: true, current: true, count: candidate });
  }
  for (const path of [...manualPaths, ...config.selected_project_paths]) {
    const key = pathKey(path);
    if (!rows.has(key)) rows.set(key, { path: displayPath(path), name: folderName(path), label: displayPath(path), available: true, current: false });
  }
  const groupedRows = new Map<string, { label: string; order: number; rows: Array<[string, ProjectRow]> }>();
  for (const entry of [...rows].sort(([, left], [, right]) => left.path.localeCompare(right.path, undefined, { sensitivity: "base" }))) {
    const normalized = displayPath(entry[1].path);
    const drive = /^([a-z]):(?:\\|$)/i.exec(normalized);
    const location = drive
      ? { key: `drive:${drive[1].toUpperCase()}`, label: t("{letter} 盘 ({drive})", { letter: drive[1].toUpperCase(), drive: `${drive[1].toUpperCase()}:` }), order: drive[1].toUpperCase().charCodeAt(0) }
      : normalized.startsWith("\\\\")
        ? { key: "network", label: t("网络位置"), order: 1000 }
        : { key: "other", label: t("其他位置"), order: 1001 };
    const group = groupedRows.get(location.key) ?? { label: location.label, order: location.order, rows: [] };
    group.rows.push(entry);
    groupedRows.set(location.key, group);
  }
  const pathsToCount = JSON.stringify([...rows].filter(([key, row]) => row.available && row.count?.file_count === undefined && !fileCounts[key]).map(([, row]) => row.path));
  useEffect(() => { onCountFiles(JSON.parse(pathsToCount) as string[]); }, [pathsToCount, onCountFiles]);
  function countLabel(key: string, row: { available: boolean; count?: LocalProjectCandidate }) {
    if (!row.available) return t("目录不可访问，无法统计");
    const result = row.count?.file_count !== undefined ? row.count : fileCounts[key];
    if (result === "failed") return t("统计失败，请重新扫描");
    if (!result || result === "counting") return t("正在统计文件…");
    return result.file_count_complete ? `${result.file_count.toLocaleString()} ${t("文件")}` : t("已统计 {count} 文件 · 部分统计，跳过 {skipped} 项", { count: result.file_count.toLocaleString(), skipped: result.skipped_entries });
  }
  async function chooseScanRoot() {
    onError(null);
    try {
      const selectedPath = await pickDirectory(t("选择项目根目录"));
      if (!selectedPath) return;
      setScanRoots((current) => {
        const paths = current.split(/\r?\n/).map((path) => displayPath(path.trim())).filter(Boolean);
        const key = pathKey(selectedPath);
        if (paths.some((path) => pathKey(path) === key)) return paths.join("\n");
        return [...paths, displayPath(selectedPath)].join("\n");
      });
    } catch (caught) {
      onError(errorMessage(caught, t));
    }
  }
  const selectAllCurrent = () => {
    setSelected((current) => {
      const next = new Set(current);
      localCandidates.forEach((candidate) => next.add(pathKey(candidate.path)));
      return next;
    });
  };
  const renderProjectRow = ([key, row]: [string, ProjectRow]) => (
    <label className="project-row" key={key}>
      <input type="checkbox" checked={selected.has(key)} onChange={() => toggleProject(key)} aria-label={`${t("选择项目")} ${row.label}`} disabled={!row.available && !selected.has(key)} />
      <span className="project-copy"><strong>{row.name}</strong><code>{row.path}</code>{!row.current && row.available && <small>{t("保留的手动或已选目录")}</small>}{!row.available && <small>{t("目录当前不可访问；可以取消选择。")}</small>}</span>
      <span className="project-meta" role="status">{countLabel(key, row)}</span>
    </label>
  );
  useEffect(() => {
    setSelected(new Set(config.selected_project_paths.map(pathKey)));
  }, [config.selected_project_paths]);
  const toggleProject = (path: string) => {
    setSelected((current) => {
      const next = new Set(current);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
  };
  const nextConfig = (): AppConfig => ({ ...config, automatic_project_scan: automaticScan, project_scan_roots: scanRoots.split(/\r?\n/).map((path) => displayPath(path.trim())).filter(Boolean), selected_project_paths: [...rows].filter(([key]) => selected.has(key)).map(([, row]) => row.path), project_selection_initialized: true });
  return (
    <div className="page">
      <header className="page-header page-header-with-action">
        <div><p className="eyebrow">PROJECTS</p><h1 ref={headingRef} tabIndex={-1}>{t("项目")}</h1><p className="page-description">{t("选择的项目会与 Git 元数据、未提交修改和 worktree 一起进入完整备份。")}</p></div>
        <button className="primary-button" type="button" disabled={scanBusy} onClick={() => void onSave(nextConfig())}>{t("保存项目选择")}</button>
      </header>
      <section className="card project-list" aria-label={t("项目文件夹")}>
        <div className="scan-settings">
          <label className="checkbox-row"><input type="checkbox" checked={automaticScan} onChange={(event) => setAutomaticScan(event.target.checked)} disabled={scanBusy} /><span>{t("启动时自动扫描项目")}</span></label>
          <label><span>{t("项目扫描目录（每行一个；留空扫描所有本地磁盘）")}</span><textarea value={scanRoots} onChange={(event) => setScanRoots(event.target.value)} disabled={scanBusy} rows={3} placeholder="F:\Projects" /></label>
          <p className="help-text">{t("指定目录按直属文件夹列出项目；子目录只计入文件数量，不再作为独立项目。留空时使用全盘智能发现。")}</p>
          <p className="help-text">{t("重新扫描将替换之前的发现结果；已保存选择和手动目录保留。旧迁移包与缓存不参与自动发现。")}</p>
          <p className="help-text">{t("完整本地项目备份包含隐藏文件、Git、依赖、构建产物及敏感文件（.env、私钥、Token）。仓库使用恢复密码加密，请勿共享密码。文件数为扫描时的普通文件数量；无法读取或未跟随的链接会标为部分统计。")}</p>
          <p className="help-text">{t(scanConfigured ? "当前为项目根目录扫描：列出直属文件夹；子目录计入文件数量。" : "当前为全盘智能发现：结果受扫描深度、时间和条目数量限制；建议选择项目根目录。")}</p>
          <div className="scan-actions">
            <button className="secondary-button" type="button" disabled={scanBusy} onClick={() => void chooseScanRoot()}><FolderOpen aria-hidden="true" />{t("选择项目根目录")}</button>
            <button className="secondary-button" type="button" disabled={scanBusy || localCandidates.length === 0} onClick={selectAllCurrent}><CheckCircle2 aria-hidden="true" />{t("选择全部本次扫描项目")}</button>
            <button className="secondary-button" type="button" disabled={scanBusy} onClick={() => void onRescan(nextConfig(), false)}><RefreshCw aria-hidden="true" />{t("重新扫描")}</button>
          </div>
        </div>
        <div className="manual-project form-grid">
          <PathField label={t("手动添加项目目录")} value={manualPath} onChange={setManualPath} title={t("选择项目目录")} placeholder="F:\\Notes\\shared" onError={onError} />
          <div><p className="help-text">{t("允许添加非 Git 普通目录。")}</p><button className="secondary-button" type="button" onClick={() => { const path = displayPath(manualPath.trim()); if (path) { setManualPaths((current) => [...current, path]); setSelected((current) => new Set(current).add(pathKey(path))); setManualPath(""); } }} disabled={!manualPath.trim()}>{t("添加目录")}</button></div>
        </div>
        {localScanState === "running" && <div className="scan-status" role="status"><LoaderCircle className="spin" aria-hidden="true" />{t("正在扫描本机项目")}<button className="text-button" type="button" onClick={onCancelLocalScan}>{t("取消扫描")}</button></div>}
        {localScanState === "complete" && <p className="help-text">{t("本机项目扫描已完成")}</p>}
        {localScanState === "partial" && <p className="help-text">{t("本机项目扫描已部分完成")}</p>}
        {localScanState === "failed" && <p className="inline-error" role="alert">{t("本机项目扫描失败")}</p>}
        {localScanState !== "idle" && localScanState !== "running" && <p className="help-text">{t("扫描位置数量")}: {scannedRootCount} · {t("候选项目数量")}: {localCandidates.length}</p>}
        {scanLimitReached && <div className="scan-warning" role="status"><TriangleAlert aria-hidden="true" /><span><strong>{t("全盘智能发现结果可能不完整")}</strong>；{t("请添加项目根目录以查看完整直属项目列表。")}</span></div>}
        <div className="scan-permission card-muted">
          <label className="checkbox-row">
            <input type="checkbox" aria-label={t("扫描受限目录时申请管理员权限")} checked={requestAdmin} onChange={(event) => setRequestAdmin(event.target.checked)} disabled={scanBusy} />
            <span><strong>{t("扫描受限目录时申请管理员权限")}</strong><small>{t("仅点击按钮时才会请求 Windows UAC；普通扫描不会被中断。")}</small></span>
          </label>
          {requestAdmin && <button className="secondary-button" type="button" onClick={() => void onRescan(nextConfig(), true)} disabled={scanBusy}><ShieldCheck aria-hidden="true" />{t("以管理员权限重新扫描")}</button>}
        </div>
        {(permissionDeniedCount > 0 || otherWarningCount > 0 || localWarnings.length > 0) && <div className="scan-warning" role="status"><TriangleAlert aria-hidden="true" /><span>{permissionDeniedCount > 0 && <>{t("已跳过 {count} 个无权限目录；可访问项目仍已显示。", { count: permissionDeniedCount })} </>}{otherWarningCount > 0 && <>{t("另有 {count} 条扫描提示。", { count: otherWarningCount })} </>}{localWarnings.length > 0 && <span>{localWarnings.join(" · ")}</span>}</span></div>}
        {rows.size === 0 && <p className="empty-state">{t("当前没有可扫描的项目。")}</p>}
        {[...groupedRows.entries()].sort(([, left], [, right]) => left.order - right.order).map(([groupKey, group]) => (
          <section className="project-location" key={groupKey} aria-label={group.label}>
            <h2 className="project-group-title">{group.label}</h2>
            {group.rows.some(([, row]) => row.current) && <div className="project-subgroup"><h3>{t("本次扫描发现")}</h3>{group.rows.filter(([, row]) => row.current).map(renderProjectRow)}</div>}
            {group.rows.some(([, row]) => !row.current) && <div className="project-subgroup"><h3>{t("已保存或手动目录")}</h3>{group.rows.filter(([, row]) => !row.current).map(renderProjectRow)}</div>}
          </section>
        ))}
      </section>
    </div>
  );
}

interface BackupsPageProps {
  visible: boolean;
  operationBusy: boolean;
  headingRef: RefObject<HTMLHeadingElement | null>;
  inventory: CodexInventory | null;
  config: AppConfig;
  onRepositoryChange: (repository: string) => Promise<void>;
  onNavigate: (view: View) => void;
  onOperationStart: () => void;
  onOperationEnd: () => void;
  onBackupStatusChange: (status: OperationStatus) => void;
  onMigrationStatusChange: (status: OperationStatus) => void;
  onNotice: (message: string | null) => void;
  onError: (message: string | null) => void;
}

function BackupsPage({ headingRef, visible, operationBusy, inventory, config, onRepositoryChange, onNavigate, onOperationStart, onOperationEnd, onBackupStatusChange, onMigrationStatusChange, onNotice, onError }: BackupsPageProps) {
  const { t, locale } = useI18n();
  const [subview, setSubview] = useState<BackupView>("local");
  const [openedViews, setOpenedViews] = useState({ export: false, import: false, history: false });
  const [migrationJobId, setMigrationJobId] = useState<string | null>(null);
  const exportHeadingRef = useRef<HTMLHeadingElement>(null);
  const importHeadingRef = useRef<HTMLHeadingElement>(null);
  const historyHeadingRef = useRef<HTMLHeadingElement>(null);
  const [repository, setRepository] = useState(config.local_repository ?? "");
  const [password, setPassword] = useState("");
  const [restoreTarget, setRestoreTarget] = useState("");
  const [rememberPassword, setRememberPassword] = useState(false);
  const [snapshots, setSnapshots] = useState<LocalSnapshotSummary[]>([]);
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<LocalSnapshot | null>(null);
  const [restoreResult, setRestoreResult] = useState<LocalRestoreReport | null>(null);

  function openMigration(next: "export" | "import" | "history") {
    setOpenedViews(current => ({ ...current, [next]: true }));
    setSubview(next);
  }

  useEffect(() => {
    if (visible) {
      (subview === "export" ? exportHeadingRef : subview === "import" ? importHeadingRef : subview === "history" ? historyHeadingRef : headingRef).current?.focus();
      document.documentElement.scrollTop = 0;
    }
  }, [subview]);

  useEffect(() => {
    if (visible && !operationBusy) setSubview("local");
    // Only reset on navigation, never while the active operation completes.
  }, [visible]);

  useEffect(() => {
    setRepository(config.local_repository ?? "");
  }, [config.local_repository]);

  const codexHome = config.codex_home ?? inventory?.codex_home ?? "";
  const projectPaths = config.selected_project_paths;

  async function startBackup() {
    if (busy || operationBusy) return;
    onError(null);
    onNotice(null);
    if (!repository.trim()) return onError(t("需要先填写完整的本地备份设置。"));
    if (!codexHome) return onError(t("需要先填写完整的本地备份设置。"));
    if (!password) return onError(t("请输入恢复密码。"));
    setBusy(true);
    onBackupStatusChange("running");
    onOperationStart();
    try {
      await onRepositoryChange(repository);
      const backup = await runLocalBackup({ codex_home: codexHome, project_paths: projectPaths, repository, recovery_password: password, remember_password: rememberPassword, source_device_id: inventory?.source_device_id });
      setResult(backup);
      setSnapshots((current) => [summaryFromSnapshot(backup), ...current.filter((item) => item.restic_snapshot_id !== backup.restic_snapshot_id)]);
      const tone = backupResultTone(backup.manifest);
      onBackupStatusChange(operationStatusForTone(tone));
      onNotice(t(tone === "success" ? "本地备份已完成" : tone === "warning" ? "本地备份已完成，存在注意事项" : "本地备份已部分完成"));
    } catch (caught) {
      onBackupStatusChange("failed");
      onError(errorMessage(caught, t));
    } finally {
      setBusy(false);
      onOperationEnd();
    }
  }

  async function refreshBackups() {
    if (busy || operationBusy) return;
    onError(null);
    if (!repository.trim()) return onError(t("需要先填写完整的本地备份设置。"));
    setBusy(true);
    onMigrationStatusChange("running");
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
    if (busy || operationBusy) return;
    onError(null);
    if (!repository.trim() || !restoreTarget.trim()) return onError(t("需要先填写完整的本地备份设置。"));
    setBusy(true);
    onOperationStart();
    try {
      const report = await restoreLocalBackup({ snapshot_id: snapshot.restic_snapshot_id, repository, recovery_password: password, target: restoreTarget });
      setRestoreResult(report);
      const tone = backupResultTone(report);
      onMigrationStatusChange(operationStatusForTone(tone));
      onNotice(t(tone === "success" ? "恢复完成" : tone === "warning" ? "恢复完成，存在注意事项" : "恢复已部分完成"));
    } catch (caught) {
      onMigrationStatusChange("failed");
      onError(errorMessage(caught, t));
    } finally {
      setBusy(false);
      onOperationEnd();
    }
  }

  async function upload(snapshot: LocalSnapshotSummary) {
    if (busy || operationBusy) return;
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

  return (
    <>
    <div className="migration-return" hidden={subview === "local"}><button className="secondary-button" type="button" onClick={() => setSubview("local")}><RotateCcw aria-hidden="true" />{t("返回本地备份")}</button></div>
    {openedViews.export && <div hidden={subview !== "export"}><fieldset className="migration-control-guard" disabled={operationBusy}><SendPage headingRef={exportHeadingRef} inventory={inventory} onOperationStart={onOperationStart} onOperationEnd={onOperationEnd} /></fieldset></div>}
    {openedViews.import && <div hidden={subview !== "import"}><ReceivePage headingRef={importHeadingRef} inventory={inventory} operationBusy={operationBusy} initialJobId={migrationJobId} onJobIdChange={setMigrationJobId} onOpenHistory={() => openMigration("history")} onOperationStart={onOperationStart} onOperationEnd={onOperationEnd} onMigrationStatusChange={onMigrationStatusChange} /></div>}
    {openedViews.history && <div hidden={subview !== "history"}><HistoryPage headingRef={historyHeadingRef} visible={visible && subview === "history"} operationBusy={operationBusy} onOperationStart={onOperationStart} onOperationEnd={onOperationEnd} /></div>}
    <div className="page" hidden={subview !== "local"}>
      <header className="page-header page-header-with-action">
        <div><p className="eyebrow">BACKUP & MIGRATION</p><h1 ref={headingRef} tabIndex={-1}>{t("备份与迁移")}</h1><p className="page-description">{t("本地备份不会访问云端。")}</p></div>
        <button className="secondary-button" type="button" onClick={() => void refreshBackups()} disabled={busy || operationBusy}><RefreshCw aria-hidden="true" />{t("刷新本地备份")}</button>
      </header>

      <div className="subnav" role="tablist" aria-label={t("迁移能力")}>
        <button role="tab" aria-selected={subview === "local"} className={subview === "local" ? "subnav-item active" : "subnav-item"} type="button" onClick={() => setSubview("local")}>{t("本地备份")}</button>
        <button role="tab" aria-selected={false} className="subnav-item" type="button" onClick={() => openMigration("export")}>{t("导出 ReHome 迁移包")}</button>
        <button role="tab" aria-selected={false} className="subnav-item" type="button" onClick={() => openMigration("import")}>{t("导入 ReHome 迁移包")}</button>
        <button role="tab" aria-selected={false} className="subnav-item" type="button" onClick={() => openMigration("history")}>{t("迁移记录")}</button>
      </div>
      {(openedViews.export || openedViews.import) && <p className="help-text">{t("迁移结果保留在对应的导出或导入页面，重新打开即可查看。")}</p>}

      <section className="card form-card">
        <div className="section-heading"><HardDrive aria-hidden="true" /><h2>{t("完整本地备份")}</h2></div>
        <p className="help-text">{t("操作说明")}: {t("选择本地目录后，应用用 restic 加密、去重并校验快照。恢复会先进入独立目录，不覆盖现有资料。")}</p>
        <div className="form-grid">
          <PathField label={t("备份目录")} description={t("备份目录用于存放加密备份仓库，不是待备份的项目目录；恢复时请选择同一个仓库。")} value={repository} onChange={setRepository} title={t("选择备份目录")} placeholder="D:\\ENHE\\backups" onError={onError} disabled={busy || operationBusy} />
          <label><span>{t("恢复密码")}</span><input type="password" value={password} onChange={(event) => setPassword(event.target.value)} autoComplete="new-password" disabled={busy || operationBusy} /></label>
        </div>
        <label className="checkbox-row"><input type="checkbox" checked={rememberPassword} onChange={(event) => setRememberPassword(event.target.checked)} disabled={busy || operationBusy} /><span><strong>{t("记住密码（仅此 Windows 用户）")}</strong><small>{t("使用 DPAPI 保护密码，供计划任务使用。")}</small></span></label>
        <div className="path-line"><span>{t("Codex 数据位置")}</span><code>{codexHome || t("未检测")}</code></div>
        <div className="path-line"><span>{t("项目")}</span><code>{projectPaths.length} · {projectPaths.join("; ") || t("仅备份 Codex 数据")}</code></div>
        <p className="help-text">{t("项目按全量备份，包含 .env、私钥和 Token；Codex 登录凭据仍排除。请保护仓库和恢复密码，缺失项会单独报告。")}</p>
        <button className="primary-button" type="button" onClick={() => void startBackup()} disabled={busy || operationBusy}>{busy ? <LoaderCircle className="spin" aria-hidden="true" /> : <LockKeyhole aria-hidden="true" />}{busy ? t("备份进行中") : t("开始本地备份")}</button>
        {result && <><BackupResultSummary manifest={result.manifest} /><p className="backup-result-meta">{result.manifest.file_count} {t("文件")} · {result.restic_snapshot_id}</p></>}
      </section>

      <section className="card">
        <div className="section-heading"><Archive aria-hidden="true" /><h2>{t("最近的本地备份")}</h2></div>
        {snapshots.length === 0 ? <p className="empty-state">{t("还没有本地备份")}</p> : <div className="snapshot-list">{snapshots.map((snapshot) => {
          const status = snapshot.integrity_status === "warning" ? "warning" : snapshot.integrity_status === "partial" || !snapshot.complete ? "partial" : "complete";
          return <div className="snapshot-row" key={snapshot.restic_snapshot_id}>
            <div><strong>{formatDate(snapshot.created_at, locale)}</strong><code>{snapshot.restic_snapshot_id}</code></div>
            <span className={`tag ${status}`}>{t(status === "warning" ? "注意事项" : status === "complete" ? "完整" : "部分完成")}</span>
            <span>{snapshot.file_count} {t("文件")}</span>
            <div className="snapshot-actions"><button className="secondary-button small" type="button" disabled={busy || operationBusy} onClick={() => void restore(snapshot)}><RotateCcw aria-hidden="true" />{t("恢复")}</button>{config.cloud.enabled && <button className="secondary-button small" type="button" disabled={busy || operationBusy} onClick={() => void upload(snapshot)}><CloudUpload aria-hidden="true" />{t("上传此快照")}</button>}</div>
          </div>;
        })}</div>}
        <PathField className="restore-target" label={t("恢复目标目录")} description={t("选择新的空目录；恢复不会覆盖你正在使用的项目或 Codex 数据。")} value={restoreTarget} onChange={setRestoreTarget} title={t("选择恢复目标目录")} placeholder="D:\\ENHE\\restored" onError={onError} disabled={busy || operationBusy} />
        {restoreResult && <><BackupResultSummary manifest={restoreResult} operation="restore" /><p className="backup-result-meta">{restoreResult.restored_root} · {restoreResult.restored_files} {t("文件")}</p></>}
        {restoreResult && <p className="help-text">{t("恢复目录内的 manifest.json 记录来源路径、排除和缺失项；它不计入项目文件数。")}</p>}
        <button type="button" className="text-button" onClick={() => onNavigate("guide")}>{t("如何恢复到本机或另一台设备？")}</button>
      </section>

      <section className="migration-note"><CloudOff aria-hidden="true" /><div><strong>{t("云端备份已关闭")}</strong><p>{t("云端关闭时不会启动远端连接，也不会上传资料。")}</p></div><button type="button" className="text-button" onClick={() => onNavigate("settings")}>{t("设置")}</button></section>
    </div>
    </>
  );
}

function summaryFromSnapshot(snapshot: LocalSnapshot): LocalSnapshotSummary {
  return { logical_backup_id: snapshot.logical_backup_id, restic_snapshot_id: snapshot.restic_snapshot_id, created_at: snapshot.manifest.created_at, file_count: snapshot.manifest.file_count, byte_count: snapshot.manifest.byte_count, integrity_status: snapshot.manifest.integrity_status, complete: snapshot.complete };
}

function mergeAutomaticConfig(config: AppConfig, inventory: CodexInventory): AppConfig {
  return {
    ...config,
    codex_home: config.codex_home?.trim() ? config.codex_home : inventory.codex_home,
    selected_project_paths: config.selected_project_paths,
    project_selection_initialized: true,
  };
}

function hasConfigChanges(before: AppConfig, after: AppConfig): boolean {
  return before.codex_home !== after.codex_home
    || before.project_selection_initialized !== after.project_selection_initialized
    || before.selected_project_paths.join("\0") !== after.selected_project_paths.join("\0");
}

interface PathFieldProps {
  description?: string;
  className?: string;
  label: string;
  value: string;
  onChange: (value: string) => void;
  title: string;
  placeholder?: string;
  disabled?: boolean;
  onError?: (message: string | null) => void;
}

function PathField({ className, label, description, value, onChange, title, placeholder, disabled, onError }: PathFieldProps) {
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
      {description && <small className="help-text">{description}</small>}
    </label>
  );
}

function displayPath(path: string): string {
  const normalized = path.replace(/\//g, "\\").replace(/^\\\\\?\\UNC\\/i, "\\\\").replace(/^\\\\\?\\/, "");
  return normalized.length > 3 ? normalized.replace(/\\+$/, "") : normalized;
}

function pathKey(path: string): string {
  return displayPath(path).toLowerCase();
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

      <section className="settings-footer"><button className="primary-button" type="button" onClick={() => { onConfigChange(draft); void onSave(draft); }}>{t("保存设置")}</button><span>{t("当前版本")} 0.1.9</span></section>
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
