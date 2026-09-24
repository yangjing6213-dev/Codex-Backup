import { act, fireEvent, render, screen } from "@testing-library/react";
import { createRef } from "react";
import { beforeEach, expect, it, vi } from "vitest";
import { I18nProvider } from "../../lib/i18n";
import type { TransactionSummary } from "../../lib/types";
import HistoryPage from "./HistoryPage";

const api = vi.hoisted(() => ({ listTransactions: vi.fn(), rollbackTransaction: vi.fn(), openPath: vi.fn() }));
vi.mock("../../lib/api", () => api);
const transactions: TransactionSummary[] = ["committed", "rollback_failed"].map((status, index) => ({
  transaction_id: `transaction-${index}`, package_id: "synthetic-package", created_at: "2026-09-20T00:00:00Z",
  status: status as TransactionSummary["status"], backup_root: "C:/Synthetic/backups",
  transaction_backup_path: `C:/Synthetic/backups/transaction-${index}`, target_codex_home: "C:/Synthetic/target",
  projects_root: "C:/Synthetic/projects", restored_project_paths: [], changed_files: 2,
}));
beforeEach(() => {
  vi.resetAllMocks(); window.localStorage.clear();
  api.listTransactions.mockResolvedValue({ transactions, warnings: [] });
});
function renderHistory(operationBusy = false) {
  const callbacks = { onOperationStart: vi.fn(), onOperationEnd: vi.fn() };
  return { ...callbacks, ...render(<I18nProvider><HistoryPage headingRef={createRef()} {...callbacks} operationBusy={operationBusy} /></I18nProvider>) };
}

it("allows reading history and retained copies but disables every rollback during a global operation", async () => {
  renderHistory(true);
  expect(await screen.findByRole("button", { name: "回滚此事务" })).toBeDisabled();
  expect(screen.getByRole("button", { name: "继续回滚事务" })).toBeDisabled();
  expect(screen.getByRole("button", { name: "刷新迁移记录" })).toBeEnabled();
  await act(async () => { fireEvent.click(screen.getAllByRole("button", { name: "显示备份" })[1]); });
  expect(api.openPath).toHaveBeenCalledWith("C:/Synthetic/backups/transaction-1", "transaction-1");
  expect(api.rollbackTransaction).not.toHaveBeenCalled();
});

it("locks all transactions during its own rollback and unlocks only after completion", async () => {
  let finish!: (value: unknown) => void;
  api.rollbackTransaction.mockImplementation(() => new Promise(resolve => { finish = resolve; }));
  const callbacks = renderHistory();
  fireEvent.click(await screen.findByRole("button", { name: "继续回滚事务" }));
  expect(api.rollbackTransaction).toHaveBeenCalledExactlyOnceWith("transaction-1", "resume");
  expect(screen.getByRole("button", { name: "回滚此事务" })).toBeDisabled();
  expect(screen.getByRole("button", { name: "继续回滚事务" })).toBeDisabled();
  await act(async () => { finish({ transaction_id: "transaction-1", completed_at: "2026-09-20T00:00:00Z", restored_files: 2, success: true }); });
  expect(callbacks.onOperationStart).toHaveBeenCalledOnce();
  expect(callbacks.onOperationEnd).toHaveBeenCalledOnce();
  expect(screen.getByRole("button", { name: "回滚此事务" })).toBeEnabled();
});
