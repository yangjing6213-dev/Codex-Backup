# ENHE Codex Backup

[English](README.en.md) · [中文使用说明](docs/USER_GUIDE.zh-CN.md) · [离线恢复说明](docs/OFFLINE_RESTORE.zh-CN.md)

## 一、这个仓库是什么？

ENHE Codex Backup 是一个独立的 Windows x64 本地优先备份、恢复和离线迁移工具。它可以在没有云端配置、没有 GPT 登录的条件下保存 Codex 资料、项目文件、Git 状态、对话和开发交接资料，并在本机或另一台设备上恢复。0.1.8 增加了项目根目录扫描引导、全盘扫描范围提示、本次结果与保留目录分组以及一键选择本次扫描项目。

它不是 OpenAI 或 ReHome 官方产品。云端默认关闭；OneDrive/rclone 只在用户完成配置并主动执行测试或上传时使用。“迁移并接入 Codex”的联网验证是独立选项，必须另行同意发送所选对话上下文，可能产生模型用量；关闭云端备份不等于禁止该验证联网。

## 二、适合谁用？

### 1、特别适合

- 使用 Codex、AI 编程工具或个人开发工作流，需要可检查的本地备份的人。
- 使用 Git、worktree、未提交修改和多个本地项目的人。
- 希望把对话、项目和交接资料一起离线保存，并在新设备上恢复的人。
- 需要简体中文/English、浅色/深色/跟随系统界面的 Windows 用户。

### 2、不适合

- 需要实时云端同步、多人协作或多云编排的人。
- 需要在没有人工确认的情况下覆盖现有项目或恢复目标的人。
- 需要官方 OpenAI 支持、账号恢复或真实会话续接保证的人。
- 需要 macOS、Linux 或未验证 Windows 版本安装包的人。

## 三、它会产出什么？

- 加密、去重、可校验的 restic 本地快照。
- 包含 Git、worktree、未提交修改、Codex 对话、索引、技能、插件和交接资料的完整备份载荷。
- 可在独立目录中检查和恢复的本地备份。
- 适用的 ReHome `.rehome` 离线迁移包。
- Windows x64 NSIS 安装包及 SHA-256 校验文件（在构建工具链可用时生成）。

## 四、具有什么价值？

它把“资料备份”和“设备迁移”放在同一个本地优先工具中，减少对云端账号和网络的依赖；同时保留 Git/worktree 和未提交工作，降低 AI 编程项目交接时只备份成品文件而丢失上下文的风险。

## 五、示例效果

启动后应用会先读取已保存设置，自动尝试 Codex 数据位置，并显示项目与对话统计。之后在本机固定磁盘后台发现项目候选；无权限目录会安全跳过并汇总：

```text
Codex 数据位置：C:\Users\<用户>\.codex
本地备份目录：%LOCALAPPDATA%\ENHE\Codex Backup\backups
已发现项目：demo（.git、package.json）
对话：按 Codex sessions / archived_sessions 统计
云端备份：已关闭；本地备份不产生远端调用
```

候选项目只会展示并等待用户勾选，不会把整个磁盘内容自动纳入备份。路径字段旁的文件夹按钮会打开 Windows 原生选择窗口；取消不会改变原值。

项目页提供“扫描受限目录时申请管理员权限”选项。只有用户勾选并点击“以管理员权限重新扫描”时才会弹出 Windows UAC；普通扫描不会被中断。

## 六、安装方法

1. 前往 [GitHub Releases 安装包下载页](https://github.com/yangjing6213-dev/Codex-Backup/releases) 查看实际已发布版本。0.1.8 的预定资产为 [Windows x64 安装包](https://github.com/yangjing6213-dev/Codex-Backup/releases/download/v0.1.8/ENHE%20Codex%20Backup_0.1.8_x64-setup.exe) 和 [SHA-256 校验文件](https://github.com/yangjing6213-dev/Codex-Backup/releases/download/v0.1.8/ENHE%20Codex%20Backup_0.1.8_x64-setup.exe.sha256)，仅在该 Release 发布后可用。安装包未进行代码签名；安装前请核对 SHA-256 校验值。
2. 使用 PowerShell 计算安装包 SHA-256，并与校验文件比对。
3. 运行安装包，按 Windows 当前用户范围完成安装。
4. 首次启动后确认本机数据位置；云端保持关闭即可完成本地备份。

本地构建：

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\bootstrap.ps1
powershell -ExecutionPolicy Bypass -File .\scripts\verify.ps1
powershell -ExecutionPolicy Bypass -File .\scripts\package.ps1
```

## 七、如何使用

1. 打开“项目”，点击“选择项目根目录”选择 `F:\Projects`，再点击“重新扫描”。结果按磁盘分组；“本次扫描发现”列出根目录直属文件夹，“已保存或手动目录”单独保留历史选择。核对项目文件夹名称和后台统计出的全部普通文件数量，再勾选需要备份的项目；“选择全部本次扫描项目”可一次纳入当前结果。
2. 打开“数据”，确认 Codex 数据位置；再到“设置”确认本地备份目录。点击字段旁的文件夹按钮即可选择路径。
3. 打开“备份与迁移”，填写恢复密码并开始本地备份。
4. 使用“刷新本地备份”查看快照，选择一个全新的恢复目标目录后恢复。
5. 需要计划任务时，启用当前用户计划任务并选择记住密码；密码只通过当前 Windows 用户 DPAPI 保护。

首次使用也可以打开“操作说明”，按流程图逐步完成上述操作。

## 八、项目工作流程

```text
读取配置 → 快速发现 Codex → 自动填充本地设置 →
后台扫描固定磁盘项目候选 → 汇总跳过的无权限目录 → 用户确认备份范围 →
restic 加密快照 → 校验/列出 → 独立目录恢复
```

迁移包使用单独的 ReHome 导出/导入入口。默认“仅恢复文件”不发送模型请求；“迁移并接入 Codex”需要关闭 Codex、检查恢复范围并单独同意联网验证。向导依次检查文件、所有计划迁移对话的 App Server 识别结果，再在一个用户选定对话的临时副本中发送固定验证消息，不向原对话发送测试消息。这不等于逐项验证全部历史、工具或原生窗口显示。

普通失败会在确认辅助进程已退出后尝试回滚；若无法确认进程结束，将停止数据库写入并保留材料供人工恢复。回滚不能撤销已经发生的联网处理或用量。“历史记录”可查看事务结果和处理建议。完整说明见 [使用指南](docs/USER_GUIDE.zh-CN.md)。

备份结果将可重建依赖、缓存和安全排除汇总为说明；数据完整性疑点显示警告，真实缺失的项目或文件仍保留缺失提示。恢复后依赖可能需要重新安装、Codex 需要重新登录，不承诺完全离线即可运行。

## 九、项目目录结构

```text
desktop/                    Tauri + React 桌面应用
desktop/src/                双语界面、设置、备份和迁移流程
desktop/src/App.tsx         主导航、项目扫描分组、操作说明和关于作者页面
desktop/src-tauri/src/      Rust 命令、发现、restic、恢复与迁移核心
desktop/src-tauri/resources/ 内置 restic/rclone 运行时
docs/                       规格、验收、兼容性、用户指南和验证证据
scripts/                    环境引导、验证、本地验收和打包脚本
tests/                      隔离测试与文档契约测试
```

## 十、注意事项

- 完整备份不会照搬 ReHome 对 `.git` 的排除规则，会保留 Git 数据和相关 worktree 资料。
- 从 0.1.3 起，完整本地项目备份包含项目中所有可读取的普通文件，包括隐藏文件、依赖、构建产物、`.env`、私钥和 Token；Codex 数据位置仍独立排除已知登录凭据等高风险数据。敏感项目文件只进入加密的 restic 仓库，请使用独立强密码并禁止把恢复结果公开或提交到 GitHub。
- 自动项目扫描只读取目录元数据和项目标记，不读取无关源文件正文；跳过系统目录、依赖目录、构建缓存、网络盘和 OneDrive 同步目录。
- 扫描有明确上限并可取消；无权限目录只显示聚合数量，其他提示最多显示少量摘要；部分结果会标记为部分完成，不能当作完整全盘扫描。
- Windows 内部使用 `\\?\` 长路径形式是正常的；配置和界面会显示普通盘符或 UNC 形式。管理员扫描只有用户明确操作时才请求 UAC。
- 恢复目标和现有资料默认不覆盖；失败或冲突会保留记录并提供回滚边界。
- 不把恢复密码、Cookie、私钥、Token、`.env` 或真实个人资料提交到仓库。

## 十一、版本说明

当前版本：`0.1.8`。本次增加项目根目录扫描引导、全盘扫描不完整提示、当前结果与保留目录分组，以及一键选择本次扫描项目；完整备份仍包含项目中的所有可读取普通文件。文件恢复、对话识别与一次临时副本发送验证分别记录，不把文件已恢复宣称为真实会话续接成功。详见 [版本更新说明](CHANGELOG.md)。[0.1.8 Release](https://github.com/yangjing6213-dev/Codex-Backup/releases/tag/v0.1.8) 已发布，包含未签名的 Windows x64 安装包和 SHA-256 文件，旧版本保留；安装前请按校验文件核对完整性。真实 OneDrive、第二设备、真实会话续接和原生界面验证仍不在已完成证据范围内，详见 [STATUS](docs/STATUS.md) 与 [ACCEPTANCE](docs/ACCEPTANCE.md)。

## 十二、相关项目

ReHome：本项目内置其适用的离线迁移能力，但完整本地备份额外保留 Git、worktree 和交接资料。

## 十三、关于作者

### 1、Enhe（恩禾）- 产品设计师 - 一人公司实践者 - AI Builder

![Enhe（恩禾）作者介绍](docs/assets/about-author.png)

用AI打造一个人公司。

- GitHub: [yangjing6213-dev](https://github.com/yangjing6213-dev)
- X/Twitter: [Amenenhe_ai](https://x.com/Amenenhe_ai)
- 网站：[www.enhe-tech.com.cn](https://www.enhe-tech.com.cn/)
- 微信：Hu-Amen
- 邮箱：amen.enhe@gmail.com

[恩禾 ENHE AI | AI工具、AI资讯、账号服务与技能课程](https://www.enhe-tech.com.cn/)

## 十四、继续探索

这个项目是我用 AI 搭建的个人生成系统里的一个工具。如果你也在用 AI 做内容、知识库、工作流或产品化，可以登录 [www.enhe-tech.com.cn](https://www.enhe-tech.com.cn/) 查看更多资料。

## 许可证

本项目沿用上游仓库的 MIT 许可；内置组件、版本和来源见 [THIRD_PARTY](docs/THIRD_PARTY.md)。

0.1.8 安装包随附许可与第三方声明文件，安装后可在程序目录的 `resources/licenses` 文件夹离线查看；许可材料是技术再分发证据，不构成法律意见。
