# 离线恢复说明

本说明不要求登录 GPT、访问网页或找回原应用缓存。只需要目标 Windows x64 设备上的 ENHE 安装目录（或标准 restic 可执行文件）、本地 restic 仓库目录和恢复密码。

此处离线恢复仅指文件恢复；不包括模型联网验证。文档对应 0.1.7（2026-09-23）；构建和安装记录见 [STATUS](STATUS.md)，不能代替恢复验收。真实账号、第二台设备及原生界面验证均为 `NOT_RUN`。

## 使用应用恢复

1. 把整个本地仓库目录复制到新设备的本地磁盘；不要把仓库放在要恢复的目标目录里面。
2. 安装并打开 ENHE Codex Backup，不配置云端。
3. 在“备份与迁移”填写仓库目录和恢复密码，刷新快照。
4. 选择一个独立的恢复目标目录并恢复。成功后检查 `backup-<id>\projects` 和 `backup-<id>\codex` 下的项目、Codex 数据和交接资料。

应用会拒绝目标与仓库重叠、已有目标覆盖、路径穿越、非法 manifest 和恢复载荷中的符号链接。恢复失败留下的 `.partial` 目录只属于本次操作；确认应用已退出后，可删除该目录。普通删除不是安全擦除。

检查清单的“安全排除 / 自动处理 / 数据警告 / 文件缺失”：Codex 凭据被排除后需重新登录；可重建依赖链接可能需要项目包管理器重新安装，不能保证恢复后离线可运行；异常 JSONL 已保留但可能影响所列对话；红色缺失项目路径表示源项目未进入快照。`warning` 不等于零影响，`partial` 不能当作完整副本。旧 manifest 不会被新分类重写。

restic 完整项目备份包含可读取普通文件、Git/worktree、依赖及敏感项目文件，Codex 数据另有安全排除。ReHome `.rehome` 包只包含所选内容，应用自身安全排除且不包含 `.git` 等，不是加密 restic 仓库；两者不能互相替代。

## 标准 restic 只读检查

在 PowerShell 中把路径替换为实际值。密码文件应由当前用户创建并限制访问，不要把密码直接写进命令行或日志。

```powershell
$restic = "C:\Program Files\ENHE Codex Backup\resources\restic.exe"
$repo = "D:\ENHE\backups"
$passwordFile = "D:\ENHE\recovery-password.txt"

& $restic --repo $repo --password-file $passwordFile snapshots
& $restic --repo $repo --password-file $passwordFile check
```

## 恢复快照

先恢复到全新的临时目录，再检查内容：

```powershell
$snapshotId = "替换为 snapshots 输出的完整 ID"
$target = "D:\ENHE\offline-restore"

& $restic --repo $repo --password-file $passwordFile restore $snapshotId --target $target
Get-ChildItem -LiteralPath (Join-Path $target "enhe-payload") -Force
```

ENHE 创建的快照载荷根目录是 `enhe-payload`。标准 restic 恢复不会让 Codex 自动显示对话，也不会执行恢复内容；需要人工检查文件，之后在 Codex 中按正常流程重新打开项目。原账号、第二台设备和会话续接不属于本说明已验证的结果。

## ReHome 文件恢复与在线接入的边界

ReHome 导入中的“仅恢复文件”执行本地恢复及校验，不证明对话已被 Codex 识别或能续聊。需要在线验证时，另行选择“迁移并接入 Codex”，完成所需登录、联网、关闭 Codex 及辅助进程，并明确同意用量；没有可映射的对话时此方式不可用。

在线方式检查全部计划内目标对话 ID 的识别情况，只在所选对话的临时分支发送一次中性消息。该分支会使用所选对话上下文联系模型服务，可能产生用量；不会向原对话添加验证消息，也不证明每个对话的完整历史、工具能力或桌面可见性。服务端处理和用量不能由本地回滚撤销。

普通验证失败且辅助进程已确认退出时会自动尝试本地回滚；若退出无法确认，则停止数据库检查点与回滚写入并保留资料供人工恢复。这些事务备份不能当作可随意清理的 restic `.partial` 目录。根据错误原因、解决方法和事务编号查看“迁移记录”，保留现有文件及自动备份，完全关闭相关进程后再判断恢复方式，不要覆盖较新的数据。完整步骤见 [中文使用说明](USER_GUIDE.zh-CN.md#迁移失败与迁移记录)。

## 仓库损坏或密码错误

先保留原仓库的只读副本，不在原目录运行修复性操作。密码错误、缺少 pack/index/config 或 `check` 报错时，记录脱敏错误信息并从另一份独立备份恢复。不要删除原仓库，不要运行来源不明的脚本或 hooks。
