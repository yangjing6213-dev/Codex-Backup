# 概览页状态卡与数据指标

## Original Goal
在概览页调整项目扫描状态位置，增加 Codex 数据统计和项目扫描、数据扫描、本地备份、迁移/恢复状态卡，并完成 0.1.9 打包、安装和 GitHub 发布。

## Task Contract
- Requirements: 两组四项指标；四个状态卡；状态跨页面保留；中英文文案；0.1.9 安装包和 Release。
- Constraints: 不改变备份规则、云端逻辑、恢复事务或 Codex 凭据保护；不使用真实个人数据；不强制推送。
- Acceptance Criteria: 计划中 Task 1–7 全部完成并有测试、构建、安装和远程发布证据。
- Deliverables: 代码、测试、双语文档、0.1.9 未签名 Windows x64 安装包、SHA-256 文件、GitHub v0.1.9 Release。
- Non-goals: 不新增持久化状态，不把状态卡当作真实 Codex 会话续接证明。
- Risks: 状态回调可能与现有异步迁移轮询重复触发；安装操作必须保留用户数据；发布资产必须限制为两个文件。
- Testing Strategy: Vitest/Testing Library、TypeScript/Vite、`scripts/verify.ps1`、`scripts/package.ps1 -SkipBootstrap`、当前用户安装检查、远程 Release 资产校验。

## Current Repository State
- Governing instructions: `AGENTS.md` and repository project guidance.
- Relevant architecture/patterns: React state in `desktop/src/App.tsx`; backup and ReHome flows in `BackupsPage` and `ReceivePage`; existing bilingual translation map and Vitest suite.
- Initial Git state: isolated detached worktree at commit `09b48a1`, based on published 0.1.8; working tree clean before implementation.

## Task Plan
| ID | Objective | Files likely affected | Acceptance criteria | Verification | Dependencies | Status |
|---|---|---|---|---|---|---|
| T1 | 固化概览指标和状态卡红测试 | `desktop/src/App.enhe.test.tsx` | 新行为测试先失败 | focused Vitest | none | pending |
| T2 | 实现概览指标、状态卡和文案样式 | `desktop/src/App.tsx`, `desktop/src/lib/i18n.tsx`, `desktop/src/App.css` | 两组指标和扫描状态卡渲染 | focused tests, build | T1 | pending |
| T3 | 连接备份/迁移结果并跨页面保留 | `desktop/src/App.tsx`, `desktop/src/features/receive/ReceivePage.tsx`, tests | success/partial/failure 状态准确 | App and Receive tests | T2 | pending |
| T4 | 双语和回归验证 | frontend tests/docs | Chinese/English 状态一致 | full frontend suite, typecheck, build | T2–T3 | pending |
| T5 | 升级 0.1.9 并更新文档 | version sources, README, CHANGELOG, docs | 版本契约和文档通过 | version/readme contracts | T4 | pending |
| T6 | 完整验证、打包、卸载和安装 | verify/package scripts, installer | 0.1.9 安装版本和用户数据通过 | full verify/package/install | T5 | pending |
| T7 | 推送 main 并创建 Release | remote main, v0.1.9 | SSH 443 快进、两个资产 | remote/tag/release checks | T6 | pending |

## Completed + Verified
- 设计文件 `docs/superpowers/specs/2026-09-25-overview-status-cards-design.md` 已提交并获用户确认。
- 实施计划 `docs/superpowers/plans/2026-09-25-overview-status-cards.md` 已提交并选择 Native 执行。

## Current Work
- T1：已写入并运行概览指标和状态卡失败测试；失败原因均为待实现的生产行为。
- T2：已实现概览页状态卡、两组指标、双语文案和响应式样式；定向测试与前端构建通过。
- T3：已通过 App 回调连接本地备份、恢复和 ReHome 迁移结果；状态跨页面保留，定向场景与完整前端测试通过。
- T4：已通过完整前端回归、TypeScript/Vite 构建和中英文场景验证。
- T5：已统一 0.1.9 元数据、可见版本、CHANGELOG、README 和双语指南；版本一致性契约通过。

## Remaining Work
- T6：全量验证已通过，正在生成最终安装包并执行替换安装；T7 尚未完成。

## Failures
- Bash 版 SDD 辅助脚本在当前 Windows 环境不可执行；已按脚本约定手动建立同路径账本，不影响实现验证。

## Evidence
- `git status --short` -> 隔离工作区在实现前干净。
- `git log -5 --oneline` -> 0.1.8 发布文档提交及计划提交可见。

## Important Decisions
- Ruling: 使用现有 0.1.8 发布隔离工作区作为基线 — 该工作区包含最新已发布代码且干净 — 若错误，风险是把新功能建立在错误版本上，因此每次发布前重新核对远端基线。
- Ruling: 状态类型保留在 `App.tsx` — 它仅用于本次运行的 UI 状态，不进入配置或后端契约 — 若错误，风险是未来组件复用需要移动类型，但当前可避免新增共享层。
- Ruling: T1 的完成条件是观察到预期红测，而不是伪造绿色；当前 fixture 保持原值，仅在指标测试中覆盖四个 Codex 统计值，避免改变既有测试语义。
- Ruling: 状态卡测试通过 `section` 节点定位“云端备份已关闭”，避免侧栏同名状态造成错误匹配。
- Ruling: 概览页状态只保存在 `AppContent` 内存中；备份结果使用既有 `backupResultTone`，迁移/恢复结果通过 ReceivePage 回调映射为成功、部分完成或失败，不新增持久化字段。
- Ruling: `rolled_back` 和 `rollback_failed` 均在概览上显示为迁移/恢复失败；它们不是成功完成，且用户应查看迁移记录处理后续动作。
- Ruling: Rust 异步备份测试的合成 `.cmd` 使用 Windows PowerShell 的绝对系统路径，避免隔离执行环境的 PATH 差异；不改变应用运行时或业务逻辑。
- Ruling: Cargo.lock 版本升级同步刷新许可清单中的规范化哈希和应用版本字段；第三方材料本身未变。

## Open Risks
- 未进行原生 UI 视觉点击验证；完成后报告 `UI NOT VISUALLY VERIFIED`，除非专门运行安装程序验证。
- 安装包未签名；用户需核对 SHA-256。

## Final Acceptance
- Status: PARTIAL
- Requirements verified: 设计和计划已确认。
- Not verified: 代码实现、测试、打包、安装和 GitHub 发布。
- Remaining risks: 见 Open Risks。
