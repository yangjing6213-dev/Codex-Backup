# ENHE Codex Backup 规格基线

## 目标

ENHE Codex Backup 是面向 Windows 11 x64 的独立桌面备份与迁移工具。它在没有云端配置、未登录 GPT 的条件下，完成选定项目、Git/worktree、Codex 本地会话和开发交接资料的加密本地备份、历史浏览、隔离恢复和回滚；云端默认关闭，按需提供 OneDrive/rclone 连接。

本规格由用户指定的《ENHE-Codex-Backup-完整开发执行指令.md》确认，实施时服从更高优先级的系统、安全和项目规则。上游 ReHome 只作为可核验的 MIT 代码基线，不代表官方合作或背书。

## 范围与非目标

- 范围：本地 restic 快照、版本清单、增量检查、恢复、Git/worktree 保留、Codex 会话采集、迁移冲突与回滚、计划任务 worker、OneDrive/rclone 适配、四项主导航、zh-CN/en 双语、浅色/深色/跟随系统、Windows x64 安装包和离线恢复说明。
- 兼容导入：保留已验证 schema 的 `.rehome` 导入/导出，明确其排除规则和格式上限；完整备份不继承 `.rehome` 的 `.git` 排除规则。
- 非目标：Company OS、Flight Control、项目管理、云同步平台、多智能体、自动开发、大模型总结、其他云盘、macOS/Linux 安装包、自动更新和遥测。

## 架构

- `desktop/` 沿用 ReHome commit `cc5daae501b22b86e1b4c2dafbd1b4af61992b48` 的 Tauri 2 + React 19 + TypeScript + Rust 结构；迁移核心和安全文件操作保留窄适配层。
- `desktop/src-tauri/src/core/restic.rs` 是 GUI 与 worker 共用的本地仓库边界：所有外部进程固定可执行文件、参数数组调用，密码通过短生命周期 password file 传递，不进入命令行、日志或错误文本。
- 本地快照使用结构化 `enhe-codex-backup` manifest 与 restic 加密仓库；项目文件按选定内容进入隔离 payload，Git 数据、worktree 元数据、会话原始记录、WAL 一致性副本和交接资料均由清单追踪。
- `cloud.rs` 只在用户主动配置/启用且每次入口重新读取开关后调用 rclone；云端使用独立 restic 仓库和 `restic copy`，不使用 `rclone sync` 复制正在变化的备份库。
- `scheduler.rs` 仅注册当前用户的独立 Windows 任务；`worker.rs` 复用同一 Rust 核心，无 GUI、无开发服务器、无高权限常驻服务。

## 数据安全边界

- 默认排除 auth.json、Cookie、私钥、真实密钥、运行时锁、缓存和可重建依赖；`.env.example` 不按前缀盲目排除。符号链接、junction、reparse point 和外部引用只记录为缺失/需选择，不静默追随。
- 来源、暂存、仓库、恢复目标逐一做 canonical/containment 校验；拒绝路径穿越、设备名、控制字符、危险重解析点和未知控制文件，恢复失败回滚且保留证据。
- 结果分别报告本地文件恢复、会话/索引导入、Codex 可见性和原会话续接；没有真实账号/设备/模型请求时不标记续接通过。

## 验收基线

对应 A01-A12 用例见 `docs/ACCEPTANCE.md`。状态只使用 `PASS`、`FAIL`、`PARTIAL`、`NOT_RUN`、`BLOCKED`；缺少 Rust/MSVC、OneDrive 授权、第二设备或正常目标账号时必须保留未验证状态，不用 mock、双临时目录或文件复制替代真实验收。
