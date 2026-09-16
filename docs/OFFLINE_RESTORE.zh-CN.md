# 离线恢复说明

本说明不要求登录 GPT、访问网页或找回原应用缓存。只需要目标 Windows x64 设备上的 ENHE 安装目录（或标准 restic 可执行文件）、本地 restic 仓库目录和恢复密码。

## 使用应用恢复

1. 把整个本地仓库目录复制到新设备的本地磁盘；不要把仓库放在要恢复的目标目录里面。
2. 安装并打开 ENHE Codex Backup，不配置云端。
3. 在“备份与迁移”填写仓库目录和恢复密码，刷新快照。
4. 选择一个独立的恢复目标目录并恢复。成功后检查 `backup-<id>\projects` 和 `backup-<id>\codex` 下的项目、Codex 数据和交接资料。

应用会拒绝目标与仓库重叠、已有目标覆盖、路径穿越、非法 manifest 和恢复载荷中的符号链接。恢复失败留下的 `.partial` 目录只属于本次操作；确认应用已退出后，可删除该目录。普通删除不是安全擦除。

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

## 仓库损坏或密码错误

先保留原仓库的只读副本，不在原目录运行修复性操作。密码错误、缺少 pack/index/config 或 `check` 报错时，记录脱敏错误信息并从另一份独立备份恢复。不要删除原仓库，不要运行来源不明的脚本或 hooks。
