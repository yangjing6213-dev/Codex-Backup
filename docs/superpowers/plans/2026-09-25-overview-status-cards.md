# 概览页状态卡与数据指标 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将项目扫描、Codex 数据扫描、本地备份和迁移/恢复状态集中到概览页，并发布 0.1.9 Windows x64 安装包。

**Architecture:** 在现有 `AppContent` 中保存会话级备份/迁移状态，由 `OverviewPage` 通过只读 props 渲染状态卡。`BackupsPage` 和 `ReceivePage` 通过明确的回调报告操作结果，不改变后端数据契约或备份规则。概览页继续使用现有项目扫描与 Codex inventory 数据，不新增持久化字段。

**Tech Stack:** React 19, TypeScript, Vitest, Testing Library, Vite, Tauri 2, Rust, PowerShell packaging/verification scripts.

**Spec:** `docs/superpowers/specs/2026-09-25-overview-status-cards-design.md`

## Global Constraints

- 不改变备份包含/排除规则、Codex 凭据保护规则、云端逻辑或恢复事务逻辑。
- 不把操作状态写入用户配置或新增数据库迁移。
- 不声称原生 Codex 会话已经续接成功；概览页只反映应用已经完成的本地操作和现有验证结果。
- 版本从 `0.1.8` 升级到 `0.1.9`。
- Release 仅上传最终安装包及其 SHA-256 文件，保留旧版本。
- 不使用真实 Codex 配置、账号、Token、私钥或个人迁移数据测试。
- 不执行 Force Push、创建 PR、修改仓库设置或上传其他资产。

## Review Focus

- 启动扫描仍在进行时，概览页不得把项目或数据扫描显示为已完成；由 Task 2 的 loading/running 测试覆盖。
- 项目扫描部分完成或失败时，状态卡必须保留 warning/error 语义；由 Task 2 的 partial/failed 测试覆盖。
- 备份完成后切换到项目或数据页再返回，完成状态必须保留；由 Task 3 的 navigation 测试覆盖。
- ReHome 导入回滚或失败时，不得显示“迁移/恢复已完成”；由 Task 3 的 failed/rolled-back 测试覆盖。
- 英文界面必须显示与中文相同的四项指标和状态语义；由 Task 4 的 bilingual test 覆盖。

---

### Task 1: 固化概览指标与状态卡的失败测试

**Files:**
- Modify: `desktop/src/App.enhe.test.tsx`
- Inspect: `desktop/src/App.tsx`, `desktop/src/lib/i18n.tsx`, `desktop/src/features/backup/BackupResultSummary.tsx`, `desktop/src/features/receive/ReceivePage.tsx`

**Interfaces:**
- Consumes: existing mocked `inventory`, `localDiscovery`, `runLocalBackup`, `restoreLocalBackup`, `getMigrationJob`, and navigation helpers in `App.enhe.test.tsx`.
- Produces: failing tests that define the overview DOM contract before implementation.

- [ ] **Step 1: Add the overview data-metric test**

Add a test after the existing overview navigation tests:

```tsx
it("copies the four Codex data metrics below the overview metrics", async () => {
  render(<App />);
  expect(await screen.findByText("扫描项目数")).toBeVisible();
  expect(screen.getAllByText("对话总数").length).toBeGreaterThanOrEqual(2);
  expect(screen.getByText("技能")).toBeVisible();
  expect(screen.getByText("插件")).toBeVisible();
  expect(screen.getByText("生成图片")).toBeVisible();
  expect(screen.getByText("27")).toBeVisible();
  expect(screen.getByText("14")).toBeVisible();
  expect(screen.getByText("21")).toBeVisible();
  expect(screen.getByText("91")).toBeVisible();
});
```

- [ ] **Step 2: Add the initial and scan-complete status-card test**

Use the existing fixture where `discoverCodex` resolves to `inventory`, then assert the four status-card labels and the completed project/data scan states:

```tsx
it("shows project and Codex data scan statuses above the cloud-off card", async () => {
  render(<App />);
  expect(await screen.findByRole("status", { name: "本机项目扫描已完成" })).toBeVisible();
  expect(screen.getByRole("status", { name: "本机Codex数据扫描已完成" })).toBeVisible();
  expect(screen.getByRole("status", { name: "本地备份尚未完成" })).toBeVisible();
  expect(screen.getByRole("status", { name: "迁移/恢复尚未完成" })).toBeVisible();
  const cloud = screen.getByText("云端备份已关闭").closest("section");
  const projectStatus = screen.getByRole("status", { name: "本机项目扫描已完成" });
  expect(projectStatus.compareDocumentPosition(cloud!)).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
});
```

- [ ] **Step 3: Add running/partial/failed scan coverage**

Extend existing discovery mocks so the test renders `localScanState === "partial"` and `codexScanFailed === true`; assert the card names are `本机项目扫描已部分完成` and `本机Codex数据扫描失败`, and never `本机Codex数据扫描已完成`.

- [ ] **Step 4: Run the focused tests and verify they fail for the missing UI contract**

Run:

```powershell
pnpm --dir desktop exec vitest --run desktop/src/App.enhe.test.tsx -t "copies the four Codex data metrics|shows project and Codex data scan statuses"
```

Expected: FAIL because the new data metric group and the four overview status cards do not yet exist.

- [ ] **Step 5: Commit the failing tests**

```powershell
git add desktop/src/App.enhe.test.tsx
git commit -m "test: specify overview status cards"
```

### Task 2: Add App-level status state and overview rendering

**Files:**
- Modify: `desktop/src/App.tsx:60-680`
- Modify: `desktop/src/lib/i18n.tsx:90-170`
- Modify: `desktop/src/App.css:100-180` and responsive sections
- Test: `desktop/src/App.enhe.test.tsx`

**Interfaces:**
- Consumes: `LocalScanState`, `CodexInventory`, `localDiscovery`, `loading`, `codexScanFailed`, and the existing `Metric` component.
- Produces: `OverviewOperationState`, `OverviewActivityState`, `StatusCard`, and `OverviewPage` props for the next task.

- [ ] **Step 1: Add UI-only state types and initial state in `App.tsx`**

Add immediately after `LocalScanState`:

```tsx
type OverviewOperationState = "idle" | "running" | "success" | "warning" | "partial" | "failed";
interface OverviewActivityState {
  backup: OverviewOperationState;
  migration: OverviewOperationState;
}
const initialOverviewActivity: OverviewActivityState = { backup: "idle", migration: "idle" };
```

Add `const [overviewActivity, setOverviewActivity] = useState(initialOverviewActivity);` inside `AppContent`; pass `loading`, `codexScanFailed`, and `overviewActivity` to `OverviewPage`.

- [ ] **Step 2: Implement deterministic scan-state labels and status metadata**

Add a local helper before `OverviewPage`:

```tsx
function overviewStatusKey(state: OverviewOperationState): string {
  return {
    idle: "尚未完成",
    running: "进行中",
    success: "已完成",
    warning: "已完成，存在注意事项",
    partial: "部分完成",
    failed: "失败",
  }[state];
}
```

Use a `StatusCard` function with props `{ title: string; status: string; tone: "idle" | "running" | "success" | "warning" | "partial" | "failed"; icon: ReactNode; onClick: () => void; actionLabel: string }`, render `role="status"` and `aria-label={`${title}${status}`}`. `running` uses `LoaderCircle`, `success` uses `CheckCircle2`, and warning/partial/failed use `TriangleAlert`.

- [ ] **Step 3: Render two metric groups and four status cards in the required order**

In `OverviewPage`, keep the existing first `metric-grid`, add a second `metric-grid overview-data-metrics` using `inventory?.counts.conversations`, `skills`, `plugins`, and `generated_images`, then add the status grid before the cloud-off `status-banner`.

Derive statuses exactly as follows:

```tsx
const projectStatus = localScanState === "running"
  ? "本机项目扫描正在进行"
  : localScanState === "partial"
    ? "本机项目扫描已部分完成"
    : localScanState === "failed"
      ? "本机项目扫描失败"
      : localScanState === "complete"
        ? "本机项目扫描已完成"
        : "本机项目扫描尚未开始";
const dataStatus = loading
  ? "本机Codex数据扫描正在进行"
  : codexScanFailed
    ? "本机Codex数据扫描失败"
    : inventory
      ? "本机Codex数据扫描已完成"
      : "本机Codex数据扫描尚未开始";
```

Each card navigates to its relevant page: project → `projects`, data → `data`, backup → `backups`, migration/recovery → `backups`.

- [ ] **Step 4: Add all Chinese and English strings**

Add translations in `desktop/src/lib/i18n.tsx` for the exact labels, statuses, button text and explanatory text used by the new cards, including:

```text
本机Codex数据扫描已完成
本机Codex数据扫描正在进行
本机Codex数据扫描失败
本机Codex数据扫描尚未开始
本机项目扫描尚未开始
本地备份尚未完成
本地备份进行中
本地备份已完成
本地备份已完成，存在注意事项
本地备份已部分完成
本地备份失败
迁移/恢复尚未完成
迁移/恢复进行中
迁移/恢复已完成
迁移/恢复已部分完成
迁移/恢复失败
查看备份与迁移
```

- [ ] **Step 5: Add responsive status-card styles**

Add `.overview-data-metrics`, `.overview-status-grid`, `.overview-status-card`, `.overview-status-card.running`, `.overview-status-card.success`, `.overview-status-card.warning`, `.overview-status-card.partial`, `.overview-status-card.failed`, and the narrow-screen single-column rule. Reuse `--accent`, `--warning`, `--danger`, `--border`, and existing panel variables.

- [ ] **Step 6: Run focused tests and build**

Run:

```powershell
pnpm --dir desktop exec vitest --run desktop/src/App.enhe.test.tsx -t "copies the four Codex data metrics|shows project and Codex data statuses"
pnpm --dir desktop run build
```

Expected: the new tests pass and the TypeScript/Vite build exits 0.

- [ ] **Step 7: Commit the overview rendering**

```powershell
git add desktop/src/App.tsx desktop/src/lib/i18n.tsx desktop/src/App.css desktop/src/App.enhe.test.tsx
git commit -m "feat: add overview scan and backup status cards"
```

### Task 3: Report backup and migration results across navigation

**Files:**
- Modify: `desktop/src/App.tsx:458-550, 930-1130`
- Modify: `desktop/src/features/receive/ReceivePage.tsx:40-280`
- Modify: `desktop/src/App.enhe.test.tsx`
- Test: `desktop/src/features/receive/ReceivePage.test.tsx`

**Interfaces:**
- Consumes: `OverviewOperationState`, `backupResultTone`, `LocalRestoreReport`, `MigrationJobSnapshot`.
- Produces: `onBackupStatusChange(status: OverviewOperationState)` and `onMigrationStatusChange(status: OverviewOperationState)` callbacks passed from `AppContent` through `BackupsPage` to `ReceivePage`.

- [ ] **Step 1: Add failing navigation-preservation tests**

Add to `App.enhe.test.tsx`:

```tsx
it("keeps a completed local backup status on the overview after navigation", async () => {
  const user = userEvent.setup();
  api.runLocalBackup.mockResolvedValue({
    logical_backup_id: "backup-1", restic_snapshot_id: "snapshot-1", complete: true,
    manifest: { created_at: "2026-09-25T00:00:00Z", file_count: 4, byte_count: 20, exclusions: [], notices: [], integrity_warnings: [], missing: [], integrity_status: "complete" },
  });
  render(<App />);
  await user.click(await screen.findByRole("button", { name: "前往备份与迁移" }));
  await user.type(screen.getByLabelText("恢复密码"), "synthetic-password");
  await user.click(screen.getByRole("button", { name: "开始本地备份" }));
  await screen.findByText("本地备份已完成");
  await user.click(screen.getByRole("button", { name: "前往概览" }));
  expect(screen.getByRole("status", { name: "本地备份已完成" })).toBeVisible();
});
```

Add a receive-page test that reports `failed` for a `rolled_back` job and never reports success.

- [ ] **Step 2: Add callback props to `BackupsPage` and wire local backup status**

Extend `BackupsPageProps` with:

```tsx
onBackupStatusChange: (status: OverviewOperationState) => void;
onMigrationStatusChange: (status: OverviewOperationState) => void;
```

Call `onBackupStatusChange("running")` before `runLocalBackup`. On success map `backupResultTone(backup.manifest)` to `success`, `warning`, or `partial`; on catch call `onBackupStatusChange("failed")`. Do not reset the state in `finally`.

For `restore`, call `onMigrationStatusChange("running")` before `restoreLocalBackup`; map `backupResultTone(report)` on success and report `failed` in the catch branch.

- [ ] **Step 3: Add callback props to `ReceivePage` and report ReHome results**

Extend `ReceivePageProps` with `onMigrationStatusChange`. In `handleRestore`, report `success`, `warning`, or `partial` from the returned restore report; in the catch branch report `failed` after clearing the stale plan. In the polling effect, when a job leaves `running`, report `success` only for `status === "succeeded"`; report `partial` for `status === "rolled_back"` and `failed` for `failed_before_write` or `rollback_failed`.

- [ ] **Step 4: Pass callbacks from `AppContent` and render activity state**

Pass the two setters through `BackupsPage`, and update `OverviewPage` props with `overviewActivity`. Use the existing `activeOperations` notice for the global in-progress message; the new cards only read the same state and never start an operation themselves.

- [ ] **Step 5: Run focused tests**

Run:

```powershell
pnpm --dir desktop exec vitest --run desktop/src/App.enhe.test.tsx -t "completed local backup status|in-flight backup"
pnpm --dir desktop exec vitest --run desktop/src/features/receive/ReceivePage.test.tsx -t "rolled back|succeeded|failed"
```

Expected: all selected tests pass, including the existing page-switching backup test.

- [ ] **Step 6: Commit the cross-page status flow**

```powershell
git add desktop/src/App.tsx desktop/src/features/receive/ReceivePage.tsx desktop/src/App.enhe.test.tsx desktop/src/features/receive/ReceivePage.test.tsx
git commit -m "feat: preserve backup and migration status in overview"
```

### Task 4: Bilingual and regression verification

**Files:**
- Modify: `desktop/src/App.enhe.test.tsx`
- Modify: `desktop/src/lib/i18n.tsx` only if a missing translation is found
- Modify: `desktop/src/App.css` only if responsive assertions expose a layout issue

**Interfaces:**
- Consumes: the completed overview/status-card implementation from Tasks 2–3.
- Produces: verified Chinese/English rendering and a clean frontend diff.

- [ ] **Step 1: Add the English overview test**

Switch locale with the existing language toggle, open the overview, and assert `Local Codex data scan completed`, `Skills`, `Plugins`, `Generated images`, and `View backups & migration` are visible.

- [ ] **Step 2: Run the complete frontend suite**

Run:

```powershell
pnpm --dir desktop test -- --run
```

Expected: all frontend test files pass with zero failures.

- [ ] **Step 3: Run typecheck and production build**

Run:

```powershell
pnpm --dir desktop exec tsc -b
pnpm --dir desktop run build
```

Expected: both exit 0.

- [ ] **Step 4: Review the diff and commit only owned files**

Run:

```powershell
git diff --check
git status --short
git diff --stat 0884d83..HEAD
```

Confirm no credentials, personal data, build outputs, or unrelated changes are staged; then commit any remaining test-only correction with an explicit file list.

### Task 5: Version 0.1.9 and documentation

**Files:**
- Modify: `desktop/package.json`
- Modify: `desktop/src-tauri/Cargo.toml`
- Modify: `desktop/src-tauri/Cargo.lock`
- Modify: `desktop/src-tauri/tauri.conf.json`
- Modify: visible version labels in `desktop/src/App.tsx`
- Modify: `tests/version_consistency_test.ps1`
- Modify: `CHANGELOG.md`, `README.md`, `README.en.md`
- Modify: `docs/STATUS.md`, `docs/ACCEPTANCE.md`, `docs/USER_GUIDE.zh-CN.md`, `docs/USER_GUIDE.en.md`, `docs/OFFLINE_RESTORE.zh-CN.md`, `docs/OFFLINE_RESTORE.en.md`, `docs/COMPATIBILITY.md`, `docs/THIRD_PARTY.md`

**Interfaces:**
- Consumes: the tested 0.1.9 UI behavior and exact package hash produced by Task 6.
- Produces: consistent version metadata, bilingual release notes, and user-facing explanations of the new overview cards.

- [ ] **Step 1: Update all application version sources to `0.1.9`**

Use the existing version-consistency pattern; update package metadata, Tauri metadata/window title, Cargo package/lock entry, visible label, test expectation and license/manifest headers without changing dependency versions.

- [ ] **Step 2: Add the 0.1.9 changelog entry**

Document the two metric groups, four session-level status cards, page-switch persistence, and the explicit non-goal that the cards do not prove native Codex conversation continuation.

- [ ] **Step 3: Update README and operational documentation**

Add a concise “概览页状态” explanation in both languages, preserve the existing warning that a status card summarizes local application evidence only, and keep the download/checksum links as pending until the Release exists.

- [ ] **Step 4: Run version and documentation contracts**

Run:

```powershell
powershell -ExecutionPolicy Bypass -File .\tests\version_consistency_test.ps1
powershell -ExecutionPolicy Bypass -File .\tests\readme_language_test.ps1
```

Expected: both exit 0.

- [ ] **Step 5: Commit version and docs**

```powershell
git add desktop/package.json desktop/src-tauri/Cargo.toml desktop/src-tauri/Cargo.lock desktop/src-tauri/tauri.conf.json desktop/src/App.tsx tests/version_consistency_test.ps1 CHANGELOG.md README.md README.en.md docs/STATUS.md docs/ACCEPTANCE.md docs/USER_GUIDE.zh-CN.md docs/USER_GUIDE.en.md docs/OFFLINE_RESTORE.zh-CN.md docs/OFFLINE_RESTORE.en.md docs/COMPATIBILITY.md docs/THIRD_PARTY.md
git commit -m "release: prepare v0.1.9"
```

### Task 6: Full verification, package, uninstall and install

**Files:**
- Modify: generated ignored verification/package reports only; restore generated reports before the final commit if they are not part of the approved manifest.
- Create: final ignored installer and adjacent `.sha256` file under `desktop/src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis/`.

**Interfaces:**
- Consumes: the complete 0.1.9 source and documentation from Tasks 1–5.
- Produces: a verified unsigned current-user installer, checksum, install evidence, and a `READY_TO_PUBLISH = YES` release manifest.

- [ ] **Step 1: Run the complete configured verification**

Run:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\verify.ps1
```

Expected: frontend tests, build, documentation/version/icon/license checks, Rust formatting/tests, updater scan and isolated restic acceptance all report PASS. Restore any timestamp-only ignored report changes before committing.

- [ ] **Step 2: Build the unsigned Windows x64 installer**

Run:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\package.ps1 -SkipBootstrap
```

Record the exact installer path, byte size, SHA-256 and sidecar content. The package must remain unsigned and current-user install mode.

- [ ] **Step 3: Replace the installed version safely**

Use the existing current-user uninstall entry for `ENHE Codex Backup 0.1.8`, confirm only the application is removed, then run the 0.1.9 installer. Read the uninstall registry entry and installed executable metadata; confirm `DisplayVersion`, `FileVersion` and `ProductVersion` are `0.1.9` and the ENHE user configuration remains present. Do not delete `.codex` or application data.

- [ ] **Step 4: Perform the final local release gate**

Review `git status`, `git diff --check`, the exact changed-file manifest, the release asset set, the independent installer hash, and sensitive-content/license results. Set `READY_TO_PUBLISH = YES` only when all required local checks passed and no workflow file or secret was added.

### Task 7: Authorized GitHub push and v0.1.9 Release

**Files:**
- Push: explicit source/docs commits from Tasks 1–5 to `main`.
- Release assets: only `ENHE.Codex.Backup_0.1.9_x64-setup.exe` and its `.sha256` file.

**Interfaces:**
- Consumes: Task 6 release gate and the user’s direct authorization to push and create a Release.
- Produces: verified remote `main`, tag `v0.1.9`, non-draft Release, and exactly two assets.

- [ ] **Step 1: Recheck SSH 443 authentication and remote drift**

Run the read-only checks immediately before writing:

```powershell
$env:GIT_SSH_COMMAND='C:/Windows/System32/OpenSSH/ssh.exe -o IdentitiesOnly=no -o IdentityFile=none -o ConnectTimeout=15 -o ConnectionAttempts=1'
$env:GIT_SSH_VARIANT='ssh'
ssh -o IdentitiesOnly=no -o IdentityFile=none -o ConnectTimeout=15 -o ConnectionAttempts=1 -p 443 -T git@ssh.github.com
git ls-remote ssh://git@ssh.github.com:443/yangjing6213-dev/Codex-Backup.git refs/heads/main
```

Stop without writing if the remote main moved from the reviewed base or authentication fails.

- [ ] **Step 2: Push ordinary fast-forward only**

```powershell
git push ssh://git@ssh.github.com:443/yangjing6213-dev/Codex-Backup.git HEAD:main
```

Verify the remote `main` hash equals the pushed HEAD. Do not force-push or create a PR.

- [ ] **Step 3: Create the release with exactly two assets**

Use `gh release create v0.1.9 --target <reviewed-head>` with the installer and sidecar paths only, then verify the Release is non-draft, the tag points to the reviewed commit, and the asset list contains exactly the two expected files.

- [ ] **Step 4: Mark the repository documentation as published**

After Release verification, update the top-level status lines and README links from “pending” to the actual `v0.1.9` URL, commit only those documentation files, and push that ordinary fast-forward commit over SSH 443. Do not change the Release assets.

- [ ] **Step 5: Final verification and report**

Run `git status --short`, remote `main`/tag checks, `gh release view`, asset-count/hash checks, and installed-version verification. Report PASS/PARTIAL accurately; note that native visual click-through remains `UI NOT VISUALLY VERIFIED` unless performed.

## Final Acceptance

- Status: PARTIAL
- Requirements verified: design, implementation tests, build, full verification, packaging, installation and GitHub publication, once evidence is collected.
- Not verified: native visual click-through and real Codex account/session continuation unless separately performed.
- Remaining risks: unsigned Windows installer; user must verify the published SHA-256 before installation.

