import { describe, expect, it } from "vitest";

import { errorMessage } from "./types";

describe("errorMessage", () => {
  it("shows a cause, solution and sanitized detail for a backup failure", () => {
    const message = errorMessage(
      { code: "backup_failed", message: "backup engine failed (exit 1): synthetic failure" },
      (key) => key,
    );

    expect(message).toContain("本地备份未完整完成；原始项目文件未被修改。");
    expect(message).toContain("原因：备份引擎未能完成操作。");
    expect(message).toContain("解决方法：请检查备份目录权限和剩余空间，确认恢复密码后重试。");
    expect(message).toContain("技术详情：backup engine failed (exit 1): synthetic failure");
  });

  it("keeps restore failures distinct from backup failures", () => {
    const restore = errorMessage(
      { code: "restore_failed", message: "synthetic restore" },
      (key) => key,
    );

    expect(restore).toContain("本地恢复失败，现有资料保持不变。");
    expect(restore).toContain("原因：备份快照未能恢复到目标目录。");
    expect(restore).not.toContain("本地备份未完整完成");
  });
});
